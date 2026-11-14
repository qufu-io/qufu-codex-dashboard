# Changelog

Notable user-facing changes. Releases use semantic versioning; published tags are not rewritten.

## Unreleased

- Default weekly quota display to API-equivalent dollars with a persistent Token/API $ switch and two decimal places.
- Derive quota-dollar projections from the observed N/C/O mix; retain unknowns, scenario caveats and official percentages.
- Simplify task/quota chart selectors to Raw Token and API $, plus weekly official percentage; remove task-spending warnings.
- Enable Auto Follow on migration, remove its Beta label, preserve verified-selection safeguards and later opt-out.
- Keep Settings and Details at the main 300px width and remove the redundant context-source caption.

## 1.0.8 — 2026-11-14

- Shorten the weekly quota card to its approximate numeric fraction; keep scenario assumptions in the tooltip and history.
- Show three decimal places on both sides of quota token fractions, with K/M/B units.
- Produce Windows packages only for new releases; preserve historical Linux downloads.

See [bilingual release notes](docs/CHANGES-1.0.8.md).

## 1.0.7 — 2026-11-13

- Add an explicitly assumed weekly Sol 6.1-equivalent view with fixed bundled API ratios and a robust scalar calibration.
- Preserve strict known-price-only and original quota history views, official percentages and all original local records.
- Label the GPT-5.4 Auto Review proxy and retain causal historical cutoffs and uncertainty.

See [bilingual release notes](docs/CHANGES-1.0.7.md).

## 1.0.6 — 2026-11-08

- Give quota amounts and learning status separate space; preserve English K/M/B token units at 300 px.
- Add local task-volume and history-relative usage notices; explain CLI Used versus Raw Total in charts.
- Fix legacy/new task response double counting and retain known usage across legacy counter resets, marking missing coverage partial.
- Preserve original historical records; old stored task graphs are not retroactively recomputed.

See [bilingual release notes](docs/CHANGES-1.0.6.md).

## 1.0.5 — 2026-11-08

- Add usage-mix-dependent raw quota projections alongside the fixed reference capacity and official percentages.
- Fix reference-based change analysis, same-regime chronological validation and plan/reset mix boundaries; clearly label provisional data.
- Preserve historical records and missing-mix gaps; restore CLI Used task-history default.
- Hide the five-hour card for personal Pro plans and clarify measured session context capacity.

See [bilingual release notes](docs/CHANGES-1.0.5.md) and [technical review](docs/PR1_REVIEW.md).

## 1.0.4 — 2026-10-29

- Isolate quota observations, history and independent hourly/weekly models by a pseudonymous verified account identity.
- Exclude account transitions, unidentified historical observations and long observation gaps from account learning; retain old data.
- Support weekly-only accounts without inventing a five-hour quota; show a short local account code in Details.
- Consolidate public source, downloads and update endpoints into this repository. Existing users must install 1.0.4 manually before removing the legacy downloads repository.

See [bilingual release notes](docs/CHANGES-1.0.4.md).

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
