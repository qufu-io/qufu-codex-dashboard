# Changelog

Notable user-facing changes. Releases use semantic versioning; published tags are not rewritten.

## 1.0.3 — 2026-10-28

- Learn five-hour and weekly N/C/O quota weights independently; remove the weekly conversion ratio and shared coefficients.
- Preserve historical fits and distinguish the previous shared method from independent calibration in history.
- Keep Version, Check for updates and the Qufu/author footer visible with Settings closed.
- Retain automatic window sizing, existing preferences and signed Windows/Linux updates.

See [bilingual release notes](docs/CHANGES-1.0.3.md).

## 1.0.2 — 2026-10-27

Same-version refresh: removed the Compact view setting; fixed startup and panel sizing, migrated legacy settings without losing other preferences, and replaced the README image with a privacy-safe illustrative screenshot. Existing 1.0.2 installations need a manual reinstall to receive this refresh; the version and original source tag remain unchanged.

### Added

- Complete task-turn accounting, task states and per-response costs for mixed-model tasks.
- Small task, context and global quota-history windows, available by clicking the associated metric.
- Persistent task facts and calibration history, with a backed-up, additive SQLite migration.
- Shared hourly/weekly quota model with a fixed uncached-input-equivalent reference unit and empirical validation.
- Official-source measurement oracle and regressions for deduplication, task snapshots and missing data.

### Fixed

- CLI Used and context baseline/rounding parity with official Codex measurements.
- Legacy/modern response overlap and cumulative task snapshots counting the same usage twice.
- Unknown cost components being treated as zero or hiding a known partial subtotal.
- Standalone test builds requesting the Vite development server instead of loading embedded assets.

### Clarified

- Official account percentages remain measured values. Learned quota capacity and API-equivalent USD are estimates.
- Local observations cannot establish an undisclosed OpenAI quota change as fact.

See [bilingual release notes](docs/CHANGES-1.0.2.md) and the [measurement contract](docs/MEASUREMENTS.md).

## 1.0.1 — 2026-10-26

Model/effort-aware pricing, safe catalog refresh, token-based quota calibration, partial subagent totals and compact settings. See [technical release notes](docs/CHANGES-1.0.1.md).

## 1.0.0 — 2026-10-19

Initial Windows/Linux desktop release: local session discovery, tokens, estimated costs, context, account limits, tray integration, localization and signed in-app updates. Originally distributed as freeware; the source was subsequently published under MIT. See [licensing transition](docs/LICENSING.md).

Author: [alper@qufu.io](mailto:alper@qufu.io).
