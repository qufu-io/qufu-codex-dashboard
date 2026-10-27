# Task state — 2026-10-27

## Scope and authorization
Implement official Codex measurement parity, whole-turn Last Request, shared quota calibration, persistent history and four small history windows. Preserve the compact dashboard, settings, read-only source access and existing records. Do not upload conversation/token data. The owner authorized GitHub publication on 2026-10-27 after the standalone correction. Release 1.0.2 supersedes the existing 1.0.1 without rewriting tags. Commit/push and signed release are now authorized, subject to platform CI gates.

## Learned from the reference
Official source: openai/codex commit b6ad8ea1fcd5562a7d3003887930be7d84824307. Keep rules and symbol references, not copied source/context logs. TokenUsage: Input includes Cache; Output includes Reasoning; Raw Total = Input + Output; CLI Used = Input - Cache + Output. Context uses the official 12,000-token baseline and remaining-percent rounding. Turn usage snapshots replace previous snapshots rather than adding. Official account percentages remain unchanged. Detailed source references and formulas are in MEASUREMENTS.md.

## Implemented
- data.rs: official cumulative snapshots distinct from response pricing, both event orders, legacy/new deduplication, missing-component flags, exact diagnostic byte boundaries and task-linked subagents without parent double counting.
- turns.rs (new): task lifecycle, latest cumulative task usage, mixed-model per-response pricing, completed/interrupted/failed states, measured end-context and ordered task history. Missing costs remain unknown/partial rather than zero.
- pricing.rs: preserve partial known costs when usage components are unavailable.
- limits.rs: additive schema-4 migration with a consistent backup; retain original rows, persist task/observation/fit history, diagnose rejected intervals and exclude superseded duplicates without deletion.
- Quota mathematics: nonnegative bounded ridge with Huber reweighting for N/C/O; require identifiable observations before independent coefficients. Capacity has a fixed uncached-input-equivalent unit, 100/a. Weekly shares these weights and learns r from simultaneous changes, starting with explicitly assumed r=0.15; weekly capacity = 100/(r*a). Integer-percent uncertainty, intervening decreases, resets, plan/group changes, saturation and held-out errors are handled. No API prices enter quota fitting. Historical validation uses only preceding observations. Capacity changes cannot prove backend policy changes.
- history.ts, main.ts, style.css, i18n.ts: four separate metric-triggered 480x300 charts; completed task Used/raw/USD bars, measured context, global hourly/weekly fit history with observed-percent alternative. Missing periods are gaps. English/Turkish and light/dark supported.
- lib.rs, capabilities/default.json: asynchronous native chart-window commands, close/drag permissions and independent main-window behavior.
- Tests/oracle: dashboard.spec.ts, fixtures/codex-measurements.json, codex_measurement_oracle.py, compare_codex_diagnostics.py. PRODUCT.md and DESIGN.md record the implemented contract.

## Tested and verified
- Rust: 61 passed, 0 failed. Includes 50 official-source oracle cases, turn/deduplication/missing-data behavior, synthetic weights and ratio, reset/rounding/outlier behavior, migration and persistence.
- Playwright: 29 passed. Existing dashboard and four charts, languages/themes, compact sizes, missing values and error states. Frontend production build passed.
- Optimized Windows build succeeded. Test binary: artifacts/test-build-history/QufuCodexDashboard-test.exe (11,392,000 bytes).
- Latest read-only real-data comparison: 9 sessions discovered; 6 had accessible rollouts and supplied 36 checks, all matched. Three unavailable rollouts were skipped, not validated.
- Migration of a consistent copy of the installed schema-0 database: all 13,989 usage rows and 32,790 quota points preserved; 2,536 tasks recovered; pre-migration backup created. Original installed database remained schema 0.
- Focused independent review fixes incorporated. Source secret scan and whitespace checks passed. Private logs and real-data diagnostics remain ignored artifacts, not release inputs.

## Not verified / limitations
Native window interaction could not be automated because the computer-use native pipe was unavailable. Playwright uses mocked IPC and does not establish native window-manager behavior. Linux compilation was not run: this host's WSL has no Rust toolchain. External/cloud usage cannot be reconstructed reliably; weak calibration is marked learning/unstable instead of inventing capacity. Historical context is recovered only when an actual valid measurement exists.

## Delivery and remaining acceptance
Release 1.0.2 is public in both source and downloads repositories. Source tag v1.0.2 identifies 357b5290abd253ff936fb1f725712701d765599d. Source CI 40575838994 passed on Windows and Ubuntu 22.04 (61 Rust and 29 Playwright tests each). Signed-package CI 40575838624 passed for Windows NSIS and Linux DEB/AppImage. All three updater signatures, signed version and tamper rejection passed. Nine public assets were downloaded without authentication and matched local SHA-256 digests; the latest manifest serves 1.0.2 with five platform target aliases. The existing installed app prevented a second clean native launch through single-instance forwarding and was preserved. Native chart interaction and clean-host installation remain manual acceptance checks. Do not repeat completed reference research or passing suites without a new change/failure.

## Standalone packaging correction
The first test executable used plain cargo build --release. In Tauri this does not select the custom-protocol production feature, so the native webview requested the configured development server at localhost:5173. Backend diagnostics bypass the webview and therefore did not detect this packaging failure. The corrected scripts/build-test.ps1 uses the Tauri CLI build --no-bundle, which embeds dist and enables the production protocol. A fresh standalone build and native WebView2 load check are pending; the previous backend/UI test results do not establish standalone startup.

### Standalone correction verified
The Tauri CLI production build succeeded. With localhost:5173 unavailable, the actual executable loaded http://tauri.localhost/ from embedded assets. Native WebView2/CDP inspection confirmed a visible rendered dashboard, five app root children, a working Tauri IPC bridge and no connection-error page. A screenshot was captured privately. Temporary debugging was stopped and the same executable was relaunched normally. This verifies native main-window startup, not every native chart/window interaction. Final SHA256: 0A96D7A57F769AFCE59B674FAA15CFCD55C97BE21ADD57086F42D425BEECDCBF. Packaging changed only; version and publication remain unchanged.

## Publication completed
Version metadata is 1.0.2. Bilingual README/release notes, changelog, measurement methodology, privacy/development/release guidance and contribution templates are published. GitHub community profile is 100%; MIT, conduct, security and contribution policies remain intact. Author/contact: alper@qufu.io. Distribution documentation commit: 27e3047. Public release: https://github.com/qufu-io/qufu-codex-dashboard-releases/releases/tag/v1.0.2 . No private diagnostics or keys were published. No further release action is pending.
