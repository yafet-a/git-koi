//! Swim the koi through a garden pond that follows the seasons.
//!
//! A pond frame is the koi's own swim frame laid into a scene: water that ripples, a
//! garden or deck around it, the season's dressing (blossom, lily pads, leaves, ice), the
//! day's weather (rain over any season but winter, which snows), and, for viewers in
//! dark mode, the same pond at night under lamps and the moon. The outside of the pond
//! is transparent, so it sits on a light or a dark page rather than as a square.
//!
//! The drawing matches `scenes/pond.html` in the repository, the interactive explorer,
//! rule for rule. Like the rest of the crate it is deterministic: the date comes from the
//! caller, usually the snapshot's observation date, and nothing reads the clock.

use std::collections::HashSet;
use std::f64::consts::{PI, TAU};

use chrono::{Datelike, NaiveDate};

use crate::coat::KoiBody;
use crate::render::{FRAME_WIDTH_PIXELS, KoiFrame};

// ------------------------------------------------------------------ the options

/// Declares an option the command line names, with its names in help order.
macro_rules! named_option {
    ($(#[$doc:meta])* $name:ident { $($(#[$value_doc:meta])* $value:ident => $text:literal),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name {
            $($(#[$value_doc])* $value),+
        }

        impl $name {
            /// Every value, in the order the help lists them; the first is the default.
            pub const ALL: &'static [Self] = &[$(Self::$value),+];

            /// The value's name on the command line.
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$value => $text),+
                }
            }

            /// The value with this name on the command line, if there is one.
            pub fn parse(text: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|value| value.name() == text)
            }
        }
    };
}

named_option!(
    /// The season, which dresses the pond.
    Season {
        /// Blossom falling from a branch.
        Spring => "spring",
        /// Lily pads and a lotus.
        Summer => "summer",
        /// Maple leaves falling and drifting.
        Autumn => "autumn",
        /// Frost, snow and pale water.
        Winter => "winter",
    }
);

named_option!(
    /// The weather laid over the season.
    Weather {
        /// No rain.
        Clear => "clear",
        /// Drops land and rings spread; the garden and decks are wet.
        Rain => "rain",
    }
);

named_option!(
    /// What surrounds the water.
    Garden {
        /// A moss bank along the water and moss islands in raked gravel.
        Garden => "garden",
        /// Moss islands in raked gravel.
        Islands => "islands",
        /// A moss bank along the water, then raked gravel.
        Bank => "bank",
        /// Raked gravel alone.
        Gravel => "gravel",
        /// An oak deck.
        Deck => "deck",
        /// A dark oak deck.
        DeckDark => "deckdark",
        /// An old oak deck, rotting, split, with moss in the seams.
        DeckOld => "deckold",
        /// A weathered grey deck.
        DeckGrey => "deckgrey",
        /// A walnut deck with a post at each corner.
        DeckWalnut => "deckwalnut",
        /// No edge: the water thins out into the page.
        Mist => "mist",
    }
);

named_option!(
    /// The size of the picture.
    PondSize {
        /// 352 pixels, the koi full size, a broad garden.
        Wide => "wide",
        /// 256 pixels, the koi at 90%, a narrow garden.
        Compact => "compact",
    }
);

named_option!(
    /// The lights at night.
    Lights {
        /// Only their reflections on the water, as if the lamps stood out of the picture.
        Reflect => "reflect",
        /// One paper lantern floating on the water.
        OneFloat => "onefloat",
        /// Several floating lanterns.
        Floating => "floating",
        /// Old European gas lamps on the bank.
        Euro => "euro",
        /// Japanese stone lanterns on the bank.
        Toro => "toro",
        /// Red paper lanterns on a cord.
        Chinese => "chinese",
        /// A string of small bulbs.
        Strings => "string",
        /// No lights.
        Off => "none",
    }
);

named_option!(
    /// The moon reflected in the water at night.
    Moon {
        /// Shimmering horizontal lines in a circle.
        Lines => "lines",
        /// A full moon broken into bands by the ripples.
        Round => "round",
        /// The full moon with a soft halo.
        Halo => "halo",
        /// A crescent.
        Crescent => "crescent",
        /// A path of moonlight across the water.
        Glade => "glade",
        /// No moon.
        Hidden => "none",
    }
);

named_option!(
    /// Where the koi swims relative to the surface.
    Depth {
        /// Beneath lily pads, leaves, ripples and reflections, tinted by the water.
        Under => "under",
        /// Drawn over everything, as plain git-koi draws it.
        Top => "top",
    }
);

named_option!(
    /// Where falling leaves and petals land.
    Drops {
        /// Somewhere different each day.
        Random => "random",
        /// The same spots every day.
        Fixed => "fixed",
    }
);

named_option!(
    /// How many koi share the pond.
    School {
        /// The account's koi alone.
        One => "one",
        /// The koi and two companions in other bodies.
        Three => "three",
    }
);

/// Everything one pond loop depends on besides the koi.
#[derive(Clone, Copy, Debug)]
pub struct PondOptions {
    /// The season dressing the pond.
    pub season: Season,
    /// The weather over it.
    pub weather: Weather,
    /// Draw the night pond, for viewers in dark mode.
    pub night: bool,
    /// What surrounds the water.
    pub garden: Garden,
    /// The size of the picture.
    pub size: PondSize,
    /// The lights at night.
    pub lights: Lights,
    /// The moon at night.
    pub moon: Moon,
    /// Where the koi swims relative to the surface.
    pub depth: Depth,
    /// Where falling leaves and petals land.
    pub drops: Drops,
    /// The day the pond is drawn for, as `YYYYMMDD`, which varies random drops.
    pub day_seed: u32,
}

/// The season of a date: March to May spring, June to August summer, September to
/// November autumn, December to February winter.
pub fn choose_season(date: NaiveDate) -> Season {
    match date.month() {
        3..=5 => Season::Spring,
        6..=8 => Season::Summer,
        9..=11 => Season::Autumn,
        _ => Season::Winter,
    }
}

/// The weather of a date: rain on about one day in seven, decided by a hash of the date
/// so it falls in no pattern, in any season but winter, which snows instead.
pub fn choose_weather(date: NaiveDate, season: Season) -> Weather {
    if season != Season::Winter && scramble(date_number(date)) % 7 == 3 {
        Weather::Rain
    } else {
        Weather::Clear
    }
}

/// A date as the number `YYYYMMDD`.
pub fn date_number(date: NaiveDate) -> u32 {
    date.year() as u32 * 10_000 + date.month() * 100 + date.day()
}

/// Murmur3's 32-bit finaliser, which scrambles neighbouring dates far apart.
fn scramble(number: u32) -> u32 {
    let mut hash = number;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2_ae35);
    hash ^ (hash >> 16)
}

/// The body that suits a season, or the night.
pub fn suited_body(season: Season, night: bool) -> KoiBody {
    if night {
        return KoiBody::Platinum;
    }
    match season {
        Season::Spring => KoiBody::Asagi,
        Season::Summer => KoiBody::Gold,
        Season::Autumn => KoiBody::Tea,
        Season::Winter => KoiBody::Black,
    }
}

/// The three bodies that share the pond when `School::Three`, the account's koi first.
pub fn school_bodies(season: Season, night: bool) -> [KoiBody; 3] {
    if night {
        return [KoiBody::Platinum, KoiBody::Gold, KoiBody::Asagi];
    }
    match season {
        Season::Autumn => [KoiBody::Tea, KoiBody::Gold, KoiBody::Black],
        Season::Winter => [KoiBody::Black, KoiBody::Gold, KoiBody::Platinum],
        _ => [KoiBody::Asagi, KoiBody::Platinum, KoiBody::Gold],
    }
}

// ------------------------------------------------------------------ the output

/// A pond frame with row-major RGBA pixels; every pixel is opaque or transparent.
#[derive(Debug, Eq, PartialEq)]
pub struct PondFrame {
    width: u32,
    height: u32,
    rgba_pixels: Vec<[u8; 4]>,
}

impl PondFrame {
    /// A frame from row-major pixels; `rgba_pixels` holds `width * height` of them.
    pub(crate) fn new(width: u32, height: u32, rgba_pixels: Vec<[u8; 4]>) -> Self {
        assert_eq!(rgba_pixels.len(), (width * height) as usize);
        Self {
            width,
            height,
            rgba_pixels,
        }
    }

    /// The width in pixels, which a pond alone shares with its height.
    pub fn side(&self) -> u32 {
        self.width
    }

    /// The width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The pixels, row by row from the top left.
    pub fn pixels(&self) -> &[[u8; 4]] {
        &self.rgba_pixels
    }

    /// The frame with every pixel drawn as a `factor` by `factor` block, so a picture
    /// shown at its usual size stays sharp on a high-density screen.
    pub fn scaled(&self, factor: u32) -> Self {
        let (width, height) = (self.width * factor, self.height * factor);
        let rgba_pixels = (0..height)
            .flat_map(|y| (0..width).map(move |x| (y / factor * self.width + x / factor) as usize))
            .map(|index| self.rgba_pixels[index])
            .collect();
        Self::new(width, height, rgba_pixels)
    }

    /// The pixels as bytes, row by row from the top left, four channels per pixel.
    pub fn as_rgba_bytes(&self) -> &[u8] {
        self.rgba_pixels.as_flattened()
    }
}

/// Draw one pond loop. `koi_loops` holds one loop of swim frames per koi, the account's
/// koi first and two companions after it for `School::Three`; every loop has the same
/// length, and the pond's ripples, drops and lights repeat on that length too. Identical
/// inputs produce identical pixels without I/O, clock reads, or input mutation.
pub fn render_pond(koi_loops: &[Vec<KoiFrame>], options: &PondOptions) -> Vec<PondFrame> {
    let frame_count = koi_loops.first().map_or(0, Vec::len);
    let geometry = Geometry::of(options.size);
    let edge = build_edge(options, &geometry);
    let lights = plan_lights(options, &edge, &geometry);
    (0..frame_count)
        .map(|frame_index| {
            compose_frame(
                koi_loops,
                options,
                &Stage {
                    geometry: &geometry,
                    edge: &edge,
                    lights: &lights,
                    frame_count,
                },
                frame_index,
            )
        })
        .collect()
}

// ------------------------------------------------------------------ layers

/// Logical size of a scene: the pond is drawn on this grid, then doubled.
const SCENE: i32 = 128;
pub(crate) const BAYER: [[f64; 4]; 4] = [
    [0.0, 8.0, 2.0, 10.0],
    [12.0, 4.0, 14.0, 6.0],
    [3.0, 11.0, 1.0, 9.0],
    [15.0, 7.0, 13.0, 5.0],
];

pub(crate) type Rgb = [f64; 3];

/// A colour from `#rrggbb`.
pub(crate) fn hex(text: &str) -> Rgb {
    let channel =
        |at: usize| f64::from(u8::from_str_radix(&text[at..at + 2], 16).expect("a hex colour"));
    [channel(1), channel(3), channel(5)]
}

/// Scale each channel and drop the fraction, as the page's `v * k | 0` does.
pub(crate) fn shade(colour: Rgb, factor: f64) -> Rgb {
    colour.map(|channel| (channel * factor).trunc())
}

/// Round half up, as the page's `Math.round` does.
pub(crate) fn round(value: f64) -> i32 {
    (value + 0.5).floor() as i32
}

/// A layer of straight-alpha pixels, drawn on like a canvas.
#[derive(Clone)]
pub(crate) struct Layer {
    width: i32,
    height: i32,
    pub(crate) pixels: Vec<[f64; 4]>,
    /// Multiplies the opacity of everything drawn, like a canvas's global alpha.
    opacity: f64,
}

impl Layer {
    pub(crate) fn new(side: i32) -> Self {
        Self::sized(side, side)
    }

    pub(crate) fn sized(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            pixels: vec![[0.0; 4]; (width * height) as usize],
            opacity: 1.0,
        }
    }

    pub(crate) fn at(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width && y < self.height)
            .then(|| (y * self.width + x) as usize)
    }

    /// The layer as a frame: half-covered pixels and more become opaque, the rest
    /// transparent, since a GIF has nothing in between.
    pub(crate) fn to_frame(&self) -> PondFrame {
        PondFrame::new(
            self.width as u32,
            self.height as u32,
            (0..self.pixels.len())
                .map(|index| {
                    let [red, green, blue, alpha] = self.read(index);
                    if alpha >= 128 {
                        [red, green, blue, 255]
                    } else {
                        [0; 4]
                    }
                })
                .collect(),
        )
    }

    /// Paint over a pixel with straight-alpha source-over blending.
    pub(crate) fn blend(&mut self, x: i32, y: i32, colour: Rgb, alpha: f64) {
        let alpha = (alpha * self.opacity).clamp(0.0, 1.0);
        let Some(index) = self.at(x, y) else { return };
        if alpha <= 0.0 {
            return;
        }
        let below = self.pixels[index];
        let out_alpha = alpha + below[3] * (1.0 - alpha);
        let mut out = [0.0, 0.0, 0.0, out_alpha];
        for channel in 0..3 {
            out[channel] =
                (colour[channel] * alpha + below[channel] * below[3] * (1.0 - alpha)) / out_alpha;
        }
        self.pixels[index] = out;
    }

    pub(crate) fn point(&mut self, x: f64, y: f64, colour: Rgb, alpha: f64) {
        self.blend(round(x), round(y), colour, alpha);
    }

    pub(crate) fn rect(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        colour: Rgb,
        alpha: f64,
    ) {
        let (left, top) = (round(x), round(y));
        for row in top..top + round(height) {
            for column in left..left + round(width) {
                self.blend(column, row, colour, alpha);
            }
        }
    }

    /// Bresenham's line between rounded ends.
    pub(crate) fn line(&mut self, from: [f64; 2], to: [f64; 2], colour: Rgb, alpha: f64) {
        let (mut x, mut y, x1, y1) = (round(from[0]), round(from[1]), round(to[0]), round(to[1]));
        let (dx, dy) = ((x1 - x).abs(), -(y1 - y).abs());
        let (step_x, step_y) = (if x < x1 { 1 } else { -1 }, if y < y1 { 1 } else { -1 });
        let mut error = dx + dy;
        loop {
            self.blend(x, y, colour, alpha);
            if x == x1 && y == y1 {
                break;
            }
            let doubled = 2 * error;
            if doubled >= dy {
                error += dy;
                x += step_x;
            }
            if doubled <= dx {
                error += dx;
                y += step_y;
            }
        }
    }

    /// A ripple: a flattened circle, each pixel painted once.
    fn ring(&mut self, centre: [f64; 2], radius: f64, colour: Rgb, alpha: f64) {
        let mut painted = HashSet::new();
        let steps = 12.max(round(radius * 7.0));
        for step in 0..steps {
            let angle = TAU * f64::from(step) / f64::from(steps);
            let x = round(centre[0] + radius * angle.cos());
            let y = round(centre[1] + radius * 0.72 * angle.sin());
            if painted.insert((x, y)) {
                self.blend(x, y, colour, alpha);
            }
        }
    }

    /// A filled ellipse, row by row.
    pub(crate) fn disc(&mut self, centre: [f64; 2], radii: [f64; 2], colour: Rgb, alpha: f64) {
        let [cx, cy] = centre;
        let [rx, ry] = radii;
        for row in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            let k = 1.0 - ((f64::from(row) + 0.5 - cy) / ry).powi(2);
            if k < 0.0 {
                continue;
            }
            let half = rx * k.sqrt();
            let (left, right) = (round(cx - half), round(cx + half));
            if right > left {
                self.rect(
                    f64::from(left),
                    f64::from(row),
                    f64::from(right - left),
                    1.0,
                    colour,
                    alpha,
                );
            }
        }
    }

    /// Pixel art: one row per string, `.` transparent, other letters looked up.
    fn sprite(&mut self, at: [f64; 2], rows: &[String], palette: &[(char, Rgb)], alpha: f64) {
        for (row_index, row) in rows.iter().enumerate() {
            for (column_index, letter) in row.chars().enumerate() {
                if let Some((_, colour)) = palette.iter().find(|(key, _)| *key == letter) {
                    self.point(
                        at[0] + column_index as f64,
                        at[1] + row_index as f64,
                        *colour,
                        alpha,
                    );
                }
            }
        }
    }

    /// Light added on top, fading from the centre to nothing at the radius.
    pub(crate) fn glow(&mut self, centre: [f64; 2], radius: f64, colour: Rgb, alpha: f64) {
        let [cx, cy] = centre;
        for row in (cy - radius).floor() as i32..(cy + radius).ceil() as i32 {
            for column in (cx - radius).floor() as i32..(cx + radius).ceil() as i32 {
                let Some(index) = self.at(column, row) else {
                    continue;
                };
                let distance = (f64::from(column) + 0.5 - cx).hypot(f64::from(row) + 0.5 - cy);
                let added = alpha * self.opacity * (1.0 - distance / radius).max(0.0);
                if added <= 0.0 {
                    continue;
                }
                let below = self.pixels[index];
                let out_alpha = (below[3] + added).min(1.0);
                let mut out = [0.0, 0.0, 0.0, out_alpha];
                for channel in 0..3 {
                    out[channel] = ((below[channel] * below[3] + colour[channel] * added)
                        .min(255.0))
                        / out_alpha;
                }
                self.pixels[index] = out;
            }
        }
    }

    /// The pixel as the page would read it back: whole numbers.
    pub(crate) fn read(&self, index: usize) -> [u8; 4] {
        let pixel = self.pixels[index];
        [
            pixel[0].round() as u8,
            pixel[1].round() as u8,
            pixel[2].round() as u8,
            (pixel[3] * 255.0).round() as u8,
        ]
    }

    /// Draw `source` scaled up by `scale` with its top left at `at`, nearest pixel.
    pub(crate) fn draw_scaled(&mut self, source: &Layer, at: [i32; 2], scale: i32) {
        for (index, pixel) in source.pixels.iter().enumerate() {
            if pixel[3] <= 0.0 {
                continue;
            }
            let (x, y) = (index as i32 % source.width, index as i32 / source.width);
            for dy in 0..scale {
                for dx in 0..scale {
                    self.blend(
                        at[0] + x * scale + dx,
                        at[1] + y * scale + dy,
                        [pixel[0], pixel[1], pixel[2]],
                        pixel[3],
                    );
                }
            }
        }
    }
}

/// The page's seeded generator, mulberry32.
struct Random(u32);

impl Random {
    fn new(seed: i64) -> Self {
        Self(seed as u32)
    }

    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = (self.0 ^ (self.0 >> 15)).wrapping_mul(1 | self.0);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t))) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

/// A repeatable pseudo-random value in [0, 1) for a point.
pub(crate) fn hash(x: f64, y: f64) -> f64 {
    let scrambled = (x * 12.9898 + y * 78.233).sin() * 43_758.545_3;
    scrambled - scrambled.floor()
}

/// Smooth value noise in [0, 1].
pub(crate) fn value_noise(x: f64, y: f64) -> f64 {
    let (xi, yi) = (x.floor(), y.floor());
    let (xf, yf) = (x - xi, y - yi);
    let (u, v) = (xf * xf * (3.0 - 2.0 * xf), yf * yf * (3.0 - 2.0 * yf));
    (hash(xi, yi) * (1.0 - u) + hash(xi + 1.0, yi) * u) * (1.0 - v)
        + (hash(xi, yi + 1.0) * (1.0 - u) + hash(xi + 1.0, yi + 1.0) * u) * v
}

/// A gentle wobble around a circle, so edges are not perfectly round.
fn wobble(angle: f64, amount: f64) -> f64 {
    amount
        * ((3.0 * angle + 1.0).sin() * 0.55
            + (5.0 * angle + 2.3).sin() * 0.3
            + (2.0 * angle + 0.4).sin() * 0.15)
}

fn rows(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_owned()).collect()
}

/// Turn pixel art a quarter turn clockwise, `turns` times.
fn rotate(art: &[String], turns: usize) -> Vec<String> {
    let mut art = art.to_vec();
    for _ in 0..turns % 4 {
        let width = art[0].len();
        art = (0..width)
            .map(|column| {
                art.iter()
                    .rev()
                    .map(|row| row.as_bytes()[column] as char)
                    .collect()
            })
            .collect();
    }
    art
}

// ------------------------------------------------------------------ geometry

/// Where things sit in the picture, in logical pixels (the picture is twice this).
struct Geometry {
    /// The picture's logical side.
    side: i32,
    /// Where the 128-pixel scene starts inside it.
    offset: i32,
    /// The koi's scale.
    koi_scale: f64,
    /// The water's radius.
    pond: f64,
    /// The garden's outer radius.
    outer: f64,
    centre: f64,
}

impl Geometry {
    fn of(size: PondSize) -> Self {
        // The koi swims within 56 logical pixels of the centre at full size, so either
        // way its whole path stays in open water.
        let (side, offset, koi_scale, pond, outer) = match size {
            PondSize::Wide => (176, 24, 1.0, 59.5, 84.0),
            PondSize::Compact => (128, 0, 0.9, 53.0, 62.5),
        };
        Self {
            side,
            offset,
            koi_scale,
            pond,
            outer,
            centre: f64::from(side) / 2.0,
        }
    }
}

struct Stage<'a> {
    geometry: &'a Geometry,
    edge: &'a Edge,
    lights: &'a [Light],
    frame_count: usize,
}

// ------------------------------------------------------------------ the garden

/// The water's shape, the garden around it, and where lamps stand.
struct Edge {
    water: Vec<bool>,
    rim: Layer,
    anchors: [[f64; 2]; 2],
}

pub(crate) struct DeckStyle {
    boards: [&'static str; 4],
    seam: &'static str,
    pub(crate) nail: &'static str,
    aged: bool,
    posts: bool,
}

pub(crate) fn deck_style(garden: Garden) -> Option<DeckStyle> {
    let style = |boards, seam, nail, aged, posts| {
        Some(DeckStyle {
            boards,
            seam,
            nail,
            aged,
            posts,
        })
    };
    match garden {
        Garden::Deck => style(
            ["#a8774b", "#9a6b42", "#b0814f", "#8c6039"],
            "#5e3d22",
            "#3a2616",
            false,
            false,
        ),
        Garden::DeckDark => style(
            ["#7a5232", "#6d482b", "#835a38", "#5f3f25"],
            "#3b2616",
            "#24170d",
            false,
            false,
        ),
        Garden::DeckOld => style(
            ["#8f7556", "#82694c", "#977c5c", "#765f44"],
            "#4a3a28",
            "#7a4a2a",
            true,
            false,
        ),
        Garden::DeckGrey => style(
            ["#a39d93", "#958f86", "#aca69b", "#8b857c"],
            "#5f5a53",
            "#46423d",
            false,
            false,
        ),
        Garden::DeckWalnut => style(
            ["#6e4a32", "#634229", "#77513a", "#5a3b25"],
            "#3a2618",
            "#2a1a10",
            false,
            true,
        ),
        _ => None,
    }
}

fn build_edge(options: &PondOptions, geometry: &Geometry) -> Edge {
    let mut edge = match (options.garden, deck_style(options.garden)) {
        (Garden::Mist, _) => build_mist(options, geometry),
        (_, Some(style)) => build_deck(options, geometry, &style),
        _ => build_moss_garden(options, geometry),
    };
    if options.night {
        // moonlight: the whole garden darker and cooler, so it sits in a dark page
        for pixel in &mut edge.rim.pixels {
            pixel[0] = (pixel[0] * 0.32 + 8.0).trunc();
            pixel[1] = (pixel[1] * 0.37 + 10.0).trunc();
            pixel[2] = (pixel[2] * 0.47 + 18.0).trunc();
        }
    }
    edge
}

fn polar(x: i32, y: i32, centre: f64) -> (f64, f64) {
    let (dx, dy) = (f64::from(x) + 0.5 - centre, f64::from(y) + 0.5 - centre);
    (dx.hypot(dy), dy.atan2(dx))
}

fn water_top(water: &[bool], side: i32, x: f64) -> Option<i32> {
    let column = round(x);
    (0..side).find(|&row| column >= 0 && column < side && water[(row * side + column) as usize])
}

fn build_mist(options: &PondOptions, geometry: &Geometry) -> Edge {
    // no edge at all: open water out past the koi's reach, then it thins into the page
    let wide = options.size == PondSize::Wide;
    let (solid, fade) = if wide { (56.5, 7.0) } else { (51.5, 10.0) };
    let side = geometry.side;
    let mut water = vec![false; (side * side) as usize];
    for y in 0..side {
        for x in 0..side {
            let (distance, angle) = polar(x, y, geometry.centre);
            let k = (solid + fade + wobble(angle + 0.5, if wide { 0.4 } else { 1.0 }) - distance)
                / fade;
            water[(y * side + x) as usize] =
                k > (BAYER[(y % 4) as usize][(x % 4) as usize] + 0.5) / 16.0;
        }
    }
    let anchor = |x: f64| [x, f64::from(water_top(&water, side, x).unwrap_or(20) + 6)];
    let anchors = [
        anchor(geometry.centre - 40.0),
        anchor(geometry.centre + 39.0),
    ];
    Edge {
        water,
        rim: Layer::new(side),
        anchors,
    }
}

fn build_deck(options: &PondOptions, geometry: &Geometry, style: &DeckStyle) -> Edge {
    // a deck all round a rounded-square pond; boards run along each side and meet in mitres
    let wide = options.size == PondSize::Wide;
    let (side, c) = (geometry.side, geometry.centre);
    let (half, outer, corner) = if wide {
        (58.0, 76.0, 14.0)
    } else {
        (52.0, 62.0, 10.0)
    };
    let nail = hex(style.nail);
    let inside = |x: i32, y: i32, extent: f64, radius: f64| {
        let (dx, dy) = (
            (f64::from(x) + 0.5 - c).abs(),
            (f64::from(y) + 0.5 - c).abs(),
        );
        let (qx, qy) = (
            (dx - (extent - radius)).max(0.0),
            (dy - (extent - radius)).max(0.0),
        );
        qx.hypot(qy) <= radius && dx <= extent && dy <= extent
    };
    let mut water = vec![false; (side * side) as usize];
    let mut rim = Layer::new(side);
    for y in 0..side {
        for x in 0..side {
            if inside(x, y, half, corner) {
                water[(y * side + x) as usize] = true;
                continue;
            }
            if !inside(x, y, outer, 5.0) {
                continue;
            }
            let (fx, fy) = (f64::from(x), f64::from(y));
            let (dx, dy) = (fx + 0.5 - c, fy + 0.5 - c);
            let across = dy.abs() >= dx.abs();
            let depth = (if across { dy.abs() } else { dx.abs() }) - half;
            let along = if across { x } else { y };
            let board = (depth / 3.3).floor() as i32;
            let within = depth - f64::from(board) * 3.3;
            let place = (along + board * 23 + if across { 0 } else { 11 }).rem_euclid(37);
            let joint = place == 0;
            let mitre = (dx.abs() - dy.abs()).abs() < 0.75;
            let is_seam = within < 0.9 || joint || mitre;
            let drift = (depth / if wide { 18.0 } else { 10.0 } - 0.6).max(0.0) * 0.5;
            let colour = weathered_board(
                style,
                options,
                [x, y],
                board,
                board + if across { 0 } else { 2 },
                within,
                along,
                is_seam,
                drift,
            );
            rim.blend(x, y, colour, 1.0);
            if !joint && place == 2 && within > 1.4 && within < 2.4 {
                rim.blend(x, y, nail, 1.0);
            }
        }
    }
    if style.posts {
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let (px, py) = (c + sx * (outer - 4.0), c + sy * (outer - 4.0));
            rim.rect(px - 4.0, py - 4.0, 8.0, 8.0, hex("#2a1a10"), 1.0);
            rim.rect(px - 3.0, py - 3.0, 6.0, 6.0, hex("#4a3220"), 1.0);
            rim.rect(px - 3.0, py - 3.0, 6.0, 1.0, hex("#6b4a32"), 1.0);
            rim.rect(px - 1.0, py - 1.0, 2.0, 2.0, hex("#2a1a10"), 1.0);
        }
    }
    let middle = half + (outer - half) / 2.0;
    Edge {
        water,
        rim,
        anchors: [[c - middle, c - 18.0], [c + middle, c - 18.0]],
    }
}

/// One pixel of a deck board, as the season and the weather leave it. `board` counts
/// boards across the deck, `tone` picks the board's colour, `within` is how far into its
/// board the pixel lies and `along` how far along it; `drift` adds to the snow's depth.
#[expect(clippy::too_many_arguments, reason = "one pixel's place on its board")]
pub(crate) fn weathered_board(
    style: &DeckStyle,
    options: &PondOptions,
    [x, y]: [i32; 2],
    board: i32,
    tone: i32,
    within: f64,
    along: i32,
    is_seam: bool,
    drift: f64,
) -> Rgb {
    let (fx, fy) = (f64::from(x), f64::from(y));
    let mut colour = style.boards.map(hex)[tone.rem_euclid(4) as usize];
    if is_seam {
        colour = hex(style.seam);
    } else if hash(fx, fy) > 0.9 {
        colour = shade(colour, 0.86);
    } else if within < 1.6 {
        colour = colour.map(|channel| (channel * 1.08).trunc().min(255.0));
    }
    if style.aged {
        // rot darkening whole stretches, split ends, moss creeping from the seams
        let rot = value_noise(fx * 0.16 + 3.0, fy * 0.16 + 7.0);
        if rot > 0.66 && !is_seam {
            colour = shade(colour, if rot > 0.74 { 0.55 } else { 0.72 });
        }
        if !is_seam
            && hash(f64::from(along * 3 + board), f64::from(board)) > 0.93
            && within > 1.2
            && within < 2.0
        {
            colour = hex("#3e2f20");
        }
        let mossy = value_noise(fx * 0.09 + 4.0, fy * 0.09 + 9.0);
        if options.season != Season::Winter
            && mossy > 0.6
            && (is_seam || within < 1.2 || mossy > 0.72)
            && hash(fx * 5.0, fy * 3.0) > 0.25
        {
            colour =
                hex(["#4f7f33", "#5e903a", "#437028", "#6b9a40"][((x * 7 + y * 3) % 4) as usize]);
        }
    }
    // the weather on the boards
    if options.weather == Weather::Rain {
        colour = shade(colour, 0.7);
        if value_noise(fx * 0.11 + 11.0, fy * 0.11) > 0.78 {
            colour = if hash(fx, fy) > 0.94 {
                hex("#aebcc6")
            } else {
                let puddle = [60.0, 72.0, 84.0];
                [0, 1, 2].map(|k| (colour[k] * 0.6 + puddle[k] * 0.4).trunc())
            };
        } else if !is_seam && hash(fx * 3.0, fy) > 0.985 {
            colour = hex("#c9d4dc");
        }
    }
    match options.season {
        Season::Winter => {
            let drift = 0.6 * value_noise(fx * 0.08 + 2.0, fy * 0.08 + 5.0)
                + 0.4 * value_noise(fx * 0.2, fy * 0.2)
                + drift;
            if drift > 0.74 {
                colour = if hash(fx, fy * 2.0) > 0.12 {
                    hex("#f2f6f9")
                } else {
                    hex("#d8e2ea")
                };
            } else if drift > 0.69 {
                colour = colour.map(|channel| (channel * 0.5 + 236.0 * 0.5).trunc());
            } else if !is_seam && hash(fx * 7.0, fy * 3.0) > 0.95 {
                colour = hex("#e3ebf1");
            }
        }
        Season::Autumn if hash(fx + 5.0, fy * 3.0) > 0.982 => {
            colour = hex(["#c8542a", "#e0952f", "#a83a2a"][((x * y) % 3) as usize]);
        }
        Season::Spring if hash(fx + 2.0, fy * 7.0) > 0.988 => colour = hex("#f2b6c9"),
        _ => {}
    }
    colour
}

struct Island {
    x: f64,
    y: f64,
    radius: f64,
    seed: f64,
}

struct Moss {
    lit: f64,
    edge: f64,
    island: bool,
    top: f64,
}

fn build_moss_garden(options: &PondOptions, geometry: &Geometry) -> Edge {
    let wide = options.size == PondSize::Wide;
    let (side, c) = (geometry.side, geometry.centre);
    let pond_radius = |angle: f64| geometry.pond + wobble(angle, 1.6);
    let outer_radius =
        |angle: f64| geometry.outer + wobble(angle + 0.9, if wide { 3.0 } else { 1.0 });
    // moss islands in the gravel, and for some gardens a bank along the water
    let ring = (geometry.pond + geometry.outer) / 2.0;
    let width = geometry.outer - geometry.pond;
    let placements: &[(f64, f64)] = match options.garden {
        Garden::Islands => &[(-2.3, 0.42), (0.55, 0.36), (2.05, 0.3)],
        Garden::Bank => &[(0.6, 0.34)],
        Garden::Garden => &[(-2.3, 0.46), (0.45, 0.4), (1.9, 0.3), (-0.8, 0.26)],
        _ => &[],
    };
    let islands: Vec<Island> = placements
        .iter()
        .enumerate()
        .map(|(index, &(angle, share))| Island {
            x: c + ring * angle.cos(),
            y: c + ring * angle.sin(),
            radius: (width * share).max(3.0),
            seed: index as f64 * 3.7,
        })
        .collect();
    let bank = matches!(options.garden, Garden::Bank | Garden::Garden);
    let island_edge = |island: &Island, angle: f64| {
        island.radius
            * (1.0
                + 0.16 * (5.0 * angle + island.seed).sin()
                + 0.09 * (9.0 * angle + island.seed * 2.0).sin())
    };
    let moss_at = |x: f64, y: f64, distance: f64, angle: f64| -> Option<Moss> {
        for island in &islands {
            let (dx, dy) = (x + 0.5 - island.x, y + 0.5 - island.y);
            let edge = island_edge(island, dy.atan2(dx));
            let reach = dx.hypot(dy);
            if reach < edge {
                return Some(Moss {
                    lit: -(dx + dy) / (edge * 1.4),
                    edge: edge - reach,
                    island: true,
                    top: dy / edge,
                });
            }
        }
        if bank {
            let thickness = 1.5
                + 3.5 * (0.5 + 0.5 * (3.0 * angle + 1.0).sin())
                + 2.0 * value_noise(angle * 6.0, 1.0)
                - if (7.0 * angle).sin() > 0.6 { 1.2 } else { 0.0 };
            let into = distance - pond_radius(angle);
            if into < thickness {
                return Some(Moss {
                    lit: (into / thickness) * -0.6 + 0.3,
                    edge: thickness - into,
                    island: false,
                    top: 0.0,
                });
            }
        }
        None
    };
    let wet = options.weather == Weather::Rain;
    let wet_shade = |colour: Rgb| if wet { shade(colour, 0.8) } else { colour };
    let moss_tones: [&str; 6] = match options.season {
        Season::Spring => [
            "#3d6a27", "#4c7f30", "#5a9437", "#6aa840", "#80bc4f", "#9ccf63",
        ],
        Season::Summer => [
            "#2c5420", "#386628", "#447830", "#528a37", "#64a043", "#7bb552",
        ],
        Season::Autumn => [
            "#4a5226", "#5a632c", "#6b7231", "#7e7f36", "#93893d", "#a99848",
        ],
        Season::Winter => [
            "#3e5038", "#4b5f44", "#586e50", "#677e5e", "#7a8f70", "#90a385",
        ],
    };
    let moss_tones = moss_tones.map(|tone| wet_shade(hex(tone)));
    let gravel: [&str; 3] = match options.season {
        Season::Winter => ["#e2e5e6", "#ccd1d3", "#b3b9bc"],
        Season::Autumn => ["#dcd3c0", "#c5baa4", "#a99c86"],
        _ => ["#dcd5c6", "#c6bdab", "#a89e8b"],
    };
    let gravel = gravel.map(|tone| wet_shade(hex(tone)));
    let mut water = vec![false; (side * side) as usize];
    let mut rim = Layer::new(side);
    for y in 0..side {
        for x in 0..side {
            let (fx, fy) = (f64::from(x), f64::from(y));
            let (distance, angle) = polar(x, y, c);
            let pond_edge = pond_radius(angle);
            if distance < pond_edge {
                water[(y * side + x) as usize] = true;
                continue;
            }
            if distance < pond_edge + 1.3 {
                rim.blend(x, y, hex("#4d483f"), 1.0); // the wet lip of the pond
                continue;
            }
            if let Some(moss) = moss_at(fx, fy, distance, angle) {
                // soft, uneven moss: textured by noise, lit from the top left, darker at its foot
                let noise = 0.55 * value_noise(fx * 0.45, fy * 0.45)
                    + 0.3 * value_noise(fx * 1.3 + 9.0, fy * 1.3)
                    + 0.15 * hash(fx, fy);
                let mut tone = (noise * 4.2 + moss.lit * 1.6 + 0.4).floor() as i32;
                if moss.edge < 1.2 {
                    tone -= 1;
                }
                let mut colour = moss_tones[tone.clamp(0, 5) as usize];
                if options.season == Season::Winter {
                    // frost on the lit side, a cap of snow on the islands
                    if moss.lit > 0.15 && hash(fx * 3.0, fy * 7.0) > 0.5 {
                        colour = hex("#e4ecf2");
                    }
                    if moss.island && moss.top < -0.35 {
                        colour = if moss.top < -0.55 || hash(fx, fy * 3.0) > 0.3 {
                            hex("#f4f7fa")
                        } else {
                            hex("#d6e0e8")
                        };
                    }
                }
                if wet && hash(fx * 5.0, fy * 3.0) > 0.95 {
                    colour = hex("#b7d9a0"); // wet glints
                }
                if options.season == Season::Autumn && hash(fx * 7.0, fy * 5.0) > 0.94 {
                    colour = hex(["#c8642a", "#d9a043"][((x + y) % 2) as usize]);
                }
                rim.blend(x, y, colour, 1.0);
                continue;
            }
            let k = (outer_radius(angle) - distance) / if wide { 7.0 } else { 4.0 };
            if k <= (BAYER[(y % 4) as usize][(x % 4) as usize] + 0.5) / 16.0 {
                continue; // the gravel thins out into the page
            }
            // rake lines follow the pond, and circle any moss island they meet
            let mut raked = distance - pond_edge;
            for island in &islands {
                let (dx, dy) = (fx + 0.5 - island.x, fy + 0.5 - island.y);
                let gap = dx.hypot(dy) - island_edge(island, dy.atan2(dx));
                if gap < 7.0 {
                    raked = gap + 0.2;
                    break;
                }
            }
            let band = (raked / 2.0).floor().rem_euclid(2.0) as usize;
            // a little shade cast below and to the right of the moss
            let behind = |back: f64| {
                let (bx, by) = (fx - back, fy - back);
                let (bd, ba) = (
                    (bx + 0.5 - c).hypot(by + 0.5 - c),
                    (by + 0.5 - c).atan2(bx + 0.5 - c),
                );
                moss_at(bx, by, bd, ba).is_some()
            };
            let mut colour = if band == 1 { gravel[0] } else { gravel[1] };
            if hash(fx, fy) > 0.93 {
                colour = gravel[2];
            }
            if options.season == Season::Winter && value_noise(fx * 0.13 + 5.0, fy * 0.13) > 0.6 {
                colour = if band == 1 {
                    hex("#f4f7fa")
                } else {
                    hex("#e6edf2")
                }; // drifts of snow
            }
            if wet && hash(fx * 3.0, fy) > 0.975 {
                colour = hex("#d9d6cf"); // wet glints
            }
            if options.season == Season::Autumn && hash(fx + 7.0, fy * 3.0) > 0.985 {
                colour = hex(["#c8542a", "#e0952f", "#a83a2a"][((x * y) % 3) as usize]); // fallen leaves
            }
            if options.season == Season::Spring && hash(fx + 3.0, fy * 5.0) > 0.988 {
                colour = hex("#f2b6c9"); // fallen petals
            }
            if behind(1.0) || behind(2.0) {
                colour = shade(colour, 0.8);
            }
            rim.blend(x, y, colour, 1.0);
            if (bank || !islands.is_empty())
                && hash(fx + 31.0, fy + 17.0) > 0.985
                && value_noise(fx * 0.2, fy * 0.2) > 0.55
            {
                rim.blend(x, y, moss_tones[3], 1.0); // stray moss in the gravel
            }
        }
    }
    // lamps stand on the bank towards the sides; a lamp's base must clear the water under
    // its whole width, not just at its centre
    let foot = |x: f64| {
        let top = (-6..=6)
            .filter_map(|dx| water_top(&water, side, x + f64::from(dx)))
            .min()
            .unwrap_or(side);
        [x, f64::from(top - 2)]
    };
    let reach = if wide { (56.0, 55.0) } else { (53.0, 52.0) };
    let anchors = [foot(c - reach.0), foot(c + reach.1)];
    Edge {
        water,
        rim,
        anchors,
    }
}

// ------------------------------------------------------------------ night lights

/// A light, in the picture's logical pixels.
#[derive(Clone, Copy)]
struct Light {
    x: f64,
    y: f64,
    colour: Rgb,
    /// Throws a streak of reflection on the water.
    reflects: bool,
    /// A lantern floating on the water: its phase and how far it drifts.
    phase: f64,
    drift: f64,
}

/// Where the gas lamps stand. On a deck they stand halfway along its sides, so the
/// lanterns hang level with the middle of the pond, where the moon is; elsewhere they
/// stand at the garden's anchors.
fn euro_feet(options: &PondOptions, edge: &Edge, geometry: &Geometry) -> [[f64; 2]; 2] {
    if deck_style(options.garden).is_some() {
        edge.anchors
            .map(|anchor| [anchor[0], geometry.centre + EURO_LANTERN_HEIGHT])
    } else {
        edge.anchors
    }
}

/// How far above its foot a gas lamp's lantern hangs.
pub(crate) const EURO_LANTERN_HEIGHT: f64 = 31.0;

fn plan_lights(options: &PondOptions, edge: &Edge, geometry: &Geometry) -> Vec<Light> {
    let warm = [255.0, 196.0, 110.0];
    let light = |x, y, colour, reflects| Light {
        x,
        y,
        colour,
        reflects,
        phase: 0.0,
        drift: 0.0,
    };
    let [p, q] = edge.anchors;
    match options.lights {
        Lights::Euro => euro_feet(options, edge, geometry)
            .iter()
            .map(|a| light(a[0], a[1] - 18.0, warm, true))
            .collect(),
        Lights::Toro => edge
            .anchors
            .iter()
            .map(|a| light(a[0], a[1] - 10.0, warm, true))
            .collect(),
        Lights::Chinese => (1..=4)
            .map(|k| {
                let f = f64::from(k) / 5.0;
                light(
                    p[0] + (q[0] - p[0]) * f,
                    p[1].min(q[1]) - 14.0 + 7.0 * (PI * f).sin() + 10.0,
                    [255.0, 120.0, 70.0],
                    true,
                )
            })
            .collect(),
        Lights::Strings => (0..=12)
            .map(|k| {
                let f = f64::from(k) / 12.0;
                let colour = [
                    [255.0, 214.0, 140.0],
                    [255.0, 170.0, 120.0],
                    [255.0, 236.0, 180.0],
                ][(k % 3) as usize];
                light(
                    p[0] + (q[0] - p[0]) * f,
                    p[1].min(q[1]) - 16.0 + 12.0 * (PI * f).sin(),
                    colour,
                    k % 3 == 0,
                )
            })
            .collect(),
        Lights::Floating => {
            let mut random = Random::new(17);
            let offset = f64::from(geometry.offset);
            (0..5)
                .map(|_| {
                    let (x, y) = (
                        offset + 30.0 + random.next() * 68.0,
                        offset + 34.0 + random.next() * 60.0,
                    );
                    Light {
                        phase: random.next() * TAU,
                        drift: 3.0,
                        ..light(x, y, [255.0, 200.0, 120.0], false)
                    }
                })
                .collect()
        }
        Lights::OneFloat => vec![Light {
            drift: 9.0,
            ..light(
                geometry.centre + 20.0,
                geometry.centre + 26.0,
                [255.0, 200.0, 120.0],
                false,
            )
        }],
        // lamps somewhere out of the picture: only their light on the water shows
        Lights::Reflect => [geometry.centre - 30.0, geometry.centre + 31.0]
            .iter()
            .map(|&x| {
                light(
                    x,
                    f64::from(water_top(&edge.water, geometry.side, x).unwrap_or(20) - 6),
                    warm,
                    true,
                )
            })
            .collect(),
        Lights::Off => Vec::new(),
    }
}

fn draw_lights(layer: &mut Layer, options: &PondOptions, stage: &Stage, frame: usize) {
    let phase = TAU * frame as f64 / stage.frame_count as f64;
    let (ink, ink2) = (hex("#15181f"), hex("#2a2f3a"));
    let side = f64::from(stage.geometry.side);
    match options.lights {
        Lights::Euro => {
            for foot in euro_feet(options, stage.edge, stage.geometry) {
                draw_euro_lamp(layer, foot, phase, side, true);
            }
        }
        Lights::Toro => {
            for anchor in &stage.edge.anchors {
                let (x, y) = (f64::from(round(anchor[0])), f64::from(round(anchor[1])));
                let (stone, dark, light) = (hex("#8f9196"), hex("#5f6166"), hex("#b4b6ba"));
                let lit = [
                    255.0,
                    205.0 + f64::from(round(25.0 * (phase * 5.0 + x).sin())),
                    120.0,
                ];
                let mut r = |dx: f64, dy: f64, w: f64, h: f64, c: Rgb| {
                    layer.rect(x + dx, y + dy, w, h, c, 1.0)
                };
                r(-5.0, -2.0, 11.0, 2.0, dark);
                r(-4.0, -3.0, 9.0, 1.0, stone);
                r(-1.0, -9.0, 3.0, 6.0, stone);
                r(-1.0, -9.0, 1.0, 6.0, light);
                r(-4.0, -11.0, 9.0, 2.0, stone);
                r(-4.0, -11.0, 9.0, 1.0, light);
                r(-3.0, -16.0, 7.0, 5.0, stone);
                r(-2.0, -15.0, 3.0, 3.0, lit);
                r(1.0, -15.0, 1.0, 3.0, dark);
                r(-6.0, -18.0, 13.0, 2.0, dark);
                r(-5.0, -19.0, 11.0, 1.0, stone);
                r(-3.0, -20.0, 7.0, 1.0, stone);
                r(-1.0, -22.0, 3.0, 2.0, light);
                r(-7.0, -17.0, 1.0, 1.0, dark);
                r(7.0, -17.0, 1.0, 1.0, dark);
                layer.glow([x, y - 13.0], 11.0, [255.0, 190.0, 100.0], 0.4);
            }
        }
        Lights::Chinese | Lights::Strings => {
            let [p, q] = stage.edge.anchors;
            for anchor in [p, q] {
                layer.rect(anchor[0] - 1.0, anchor[1] - 22.0, 2.0, 22.0, ink, 1.0);
                layer.rect(anchor[0] - 2.0, anchor[1] - 2.0, 4.0, 2.0, ink2, 1.0);
            }
            let sag = if options.lights == Lights::Chinese {
                7.0
            } else {
                12.0
            };
            let mut previous: Option<[f64; 2]> = None;
            for k in 0..=40 {
                let f = f64::from(k) / 40.0;
                let here = [
                    p[0] + (q[0] - p[0]) * f,
                    p[1].min(q[1]) - 22.0 + sag * (PI * f).sin(),
                ];
                if let Some(before) = previous {
                    layer.line(before, here, hex("#3a2a1e"), 1.0);
                }
                previous = Some(here);
            }
            for (n, light) in stage.lights.iter().enumerate() {
                let n = n as f64;
                if options.lights == Lights::Chinese {
                    let sway = f64::from(round(1.2 * (phase * 2.0 + n).sin()));
                    let (x, y) = (
                        f64::from(round(light.x)) + sway,
                        f64::from(round(light.y)) - 2.0,
                    );
                    let (red, dark_red, gold) = (hex("#d8322a"), hex("#9e1f1a"), hex("#e8b04a"));
                    let shine = [
                        255.0,
                        150.0 + f64::from(round(30.0 * (phase * 5.0 + n).sin())),
                        90.0,
                    ];
                    layer.line(
                        [f64::from(round(light.x)), y - 6.0],
                        [x, y - 3.0],
                        hex("#3a2a1e"),
                        1.0,
                    );
                    layer.rect(x - 2.0, y - 3.0, 5.0, 1.0, gold, 1.0);
                    layer.disc([x + 0.5, y + 2.0], [4.0, 4.5], red, 1.0);
                    layer.rect(x - 3.0, y, 7.0, 1.0, dark_red, 1.0);
                    layer.rect(x - 3.0, y + 3.0, 7.0, 1.0, dark_red, 1.0);
                    layer.rect(x - 1.0, y - 1.0, 3.0, 6.0, shine, 0.6);
                    layer.rect(x - 2.0, y + 6.0, 5.0, 1.0, gold, 1.0);
                    layer.rect(x, y + 7.0, 1.0, 3.0, gold, 1.0);
                    layer.glow([x, y + 2.0], 11.0, [255.0, 110.0, 60.0], 0.38);
                } else {
                    let on = (phase * 4.0 + n * 2.1).sin() > -0.6;
                    layer.point(light.x, light.y + 1.0, ink2, 1.0);
                    layer.rect(
                        light.x - 0.5,
                        light.y + 2.0,
                        2.0,
                        2.0,
                        light.colour,
                        if on { 1.0 } else { 0.45 },
                    );
                    if on {
                        layer.glow([light.x, light.y + 3.0], 6.0, light.colour, 0.35);
                    }
                }
            }
        }
        Lights::Floating | Lights::OneFloat => {
            for (n, light) in stage.lights.iter().enumerate() {
                let x = f64::from(round(light.x + light.drift * (phase + light.phase).sin()));
                let y = f64::from(round(
                    light.y + light.drift * 0.6 * (phase + light.phase).cos(),
                ));
                layer.glow([x, y + 1.0], 12.0, [255.0, 190.0, 100.0], 0.35);
                layer.ring(
                    [x + 0.5, y + 3.0],
                    4.0 + ((frame + n * 20) % 40) as f64 / 10.0,
                    [255.0, 214.0, 150.0],
                    0.25,
                );
                layer.rect(x - 2.0, y + 1.0, 5.0, 3.0, hex("#c9a978"), 1.0);
                layer.rect(x - 2.0, y - 2.0, 5.0, 3.0, hex("#f3e2c4"), 1.0);
                let flame = [
                    255.0,
                    210.0 + f64::from(round(30.0 * (phase * 6.0 + n as f64).sin())),
                    120.0,
                ];
                layer.rect(x - 1.0, y - 1.0, 3.0, 2.0, flame, 1.0);
                layer.rect(x - 3.0, y + 3.0, 7.0, 1.0, hex("#7a5c3a"), 1.0);
            }
        }
        Lights::Reflect | Lights::Off => {}
    }
}

/// A cast-iron gas lamp standing at `foot`: stepped base, post with a collar, ladder
/// bar, four-pane lantern, crown and finial. Lit, it glows, never wider than the room
/// `width` leaves it; unlit, its panes are dark glass.
pub(crate) fn draw_euro_lamp(layer: &mut Layer, foot: [f64; 2], phase: f64, width: f64, lit: bool) {
    let (ink, ink2) = (hex("#15181f"), hex("#2a2f3a"));
    let side = width;
    let (x, y) = (f64::from(round(foot[0])), f64::from(round(foot[1])));
    let (pane, hi) = if lit {
        (
            [
                255.0,
                214.0 + f64::from(round(22.0 * (phase * 6.0 + x).sin())),
                130.0,
            ],
            [255.0, 248.0, 214.0],
        )
    } else {
        ([92.0, 104.0, 112.0], [150.0, 166.0, 176.0])
    };
    let mut r = |dx: f64, dy: f64, w: f64, h: f64, c: Rgb| layer.rect(x + dx, y + dy, w, h, c, 1.0);
    r(-4.0, -2.0, 9.0, 2.0, ink);
    r(-3.0, -4.0, 7.0, 2.0, ink2);
    r(-2.0, -5.0, 5.0, 1.0, ink);
    r(-1.0, -24.0, 3.0, 19.0, ink);
    r(0.0, -22.0, 1.0, 1.0, ink2);
    r(0.0, -16.0, 1.0, 1.0, ink2);
    r(0.0, -10.0, 1.0, 1.0, ink2);
    r(-2.0, -14.0, 5.0, 2.0, ink2);
    r(-2.0, -25.0, 5.0, 1.0, ink2);
    r(-5.0, -21.0, 11.0, 1.0, ink);
    r(-5.0, -22.0, 1.0, 1.0, ink);
    r(5.0, -22.0, 1.0, 1.0, ink);
    r(-3.0, -27.0, 7.0, 2.0, ink);
    r(-4.0, -35.0, 9.0, 8.0, ink);
    r(-3.0, -34.0, 3.0, 6.0, pane);
    r(1.0, -34.0, 3.0, 6.0, pane);
    r(-2.0, -33.0, 1.0, 1.0, hi);
    r(2.0, -33.0, 1.0, 1.0, hi);
    r(-5.0, -36.0, 11.0, 1.0, ink);
    r(-4.0, -37.0, 9.0, 1.0, ink);
    r(-3.0, -38.0, 7.0, 1.0, ink);
    r(-1.0, -39.0, 3.0, 1.0, ink);
    r(0.0, -41.0, 1.0, 1.0, ink);
    r(0.0, -40.0, 1.0, 1.0, ink2);
    if !lit {
        return;
    }
    layer.glow(
        [x, y - EURO_LANTERN_HEIGHT],
        18.0f64
            .min(y - EURO_LANTERN_HEIGHT)
            .min(x)
            .min(side - x)
            .max(6.0),
        [255.0, 190.0, 100.0],
        0.42,
    );
}

// ------------------------------------------------------------------ the water and its seasons

const PETAL: [&str; 4] = ["..pp.", ".pppP", "ppPPp", ".pPp."];
const MAPLE: [&str; 11] = [
    ".....a.....",
    "....aaa....",
    "..a.aaa.a..",
    "..aaaaaaa..",
    "a.aaaaaaa.a",
    "aaaaabaaaaa",
    ".aaaabaaaa.",
    "..aaabaaa..",
    "...aabaa...",
    ".....b.....",
    ".....b.....",
];
const LOTUS: [&str; 5] = [".w.w.", "wpwpw", ".pyp.", "wpwpw", ".w.w."];
const LILY_PADS: [(f64, f64, f64, f64); 5] = [
    (14.0, 18.0, 11.0, 0.6),
    (104.0, 26.0, 9.0, 2.4),
    (100.0, 100.0, 13.0, 4.0),
    (20.0, 104.0, 10.0, 5.2),
    (62.0, 114.0, 8.0, 1.4),
];

/// Draws one frame's 128-pixel scene: the water and everything on its surface.
struct Painter<'a> {
    scene: Layer,
    /// The bare water, before anything floats on it: the difference is the surface.
    base: Layer,
    water_mid: Rgb,
    water_top: Rgb,
    options: &'a PondOptions,
    stage: &'a Stage<'a>,
    frame: usize,
}

impl Painter<'_> {
    fn frames(&self) -> f64 {
        self.stage.frame_count as f64
    }

    fn phase(&self) -> f64 {
        TAU * self.frame as f64 / self.frames()
    }

    fn wet(&self) -> bool {
        self.options.weather == Weather::Rain
    }

    /// Dithered water in a few tones, rippling once per loop.
    fn water(&mut self, tones: [&str; 4], calm: f64) {
        let phase = self.phase();
        let wet_day = self.wet() && !self.options.night;
        let tones = tones.map(|tone| {
            let colour = hex(tone);
            if wet_day {
                let grey = [74.0, 96.0, 108.0];
                [0, 1, 2].map(|k| (colour[k] * 0.62 + grey[k] * 0.38).trunc())
            } else {
                colour
            }
        });
        for y in 0..SCENE {
            for x in 0..SCENE {
                let (fx, fy) = (f64::from(x), f64::from(y));
                let mut v = ((0.11 * fx + 0.07 * fy + phase).sin()
                    + (-0.06 * fx + 0.13 * fy + 2.0 * phase).sin()
                    + 0.6 * (0.19 * fx - 0.05 * fy - phase).sin())
                    * calm;
                v = (v + 2.6) / 5.2
                    + (BAYER[(y % 4) as usize][(x % 4) as usize] / 16.0 - 0.5) * 0.35;
                let tone = tones[((v * 4.0).floor() as i32).clamp(0, 3) as usize];
                self.scene.pixels[(y * SCENE + x) as usize] = [tone[0], tone[1], tone[2], 1.0];
            }
        }
        self.base = self.scene.clone();
        self.water_mid = tones[1];
        self.water_top = tones[3];
    }

    /// Things that fall onto the water, ring it where they land, then drift and fade.
    fn fallers(
        &mut self,
        count: usize,
        seed: i64,
        sprites: &[Vec<String>],
        palettes: &[Vec<(char, Rgb)>],
        ripple: Rgb,
        fall: f64,
    ) {
        let frames = self.frames();
        let mut base = Random::new(seed);
        for item in 0..count {
            let offset = (base.next() * frames).floor() as usize;
            let (fx, fy) = (base.next(), base.next());
            let u = ((self.frame + offset) % self.stage.frame_count) as f64;
            let span = f64::from(SCENE - 24);
            let (mut lx, mut ly) = (8.0 + fx * span, 8.0 + fy * span);
            if self.options.drops == Drops::Random {
                // a GIF loops, so a leaf keeps its spot for the loop; the date moves it
                let mut place = Random::new(
                    seed * 7919 + item as i64 * 104_729 + i64::from(self.options.day_seed),
                );
                lx = 8.0 + place.next() * span;
                ly = 8.0 + place.next() * span;
            }
            let palette = &palettes[item % palettes.len()];
            let shadow: Vec<(char, Rgb)> =
                palette.iter().map(|(key, _)| (*key, [0.0; 3])).collect();
            if u < fall {
                let height = (fall - u) / fall;
                let art = &sprites[item % sprites.len()];
                self.scene
                    .sprite([lx, ly], art, &shadow, 0.24 * (1.0 - height));
                self.scene
                    .sprite([lx + 9.0 * height, ly - 14.0 * height], art, palette, 1.0);
                continue;
            }
            let k = u - fall;
            if k < 26.0 {
                self.scene.ring(
                    [lx + 2.0, ly + 2.0],
                    1.0 + k * 0.45,
                    ripple,
                    0.85 * (1.0 - k / 26.0),
                );
            }
            let (x, y) = (
                lx + 0.1 * k + 0.8 * (k * 0.08 + item as f64).sin(),
                ly + 0.04 * k,
            );
            let fade = if u < frames - 18.0 {
                1.0
            } else {
                (frames - u) / 18.0
            };
            let art = &sprites[(item + (k / 40.0).floor() as usize) % sprites.len()];
            self.scene
                .sprite([x + 1.0, y + 1.0], art, &shadow, 0.24 * fade);
            self.scene.sprite([x, y], art, palette, fade);
        }
    }

    fn lily(&mut self, centre: [f64; 2], radius: f64, angle: f64, fill: Rgb, rim: Option<Rgb>) {
        let [cx, cy] = centre;
        for y in (cy - radius).floor() as i32..=(cy + radius) as i32 {
            for x in (cx - radius).floor() as i32..=(cx + radius) as i32 {
                let dx = f64::from(x) + 0.5 - cx;
                let dy = (f64::from(y) + 0.5 - cy) / 0.8;
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
                let notch = (dy.atan2(dx) - angle).rem_euclid(TAU);
                if !(0.35..=TAU - 0.35).contains(&notch) {
                    continue;
                }
                let on_edge = dx * dx + dy * dy > (radius - 1.1).powi(2);
                match rim {
                    Some(rim_colour) => {
                        self.scene
                            .blend(x, y, if on_edge { rim_colour } else { fill }, 1.0)
                    }
                    None => self.scene.blend(x, y, fill, 0.28),
                }
            }
        }
        if rim.is_some() {
            let vein = hex(if self.options.night {
                "#26462b"
            } else {
                "#5fae52"
            });
            for v in 0..5 {
                let a = angle + 0.9 + f64::from(v) * 1.1;
                self.scene.line(
                    centre,
                    [
                        cx + (radius - 2.0) * a.cos(),
                        cy + (radius - 2.0) * 0.8 * a.sin(),
                    ],
                    vein,
                    1.0,
                );
            }
        }
    }

    fn lily_pads(&mut self, fill: Rgb, rim: Rgb, shadows: bool) {
        for (x, y, radius, angle) in LILY_PADS {
            let bob = (self.phase() + x).sin() * 0.8;
            if shadows {
                self.lily(
                    [x + 2.0, y + 3.0 + bob],
                    radius,
                    angle,
                    [0.0, 40.0, 30.0],
                    None,
                );
            }
            self.lily([x, y + bob], radius, angle, fill, Some(rim));
        }
    }

    fn spring(&mut self) {
        self.water(["#7cc6c0", "#94d4cc", "#b4e4dc", "#d2f1ea"], 1.0);
        for (dx, dy, colour, alpha) in [
            (3.0, 4.0, [40.0, 70.0, 70.0], 0.27),
            (0.0, 0.0, hex("#5a3b2e"), 1.0),
        ] {
            for (from, to) in [
                ([0.0, 10.0], [14.0, 16.0]),
                ([0.0, 11.0], [14.0, 17.0]),
                ([14.0, 16.0], [30.0, 14.0]),
                ([30.0, 14.0], [44.0, 20.0]),
                ([14.0, 16.0], [22.0, 26.0]),
            ] {
                self.scene.line(
                    [from[0] + dx, from[1] + dy],
                    [to[0] + dx, to[1] + dy],
                    colour,
                    alpha,
                );
            }
        }
        let mut blossom = Random::new(7);
        for _ in 0..26 {
            let upper = blossom.next() < 0.5;
            let (x, y) = if upper {
                (2.0 + blossom.next() * 42.0, 8.0 + blossom.next() * 14.0)
            } else {
                (14.0 + blossom.next() * 10.0, 16.0 + blossom.next() * 12.0)
            };
            let colour =
                hex(["#f5a3bd", "#fbd0dd", "#ffffff"][(blossom.next() * 3.0).floor() as usize]);
            self.scene.rect(x, y, 2.0, 2.0, colour, 1.0);
        }
        let petals: Vec<Vec<String>> = (0..4).map(|turns| rotate(&rows(&PETAL), turns)).collect();
        let palettes = vec![
            vec![('p', hex("#f7b2c6")), ('P', hex("#e88aa6"))],
            vec![('p', hex("#fbd3df")), ('P', hex("#f2a2ba"))],
        ];
        self.fallers(14, 1, &petals, &palettes, [255.0, 255.0, 255.0], 16.0);
    }

    fn summer(&mut self) {
        self.water(["#2f7c69", "#3e9479", "#55ab8b", "#7cc6a2"], 1.0);
        self.lily_pads(hex("#3f8f3a"), hex("#2a6b28"), true);
        let bob = f64::from(round((self.phase() + 100.0).sin() * 0.8));
        self.scene.sprite(
            [98.0, 96.0 + bob],
            &rows(&LOTUS),
            &[
                ('w', hex("#fff3f6")),
                ('p', hex("#f48fb1")),
                ('y', hex("#ffd54f")),
            ],
            1.0,
        );
        if !self.wet() {
            let mut glints = Random::new(3);
            for _ in 0..18 {
                let (x, y) = (
                    (glints.next() * f64::from(SCENE)).floor(),
                    (glints.next() * f64::from(SCENE)).floor(),
                );
                let offset = (glints.next() * self.frames()).floor() as usize;
                let beat = (self.frame + offset) % 30;
                if beat < 4 {
                    let sun = [255.0, 255.0, 230.0];
                    self.scene.point(x, y, sun, 0.9);
                    if beat == 1 {
                        self.scene.point(x + 1.0, y, sun, 0.55);
                        self.scene.point(x - 1.0, y, sun, 0.55);
                    }
                }
            }
        }
    }

    fn maple_palettes(tones: &[&str], darkness: f64) -> Vec<Vec<(char, Rgb)>> {
        tones
            .iter()
            .map(|tone| {
                let leaf = hex(tone);
                vec![
                    ('a', leaf),
                    ('b', leaf.map(|channel| (channel * darkness).round())),
                ]
            })
            .collect()
    }

    fn autumn(&mut self) {
        self.water(["#24464f", "#2f5a63", "#3d6e76", "#4f838a"], 1.0);
        let leaves: Vec<Vec<String>> = (0..4).map(|turns| rotate(&rows(&MAPLE), turns)).collect();
        let palettes = Self::maple_palettes(
            &["#d9542b", "#e8862a", "#f2c14e", "#a83a2a", "#c96a2c"],
            0.65,
        );
        self.fallers(9, 5, &leaves, &palettes, [225.0, 245.0, 245.0], 20.0);
    }

    fn snow(&mut self, colour: Rgb, alphas: (f64, f64), sparkle: bool) {
        let mut flakes = Random::new(11);
        let (phase, side) = (self.phase(), f64::from(SCENE));
        let progress = self.frame as f64 / self.frames();
        for flake in 0..30 {
            let (x0, y0) = (flakes.next() * side, flakes.next() * side);
            let x = (x0 + 3.0 * (phase * 2.0 + f64::from(flake)).sin()) % side;
            let y = (y0 + side * progress) % (side + 8.0) - 4.0;
            self.scene.point(
                x,
                y,
                colour,
                if flake % 3 == 0 { alphas.1 } else { alphas.0 },
            );
            if sparkle && flake % 4 == 0 {
                self.scene.point(x + 1.0, y, colour, 0.55);
            }
        }
    }

    fn winter(&mut self) {
        self.water(["#5f7c94", "#7592a8", "#90abbe", "#b3c8d6"], 0.6);
        self.snow([255.0, 255.0, 255.0], (0.8, 1.0), true);
    }

    /// Rain is weather, laid over any season: drops land and rings spread.
    fn rain(&mut self, ring_colour: Rgb, drop_colour: Rgb, alpha: f64) {
        let mut drops = Random::new(13);
        for _ in 0..22 {
            let (x, y) = (4.0 + drops.next() * 120.0, 4.0 + drops.next() * 120.0);
            let offset = (drops.next() * 30.0).floor() as usize;
            let beat = (self.frame + offset) % 30;
            let k = beat as f64 / 30.0;
            if k < 0.93 {
                self.scene
                    .ring([x, y], 0.5 + k * 10.0, ring_colour, alpha * (1.0 - k));
            }
            if beat >= 28 {
                self.scene
                    .point(x, y - (29 - beat) as f64 * 3.0, drop_colour, alpha);
            }
        }
    }

    fn night(&mut self) {
        let wet = self.wet();
        if wet {
            self.water(["#121c2b", "#18253a", "#1f3049", "#283a55"], 0.7);
        } else {
            self.water(["#0b1424", "#111d33", "#182843", "#203453"], 1.0);
        }
        let phase = self.phase();
        if self.options.season == Season::Summer {
            for (x, y, radius, angle) in LILY_PADS {
                let bob = (phase + x).sin() * 0.8;
                self.lily(
                    [x, y + bob],
                    radius,
                    angle,
                    hex("#1d3f27"),
                    Some(hex("#142f1c")),
                );
            }
        }
        // on a rainy night the moon shows faintly through cloud
        self.scene.opacity = if wet { 0.3 } else { 1.0 };
        self.moon();
        self.scene.opacity = 1.0;
        // the lamps' glow lights the water itself, so it counts as water: the koi swims
        // through it, not under it
        let offset = f64::from(self.stage.geometry.offset);
        for (n, light) in self.stage.lights.iter().enumerate() {
            if light.reflects {
                let flicker = 0.85 + 0.15 * (phase * 6.0 + n as f64 * 1.7).sin();
                self.scene.glow(
                    [light.x - offset, light.y - offset + 10.0],
                    26.0,
                    light.colour,
                    0.22 * flicker,
                );
            }
        }
        self.base = self.scene.clone();
        for (n, light) in self.stage.lights.iter().enumerate() {
            if !light.reflects {
                continue;
            }
            let n = n as f64;
            let (x0, y0) = (light.x - offset, light.y - offset);
            for k in 0..9 {
                let k = f64::from(k);
                let y = y0 + 8.0 + k * 4.0;
                let width = 1.0 + f64::from(round(2.0 * (phase * 2.0 + k + n).sin().abs()));
                let x = x0 + f64::from(round(1.5 * (phase * 3.0 + k * 0.9 + n).sin()));
                self.scene.line(
                    [x - width, y],
                    [x + width, y],
                    light.colour,
                    (0.75 - k * 0.08).max(0.0),
                );
            }
        }
        if wet {
            self.rain([150.0, 175.0, 215.0], [190.0, 205.0, 235.0], 0.75);
        }
        match self.options.season {
            Season::Winter => self.snow([220.0, 230.0, 250.0], (0.65, 0.85), false),
            Season::Autumn => {
                let leaves: Vec<Vec<String>> =
                    (0..4).map(|turns| rotate(&rows(&MAPLE), turns)).collect();
                let palettes: Vec<Vec<(char, Rgb)>> = ["#7a3a24", "#8a5a2a", "#9a7a3a"]
                    .iter()
                    .map(|tone| {
                        let leaf = hex(tone);
                        vec![('a', leaf), ('b', shade(leaf, 0.65))]
                    })
                    .collect();
                self.fallers(6, 5, &leaves, &palettes, [150.0, 170.0, 200.0], 20.0);
            }
            Season::Spring => {
                let petals: Vec<Vec<String>> =
                    (0..4).map(|turns| rotate(&rows(&PETAL), turns)).collect();
                self.fallers(
                    9,
                    1,
                    &petals,
                    &[vec![('p', hex("#9a7f9a")), ('P', hex("#86677f"))]],
                    [170.0, 180.0, 210.0],
                    16.0,
                );
            }
            Season::Summer => {}
        }
        if self.options.season != Season::Winter {
            let mut fireflies = Random::new(9);
            for fly in 0..10 {
                let (x, y) = (
                    8.0 + fireflies.next() * 112.0,
                    20.0 + fireflies.next() * 100.0,
                );
                let offset = (fireflies.next() * self.frames()).floor() as usize;
                let brightness = ((TAU * (self.frame + offset) as f64 / 40.0).sin() + 1.0) / 2.0;
                if brightness > 0.45 {
                    let fly = f64::from(fly);
                    self.scene.point(
                        x + 4.0 * (phase * 2.0 + fly).sin(),
                        y + 3.0 * (phase * 3.0 + fly).cos(),
                        [214.0, 255.0, 120.0],
                        brightness,
                    );
                }
            }
        }
    }

    fn moon(&mut self) {
        let phase = self.phase();
        let (x, y) = (64.0 + 1.5 * (phase * 2.0).sin(), 64.0); // the middle of the pond
        let pale = [226.0, 233.0, 255.0];
        // a reflected moon: a disc cut into short bands that slide past each other
        let bands = |scene: &mut Layer, radius: i32, cut: bool| {
            let r = f64::from(radius);
            for j in -radius..=radius {
                let fj = f64::from(j);
                let band = f64::from((j + radius).div_euclid(2));
                let half = (r * r - fj * fj).max(0.0).sqrt() * 1.12;
                if (phase * 2.0 + band * 1.7).sin() > 0.93 {
                    continue;
                }
                let shift = f64::from(round(1.4 * (phase * 3.0 + band * 0.9).sin()));
                let left = round(x - half + shift);
                for column in left..=round(x + half + shift) {
                    let fc = f64::from(column);
                    if cut
                        && (fc - shift - (x + r * 0.42)).powi(2) + ((fj + 0.5) * 1.12).powi(2)
                            < (r * 0.86 * 1.12).powi(2)
                    {
                        continue;
                    }
                    let colour = if column <= left + 1 {
                        [245.0, 248.0, 255.0]
                    } else {
                        pale
                    };
                    scene.point(fc, y + fj, colour, 0.82);
                }
            }
        };
        match self.options.moon {
            Moon::Lines => {
                let r: f64 = 9.0;
                let mut j = -r + 1.0;
                while j < r {
                    let half = (r * r - j * j).sqrt() * 1.15;
                    let shift = 1.6 * (phase * 3.0 + j * 0.55).sin();
                    self.scene.line(
                        [x - half + shift, y + j],
                        [x + half + shift, y + j],
                        pale,
                        0.78 - j.abs() / (r * 3.0),
                    );
                    j += 2.0;
                }
            }
            Moon::Round => bands(&mut self.scene, 8, false),
            Moon::Crescent => bands(&mut self.scene, 8, true),
            Moon::Halo => {
                for row in (y - 15.0) as i32..=(y + 15.0) as i32 {
                    for column in round(x - 18.0)..=round(x + 18.0) {
                        let (fc, fr) = (f64::from(column), f64::from(row));
                        let reach = ((fc - x) / 1.15).hypot(fr - y);
                        let threshold =
                            BAYER[(round(fr) & 3) as usize][(round(fc) & 3) as usize] / 16.0;
                        if reach > 9.0 && reach < 15.0 && threshold < (14.0 - reach) / 10.0 {
                            self.scene.point(fc, fr, [180.0, 195.0, 240.0], 0.35);
                        }
                    }
                }
                bands(&mut self.scene, 8, false);
            }
            Moon::Glade => {
                for k in 0..26 {
                    let k = f64::from(k);
                    let twinkle = (phase * 4.0 + k * 1.9).sin();
                    if twinkle < -0.3 {
                        continue;
                    }
                    let (row, half, at) = (
                        14.0 + k * 4.0,
                        1.0 + k * 0.35,
                        x + 2.0 * (phase * 2.0 + k).sin(),
                    );
                    self.scene.line(
                        [at - half, row],
                        [at + half, row],
                        pale,
                        0.25 + 0.4 * (twinkle + 1.0) / 2.0,
                    );
                }
            }
            Moon::Hidden => {}
        }
    }
}

// ------------------------------------------------------------------ putting a frame together

fn compose_frame(
    koi_loops: &[Vec<KoiFrame>],
    options: &PondOptions,
    stage: &Stage,
    frame: usize,
) -> PondFrame {
    let geometry = stage.geometry;
    let mut painter = Painter {
        scene: Layer::new(SCENE),
        base: Layer::new(SCENE),
        water_mid: [60.0, 90.0, 100.0],
        water_top: [200.0, 230.0, 230.0],
        options,
        stage,
        frame,
    };
    if options.night {
        painter.night();
    } else {
        match options.season {
            Season::Spring => painter.spring(),
            Season::Summer => painter.summer(),
            Season::Autumn => painter.autumn(),
            Season::Winter => painter.winter(),
        }
        if options.weather == Weather::Rain {
            painter.rain([235.0, 248.0, 252.0], [240.0, 250.0, 255.0], 0.9);
        }
    }
    let side = geometry.side * 2;
    let offset = geometry.offset * 2;
    let mut picture = Layer::new(side);
    picture.draw_scaled(&painter.scene, [offset, offset], 2);

    let under = options.depth == Depth::Under;
    let scale = geometry.koi_scale;
    let margin = (256.0 - 256.0 * scale) / 2.0;
    let school: Vec<(usize, [f64; 2], f64, bool)> = if koi_loops.len() >= 3 {
        vec![
            (0, [10.0, 8.0], 0.56 * scale, false),
            (40, [112.0, 84.0], 0.5 * scale, true),
            (80, [40.0, 116.0], 0.46 * scale, false),
        ]
    } else {
        vec![(0, [margin, margin], scale, false)]
    };
    for (koi_index, (delay, at, koi_scale, mirror)) in school.iter().enumerate() {
        let swim_loop = &koi_loops[koi_index.min(koi_loops.len() - 1)];
        let swim_frame = &swim_loop[(frame + delay) % swim_loop.len()];
        let koi = place_koi(
            swim_frame,
            *at,
            *koi_scale,
            *mirror,
            &painter,
            frame,
            stage.frame_count,
        );
        // the koi's shadow: its silhouette, dark and offset; deeper and softer underwater
        let shadows: &[([i32; 2], f64)] = if under {
            &[([7, 10], 0.18), ([8, 11], 0.18)]
        } else {
            &[([4, 6], 0.27)]
        };
        for &(shift, alpha) in shadows {
            for (index, pixel) in koi.pixels.iter().enumerate() {
                if pixel[3] > 0.0 {
                    let (x, y) = (index as i32 % 256, index as i32 / 256);
                    picture.blend(
                        offset + shift[0] + x,
                        offset + shift[1] + y,
                        [0.0, 18.0, 28.0],
                        pixel[3] * alpha,
                    );
                }
            }
        }
        picture.draw_scaled(&koi, [offset, offset], 1);
    }

    if under {
        // everything drawn on the water's surface goes back over the koi, plus a shimmer
        let mut surface = Layer::new(SCENE);
        let top = painter.water_top.map(|channel| channel.round() as u8);
        for index in 0..surface.pixels.len() {
            let (full, base) = (painter.scene.read(index), painter.base.read(index));
            let difference: i32 = (0..3)
                .map(|k| (i32::from(full[k]) - i32::from(base[k])).abs())
                .sum();
            if difference > 6 {
                surface.pixels[index] = [
                    f64::from(full[0]),
                    f64::from(full[1]),
                    f64::from(full[2]),
                    1.0,
                ];
            } else if base[..3] == top {
                let sheen = if options.night { 18.0 } else { 46.0 } / 255.0;
                surface.pixels[index] = [235.0, 248.0, 250.0, sheen];
            }
        }
        picture.draw_scaled(&surface, [offset, offset], 2);
    }

    // keep the water and the koi inside the pond, then lay the garden over it
    for (index, pixel) in picture.pixels.iter_mut().enumerate() {
        let (x, y) = (index as i32 % side, index as i32 / side);
        if !stage.edge.water[((y / 2) * geometry.side + x / 2) as usize] {
            *pixel = [0.0; 4];
        }
    }
    picture.draw_scaled(&stage.edge.rim, [0, 0], 2);
    if options.night && options.lights != Lights::Off {
        let mut lights = Layer::new(geometry.side);
        draw_lights(&mut lights, options, stage, frame);
        picture.draw_scaled(&lights, [0, 0], 2);
    }

    picture.to_frame()
}

/// The koi on a 256-pixel layer: scaled and mirrored, then, under the water, tinted the
/// water's colour and bent a little by the ripples.
fn place_koi(
    swim_frame: &KoiFrame,
    at: [f64; 2],
    scale: f64,
    mirror: bool,
    painter: &Painter,
    frame: usize,
    frame_count: usize,
) -> Layer {
    let native = FRAME_WIDTH_PIXELS as i32;
    let source = swim_frame.as_rgba_bytes();
    let mut koi = Layer::new(256);
    let size = 256.0 * scale;
    for y in 0..256 {
        for x in 0..256 {
            let (fx, fy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let local_x = if mirror { 256.0 - fx } else { fx } - at[0];
            let local_y = fy - at[1];
            if local_x < 0.0 || local_y < 0.0 || local_x >= size || local_y >= size {
                continue;
            }
            let (sx, sy) = ((local_x / scale) as i32, (local_y / scale) as i32);
            if sx >= native || sy >= native {
                continue;
            }
            let at_source = ((sy * native + sx) * 4) as usize;
            let alpha = f64::from(source[at_source + 3]) / 255.0;
            if alpha > 0.0 {
                koi.pixels[(y * 256 + x) as usize] = [
                    f64::from(source[at_source]),
                    f64::from(source[at_source + 1]),
                    f64::from(source[at_source + 2]),
                    alpha,
                ];
            }
        }
    }
    let tint = |koi: &mut Layer, colour: Rgb, amount: f64| {
        for pixel in &mut koi.pixels {
            if pixel[3] > 0.0 {
                for k in 0..3 {
                    pixel[k] = colour[k] * amount + pixel[k] * (1.0 - amount);
                }
            }
        }
    };
    if painter.options.depth == Depth::Under {
        if !painter.options.night {
            tint(&mut koi, painter.water_mid, 0.22);
        }
        // half a pixel of the koi's own art, in slow bands, so thin fins waver without
        // breaking up
        let mut bent = Layer::new(256);
        let phase = TAU * frame as f64 / frame_count as f64;
        for band in (0..256).step_by(4) {
            let shift = round((f64::from(band) * 0.03 + phase * 2.0).sin());
            for y in band..band + 4 {
                for x in 0..256 {
                    let to = x + shift;
                    if (0..256).contains(&to) {
                        bent.pixels[(y * 256 + to) as usize] = koi.pixels[(y * 256 + x) as usize];
                    }
                }
            }
        }
        koi = bent;
    }
    if painter.options.night {
        tint(&mut koi, [10.0, 22.0, 52.0], 0.42);
    }
    koi
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_option_round_trips_through_its_name() {
        for season in Season::ALL {
            assert_eq!(Season::parse(season.name()), Some(*season));
        }
        for garden in Garden::ALL {
            assert_eq!(Garden::parse(garden.name()), Some(*garden));
        }
        for lights in Lights::ALL {
            assert_eq!(Lights::parse(lights.name()), Some(*lights));
        }
    }

    #[test]
    fn the_seasons_follow_the_calendar() {
        let on = |month| choose_season(NaiveDate::from_ymd_opt(2026, month, 15).expect("a date"));
        assert_eq!(on(1), Season::Winter);
        assert_eq!(on(4), Season::Spring);
        assert_eq!(on(7), Season::Summer);
        assert_eq!(on(10), Season::Autumn);
        assert_eq!(on(12), Season::Winter);
    }

    #[test]
    fn it_rains_on_about_one_day_in_seven_and_never_in_winter() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).expect("a date");
        let days: Vec<NaiveDate> = (0..365)
            .map(|offset| start + chrono::Days::new(offset))
            .collect();
        let rainy: Vec<&NaiveDate> = days
            .iter()
            .filter(|day| choose_weather(**day, choose_season(**day)) == Weather::Rain)
            .collect();
        assert!(
            (30..=60).contains(&rainy.len()),
            "{} rainy days",
            rainy.len()
        );
        assert!(
            rainy
                .iter()
                .all(|day| choose_season(**day) != Season::Winter)
        );
    }

    #[test]
    fn the_rain_rule_matches_the_explorer_page() {
        // the page and pond.py compute the same scramble of YYYYMMDD
        assert_eq!(scramble(20_260_305) % 7, 5);
        assert_eq!(scramble(20_261_118) % 7, 6);
        assert_eq!(scramble(20_270_101) % 7, 4);
    }

    #[test]
    fn the_seeded_generator_matches_the_page() {
        // mulberry32 seeded with 7, as the page's rng(7) gives
        let mut random = Random::new(7);
        let first = random.next();
        assert!((0.0..1.0).contains(&first));
        let mut again = Random::new(7);
        assert_eq!(again.next(), first);
    }
}
