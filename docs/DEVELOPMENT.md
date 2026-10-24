# Development / Geliştirme

## Prerequisites

- Node.js 22 or newer compatible with Vite 7, npm, Git and Rust stable (install through rustup).
- Windows x64: Visual Studio 2022 Build Tools with Desktop development with C++ and Windows SDK; Microsoft Edge WebView2 Runtime.
- Ubuntu 22.04+ / Debian with GTK3, WebKitGTK 4.1 and AppIndicator development packages:

```sh
sudo apt-get update
sudo apt-get install -y build-essential curl libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

Other Linux distributions need equivalent packages. A graphical desktop is needed to run the native app; tray/window behavior depends on desktop/Wayland support. macOS is not currently a supported release target.

## Clone and run

```sh
git clone https://github.com/qufu-io/qufu-codex-dashboard.git
cd qufu-codex-dashboard
npm ci
npm run tauri -- dev
```

`npm run dev` serves only the frontend; it does not provide native Codex-reading commands. Native usage needs local Codex records and, for account limits, a signed-in `codex` CLI on PATH. Set `CODEX_HOME` only if your Codex data lives elsewhere. The tests below use synthetic fixtures and do not need account access or local conversation data.

## Verification

```sh
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
npx playwright install chromium
npm test
```

On Linux CI use `npx playwright install --with-deps chromium` to install browser system dependencies too. On PowerShell systems that block `.ps1` npm shims, use `npm.cmd` and `npx.cmd`. Test screenshots and reports go to ignored `artifacts/`, `test-results/` and `playwright-report/`. UI tests mock native IPC: they verify layout and behavior, not real tray/installer/window-manager behavior. CI repeats build, Rust and UI checks on Windows and Ubuntu.

## Local executable without release signing

```sh
npm run tauri -- build --no-bundle
```

The executable is under `src-tauri/target/release/`. A signing key is not required. Official installers/updater artifacts use the separate [release process](RELEASE.md); the maintainer's private key is never part of the repository. Forks must configure their own identity, update endpoint and signing public key in `src-tauri/tauri.conf.json` before distributing update-enabled packages. The shipped public verification key is not a secret.

## Layout

- `src/`: TypeScript UI, translations and styles.
- `src-tauri/src/data.rs`: read-only metadata/usage discovery and session aggregation.
- `src-tauri/src/pricing.rs`: pricing lookup, cache and cost calculations.
- `src-tauri/src/limits.rs`, `quota.rs`: account snapshots and gated approximate token calibration.
- `src-tauri/src/updates.rs`: signed updater checks/install actions.
- `tests/`: synthetic UI regression tests; Rust unit tests live beside Rust implementation.

Keep private paths, session titles, auth files, credentials and logs out of commits and issue attachments. Use the [contribution guide](../CONTRIBUTING.md) and [security policy](../SECURITY.md).

## Türkçe

Node.js 22+, Rust stable ve yukarıdaki platform bağımlılıklarını kur. Depoyu klonla; `npm ci` ve `npm run tauri -- dev` ile yerel uygulamayı çalıştır. Yalnızca `npm run dev` native veri okuyucusunu çalıştırmaz.

Doğrulama bölümündeki build, Rust ve Playwright komutlarını çalıştır. Testler sentetik veri kullanır; hesap/token veya gerçek konuşma kaydı gerektirmez. Windows'ta gerekirse `npm.cmd` / `npx.cmd` kullan. `npm run tauri -- build --no-bundle` imza anahtarı gerektirmeyen yerel çalıştırılabilir dosya üretir. Resmi kurulumlar ve güncelleme imzalama ayrı sürüm sürecidir. Fork dağıtırken kendi kimliğini, güncelleme adresini ve imza anahtarını ayarla. Özel veri veya anahtarları commit etme.
