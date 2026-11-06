# GameScape Documentation

This directory holds the project's product and engineering documentation.

## Structure

| Path | Contents |
|---|---|
| `product/` | Product requirements, desktop feature spec, website spec, UI/UX spec (migrated from the planning set as implementation proceeds) |
| `engineering/` | Architecture, security & privacy plan, testing plan, release plan |
| `engineering/adr/` | Architecture Decision Records — one file per significant decision |
| `community/` | Moderation policy, content policy, takedown process, community-standards compliance report |

## Current state

**Phase 1 — Project foundations.** The repository scaffolding, community files, and automated privacy/community gates exist. The desktop app and website scaffolds land next (see roadmap phases in the planning set).

## Key reading order

1. `engineering/adr/` — the recorded decisions (start with ADR-0001).
2. `product/` — what is being built and to what standard (being migrated in).
3. `engineering/security-privacy.md` — rules every contributor must follow (being migrated in).

## Conventions

- No personal data, secrets, or private media in any document (fictional examples only).
- Screenshots must be sanitised before being added; images belong under `docs/assets/`.
- ADRs: one decision per file, numbered `NNNN-title.md`, status line at the top.
