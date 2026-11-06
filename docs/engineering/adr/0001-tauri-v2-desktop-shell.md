# ADR-0001: Tauri v2 as the desktop application shell

- **Status:** Accepted
- **Date:** 2026-11 (Phase 1)

## Context

GameScape is an always-running Windows desktop application that must:

- render animated wallpapers *behind desktop icons* (undocumented shell technique, Win32 heavy);
- manage per-monitor windows, transparent no-activate overlay windows, global hotkeys, and power/fullscreen state watching;
- keep a small memory footprint and tiny installer, because gamers judge it while playing;
- share design language and types with a static website built with web technologies;
- expose as little attack surface as possible (updater signatures, capability allowlists).

The realistic candidates were Electron, Tauri v2 (system WebView2 + Rust core), and a fully native .NET (WinUI 3 / WPF) application.

## Decision

**Tauri v2** with a **Rust core** and a **React + TypeScript** front-end.

Rust owns: Win32/COM integration (desktop host, `IDesktopWallpaper`, power state), SQLite storage, validation/hashing, download/update engines. React owns: all user interface. WebView2 renders wallpaper and widget surfaces via the same web stack used in previews.

## Consequences

**Positive**

- Small installers (~5–10 MB class, plus WebView2 which is already present on modern Windows) and low idle memory versus Electron.
- Direct, well-typed Win32 access in Rust is the right tool for the desktop-integration work; prior art exists for WorkerW/Progman reparenting in Tauri.
- One language for web UI and website; Rust for systems work.
- Mandatory-signed updater supported natively via the Tauri updater plugin.

**Negative / risks**

- Depends on the WebView2 Evergreen runtime (bootstrapper provided by the installer; version varies per user — mitigated by a version floor and graceful degradation).
- Behind-icons compositing with WebView2 requires specific window styles (`WS_EX_LAYERED` with full alpha on raised desktops); verified by Phase 0 spikes S1–S3 with documented fallbacks.
- Some Chromium switches are unavailable in Edge; background-rendering behaviour must be verified (Spike S3).

## Alternatives considered

| Alternative | Why not |
|---|---|
| Electron | 10–20× installer size, higher idle RAM — contradicts the product's performance-first principle; would still need native FFI for Win32 work |
| .NET WinUI 3 / WPF | Strong Windows integration but a second UI stack; slower rich-UI iteration; website would not share components |
| Hybrid (Tauri UI + native helper process) | Reserved as a contingency only; extra IPC and update surface |

## References

- Phase 0 spikes S1–S7 (verification gates) — `docs/engineering/` (migrating in).
