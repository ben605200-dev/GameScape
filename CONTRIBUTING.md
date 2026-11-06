# Contributing to GameScape

Thanks for your interest in GameScape! This document explains how to set up the project, what we expect from contributions, and how changes get merged.

By participating, you agree to follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to contribute

- **Bug reports** — use the bug report issue template.
- **Feature requests** — use the feature request template (check the roadmap in `docs/` first).
- **Code** — desktop app, website, tooling, tests, documentation.
- **Wallpapers** — community submissions open with the public gallery release (v1.1). Every submission must have documented rights; see `docs/` for the content policy when published.
- **Translations** — planned after v1.0 (the codebase externalises strings from day one).

## Development setup

**Requirements**

| Tool | Version | Notes |
|---|---|---|
| Windows | 11 (primary) / 10 21H2+ | Desktop app work requires Windows |
| Node.js | >= 22 | Latest LTS recommended |
| pnpm | 12.x | `npm install -g pnpm` |
| Rust | stable (MSVC) | via [rustup](https://rustup.rs) |
| Visual Studio Build Tools | latest | "Desktop development with C++" workload required for Rust on Windows |
| WebView2 runtime | evergreen | Preinstalled on modern Windows |

**Windows note:** if PowerShell blocks `npm.ps1` / `pnpm.ps1` (execution policy), use the `.cmd` shims (`npm.cmd`, `pnpm.cmd`) or set your own policy. Do not weaken machine-wide policies just for this repo.

**Getting started**

```text
git clone https://github.com/OWNER/gamescape.git
cd gamescape
pnpm install
```

Further app/site run instructions are added to this file as each workspace lands (see `docs/README.md` for the plan and roadmap).

## Repository layout

| Path | Contents |
|---|---|
| `apps/desktop` | Windows application (Tauri + Rust core + React UI) |
| `apps/website` | Public website (Astro) |
| `packages/catalogue` | Public wallpaper catalogue (schema, entries, thumbnails, tooling) |
| `packages/shared-types` | Types shared between app, site, and tooling |
| `packages/ui` | Design tokens and shared components |
| `docs/` | Product and engineering documentation, ADRs |
| `scripts/` | Repository automation (privacy scan, community checks, catalogue tooling) |
| `.github/` | Workflows, templates, CODEOWNERS |

## Coding standards

- **TypeScript**: strict mode; no `any` without a comment explaining why.
- **Rust**: `cargo fmt` and `cargo clippy` clean (warnings denied in CI).
- **Commits**: conventional-commit subjects — `feat:`, `fix:`, `docs:`, `catalogue:`, `ci:`, `chore:`, `refactor:`, `test:`.
- **Branches**: `feat/…`, `fix/…`, `docs/…`, `catalogue/…`.
- **Languages**: UI copy in English; keep sentences plain (the interface avoids technical jargon).

## Developer Certificate of Origin (DCO)

All commits must be signed off:

```text
git commit -s -m "feat: add widget snapping"
```

This adds `Signed-off-by: Your Name <you@example.com>` certifying you have the right to submit the work under the project's license ([Apache-2.0](LICENSE)). Contributions are accepted under the same license (inbound = outbound).

Use a GitHub `noreply` email address for commits if you want to keep your personal email private.

## Pull request process

1. Fork the repository and create a branch from `main`.
2. Make your change, add or update tests, and run the checks:
   - `pnpm lint`, `pnpm typecheck`, `pnpm test` (as workspaces land)
   - `pnpm privacy:scan` — must pass
3. Open a PR using the template; link related issues.
4. Required checks and a maintainer review must pass (CODEOWNERS may require additional reviewers for sensitive paths).
5. PRs are squash-merged. `main` is protected and linear.

Keep PRs focused: one thing per PR, small diffs, clear description of what was tested.

## Privacy rules for contributors (mandatory)

The repository and website are **public**. Never include:

- Personal information: usernames, real names, emails (except public project contacts), home-directory paths, computer names, IP addresses.
- Screenshots containing any of the above (use the sanitisation checklist; fictional content only).
- Secrets: keys, tokens, passwords, certificates — see [SECURITY.md](SECURITY.md) for how we handle accidental exposure (rotate first, then clean history).
- Private media or user-generated content.

CI runs a privacy scan on every push and pull request. Findings block merges — and remember that removing a file does not remove it from Git history.

## Catalogue contributions

Catalogue entries follow a strict schema and a rights policy:

- Only content with documented rights (original work, CC0/CC BY family, or verified permission).
- Every entry carries licence metadata and (where required) an attribution string.
- Exact-duplicate hashes and near-duplicate checks run in CI.

The full submission guide is published with the gallery release (`docs/` will contain the detailed policy).

## Security

**Do not open public issues for security problems.** See [SECURITY.md](SECURITY.md) for private reporting.

## Questions

See [SUPPORT.md](SUPPORT.md). For anything ambiguous, open a Discussion once enabled — or an issue with the question label.
