//! The garden beside the pond: a boardwalk from the pond's deck out to a second deck,
//! where the year's contributions grow in a planter, the activity overview grows as a
//! round garden bed, and a bench sits under a gas lamp.
//!
//! Everything is drawn from straight above, like the pond, in the pond's own deck boards,
//! so it reads as one garden. The pond's frames are laid over it unchanged, so the pond
//! is exactly the pond `render_pond` draws.

use std::f64::consts::TAU;

use chrono::{Datelike, Days, NaiveDate};

use crate::history::ContributionHistory;
use crate::pond::{
    DeckStyle, Garden, Layer, Lights, PondFrame, PondOptions, Rgb, Season, deck_style,
    draw_euro_lamp, hash, hex, value_noise, weathered_board,
};

/// What the garden shows besides the pond.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Yard {
    /// The last 53 weeks of contributions, Sunday first, as GitHub's graph lays them
    /// out; days after the observation date are `None`.
    pub weeks: Vec<[Option<u32>; 7]>,
    /// The year's commits, pull requests, issues and reviews, GitHub's activity overview.
    pub activity: Option<Activity>,
}

/// The four kinds of contribution GitHub's activity overview compares.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Activity {
    /// Commits.
    pub commits: u32,
    /// Pull requests opened.
    pub pull_requests: u32,
    /// Issues opened.
    pub issues: u32,
    /// Pull requests reviewed.
    pub reviews: u32,
}

impl Activity {
    /// Four counts written `COMMITS,PULL_REQUESTS,ISSUES,REVIEWS`.
    pub fn parse(text: &str) -> Option<Self> {
        let counts: Vec<u32> = text
            .split(',')
            .map(|count| count.trim().parse().ok())
            .collect::<Option<_>>()?;
        let [commits, pull_requests, issues, reviews] = counts[..] else {
            return None;
        };
        Some(Self {
            commits,
            pull_requests,
            issues,
            reviews,
        })
    }
}

impl Yard {
    /// The garden for a history: its last 53 weeks, ending on the observation date.
    pub fn from_history(history: &ContributionHistory, activity: Option<Activity>) -> Self {
        let last = history.observed_on;
        let first_sunday =
            last - Days::new(u64::from(last.weekday().num_days_from_sunday())) - Days::new(52 * 7);
        let count_on = |date: NaiveDate| {
            history
                .days
                .binary_search_by_key(&date, |day| day.date)
                .map_or(0, |index| history.days[index].contribution_count)
        };
        let weeks = (0..53)
            .map(|week| {
                std::array::from_fn(|day| {
                    let date = first_sunday + Days::new(week * 7 + day as u64);
                    (date <= last).then(|| count_on(date))
                })
            })
            .collect();
        Self { weeks, activity }
    }

    /// How full each day's tuft is, 0 to 4, split at the quartiles of the active days
    /// as GitHub's graph does; `None` for days not yet come.
    fn levels(&self) -> Vec<[Option<u8>; 7]> {
        let mut active: Vec<u32> = self
            .weeks
            .iter()
            .flatten()
            .flatten()
            .copied()
            .filter(|&count| count > 0)
            .collect();
        active.sort_unstable();
        let quartile = |fraction: f64| {
            active
                .get((active.len() as f64 * fraction) as usize)
                .copied()
                .unwrap_or(0)
        };
        let (q1, q2, q3) = (quartile(0.25), quartile(0.5), quartile(0.75));
        self.weeks
            .iter()
            .map(|week| {
                week.map(|count| {
                    count.map(|count| match count {
                        0 => 0,
                        _ if count <= q1 => 1,
                        _ if count <= q2 => 2,
                        _ if count <= q3 => 3,
                        _ => 4,
                    })
                })
            })
            .collect()
    }
}

/// The garden's logical width; the picture is twice this, the pond's 176 on the left.
pub const GARDEN_WIDTH: i32 = 376;
const HEIGHT: i32 = 176;

const WALKWAY: [i32; 4] = [160, 78, 26, 20];
const PLATFORM: [i32; 4] = [184, 28, 188, 120];
const PLANTER: [i32; 2] = [194, 36];
const BED: [f64; 3] = [338.0, 104.0, 26.0];
const BENCH: [i32; 3] = [204, 112, 60];
const LAMP: [f64; 2] = [278.0, 132.0];
const BOLLARDS: [[f64; 2]; 4] = [
    [200.0, 142.0],
    [246.0, 142.0],
    [300.0, 142.0],
    [352.0, 142.0],
];

/// Lay each pond frame into the garden. `pond_frames` are `render_pond`'s frames for the
/// wide size, drawn with the same `options`.
pub fn render_garden(
    pond_frames: &[PondFrame],
    options: &PondOptions,
    yard: &Yard,
) -> Vec<PondFrame> {
    let ground = draw_ground(options, yard);
    let mut picture = Layer::sized(GARDEN_WIDTH * 2, HEIGHT * 2);
    picture.draw_scaled(&ground, [0, 0], 2);
    let frame_count = pond_frames.len();
    pond_frames
        .iter()
        .enumerate()
        .map(|(frame, pond)| {
            let mut scene = picture.clone();
            let pond_width = pond.width() as i32;
            for (index, pixel) in pond.pixels().iter().enumerate() {
                if pixel[3] == 255 {
                    let (x, y) = (index as i32 % pond_width, index as i32 / pond_width);
                    scene.blend(x, y, pixel_colour(*pixel), 1.0);
                }
            }
            let mut lamp = Layer::sized(GARDEN_WIDTH, HEIGHT);
            let phase = TAU * frame as f64 / frame_count as f64;
            let lit = options.night && options.lights != Lights::Off;
            draw_euro_lamp(&mut lamp, LAMP, phase, f64::from(GARDEN_WIDTH), lit);
            scene.draw_scaled(&lamp, [0, 0], 2);
            scene.to_frame()
        })
        .collect()
}

fn pixel_colour(pixel: [u8; 4]) -> Rgb {
    [
        f64::from(pixel[0]),
        f64::from(pixel[1]),
        f64::from(pixel[2]),
    ]
}

/// Everything that does not move: boards, planter, bed, bench, moss and low lights.
fn draw_ground(options: &PondOptions, yard: &Yard) -> Layer {
    let style = deck_style(options.garden)
        .or_else(|| deck_style(Garden::DeckOld))
        .expect("the old oak deck has a style");
    let mut ground = Layer::sized(GARDEN_WIDTH, HEIGHT);
    boards(&mut ground, &style, options, WALKWAY, true);
    boards(&mut ground, &style, options, PLATFORM, false);
    planter(&mut ground, yard);
    bed(&mut ground, yard.activity);
    bench(&mut ground);
    let clumps: &[[f64; 3]] = match options.season {
        Season::Summer => &[
            [190.0, 142.0, 8.0],
            [366.0, 34.0, 6.0],
            [190.0, 34.0, 5.0],
            [282.0, 144.0, 5.0],
            [368.0, 140.0, 5.0],
        ],
        Season::Winter => &[],
        _ => &[[190.0, 142.0, 6.0], [366.0, 34.0, 5.0], [282.0, 144.0, 4.0]],
    };
    for (n, &[x, y, radius]) in clumps.iter().enumerate() {
        moss(&mut ground, [x, y], radius, n as f64 * 3.1);
    }
    if options.season == Season::Winter {
        snow_over(&mut ground);
    }
    if options.night {
        // moonlight, as on the pond's own deck
        for pixel in &mut ground.pixels {
            pixel[0] = (pixel[0] * 0.32 + 8.0).trunc();
            pixel[1] = (pixel[1] * 0.37 + 10.0).trunc();
            pixel[2] = (pixel[2] * 0.47 + 18.0).trunc();
        }
    }
    for at in BOLLARDS {
        bollard(
            &mut ground,
            at,
            options.night && options.lights != Lights::Off,
        );
    }
    ground
}

/// Deck boards over `[left, top, width, height]`: a walkway's boards run across it; a
/// platform's run its length inside a mitred frame, like the pond's deck.
fn boards(
    layer: &mut Layer,
    style: &DeckStyle,
    options: &PondOptions,
    [left, top, width, height]: [i32; 4],
    across: bool,
) {
    let nail = hex(style.nail);
    for y in top..top + height {
        for x in left..left + width {
            let (dl, dr, dt, db) = (
                x - left,
                left + width - 1 - x,
                y - top,
                top + height - 1 - y,
            );
            let frame = !across && dl.min(dr).min(dt).min(db) < 4;
            let (depth, along, tone, mitre) = if across {
                (f64::from(dl), y, 0, false)
            } else if frame {
                let horizontal = dt.min(db) <= dl.min(dr);
                let mitre = (dt.min(db) - dl.min(dr)).abs() < 1;
                let edge = dl.min(dr).min(dt).min(db);
                (
                    f64::from(edge),
                    if horizontal { x } else { y },
                    if horizontal { 1 } else { 3 },
                    mitre,
                )
            } else {
                (f64::from(dt - 4) + 0.5, x, 0, false)
            };
            let board = if frame {
                0
            } else {
                (depth / 3.3).floor() as i32
            };
            let within = if frame {
                depth + 1.0
            } else {
                depth - f64::from(board) * 3.3
            };
            let place = (along + board * 23 + if frame { 11 } else { 0 }).rem_euclid(37);
            let joint = place == 0 && !frame;
            let is_seam = (!frame && within < 0.9) || joint || mitre || (frame && depth >= 3.0);
            let colour = weathered_board(
                style,
                options,
                [x, y],
                board,
                board + tone,
                within,
                along,
                is_seam,
                0.0,
            );
            layer.blend(x, y, colour, 1.0);
            if !joint && !is_seam && place == 2 && within > 1.4 && within < 2.4 {
                layer.blend(x, y, nail, 1.0);
            }
        }
    }
}

fn rect(layer: &mut Layer, x: i32, y: i32, width: i32, height: i32, colour: &str) {
    layer.rect(
        f64::from(x),
        f64::from(y),
        f64::from(width),
        f64::from(height),
        hex(colour),
        1.0,
    );
}

/// The year's contributions from above: a long oak planter with one tuft of moss per
/// day, fuller and brighter the more was done that day; quiet days are bare soil.
fn planter(layer: &mut Layer, yard: &Yard) {
    let [left, top] = PLANTER;
    let levels = yard.levels();
    let (width, height) = (levels.len() as i32 * 3 + 9, 7 * 3 + 9);
    layer.rect(
        f64::from(left + 2),
        f64::from(top + 2),
        f64::from(width),
        f64::from(height),
        hex("#2a2418"),
        0.3,
    );
    for k in 0..3 {
        let colour = ["#a38563", "#82694c", "#4a3a28"][k as usize];
        let far = if k == 0 { "#5d4b36" } else { colour };
        rect(layer, left + k, top + k, width - 2 * k, 1, colour);
        rect(layer, left + k, top + height - 1 - k, width - 2 * k, 1, far);
        rect(layer, left + k, top + k, 1, height - 2 * k, colour);
        rect(layer, left + width - 1 - k, top + k, 1, height - 2 * k, far);
    }
    for y in 3..height - 3 {
        for x in 3..width - 3 {
            let soil = if hash(f64::from(x), f64::from(y)) > 0.8 {
                "#4a3828"
            } else {
                "#3a2b1e"
            };
            rect(layer, left + x, top + y, 1, 1, soil);
        }
    }
    rect(layer, left + 3, top + 3, width - 6, 1, "#2a1f15");
    for (week, days) in levels.iter().enumerate() {
        for (day, level) in days.iter().enumerate() {
            let Some(level) = *level else { continue };
            let (x, y) = (left + 5 + week as i32 * 3, top + 5 + day as i32 * 3);
            match level {
                0 => {
                    if hash(week as f64, day as f64) > 0.6 {
                        rect(
                            layer,
                            x + (week as i32 & 1),
                            y + (day as i32 & 1),
                            1,
                            1,
                            "#5a4532",
                        );
                    }
                }
                1 => {
                    rect(layer, x, y + 1, 1, 1, "#2f5e22");
                    rect(layer, x + 1, y, 1, 1, "#447830");
                }
                _ => {
                    let [dark, light] = [
                        ["#447830", "#5e923a"],
                        ["#5e923a", "#7cae4f"],
                        ["#7cae4f", "#b4dc6a"],
                    ][usize::from(level) - 2];
                    rect(layer, x, y, 2, 2, dark);
                    rect(layer, x, y, 1, 1, light);
                    if level == 4 {
                        rect(layer, x + 1, y, 1, 1, "#eef7c8");
                    }
                }
            }
        }
    }
}

/// The activity overview as a round bed: four gravel paths are its axes, where GitHub
/// draws them (reviews up, issues right, pull requests down, commits left), and the moss
/// grows out along each as far as that share of the year reaches.
fn bed(layer: &mut Layer, activity: Option<Activity>) {
    let [cx, cy, radius] = BED;
    let r = radius as i32;
    layer.disc([cx + 3.0, cy + 3.0], [radius, radius], hex("#2a2418"), 0.3);
    let kerb = ["#6e737a", "#8a8f96", "#a5a9ae", "#5c6066"].map(hex);
    for y in -r..=r {
        for x in -r..=r {
            let distance = f64::from(x).hypot(f64::from(y));
            if distance >= radius + 0.5 {
                continue;
            }
            let (px, py) = (cx as i32 + x, cy as i32 + y);
            let colour = if distance > radius - 2.5 {
                let segment = ((f64::from(y).atan2(f64::from(x)) + TAU / 2.0) / TAU * 26.0) as i32;
                if x + y < 0 {
                    kerb[(segment % 2 + 1) as usize]
                } else {
                    kerb[if segment % 2 == 1 { 0 } else { 3 }]
                }
            } else if hash(f64::from(px), f64::from(y)) > 0.82 {
                hex("#4a3828")
            } else {
                hex("#3a2b1e")
            };
            layer.blend(px, py, colour, 1.0);
        }
    }
    let inner = radius - 4.0;
    let axes = [[0.0, -1.0], [1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]];
    for [ax, ay] in axes {
        for k in 2..=inner as i32 {
            let gravel = if k % 3 == 0 { "#9d927d" } else { "#b9ae98" };
            layer.blend(
                cx as i32 + (ax * f64::from(k)) as i32,
                cy as i32 + (ay * f64::from(k)) as i32,
                hex(gravel),
                1.0,
            );
        }
    }
    let Some(activity) = activity else { return };
    let shares = [
        activity.reviews,
        activity.issues,
        activity.pull_requests,
        activity.commits,
    ]
    .map(f64::from);
    let most = shares.iter().copied().fold(0.0, f64::max);
    if most <= 0.0 {
        return;
    }
    let points: Vec<[f64; 2]> = axes
        .iter()
        .zip(shares)
        .map(|(&[ax, ay], share)| {
            let reach = inner * (0.14 + 0.86 * share / most);
            [cx + ax * reach, cy + ay * reach]
        })
        .collect();
    let inside = |px: f64, py: f64| {
        (0..4).any(|i| in_triangle([px, py], [cx, cy], points[i], points[(i + 1) % 4]))
    };
    let moss_tones = ["#2f5e22", "#447830", "#5e923a", "#7cae4f"].map(hex);
    for y in -inner as i32..=inner as i32 {
        for x in -inner as i32..=inner as i32 {
            let (px, py) = (cx as i32 + x, cy as i32 + y);
            let (fx, fy) = (f64::from(px) + 0.5, f64::from(py) + 0.5);
            if !inside(fx, fy) {
                continue;
            }
            let edge = !inside(fx + 1.0, fy + 1.0) || !inside(fx - 1.0, fy - 1.0);
            let light = value_noise(f64::from(px) * 0.6, f64::from(py) * 0.6) * 0.6
                - f64::from(x + y) / (inner * 3.0);
            let tone = if edge {
                0
            } else {
                ((light + 0.2) * 3.3).clamp(1.0, 3.0) as usize
            };
            layer.blend(px, py, moss_tones[tone], 1.0);
        }
    }
    for [px, py] in points {
        layer.rect(px - 1.0, py - 1.0, 2.0, 2.0, hex("#f2e6a0"), 1.0);
        layer.rect(px - 1.0, py - 1.0, 1.0, 1.0, hex("#ffffff"), 1.0);
    }
    layer.rect(cx - 1.0, cy - 1.0, 2.0, 2.0, hex("#a5a9ae"), 1.0);
}

fn in_triangle(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    let side = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        (p[0] - r[0]) * (q[1] - r[1]) - (q[0] - r[0]) * (p[1] - r[1])
    };
    let (d1, d2, d3) = (side(p, a, b), side(p, b, c), side(p, c, a));
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

/// A park bench from above: oak slats on black iron ends, the backrest along the front.
fn bench(layer: &mut Layer) {
    let [x, y, width] = BENCH;
    layer.rect(
        f64::from(x + 1),
        f64::from(y + 2),
        f64::from(width + 4),
        15.0,
        hex("#2a2418"),
        0.3,
    );
    for slat in 0..4 {
        let sy = y + slat * 3;
        rect(
            layer,
            x,
            sy,
            width,
            2,
            ["#a8875c", "#b8986a"][(slat % 2) as usize],
        );
        rect(layer, x, sy, width, 1, "#ccb084");
        layer.rect(
            f64::from(x),
            f64::from(sy + 2),
            f64::from(width),
            1.0,
            hex("#3e2f20"),
            0.5,
        );
    }
    rect(layer, x, y + 12, width, 2, "#8f7556");
    rect(layer, x, y + 12, width, 1, "#a38563");
    for end in [x - 2, x + width / 2 - 1, x + width - 1] {
        rect(layer, end, y - 1, 3, 16, "#2b2a28");
        rect(layer, end, y - 1, 3, 1, "#55534f");
    }
}

/// A clump of moss over the boards, lit from the top left.
fn moss(layer: &mut Layer, [cx, cy]: [f64; 2], radius: f64, seed: f64) {
    let tones = ["#437028", "#4f7f33", "#5e903a", "#6b9a40"].map(hex);
    let r = radius as i32;
    for y in -r..=r {
        for x in -r..=r {
            let (fx, fy) = (f64::from(x), f64::from(y));
            let spread = fx.hypot(fy) / radius
                + (value_noise((fx + cx) * 0.5 + seed, (fy + cy) * 0.5) - 0.5) * 0.7;
            if spread >= 1.0 {
                continue;
            }
            let light = (-fx - fy) / (radius * 2.2)
                + (value_noise((fx + cx) * 0.9, (fy + cy) * 0.9 + seed) - 0.5) * 0.6;
            let tone = if spread > 0.85 {
                0
            } else {
                ((light + 0.5) * 4.0).clamp(0.0, 3.0) as usize
            };
            layer.blend(cx as i32 + x, cy as i32 + y, tones[tone], 1.0);
        }
    }
}

/// A low light set into the edge of the deck, lit at night.
fn bollard(layer: &mut Layer, [x, y]: [f64; 2], lit: bool) {
    if lit {
        layer.glow([x, y - 3.0], 12.0, [255.0, 190.0, 100.0], 0.4);
    }
    layer.rect(x - 2.0, y - 2.0, 4.0, 4.0, hex("#24262b"), 1.0);
    layer.rect(
        x - 1.0,
        y - 1.0,
        2.0,
        2.0,
        if lit {
            [255.0, 236.0, 170.0]
        } else {
            hex("#9aa3a8")
        },
        1.0,
    );
}

/// Winter: snow settles on the planter, the bed and the bench as it does on the boards.
fn snow_over(layer: &mut Layer) {
    let [left, top] = PLANTER;
    let [cx, cy, radius] = BED;
    let [bx, by, bw] = BENCH;
    let areas = [
        [left, top, 168, 30],
        [
            (cx - radius) as i32,
            (cy - radius) as i32,
            (radius * 2.0) as i32 + 1,
            (radius * 2.0) as i32 + 1,
        ],
        [bx - 2, by - 1, bw + 4, 16],
    ];
    for [x0, y0, width, height] in areas {
        for y in y0..y0 + height {
            for x in x0..x0 + width {
                let Some(index) = layer.at(x, y) else {
                    continue;
                };
                if layer.pixels[index][3] <= 0.0 {
                    continue;
                }
                let (fx, fy) = (f64::from(x), f64::from(y));
                let drift = 0.6 * value_noise(fx * 0.08 + 2.0, fy * 0.08 + 5.0)
                    + 0.4 * value_noise(fx * 0.2, fy * 0.2);
                let pixel = &mut layer.pixels[index];
                if drift > 0.74 {
                    let snow = if hash(fx, fy * 2.0) > 0.12 {
                        hex("#f2f6f9")
                    } else {
                        hex("#d8e2ea")
                    };
                    *pixel = [snow[0], snow[1], snow[2], 1.0];
                } else if drift > 0.69 {
                    for channel in &mut pixel[..3] {
                        *channel = (*channel * 0.5 + 236.0 * 0.5).trunc();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::ContributionDay;

    fn history(observed_on: &str, days: u32) -> ContributionHistory {
        let last = NaiveDate::parse_from_str(observed_on, "%Y-%m-%d").unwrap();
        let first = last - Days::new(u64::from(days) - 1);
        ContributionHistory {
            observed_on: last,
            days: (0..days)
                .map(|n| ContributionDay {
                    date: first + Days::new(u64::from(n)),
                    contribution_count: n % 5,
                })
                .collect(),
        }
    }

    #[test]
    fn the_planter_holds_53_weeks_ending_on_the_observation_date() {
        // 2026-11-17 is a Tuesday, so its week shows Sunday to Tuesday
        let yard = Yard::from_history(&history("2026-11-17", 400), None);
        assert_eq!(yard.weeks.len(), 53);
        let last_week = yard.weeks[52];
        assert!(last_week[..3].iter().all(Option::is_some));
        assert!(last_week[3..].iter().all(Option::is_none));
        assert!(yard.weeks[..52].iter().flatten().all(Option::is_some));
    }

    #[test]
    fn days_before_the_history_count_as_quiet() {
        let yard = Yard::from_history(&history("2026-11-17", 10), None);
        assert_eq!(yard.weeks[0], [Some(0); 7]);
    }

    #[test]
    fn levels_split_active_days_at_their_quartiles() {
        let yard = Yard {
            weeks: vec![[Some(0), Some(1), Some(2), Some(3), Some(4), Some(40), None]],
            activity: None,
        };
        assert_eq!(
            yard.levels(),
            vec![[Some(0), Some(1), Some(1), Some(2), Some(3), Some(4), None]]
        );
    }

    #[test]
    fn the_garden_keeps_the_pond_exactly_and_adds_a_deck_beside_it() {
        let mut pixels = vec![[0; 4]; 352 * 352];
        pixels[100 * 352 + 120] = [10, 200, 30, 255];
        let pond = [PondFrame::new(352, 352, pixels)];
        let options = PondOptions {
            season: Season::Autumn,
            weather: crate::pond::Weather::Clear,
            night: true,
            garden: Garden::DeckOld,
            size: crate::pond::PondSize::Wide,
            lights: Lights::Euro,
            moon: crate::pond::Moon::Lines,
            depth: crate::pond::Depth::Under,
            drops: crate::pond::Drops::Random,
            day_seed: 20_261_117,
        };
        let yard = Yard::from_history(&history("2026-11-17", 400), Activity::parse("11,11,4,2"));
        let garden = render_garden(&pond, &options, &yard);
        let frame = &garden[0];
        assert_eq!((frame.width(), frame.height()), (752, 352));
        assert_eq!(frame.pixels()[100 * 752 + 120], [10, 200, 30, 255]);
        // the deck is drawn and the page around it stays transparent
        assert_eq!(frame.pixels()[200 * 752 + 500][3], 255);
        assert_eq!(frame.pixels()[10 * 752 + 500][3], 0);
    }

    #[test]
    fn activity_reads_four_counts_in_order() {
        assert_eq!(
            Activity::parse("11, 11,4,2"),
            Some(Activity {
                commits: 11,
                pull_requests: 11,
                issues: 4,
                reviews: 2
            })
        );
        assert_eq!(Activity::parse("1,2,3"), None);
        assert_eq!(Activity::parse("1,2,3,x"), None);
    }
}
