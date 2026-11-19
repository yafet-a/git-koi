# Changelog

Notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0] - 2026-11-19

### Added

- `--layout garden` (Action input `layout`) draws a wider picture: the pond on
  the left and a boardwalk out to a deck with the last year's contributions in a
  planter, the activity overview as a round garden bed, and a bench under a gas
  lamp. It also writes `koi-pond.gif` and `koi-pond-night.gif`, the pond alone,
  for narrow screens. `--activity` gives the bed its four counts; the Action
  reads them from GitHub.
- `--scale 2` (Action input `scale`) draws each pixel 2 by 2, so the picture
  stays sharp on high-density screens.

### Changed

- On a deck, the gas lamps (`lights: euro`) stand halfway along the sides, so
  their lanterns hang level with the middle of the pond, where the moon is.
- Pond GIFs store only the pixels that change after the first frame, so they are
  several times smaller with the same pixels.

## [1.1.0] - 2026-11-18

This fork's first release: the koi gets a pond.

### Added

- `git-koi pond` swims the koi through a garden pond and writes `koi.gif`, and
  with `--night` also `koi-night.gif`, the same pond at night for viewers in
  dark mode. The pond follows the season of the snapshot's observation date,
  with rain on about one day in seven outside winter, and takes options for the
  garden or deck around it, its size, the lights and moon at night, whether the
  koi swims under the surface, where falling leaves land, and a school of three.
- The Action passes those options through: `scene`, `weather`, `night`, `size`,
  `garden`, `lights`, `moon`, `depth`, `drops`, `koi`, `body: auto` and
  `night-body`. With `scene: none`, the default, it does exactly what it did.
- The Action runs on Windows runners, downloading and checking the release's
  `.zip` (#36).

### Changed

- Releases come from this fork, and the release workflow no longer publishes the
  crate, whose name belongs to the original project.
- `git-koi render` prints nothing when it succeeds. The line it printed named
  the behavior, date, and body, which `preview.html` already shows (#43).

### Fixed

- When GitHub's contribution calendar leaves out a day or reports one twice,
  `git-koi collect` says so and asks to try again, instead of naming a repair
  only the author of a snapshot file can make (#56).

## [1.0.2] - 2026-10-31

### Fixed

- `git-koi collect` requests again any contribution days GitHub leaves out of an
  answer, instead of failing with "missing contribution date". GitHub left out
  days for some accounts whose history crosses a February 29 (#52).

## [1.0.1] - 2026-10-28

### Changed

- The Action is listed as `git-profile-koi`, since a GitHub user already holds
  the name `git-koi`, and its description fits the Marketplace limit.

## [1.0.0] - 2026-10-28

### Added

- `git-koi collect`, which gathers an account's public snapshot through the
  GitHub CLI, and `--help` and `--version` on the command.
- `git-koi render`, which draws the day's koi as a looping GIF, a still PNG, and
  a preview page, in six bodies.
- A GitHub Action that downloads the release binary and runs both inside a
  profile repository's workflow.
- The `git-koi` library: account history, koi development, behavior, motion,
  markings, rendering, and encoding.

[Unreleased]: https://github.com/yafet-a/git-koi/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/yafet-a/git-koi/compare/v1.0.2...v1.1.0
[1.0.2]: https://github.com/yalongwastaken/git-koi/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/yalongwastaken/git-koi/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/yalongwastaken/git-koi/releases/tag/v1.0.0
