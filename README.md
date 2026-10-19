# Qufu Codex Dashboard 1.0

A compact desktop monitor for local Codex sessions on Windows and Linux.

![Qufu](public/logo.png)

## Install / Kurulum

Open **Releases → 1.0 → Assets** in the GitHub repository. For a private repository, sign in with an account invited by the owner.

- **Windows:** download `*-setup.exe`, run it, then open Qufu Codex Dashboard from the Start menu. Microsoft Edge WebView2 is required; the installer can obtain it when missing. A portable `.exe` is also available.
- **Ubuntu/Debian:** download the `.deb` and install with `sudo apt install ./downloaded-file.deb`.
- **Other Linux distributions:** download the `.AppImage`, run `chmod +x downloaded-file.AppImage`, then launch it. WebKitGTK 4.1 and a working graphical session are required. An AppIndicator tray extension may be needed on GNOME. Wayland compositors can restrict window positioning and always-on-top behavior.

Türkçe: GitHub’daki **Releases → 1.0 → Assets** bölümünü aç. Windows için `*-setup.exe`, Ubuntu/Debian için `.deb`, diğer Linux dağıtımları için `.AppImage` indir. Private repo için davet edilmiş GitHub hesabınla giriş yap.

Private downloads: [Releases](https://github.com/qufu-io/qufu-codex-dashboard/releases). Access requires a GitHub account invited to this repository; sharing the URL alone does not grant access.

## Use

Click the workspace/session name to select a session. The logo is decorative. Use Settings for Turkish/English, transparency, theme, compact view and always-on-top. Defaults: English, 33% transparency and a compact window (300 px wide, height fitted to content). Window geometry and subsequent preferences are remembered.

Closing the window hides it to the tray. Use the tray menu to quit. Auto Follow is off by default and retains manual selection when Desktop selection cannot be verified.

## Data and estimates

- Reads local Codex metadata and session logs without modifying them. Uses `CODEX_HOME`, or `~/.codex` by default.
- Shows non-archived main sessions with confirmed usage. Internal reviews and linked agents are excluded from the picker and accounted separately. No date cutoff or hard-coded session names.
- Cache is part of Input; reasoning is part of Output. Neither is counted twice. Response IDs deduplicate usage. Earlier cumulative usage is preserved when detailed history is missing.
- Tokens/sec is the last 60 seconds of usage divided by 60.
- Unknown usage/pricing is shown as unavailable, not zero. USD is an estimate of API-equivalent cost, not a subscription invoice.
- On every launch, downloads the current public OpenAI [pricing table](https://developers.openai.com/api/docs/pricing). Valid prices are cached locally. Connection or parsing errors preserve the last valid cache; the included price table is the first-launch fallback.
- Costs are repriced from short/long-context usage buckets without rescanning logs. Standard, Fast, Ultrafast, Flex and Batch are used only when the model and tier have published rates.
- The installed Codex CLI supplies account limits through `account/rateLimits/read`. This app never reads authentication tokens. When unavailable, a dated local observation is shown without pretending to be live.
- Cloud/ChatGPT conversations without local usage records are not included. Unattributed internal usage is not guessed into a parent session.

Only public price requests and the installed Codex account reader use the network. Conversations and usage are not uploaded to a dashboard server.

## Build

Requires Node.js 22+, Rust stable and platform build dependencies.

```sh
npm ci
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
npm test
```

Windows: Visual Studio C++ Build Tools and WebView2; run `powershell -File scripts/build.ps1` to create the installer and portable app.

Linux: GTK3, WebKitGTK 4.1, AppIndicator, librsvg, a C/C++ toolchain and patchelf. `scripts/build-linux.sh` uses an isolated build cache and creates `.deb`/`.AppImage` packages. Its local host baseline is that of the build distribution. GitHub release builds use Ubuntu 22.04 for wider compatibility.

The GitHub Actions workflow builds both platforms into a **draft** release. Review successful checks and artifacts before publishing. Local test artifacts, Codex data and build tools are excluded from Git uploads.

## Author

[qufu.io](https://qufu.io)
alper@qufu.io

Copyright © 2026 alper@qufu.io. [MIT License](LICENSE).

The main display shows total tokens, Input/Output/Cache, last completed model response tokens with its own Input/Output/Cache, last measured context usage, and session USD. Context uses the session-reported capacity without guessing from the subscription. Details show the current account subscription when reported by Codex. Tokens/sec is shown at the upper right.

Current 1.0 layout: session USD alongside total tokens, last-response USD alongside last-response tokens, Input/Output/Cache under each row. The workspace line shows only the path. Subagent totals and records appear only in Details. System icons use the approved black-frame Lucide square-activity adaptation; the in-app Qufu logo is unchanged. Icon license: [Lucide ISC](docs/third-party/LUCIDE-LICENSE).

Approved 1.0 update: Square Activity system icon with black outer frame; in-app Qufu logo unchanged. The extra hide-to-tray button is removed. Displayed percentages use a leading percent sign and USD values use a trailing dollar sign. Context displays measured used/capacity tokens. Quota token fractions are approximate equivalents from local price-weighted calibration, not official token quotas or measured account-wide token spending. Their numerator is mapped from the reported account percentage; local reference usage, held-out error and sample counts are in the tooltip. Calibration runs in the background, scans incrementally at 30-second intervals, and stays unavailable when pricing coverage or validation is insufficient. Internal guardian/auto-review records remain in session details but are excluded from the public-model reference mix for quota estimates.

The workspace/session heading is repeated above totals for readability. A divider separates total and last-response usage. Context changes to a subdued warning color above %80; account windows do so above %90.
