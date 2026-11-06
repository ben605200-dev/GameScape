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
