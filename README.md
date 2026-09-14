# Codenotch for Windows (codenotch4win)

[한국어 README](README.ko.md) · [PRD (Korean)](docs/PRD.md)

A Windows build of [Codenotch](https://github.com/vinzdg/codenotch) — the usage notch that
sits on the edge of your screen and answers two questions at a glance:
**how much of my AI allowance is left**, and **is Claude still working**.

This repository is a fork of [Im-Midi/codenotch-windows](https://github.com/Im-Midi/codenotch-windows)
(the Windows port merged into upstream as its `windows/` tree). The port itself is mature —
what it lacked was distribution. This fork exists to close that gap:

- **Prebuilt binaries** — download and run, no Rust toolchain required
- **Auto-update** — the app keeps itself current from GitHub Releases
- **Start with Windows** — one checkbox in Settings (already in the port; enabled on first run)
- **Korean localization** — on top of the existing i18n layer

See the [roadmap](#roadmap) for where each of these stands.

## What it shows

Same design language as the macOS original (inverse-rounded pill, colour-graded rings,
hover card with per-window bars), rebuilt for Windows in Rust + Tauri 2 / WebView2.
No code is copied from the Swift app; the providers are reimplemented from their
documented behaviour and the wire formats.

| Cell | Source | How it reads it |
|---|---|---|
| **Claude** | `GET https://api.anthropic.com/api/oauth/usage` with the token Claude Code keeps in `~/.claude/.credentials.json` | Session / weekly windows, 429 back-off, stale readings dimmed with their age. A thin arc spins while a Claude session is working, and pulses amber when one is waiting on you. |
| **Codex** | The local Codex sign-in in `~/.codex/auth.json` (read only), falling back to the newest session snapshot | Live primary/secondary windows (5h + weekly on paid plans, monthly on free). |
| **Cursor** | The editor's own session from `state.vscdb` → `cursor.com/api/usage-summary` | Included usage / API usage / on-demand, reset at billing-cycle end. |
| **Antigravity** | Official `agy` CLI `/usage` print when installed; otherwise the local `language_server` bridge | Official quota rows (Gemini & Claude/GPT, 5h/weekly) without running the full IDE. |

Providers that are not installed simply do not get a cell.

## Install

> **Status:** first binary release is milestone [M1](../../milestones). Until it ships, build from source (below).

Two flavours will be published on the [Releases](../../releases) page:

- **`Codenotch-Setup-x.y.z.exe`** (NSIS installer) — recommended. Installs per-user (no admin),
  and is the **auto-update channel**: the app checks GitHub Releases and updates itself.
- **`codenotch-x.y.z-portable.exe`** — single portable executable, run from anywhere.
  No auto-update; it notifies you when a new version is available.

Requirements: Windows 10/11 with the WebView2 runtime (preinstalled on Windows 11;
the installer bootstraps it on Windows 10).

## Build from source

Prerequisites: Rust (MSVC toolchain), WebView2 runtime.

```powershell
cargo build --release
.\target\release\codenotch.exe          # pill appears on the right edge of the primary monitor
.\target\release\codenotch.exe doctor   # self-diagnosis: credentials, data sources, icons, hooks
```

Tray menu: **Settings…**, **Refresh usage now**, **Quit**. Everything else is in the settings
window: the taskbar icon, which rings the notch shows, its size, **start with Windows**, the
language, Claude Code hooks, reset position, and the data folder (`%APPDATA%\codenotch` —
logs, persisted readings, icon overrides).

## Privacy & security

- Credentials are read from the files each vendor's own app already keeps on your machine,
  and are only ever sent to that vendor's own endpoint. Nothing is sent anywhere else.
- No telemetry, no analytics.
- Auto-update artifacts are signed; the updater verifies signatures before applying.
- Runs entirely per-user; no administrator rights needed.

## Roadmap

Work is tracked with [issues](../../issues) and [milestones](../../milestones):

| Milestone | Goal |
|---|---|
| **v0.4.0 — First binary release** | GitHub Actions release pipeline; installer + portable exe downloadable from Releases |
| **v0.5.0 — Auto-update** | `tauri-plugin-updater` + signed `latest.json` on GitHub Releases |
| **v0.6.0 — First-run experience** | Onboarding (offer autostart), Korean localization, Windows 10 verification |
| **v1.0.0 — Expansion** | More providers (Gemini CLI, GitHub Copilot, …), upstream sync & contribution |

## Relationship to upstream

The port follows the upstream design and provider semantics. It is developed at
[Im-Midi/codenotch-windows](https://github.com/Im-Midi/codenotch-windows) and offered to
[vinzdg/codenotch](https://github.com/vinzdg/codenotch) as its `windows/` tree.
This fork tracks that work (remote `upstream`) and intends to contribute the release and
auto-update pipeline back once proven.

### Icons

Provider marks are the SVGs from [`@lobehub/icons-static-svg`](https://github.com/lobehub/lobe-icons)
(MIT), embedded unmodified — see `codenotch/glyphs/NOTICE.md`. Drop your own
`claude|codex|cursor|gemini.svg` (or `.png`) into `%APPDATA%\codenotch\glyphs\` to override.
The marks remain the trademarks of their owners.

## License

MIT — see [`LICENSE`](LICENSE). The Codenotch design and name belong to the
[upstream author](https://github.com/vinzdg); the Windows port is by
[Im-Midi](https://github.com/Im-Midi); this fork adds distribution on top.
