<div align="center">

# GameScape

**An all-in-one Windows desktop customisation platform for gamers.**

Animated wallpapers · Desktop widgets · Saved layouts · Your own library

> **Status: pre-release.** GameScape is under active development.
> There are no public downloads yet — watch this repository or the
> [Releases](../../releases) page. See [docs/README.md](docs/README.md) for the plan.

</div>

[![Privacy scan](https://github.com/ben605200-dev/GameScape/actions/workflows/privacy-scan.yml/badge.svg)](https://github.com/ben605200-dev/GameScape/actions/workflows/privacy-scan.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

<!-- More badges as workflows land (ci.yml, release-desktop.yml). -->

## Overview

GameScape brings together the things gamers actually want from desktop customisation, without the usual trade-offs:

- **Wallpapers that move** — static images, animated GIFs, and MP4 video wallpapers rendered *behind your desktop icons*.
- **Widgets that belong on a desktop** — clocks, date and calendar designs, and more, placed where you want them.
- **Layouts** — complete desktop setups saved and switched in one click.
- **Your own library** — import your own wallpapers; private files never leave your PC.
- **Performance first** — animated wallpapers pause automatically when you launch a game or switch to battery.

## Features (planned for v1.0)

| Area | What ships |
|---|---|
| Wallpapers | Discovery grid, search, filters, collections, downloads, rotation |
| Personal imports | JPG/PNG/WEBP/GIF/MP4, drag-and-drop, batch import, metadata editing |
| Animated desktop | GIF + MP4 wallpapers, per-monitor control, auto-pause under games/battery |
| Widgets | 10 clock styles, date & calendar widgets, full customisation, edit mode |
| Layouts | Save/switch desktop setups, themes, accent colours, presets |
| Privacy | Local-first: no accounts, no telemetry, nothing uploaded without explicit action |

## System requirements (planned)

- Windows 11 (primary) or Windows 10 21H2+ (best effort)
- 64-bit (x64)
- WebView2 runtime (preinstalled on modern Windows; the installer provisions it)
- 4 GB RAM minimum, 8 GB recommended
- Hardware video decoding recommended for high-resolution animated wallpapers

## Installation

No public builds exist yet. When the first release ships:

1. Download the installer from [Releases](../../releases).
2. Verify the SHA-256 checksum published with the release.
3. Run the installer (Windows SmartScreen may warn while the build is unsigned — this will be explained on the download page).

## Privacy first

- Your imported wallpapers, layouts, and history stay on your PC.
- No account is required. No telemetry is collected.
- Nothing is ever uploaded automatically. Community submissions (when introduced) are explicitly opt-in and reviewed.

See the project documentation for details as it is published.

## Screenshots

Screenshots will be added here before the first public release (sanitised: fictional library content, no personal data).

## Documentation

- [docs/README.md](docs/README.md) — documentation index (plan, architecture, ADRs).
- Architecture decisions live in `docs/engineering/adr/`.

## Support

- Bugs and feature requests: [Issues](../../issues) (please use the templates).
- Questions and ideas: Discussions (to be enabled before launch).
- Security vulnerabilities: **never** in a public issue — see [SECURITY.md](SECURITY.md).
- General help: [SUPPORT.md](SUPPORT.md).

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

GameScape source code is licensed under the [Apache License 2.0](LICENSE). See [NOTICE](NOTICE) for third-party attributions.

The **GameScape name and logo are not covered by the code license** (all rights reserved) — see `docs/community/` when published.

## Disclaimer

GameScape is an independent project. It is not affiliated with, endorsed by, or sponsored by any game studio, publisher, or platform.
