use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use chrono::{DateTime, NaiveDate};
use git_koi::behavior::{Behavior, select_behavior};
use git_koi::coat::KoiBody;
use git_koi::collect::{GhCli, collect_account_snapshot};
use git_koi::history::AccountSnapshot;
use git_koi::koi::derive_koi_development;
use git_koi::motion::frame_count;
use git_koi::output::{encode_gif, encode_png, encode_pond_gif, encode_preview_html};
use git_koi::pond::{
    Depth, Drops, Garden, Lights, Moon, PondOptions, PondSize, School, Season, Weather,
    choose_season, choose_weather, date_number, render_pond, school_bodies, suited_body,
};
use git_koi::render::{KoiFrame, render_koi, render_swim_frame};

/// The command's usage, listing the bodies from [`BODY_NAMES`] so the help names every
/// body the parser accepts.
fn build_usage_text() -> String {
    let body_names = list_body_names();
    format!(
        "Usage: git-koi collect LOGIN [--output FILE]
       git-koi render SNAPSHOT --output-dir DIR [--body BODY]
       git-koi pond SNAPSHOT --output-dir DIR [POND OPTIONS]
       git-koi --help | --version
Collect LOGIN's public GitHub snapshot as JSON through `gh api`: public owned and
contributed repositories, and daily contributions from the account's creation
through yesterday (UTC), the last complete day.
Writes to standard output, or replaces FILE only after collection succeeds.
Requires an authenticated GitHub CLI; in GitHub Actions, set GH_TOKEN to github.token.
Render a collected SNAPSHOT into DIR: koi.gif swims the behavior chosen for its
observation date, koi.png is the still koi, and preview.html shows both on light and
dark backgrounds. BODY is {body_names}.
The files are replaced only after all of them render.
Render prints nothing on success; errors go to stderr and exit 1; invalid usage exits 2.
Swim the koi through a garden pond into DIR/koi.gif, and with --night also
DIR/koi-night.gif, the same pond at night for viewers in dark mode. The date is the
snapshot's observation date unless --date YYYY-MM-DD names another. POND OPTIONS,
each followed by a value, the first value being the default:
  --season   auto (from the date), {seasons}
  --weather  auto (rain about one day in seven, never in winter), {weathers}
  --garden   {gardens}
  --size     {sizes}
  --lights   {lights}
  --moon     {moons}
  --depth    {depths}
  --drops    {drops}
  --koi      {schools}
  --body, --night-body   auto (suits the season; platinum at night) or a body
and --night on its own.
Examples: git-koi collect octocat --output snapshot.json
          git-koi render snapshot.json --output-dir koi --body asagi
          git-koi pond snapshot.json --output-dir koi --body asagi --night --lights euro",
        seasons = list_names(Season::ALL.iter().map(|value| value.name())),
        weathers = list_names(Weather::ALL.iter().map(|value| value.name())),
        gardens = list_names(Garden::ALL.iter().map(|value| value.name())),
        sizes = list_names(PondSize::ALL.iter().map(|value| value.name())),
        lights = list_names(Lights::ALL.iter().map(|value| value.name())),
        moons = list_names(Moon::ALL.iter().map(|value| value.name())),
        depths = list_names(Depth::ALL.iter().map(|value| value.name())),
        drops = list_names(Drops::ALL.iter().map(|value| value.name())),
        schools = list_names(School::ALL.iter().map(|value| value.name())),
    )
}

/// Each body's name on the command line.
const BODY_NAMES: [(&str, KoiBody); 6] = [
    ("white", KoiBody::White),
    ("black", KoiBody::Black),
    ("asagi", KoiBody::Asagi),
    ("tea", KoiBody::Tea),
    ("gold", KoiBody::Gold),
    ("platinum", KoiBody::Platinum),
];

/// Names joined with commas.
fn list_names<'a>(names: impl Iterator<Item = &'a str>) -> String {
    names.collect::<Vec<_>>().join(", ")
}

/// The body names in table order, the default marked and the last joined with "or".
fn list_body_names() -> String {
    let listed_names: Vec<String> = BODY_NAMES
        .iter()
        .map(|(body_name, koi_body)| {
            if *koi_body == KoiBody::default() {
                format!("{body_name} (the default)")
            } else {
                (*body_name).to_owned()
            }
        })
        .collect();
    let (last_name, leading_names) = listed_names
        .split_last()
        .expect("the table names at least one body");
    format!("{}, or {last_name}", leading_names.join(", "))
}

fn main() -> ExitCode {
    let program_arguments: Vec<OsString> = env::args_os().skip(1).collect();
    let argument_texts: Vec<Option<&str>> = program_arguments
        .iter()
        .map(|argument| argument.to_str())
        .collect();
    match argument_texts.as_slice() {
        [Some("--help")] | [Some("collect" | "render"), Some("--help")] => {
            println!("{}", build_usage_text());
            ExitCode::SUCCESS
        }
        [Some("--version")] => {
            println!("git-koi {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [Some("collect"), Some(login)] if !login.starts_with('-') => run_collect(login, None),
        [Some("collect"), Some(login), Some("--output"), _] if !login.starts_with('-') => {
            run_collect(login, Some(Path::new(&program_arguments[3])))
        }
        [Some("render"), Some(snapshot_path), Some("--output-dir"), _]
            if !snapshot_path.starts_with('-') =>
        {
            run_render(
                Path::new(snapshot_path),
                Path::new(&program_arguments[3]),
                KoiBody::default(),
            )
        }
        [
            Some("render"),
            Some(snapshot_path),
            Some("--output-dir"),
            _,
            Some("--body"),
            Some(body_name),
        ] if !snapshot_path.starts_with('-') && parse_body(body_name).is_some() => run_render(
            Path::new(snapshot_path),
            Path::new(&program_arguments[3]),
            parse_body(body_name).expect("the guard parsed the body"),
        ),
        [Some("pond"), Some("--help")] => {
            println!("{}", build_usage_text());
            ExitCode::SUCCESS
        }
        [
            Some("pond"),
            Some(snapshot_path),
            Some("--output-dir"),
            _,
            ..,
        ] if !snapshot_path.starts_with('-') => match parse_pond_request(&argument_texts[4..]) {
            Some(pond_request) => run_pond(
                Path::new(snapshot_path),
                Path::new(&program_arguments[3]),
                &pond_request,
            ),
            None => {
                eprintln!("{}", build_usage_text());
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("{}", build_usage_text());
            ExitCode::from(2)
        }
    }
}

/// The pond command's choices; `None` for a season, weather or body means "decide".
struct PondRequest {
    date: Option<NaiveDate>,
    season: Option<Season>,
    weather: Option<Weather>,
    garden: Garden,
    size: PondSize,
    lights: Lights,
    moon: Moon,
    depth: Depth,
    drops: Drops,
    school: School,
    body: Option<KoiBody>,
    night_body: Option<KoiBody>,
    night: bool,
}

/// Read the pond options after `--output-dir DIR`; `None` for anything unknown.
fn parse_pond_request(arguments: &[Option<&str>]) -> Option<PondRequest> {
    let mut request = PondRequest {
        date: None,
        season: None,
        weather: None,
        garden: Garden::ALL[0],
        size: PondSize::ALL[0],
        lights: Lights::ALL[0],
        moon: Moon::ALL[0],
        depth: Depth::ALL[0],
        drops: Drops::ALL[0],
        school: School::ALL[0],
        body: None,
        night_body: None,
        night: false,
    };
    let auto_or = |value: &str, parse: fn(&str) -> Option<KoiBody>| match value {
        "auto" => Some(None),
        _ => parse(value).map(Some),
    };
    let mut remaining = arguments.iter();
    while let Some(flag) = remaining.next() {
        if *flag == Some("--night") {
            request.night = true;
            continue;
        }
        let value = (*remaining.next()?)?;
        match (*flag)? {
            "--date" => request.date = Some(NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?),
            "--season" if value == "auto" => request.season = None,
            "--season" => request.season = Some(Season::parse(value)?),
            "--weather" if value == "auto" => request.weather = None,
            "--weather" => request.weather = Some(Weather::parse(value)?),
            "--garden" => request.garden = Garden::parse(value)?,
            "--size" => request.size = PondSize::parse(value)?,
            "--lights" => request.lights = Lights::parse(value)?,
            "--moon" => request.moon = Moon::parse(value)?,
            "--depth" => request.depth = Depth::parse(value)?,
            "--drops" => request.drops = Drops::parse(value)?,
            "--koi" => request.school = School::parse(value)?,
            "--body" => request.body = auto_or(value, parse_body)?,
            "--night-body" => request.night_body = auto_or(value, parse_body)?,
            _ => return None,
        }
    }
    Some(request)
}

fn run_pond(snapshot_path: &Path, output_directory: &Path, request: &PondRequest) -> ExitCode {
    let pond_result = draw_ponds(snapshot_path, request).and_then(|output_files| {
        fs::create_dir_all(output_directory)
            .map_err(|create_error| format!("{}: {create_error}", output_directory.display()))?;
        for (file_name, file_bytes) in output_files {
            replace_file(&output_directory.join(file_name), &file_bytes)?;
        }
        Ok(())
    });
    match pond_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_message) => {
            eprintln!("git-koi: {error_message}");
            ExitCode::FAILURE
        }
    }
}

/// Draw and encode the day's pond, and the night's with --night, in memory, so neither
/// is written unless both succeed.
fn draw_ponds(
    snapshot_path: &Path,
    request: &PondRequest,
) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let snapshot_json = fs::read_to_string(snapshot_path)
        .map_err(|read_error| format!("{}: {read_error}", snapshot_path.display()))?;
    let account_snapshot: AccountSnapshot = serde_json::from_str(&snapshot_json)
        .map_err(|parse_error| format!("{}: {parse_error}", snapshot_path.display()))?;
    let koi_development = derive_koi_development(&account_snapshot)
        .map_err(|development_error| development_error.to_string())?;
    let behavior = select_behavior(&koi_development);
    let date = request.date.unwrap_or(koi_development.observed_on);
    let season = request.season.unwrap_or_else(|| choose_season(date));
    let weather = request
        .weather
        .unwrap_or_else(|| choose_weather(date, season));
    let mut ponds = Vec::new();
    let times = if request.night {
        &[false, true][..]
    } else {
        &[false][..]
    };
    for &night in times {
        let chosen_body = if night {
            request.night_body
        } else {
            request.body
        };
        let bodies: Vec<KoiBody> = match request.school {
            School::Three => school_bodies(season, night).to_vec(),
            School::One => vec![chosen_body.unwrap_or_else(|| suited_body(season, night))],
        };
        let koi_loops: Vec<Vec<KoiFrame>> = bodies
            .iter()
            .map(|&koi_body| {
                (0..frame_count(behavior))
                    .map(|frame_index| {
                        render_swim_frame(&koi_development, koi_body, behavior, frame_index)
                    })
                    .collect()
            })
            .collect();
        let pond_options = PondOptions {
            season,
            weather,
            night,
            garden: request.garden,
            size: request.size,
            lights: request.lights,
            moon: request.moon,
            depth: request.depth,
            drops: request.drops,
            day_seed: date_number(date),
        };
        let gif_bytes = encode_pond_gif(&render_pond(&koi_loops, &pond_options))
            .map_err(|encoding_error| encoding_error.to_string())?;
        ponds.push((if night { "koi-night.gif" } else { "koi.gif" }, gif_bytes));
    }
    Ok(ponds)
}

fn run_collect(login: &str, output_path: Option<&Path>) -> ExitCode {
    let collection_result = collect_account_snapshot(&GhCli, login, current_utc_date())
        .map_err(|collection_error| collection_error.to_string())
        .and_then(|account_snapshot| {
            serde_json::to_string_pretty(&account_snapshot)
                .map_err(|serialize_error| serialize_error.to_string())
        })
        .and_then(|snapshot_json| write_snapshot(&format!("{snapshot_json}\n"), output_path));
    match collection_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_message) => {
            eprintln!("git-koi: {error_message}");
            ExitCode::FAILURE
        }
    }
}

fn run_render(snapshot_path: &Path, output_directory: &Path, koi_body: KoiBody) -> ExitCode {
    let render_result = render_day(snapshot_path, koi_body).and_then(|output_files| {
        fs::create_dir_all(output_directory)
            .map_err(|create_error| format!("{}: {create_error}", output_directory.display()))?;
        for (file_name, file_bytes) in output_files {
            replace_file(&output_directory.join(file_name), &file_bytes)?;
        }
        Ok(())
    });
    match render_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_message) => {
            eprintln!("git-koi: {error_message}");
            ExitCode::FAILURE
        }
    }
}

/// Render and encode the day's files in memory, by name, so none is written unless all
/// succeed. The preview's caption names the behavior, observation date, and body.
fn render_day(
    snapshot_path: &Path,
    koi_body: KoiBody,
) -> Result<[(&'static str, Vec<u8>); 3], String> {
    let snapshot_json = fs::read_to_string(snapshot_path)
        .map_err(|read_error| format!("{}: {read_error}", snapshot_path.display()))?;
    let account_snapshot: AccountSnapshot = serde_json::from_str(&snapshot_json)
        .map_err(|parse_error| format!("{}: {parse_error}", snapshot_path.display()))?;
    let koi_development = derive_koi_development(&account_snapshot)
        .map_err(|development_error| development_error.to_string())?;
    let behavior = select_behavior(&koi_development);
    let swim_frames: Vec<KoiFrame> = (0..frame_count(behavior))
        .map(|frame_index| render_swim_frame(&koi_development, koi_body, behavior, frame_index))
        .collect();
    let gif_bytes =
        encode_gif(&swim_frames).map_err(|encoding_error| encoding_error.to_string())?;
    let png_bytes = encode_png(&render_koi(&koi_development, koi_body))
        .map_err(|encoding_error| encoding_error.to_string())?;
    let caption = format!(
        "{} for {}, {} body",
        describe_behavior(behavior),
        koi_development.observed_on,
        name_body(koi_body)
    );
    let preview_html = encode_preview_html(&caption);
    Ok([
        ("koi.gif", gif_bytes),
        ("koi.png", png_bytes),
        ("preview.html", preview_html.into_bytes()),
    ])
}

fn parse_body(body_name: &str) -> Option<KoiBody> {
    BODY_NAMES
        .iter()
        .find(|(named_body_name, _)| *named_body_name == body_name)
        .map(|(_, koi_body)| *koi_body)
}

fn name_body(koi_body: KoiBody) -> &'static str {
    BODY_NAMES
        .iter()
        .find(|(_, named_body)| *named_body == koi_body)
        .map(|(body_name, _)| *body_name)
        .expect("every body has a name")
}

fn describe_behavior(behavior: Behavior) -> &'static str {
    match behavior {
        Behavior::DeepRest => "deep rest",
        Behavior::Rest => "rest",
        Behavior::SurfaceFeeding => "surface feeding",
        Behavior::Dart => "dart",
        Behavior::Cruise => "cruise",
    }
}

// GitHub dates its contribution calendar in UTC.
fn current_utc_date() -> NaiveDate {
    let seconds_since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the system clock is after 1970")
        .as_secs();
    DateTime::from_timestamp(
        i64::try_from(seconds_since_epoch).expect("the system clock is in range"),
        0,
    )
    .expect("the system clock is in range")
    .date_naive()
}

/// Write to standard output, or replace the file.
fn write_snapshot(snapshot_json: &str, output_path: Option<&Path>) -> Result<(), String> {
    let Some(output_path) = output_path else {
        return io::stdout()
            .write_all(snapshot_json.as_bytes())
            .map_err(|write_error| format!("standard output: {write_error}"));
    };
    replace_file(output_path, snapshot_json.as_bytes())
}

/// Replace the file through a sibling temporary file named after it, so a failed write
/// never leaves a partial file in place.
fn replace_file(output_path: &Path, file_bytes: &[u8]) -> Result<(), String> {
    let mut temporary_name = output_path.file_name().unwrap_or_default().to_owned();
    temporary_name.push(".tmp");
    let temporary_path = output_path.with_file_name(temporary_name);
    fs::write(&temporary_path, file_bytes)
        .and_then(|()| fs::rename(&temporary_path, output_path))
        .map_err(|write_error| {
            let _ = fs::remove_file(&temporary_path);
            format!("{}: {write_error}", output_path.display())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_usage_lists_every_body_the_parser_accepts_and_marks_the_default() {
        let usage_text = build_usage_text();
        let body_sentence = usage_text
            .split_once("BODY is ")
            .and_then(|(_, sentence_onward)| sentence_onward.split_once('.'))
            .map(|(body_sentence, _)| body_sentence)
            .expect("the usage has a sentence listing the bodies");
        for (body_name, _) in BODY_NAMES {
            assert!(
                body_sentence.contains(body_name),
                "the usage leaves out {body_name}: {body_sentence}"
            );
        }
        let default_name = name_body(KoiBody::default());
        assert!(
            body_sentence.contains(&format!("{default_name} (the default)")),
            "{body_sentence}"
        );
    }
}
