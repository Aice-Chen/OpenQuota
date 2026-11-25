<p align="center">
  <img src="assets/openquota-icon.png" alt="OpenQuota logo" width="88">
</p>

<h1 align="center">OpenQuota (Windows Enhanced Fork)</h1>

<p align="center">
  Track usage and limits across your AI coding tools on Windows — independently maintained fork of <a href="https://github.com/deviffyy/OpenQuota">deviffyy/OpenQuota</a> (MIT License).<br>
  基于 MIT 协议的 Windows 独立演进增强版：原生中英文切换（含系统托盘与通知）、修复 Antigravity 额度读取、独立签名自动更新与模型定价热更新。
</p>

<p align="center">
  <a href="https://github.com/Aice-Chen/OpenQuota/actions/workflows/ci.yml"><img src="https://github.com/Aice-Chen/OpenQuota/actions/workflows/ci.yml/badge.svg" alt="Windows CI status"></a>
  <a href="https://github.com/Aice-Chen/OpenQuota/actions/workflows/build-windows.yml"><img src="https://github.com/Aice-Chen/OpenQuota/actions/workflows/build-windows.yml/badge.svg" alt="Windows release build status"></a>
  <a href="https://github.com/Aice-Chen/OpenQuota/releases/latest"><img src="https://img.shields.io/github/v/release/Aice-Chen/OpenQuota" alt="Latest release"></a>
  <a href="https://github.com/Aice-Chen/OpenQuota/releases"><img src="https://img.shields.io/github/downloads/Aice-Chen/OpenQuota/total" alt="Total downloads"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT license"></a>
</p>

OpenQuota brings usage data from Claude Code, Codex, Cursor, Antigravity, Copilot, and other AI
coding providers into one compact Windows tray or floating panel. See session and weekly limits,
reset times, token usage, and estimated spend at a glance.

<p align="center">
  <img src="assets/openquota-demo.gif" alt="OpenQuota dashboard showing animated AI usage in light and dark themes" width="840">
</p>

## Download (Windows)

| Platform      | Installer                                                                        | Download                                                                       |
| ------------- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Windows (x64) | `OpenQuota_<version>_x64-setup.exe` (NSIS current-user installer, auto-updating) | [Download for Windows](https://github.com/Aice-Chen/OpenQuota/releases/latest) |

OpenQuota checks for updates automatically. Update payloads are cryptographically signed with this
fork's Minisign updater key (`plugins.updater.pubkey`). Because the installer is not
Authenticode-signed with a paid EV certificate, Windows SmartScreen may ask for confirmation on
first download — download OpenQuota only from this repository's official Releases page.

## Supported providers

- **[Claude Code](docs/providers/claude.md)** — multiple accounts, session and weekly limits,
  model-specific usage, token history, and estimated spend
- **[Codex](docs/providers/codex.md)** — session and weekly limits, credits, token history, model
  breakdown, and estimated spend
- **[Cursor](docs/providers/cursor.md)** — total, Auto and API usage, credits, token history, and
  estimated spend
- **[Antigravity](docs/providers/antigravity.md)** — Gemini and Claude quota pools with dynamic
  bucket support
- **[Copilot](docs/providers/copilot.md)** — premium requests, extra usage, chat and completion
  quotas, plus organization billing
- **[Devin](docs/providers/devin.md)** — daily and weekly limits, reset times, and extra usage balance
- **[Grok](docs/providers/grok.md)** — weekly allowance, extra usage status, token history, and
  estimated spend
- **[OpenCode](docs/providers/opencode.md)** — OpenCode Go session, weekly and monthly spend caps,
  plus local hosted usage history
- **[OpenRouter](docs/providers/openrouter.md)** — credit balance and daily, weekly and monthly spend
  (API key)
- **[Z.ai](docs/providers/zai.md)** — GLM Coding Plan session, weekly, and web-search quotas (API key)
- **[Kimi](docs/providers/kimi.md)** — Kimi Code session and weekly quotas (API key)
- **[MiniMax](docs/providers/minimax.md)** — Token Plan session and weekly quotas (API key)

Most providers read credentials already stored on your computer. OpenRouter, Z.ai, Kimi, and MiniMax
use API keys stored securely in Windows Credential Manager.

## Features

- **Tray or floating dashboard.** View quotas in a compact popup anchored to the Windows system
  tray, or keep the panel open as a floating desktop window.
- **Native Chinese / English UI.** Full Simplified Chinese and English support across UI, tray
  tooltips, context menu, provider error messages, and notifications.
- **Used or left & pacing alerts.** Toggle consumed vs. remaining display and see whether current
  usage is on pace to last until the next reset.
- **Usage history & pricing auto-sync.** Review today, yesterday, and 30-day token usage and
  estimated spend with automatic daily pricing updates.
- **Custom layouts.** Reorder providers and metrics, hide rows, rename cards, and pin metrics to the
  tray.
- **Local-first privacy.** Runs locally with no account, cloud backend, analytics, or telemetry.

## Development

Requirements (Windows):

- Node.js 22 or later
- pnpm 11.11.0
- Stable Rust toolchain + WebView2 / Visual Studio C++ Build Tools
  ([Tauri 2 Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows))

Install dependencies and start the development app:

```sh
corepack pnpm install --frozen-lockfile
corepack pnpm tauri dev
```

Run checks and build the Windows installer:

```sh
corepack pnpm verify
corepack pnpm build:installer
```

See [docs/releasing.md](docs/releasing.md) for the `pnpm bump` and GitHub Actions release workflow.

## Acknowledgements

Forked from [deviffyy/OpenQuota](https://github.com/deviffyy/OpenQuota) (MIT License), originally
inspired by [OpenUsage](https://github.com/robinebers/openusage).

## License

[MIT](LICENSE)
