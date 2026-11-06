# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 1 foundations: monorepo workspace (pnpm + Turborepo), root hygiene files.
- Privacy gate: `scripts/privacy-scan.mjs` and CI workflow.
- Community standards gate: `scripts/community-standards-check.mjs` and CI workflow.
- Community files: README, LICENSE (Apache-2.0), NOTICE, CONTRIBUTING, CODE_OF_CONDUCT (Contributor Covenant 2.1), SECURITY, SUPPORT, issue and PR templates.
- Desktop app scaffold: Tauri v2 + React 19 + Vite 8 (`apps/desktop`), branded for GameScape and integrated into the monorepo. Rust side compiles clean (`cargo check`); frontend builds clean.
- Phase 0 spike S1/S2-lite **passed** — WebView2 surfaces re-parented into the Windows desktop layer (behind desktop icons): classic topology, two monitors with a negative-coordinate arrangement, live rendering visually verified, clean teardown. See `docs/engineering/spike-s1-results.md`.
- Design tokens package (`@gamescape/ui`) — dark/light themes, typography, spacing, radius and motion tokens consumed by the app.
- Desktop app shell: sidebar navigation (9 sections), top bar with theme switcher, Home dashboard with live status cards; theme preference persisted through the settings store.
- Local storage foundation: JSON settings store and a SQLite database with versioned migrations under `%APPDATA%\GameScape`.
- Bundled seed catalogue (fictional preview data) with Rust-side validation, powering the Home shelf and the Wallpapers grid.
- ADR-0002: shell styling approach (tokens-first; Tailwind adopted with the Phase 3 screens).
- Interactive wallpaper prototype **validated on the desktop**: a cursor-reactive light field renders behind desktop icons, driven by a global-cursor bridge in the core (no input capture, no focus stealing). Evidence and limitations in `docs/engineering/spike-s1-results.md`.
- Wallpaper detail pages: clickable cards, full metadata, and a live pointer-reactive preview for interactive entries. The `interactive` flag now lives in the catalogue schema, the Rust model, and the seed.
- Fixed: the spike launch gate now recognises the interactive prototype mode.
- Personal Library **import pipeline** (Phase 3 start): file-picker and drag-and-drop imports for JPG/PNG/WEBP/GIF with magic-byte validation, SHA-256 duplicate detection, decode probing, managed copies in the library folder, JPEG thumbnails, and per-file result reporting (imported / duplicate / unsupported / invalid / corrupted / failed).
- Library page: real grid with thumbnails served through the Tauri asset protocol, per-item removal, and live import progress rows.
- Dev automation hooks (`GAMESCAPE_DEV_IMPORT`, `GAMESCAPE_DEV_FAVOURITE`, `GAMESCAPE_START_HASH`) for local test harnesses — never active in normal use.
- Catalogue **search, filters and sorting**: instant search across titles/games/tags; filters for game, media type, orientation and resolution (both derived from dimensions); newest/A-Z sorting; live result count and clear-all.
- **Favourites**: star toggle on cards and detail pages, persisted in SQLite, with a working Favourites page.
- Seed catalogue expanded from 5 to 14 fictional entries across 4 games and all three media types — including portrait and ultrawide layouts so filters have real variety.
- Shared `WallpaperCard` component now used by Wallpapers, Favourites and the Home shelf.
- Interactive previews are now **per-wallpaper**: an `effect` descriptor (`orbit` / `ribbon` / `grid`) rides in the catalogue schema (validated in the Rust model) and each effect renders tinted with the wallpaper's own placeholder palette. Fixes a gap where every interactive preview rendered the same hardcoded effect.
