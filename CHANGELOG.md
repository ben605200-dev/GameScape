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
