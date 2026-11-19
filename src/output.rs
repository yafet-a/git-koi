//! Image and animation encoding; callers own destinations and publication.
//!
//! Every export is [`STATIC_EXPORT_SCALE`] times the native canvas on each side, so the
//! pixel art stays crisp: [`encode_png`] for the still, [`encode_svg`] for a vector
//! document of the same pixels, [`encode_gif`] for a behavior's loop, and
//! [`encode_preview_html`] for a page that shows the GIF and PNG beside it on GitHub's
//! light and dark page colors.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt;

use crate::motion::FRAMES_PER_SECOND;
use crate::pond::PondFrame;
use crate::render::{FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, KoiFrame};

/// Each native pixel becomes this many export pixels on each side.
pub const STATIC_EXPORT_SCALE: u32 = 2;
/// Width of every exported image.
pub const STATIC_WIDTH_PIXELS: u32 = FRAME_WIDTH_PIXELS * STATIC_EXPORT_SCALE;
/// Height of every exported image.
pub const STATIC_HEIGHT_PIXELS: u32 = FRAME_HEIGHT_PIXELS * STATIC_EXPORT_SCALE;

/// Encode a static PNG by repeating each native pixel in both dimensions.
/// Preserve its color and alpha without interpolation, timestamps, I/O, or mutation.
/// Encoding errors propagate before the caller publishes any resulting bytes.
pub fn encode_png(koi_frame: &KoiFrame) -> Result<Vec<u8>, png::EncodingError> {
    let native_pixels = koi_frame.as_rgba_bytes().as_chunks::<4>().0;
    let mut export_pixels = vec![[0; 4]; (STATIC_WIDTH_PIXELS * STATIC_HEIGHT_PIXELS) as usize];
    for (pixel_index, pixel_channels) in export_pixels.iter_mut().enumerate() {
        let native_x = pixel_index as u32 % STATIC_WIDTH_PIXELS / STATIC_EXPORT_SCALE;
        let native_y = pixel_index as u32 / STATIC_WIDTH_PIXELS / STATIC_EXPORT_SCALE;
        *pixel_channels = native_pixels[(native_y * FRAME_WIDTH_PIXELS + native_x) as usize];
    }
    let mut png_bytes = Vec::new();
    let mut png_encoder =
        png::Encoder::new(&mut png_bytes, STATIC_WIDTH_PIXELS, STATIC_HEIGHT_PIXELS);
    png_encoder.set_color(png::ColorType::Rgba);
    png_encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = png_encoder.write_header()?;
    png_writer.write_image_data(export_pixels.as_flattened())?;
    png_writer.finish()?;
    Ok(png_bytes)
}

/// Encode a self-contained SVG at the static export size with native pixel coordinates.
/// Equal neighboring pixels share a rectangle within their row. Transparent pixels
/// are omitted; visible pixels retain their color and alpha. Identical frames produce
/// identical documents without I/O, timestamps, external assets, or input mutation.
pub fn encode_svg(koi_frame: &KoiFrame) -> String {
    let mut svg_document = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{STATIC_WIDTH_PIXELS}\" height=\"{STATIC_HEIGHT_PIXELS}\" viewBox=\"0 0 {FRAME_WIDTH_PIXELS} {FRAME_HEIGHT_PIXELS}\" shape-rendering=\"crispEdges\">\n"
    );
    let native_pixels = koi_frame.as_rgba_bytes().as_chunks::<4>().0;
    let mut pixel_index = 0;
    while pixel_index < native_pixels.len() {
        let pixel_channels = native_pixels[pixel_index];
        let pixel_x = pixel_index % FRAME_WIDTH_PIXELS as usize;
        let pixel_y = pixel_index / FRAME_WIDTH_PIXELS as usize;
        let row_end_index = (pixel_y + 1) * FRAME_WIDTH_PIXELS as usize;
        let run_length_pixels = native_pixels[pixel_index..row_end_index]
            .iter()
            .take_while(|next_pixel| **next_pixel == pixel_channels)
            .count();
        if pixel_channels[3] != 0 {
            let [red_channel, green_channel, blue_channel, alpha_channel] = pixel_channels;
            let fill_opacity = f64::from(alpha_channel) / 255.0;
            svg_document.push_str(&format!(
                "  <rect x=\"{pixel_x}\" y=\"{pixel_y}\" width=\"{run_length_pixels}\" height=\"1\" fill=\"#{red_channel:02x}{green_channel:02x}{blue_channel:02x}\" fill-opacity=\"{fill_opacity}\"/>\n"
            ));
        }
        pixel_index += run_length_pixels;
    }
    svg_document.push_str("</svg>\n");
    svg_document
}

/// The preview page up to its caption; GitHub's light and dark page colors follow.
const PREVIEW_HEAD: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>git-koi preview</title>
<style>
  body { margin: 0; font: 14px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; }
  h1, h2 { margin: 0; font-size: 14px; font-weight: 600; }
  header, section { display: grid; gap: 12px; padding: 16px; }
  .light { background: #ffffff; color: #1f2328; }
  .dark { background: #0d1117; color: #f0f6fc; }
  .images { display: flex; flex-wrap: wrap; gap: 16px; }
  figure { margin: 0; }
  img { display: block; max-width: 100%; height: auto; image-rendering: pixelated; }
</style>
</head>
<body>
<header class="light"><h1>"#;
const PREVIEW_PANELS: &str = r#"</h1></header>
<section class="light">
<h2>GitHub light</h2>
<div class="images">
<figure><img src="koi.gif" width="512" height="512" alt="The koi swimming"><figcaption>koi.gif</figcaption></figure>
<figure><img src="koi.png" width="512" height="512" alt="The koi, still"><figcaption>koi.png</figcaption></figure>
</div>
</section>
<section class="dark">
<h2>GitHub dark</h2>
<div class="images">
<figure><img src="koi.gif" width="512" height="512" alt="The koi swimming"><figcaption>koi.gif</figcaption></figure>
<figure><img src="koi.png" width="512" height="512" alt="The koi, still"><figcaption>koi.png</figcaption></figure>
</div>
</section>
</body>
</html>
"#;

/// Build a page that previews `koi.gif` and `koi.png`, read from beside it, on GitHub's
/// light and dark page colors under the given caption. It draws nothing itself and loads
/// no scripts or outside files, so it shows exactly what was generated.
pub fn encode_preview_html(caption: &str) -> String {
    let escaped_caption = caption
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!("{PREVIEW_HEAD}{escaped_caption}{PREVIEW_PANELS}")
}

/// One GIF palette holds 256 entries, and one of them marks transparent pixels.
const GIF_COLOR_LIMIT: usize = 255;
const GIF_TRANSPARENT_INDEX: u8 = 0;

/// Encode a seamless loop as an animated GIF at the static export size, repeating each
/// native pixel in both dimensions. The GIF repeats forever at `FRAMES_PER_SECOND`; each
/// delay, in hundredths of a second, is the step in a rounded running total, so a loop
/// lasts exactly its frame count divided by the frame rate. Each frame stores only the
/// box around its drawn pixels, with an exact palette of its colors, and that box clears
/// before the next frame. Colors are never approximated: a frame with more than 255
/// colors or a partly transparent pixel fails instead. Identical frames produce
/// identical bytes without I/O, timestamps, or input mutation.
pub fn encode_gif(koi_frames: &[KoiFrame]) -> Result<Vec<u8>, GifEncodingError> {
    if koi_frames.is_empty() {
        return Err(GifEncodingError::NoFrames);
    }
    let mut gif_encoder = gif::Encoder::new(
        Vec::new(),
        STATIC_WIDTH_PIXELS as u16,
        STATIC_HEIGHT_PIXELS as u16,
        &[],
    )?;
    gif_encoder.set_repeat(gif::Repeat::Infinite)?;
    for (frame_index, koi_frame) in koi_frames.iter().enumerate() {
        let native_pixels = koi_frame.as_rgba_bytes().as_chunks::<4>().0;
        let [box_left, box_top, box_width, box_height] = find_drawn_box(native_pixels);
        let mut palette_indices: BTreeMap<[u8; 3], u8> = BTreeMap::new();
        let mut palette_channels = vec![0; 3];
        let mut native_indices = Vec::with_capacity((box_width * box_height) as usize);
        for native_y in box_top..box_top + box_height {
            for native_x in box_left..box_left + box_width {
                let [red, green, blue, alpha] =
                    native_pixels[(native_y * FRAME_WIDTH_PIXELS + native_x) as usize];
                let palette_index = match alpha {
                    0 => GIF_TRANSPARENT_INDEX,
                    255 => match palette_indices.get(&[red, green, blue]) {
                        Some(&palette_index) => palette_index,
                        None if palette_indices.len() == GIF_COLOR_LIMIT => {
                            return Err(GifEncodingError::TooManyColors { frame_index });
                        }
                        None => {
                            let palette_index = palette_indices.len() as u8 + 1;
                            palette_indices.insert([red, green, blue], palette_index);
                            palette_channels.extend([red, green, blue]);
                            palette_index
                        }
                    },
                    _ => return Err(GifEncodingError::PartialTransparency { frame_index }),
                };
                native_indices.push(palette_index);
            }
        }
        let mut export_indices = Vec::with_capacity(
            native_indices.len() * (STATIC_EXPORT_SCALE * STATIC_EXPORT_SCALE) as usize,
        );
        for native_row in native_indices.chunks(box_width as usize) {
            let export_row: Vec<u8> = native_row
                .iter()
                .flat_map(|&palette_index| [palette_index; STATIC_EXPORT_SCALE as usize])
                .collect();
            for _ in 0..STATIC_EXPORT_SCALE {
                export_indices.extend_from_slice(&export_row);
            }
        }
        gif_encoder.write_frame(&gif::Frame {
            delay: calculate_elapsed_hundredths(frame_index + 1)
                - calculate_elapsed_hundredths(frame_index),
            dispose: gif::DisposalMethod::Background,
            transparent: Some(GIF_TRANSPARENT_INDEX),
            left: (box_left * STATIC_EXPORT_SCALE) as u16,
            top: (box_top * STATIC_EXPORT_SCALE) as u16,
            width: (box_width * STATIC_EXPORT_SCALE) as u16,
            height: (box_height * STATIC_EXPORT_SCALE) as u16,
            palette: Some(palette_channels),
            buffer: Cow::Owned(export_indices),
            ..gif::Frame::default()
        })?;
    }
    Ok(gif_encoder.into_inner()?)
}

/// Encode a pond loop as an animated GIF at the frames' own size. A pond has gradients
/// and glow, so unlike the koi it needs a reduced palette: one shared palette of 255
/// colours is learned from a sample of the frames (NeuQuant), so colours hold still from
/// frame to frame, and index 0 marks the transparent outside. Delays follow
/// `FRAMES_PER_SECOND` as `encode_gif`'s do. Identical frames produce identical bytes.
///
/// Most of a pond holds still while the koi swims, so after the first frame each frame
/// stores only what changed: the box around the changed pixels, with the pixels inside
/// it that did not change left transparent, drawn over the frame before. That holds only
/// while the transparent outside stays put, so a loop whose outside changes stores every
/// frame whole.
pub fn encode_pond_gif(pond_frames: &[PondFrame]) -> Result<Vec<u8>, GifEncodingError> {
    let Some(first_frame) = pond_frames.first() else {
        return Err(GifEncodingError::NoFrames);
    };
    let (width, height) = (first_frame.width() as usize, first_frame.height() as usize);
    let (palette_channels, index_frames) = index_pond_frames(pond_frames);
    let mut gif_encoder =
        gif::Encoder::new(Vec::new(), width as u16, height as u16, &palette_channels)?;
    gif_encoder.set_repeat(gif::Repeat::Infinite)?;
    let outside_holds = index_frames.windows(2).all(|pair| {
        pair[0].iter().zip(&pair[1]).all(|(before, after)| {
            (*before == GIF_TRANSPARENT_INDEX) == (*after == GIF_TRANSPARENT_INDEX)
        })
    });
    for (frame_index, frame_indices) in index_frames.iter().enumerate() {
        let delay = calculate_elapsed_hundredths(frame_index + 1)
            - calculate_elapsed_hundredths(frame_index);
        let previous = frame_index
            .checked_sub(1)
            .map(|before| &index_frames[before]);
        let gif_frame = match previous.filter(|_| outside_holds) {
            None => gif::Frame {
                delay,
                dispose: if outside_holds {
                    gif::DisposalMethod::Keep
                } else {
                    gif::DisposalMethod::Background
                },
                transparent: Some(GIF_TRANSPARENT_INDEX),
                width: width as u16,
                height: height as u16,
                buffer: Cow::Borrowed(frame_indices),
                ..gif::Frame::default()
            },
            Some(previous) => {
                let changed = |index: usize| frame_indices[index] != previous[index];
                let mut changed_box: Option<[usize; 4]> = None;
                for index in (0..frame_indices.len()).filter(|&index| changed(index)) {
                    let (x, y) = (index % width, index / width);
                    let [left, top, right, bottom] = changed_box.get_or_insert([x, y, x, y]);
                    *left = (*left).min(x);
                    *top = (*top).min(y);
                    *right = (*right).max(x);
                    *bottom = (*bottom).max(y);
                }
                // a frame with no change keeps one unchanged pixel, since a GIF frame
                // cannot be empty
                let [left, top, right, bottom] = changed_box.unwrap_or([0; 4]);
                let buffer = (top..=bottom)
                    .flat_map(|y| (left..=right).map(move |x| y * width + x))
                    .map(|index| {
                        if changed(index) {
                            frame_indices[index]
                        } else {
                            GIF_TRANSPARENT_INDEX
                        }
                    })
                    .collect();
                gif::Frame {
                    delay,
                    dispose: gif::DisposalMethod::Keep,
                    transparent: Some(GIF_TRANSPARENT_INDEX),
                    left: left as u16,
                    top: top as u16,
                    width: (right - left + 1) as u16,
                    height: (bottom - top + 1) as u16,
                    buffer: Cow::Owned(buffer),
                    ..gif::Frame::default()
                }
            }
        };
        gif_encoder.write_frame(&gif_frame)?;
    }
    Ok(gif_encoder.into_inner()?)
}

/// The shared palette as RGB channels, transparent first, and each frame's pixels as
/// indices into it.
fn index_pond_frames(pond_frames: &[PondFrame]) -> (Vec<u8>, Vec<Vec<u8>>) {
    let sample_step = (pond_frames.len() / 8).max(1);
    let mut sample_pixels = Vec::new();
    for pond_frame in pond_frames.iter().step_by(sample_step) {
        for pixel in pond_frame.as_rgba_bytes().as_chunks::<4>().0 {
            if pixel[3] == 255 {
                sample_pixels.extend_from_slice(pixel);
            }
        }
    }
    if sample_pixels.is_empty() {
        sample_pixels.extend_from_slice(&[0, 0, 0, 255]);
    }
    let quantizer = color_quant::NeuQuant::new(10, GIF_COLOR_LIMIT, &sample_pixels);
    let mut palette_channels = vec![0; 3];
    palette_channels.extend(quantizer.color_map_rgb());
    palette_channels.resize(256 * 3, 0);
    let mut known_indices: HashMap<[u8; 4], u8> = HashMap::new();
    let index_frames = pond_frames
        .iter()
        .map(|pond_frame| {
            pond_frame
                .as_rgba_bytes()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| {
                    if pixel[3] == 0 {
                        GIF_TRANSPARENT_INDEX
                    } else {
                        *known_indices
                            .entry(*pixel)
                            .or_insert_with(|| quantizer.index_of(pixel) as u8 + 1)
                    }
                })
                .collect()
        })
        .collect();
    (palette_channels, index_frames)
}

/// The native box `[left, top, width, height]` around a frame's drawn pixels; a frame
/// with none keeps one transparent pixel, since a GIF frame cannot be empty.
fn find_drawn_box(native_pixels: &[[u8; 4]]) -> [u32; 4] {
    let mut drawn_edges: Option<[u32; 4]> = None;
    for (pixel_index, pixel_channels) in native_pixels.iter().enumerate() {
        if pixel_channels[3] == 0 {
            continue;
        }
        let pixel_x = pixel_index as u32 % FRAME_WIDTH_PIXELS;
        let pixel_y = pixel_index as u32 / FRAME_WIDTH_PIXELS;
        let [left, top, right, bottom] =
            drawn_edges.get_or_insert([pixel_x, pixel_y, pixel_x, pixel_y]);
        *left = (*left).min(pixel_x);
        *top = (*top).min(pixel_y);
        *right = (*right).max(pixel_x);
        *bottom = (*bottom).max(pixel_y);
    }
    drawn_edges.map_or([0, 0, 1, 1], |[left, top, right, bottom]| {
        [left, top, right + 1 - left, bottom + 1 - top]
    })
}

/// Hundredths of a second from the start of the loop to the given frame, rounded.
fn calculate_elapsed_hundredths(frame_index: usize) -> u16 {
    let frames_per_second = FRAMES_PER_SECOND as usize;
    u16::try_from((frame_index * 200 + frames_per_second) / (2 * frames_per_second))
        .expect("loops last under ten minutes")
}

/// Why a GIF was not produced.
#[derive(Debug)]
pub enum GifEncodingError {
    /// No frames were given.
    NoFrames,
    /// A frame has more opaque colors than one GIF palette holds.
    TooManyColors {
        /// The frame, counted from zero.
        frame_index: usize,
    },
    /// A frame has a pixel that is neither opaque nor fully transparent.
    PartialTransparency {
        /// The frame, counted from zero.
        frame_index: usize,
    },
    /// The GIF encoder failed; carries its own error.
    Encoder(gif::EncodingError),
}

impl fmt::Display for GifEncodingError {
    fn fmt(&self, output_formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFrames => write!(output_formatter, "a GIF needs at least one frame"),
            Self::TooManyColors { frame_index } => write!(
                output_formatter,
                "frame {frame_index} has more than {GIF_COLOR_LIMIT} colors, more than one GIF palette holds"
            ),
            Self::PartialTransparency { frame_index } => write!(
                output_formatter,
                "frame {frame_index} has partly transparent pixels, which a GIF cannot show"
            ),
            Self::Encoder(encoding_error) => {
                write!(output_formatter, "GIF encoding failed: {encoding_error}")
            }
        }
    }
}

impl Error for GifEncodingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encoder(encoding_error) => Some(encoding_error),
            _ => None,
        }
    }
}

impl From<gif::EncodingError> for GifEncodingError {
    fn from(encoding_error: gif::EncodingError) -> Self {
        Self::Encoder(encoding_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_PIXEL_COUNT: usize = (FRAME_WIDTH_PIXELS * FRAME_HEIGHT_PIXELS) as usize;

    /// A frame with one opaque pixel of each given color, in reading order.
    fn frame_with_colors(colors: impl IntoIterator<Item = [u8; 4]>) -> KoiFrame {
        let mut rgba_pixels = vec![[0; 4]; FRAME_PIXEL_COUNT];
        for (pixel_channels, color) in rgba_pixels.iter_mut().zip(colors) {
            *pixel_channels = color;
        }
        KoiFrame::from_rgba_pixels(rgba_pixels)
    }

    fn distinct_colors(color_count: u32) -> impl Iterator<Item = [u8; 4]> {
        (0..color_count).map(|color_index| [color_index as u8, (color_index >> 8) as u8, 7, 255])
    }

    fn total_hundredths(gif_bytes: &[u8]) -> u32 {
        let mut gif_decoder = gif::DecodeOptions::new().read_info(gif_bytes).unwrap();
        let mut total = 0;
        while let Some(gif_frame) = gif_decoder.read_next_frame().unwrap() {
            total += u32::from(gif_frame.delay);
        }
        total
    }

    #[test]
    fn whole_loops_last_exactly_their_frames_at_fifteen_per_second() {
        for (loop_frame_count, loop_hundredths) in [(120, 800), (90, 600)] {
            let koi_frames: Vec<KoiFrame> = (0..loop_frame_count)
                .map(|_| frame_with_colors(distinct_colors(1)))
                .collect();
            assert_eq!(
                total_hundredths(&encode_gif(&koi_frames).unwrap()),
                loop_hundredths
            );
        }
    }

    #[test]
    fn a_frame_may_use_every_palette_color_but_no_more() {
        assert!(encode_gif(&[frame_with_colors(distinct_colors(255))]).is_ok());
        let crowded_frames = [
            frame_with_colors(distinct_colors(1)),
            frame_with_colors(distinct_colors(256)),
        ];
        assert!(matches!(
            encode_gif(&crowded_frames),
            Err(GifEncodingError::TooManyColors { frame_index: 1 })
        ));
    }

    #[test]
    fn partly_transparent_pixels_fail_instead_of_rounding() {
        assert!(matches!(
            encode_gif(&[frame_with_colors([[200, 100, 50, 128]])]),
            Err(GifEncodingError::PartialTransparency { frame_index: 0 })
        ));
    }

    /// A small pond loop: a still transparent border, a still ground, and one pixel
    /// that walks along a row, or with `moving_outside` a ground that shrinks.
    fn pond_loop(moving_outside: bool) -> Vec<PondFrame> {
        (0..6)
            .map(|frame| {
                let pixels = (0..64)
                    .map(|index| {
                        let (x, y) = (index % 8, index / 8);
                        let edge = if moving_outside { frame / 2 } else { 0 };
                        if x <= edge || y == 0 || x == 7 || y == 7 {
                            [0; 4]
                        } else if y == 3 && x == 1 + frame % 6 {
                            [250, 200, 40, 255]
                        } else {
                            [40, 90, (100 + x * 10) as u8, 255]
                        }
                    })
                    .collect();
                PondFrame::new(8, 8, pixels)
            })
            .collect()
    }

    /// Draw every frame of a GIF over the last, as a browser does, as palette indices.
    fn composite_indices(gif_bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::Indexed);
        let mut gif_decoder = options.read_info(gif_bytes).unwrap();
        let width = usize::from(gif_decoder.width());
        let mut canvas = vec![GIF_TRANSPARENT_INDEX; width * usize::from(gif_decoder.height())];
        let mut shown = Vec::new();
        while let Some(gif_frame) = gif_decoder.read_next_frame().unwrap() {
            let (left, top) = (usize::from(gif_frame.left), usize::from(gif_frame.top));
            for (offset, &index) in gif_frame.buffer.iter().enumerate() {
                let (x, y) = (
                    offset % usize::from(gif_frame.width),
                    offset / usize::from(gif_frame.width),
                );
                if Some(index) != gif_frame.transparent {
                    canvas[(top + y) * width + left + x] = index;
                } else if gif_frame.dispose == gif::DisposalMethod::Background {
                    canvas[(top + y) * width + left + x] = GIF_TRANSPARENT_INDEX;
                }
            }
            shown.push(canvas.clone());
        }
        shown
    }

    #[test]
    fn a_pond_shows_every_frame_exactly_while_storing_only_changes() {
        for moving_outside in [false, true] {
            let pond_frames = pond_loop(moving_outside);
            let gif_bytes = encode_pond_gif(&pond_frames).unwrap();
            assert_eq!(
                composite_indices(&gif_bytes),
                index_pond_frames(&pond_frames).1
            );
        }
        let gif_bytes = encode_pond_gif(&pond_loop(false)).unwrap();
        let mut gif_decoder = gif::DecodeOptions::new()
            .read_info(gif_bytes.as_slice())
            .unwrap();
        gif_decoder.read_next_frame().unwrap();
        let second = gif_decoder.read_next_frame().unwrap().unwrap();
        // the walking pixel left one place and arrived at the next: two pixels
        assert_eq!((second.width, second.height), (2, 1));
    }

    #[test]
    fn an_empty_frame_keeps_one_transparent_pixel() {
        let gif_bytes = encode_gif(&[frame_with_colors([])]).unwrap();
        let mut gif_decoder = gif::DecodeOptions::new()
            .read_info(gif_bytes.as_slice())
            .unwrap();
        let gif_frame = gif_decoder.read_next_frame().unwrap().unwrap();
        assert_eq!((gif_frame.width, gif_frame.height), (2, 2));
        assert!(
            gif_frame
                .buffer
                .chunks(4)
                .all(|pixel_channels| pixel_channels[3] == 0)
        );
    }
}
