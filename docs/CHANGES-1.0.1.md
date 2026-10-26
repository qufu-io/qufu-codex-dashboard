# 1.0.1 — usage fixes and compact settings

The owner accepted these fixes and explicitly authorized version **1.0.1**, commit, push and release on 2026-10-26. Signed Windows/Linux packages must pass CI and signature checks before publication.

## Root causes and fixes

- Pricing assumed positional columns and three/four/eight price cells. The official table now places Output before cached input and exposes separate 30m/12h writes. A fetch could succeed while omitting a used model or misreading its columns. Lookup also required exact model strings; refresh replaced the entire old catalog; any unknown model poisoned the full session sum. The parser now reads column names, preserves context/tier boundaries, safely merges verified catalogs and versions caches so obsolete positional-parser data cannot override fresh bundled rates. Exact rates win, then normalized reasoning effort, known latest/date variants and a restricted adjacent minor version within the same major/model flavor and service tier. Conflicting neighboring price bands and unrelated/pro/mini/cyber variants do not receive arbitrary fallback prices. Retained/fallback rates are approximate. Unknown components preserve the known subtotal and expose partial status.
- Last-response costs use that response's model/tier and usage, retaining them across later context changes. Explicit null thread-tier overrides clear Priority. Priority maps to Fast according to the current official naming; neither falls back to Standard. Cache writes replace the corresponding input rate, never add a surcharge. Reasoning remains a subset of Output.
- Quota calibration depended on USD, required 98% pricing coverage and dropped samples after 14 days. It now uses measured non-cached input, cached input and output with no monetary coefficients. All local threads (including archived descendants) feed persistent SQLite observations, response IDs deduplicate copies, and prefix-checked scan cursors avoid repeated historical reads. Rollout data and Codex databases remain read-only; only the dashboard's own app-data database is written.
- Subagent aggregation previously hid all tokens if one child was missing and used an all-or-nothing USD sum. The reader now preserves verified subtotals, separately flags completeness, resolves nested relations and moved archived rollouts, and reads complete JSON tails before newline termination. Stable anonymous keys prevent repeated tails without response IDs from being counted twice. Independent user forks stay separate even when their rollout parent metadata is cached. Missing records stay unknown.
- Settings had 16px gaps, large grid/panel/author spacing and separate update paragraphs. Toggle rows are grouped, grid gaps are 7px, selects retain a 24px minimum height and right-side arrow spacing, API notes stay, and ordinary update status shares the action row. Detailed failures/install messages can wrap below. The redundant Auto Follow explanation is removed in both languages; behavior is unchanged.

## Statistical method

Initial capacity is observed tokens divided by the observed fraction of quota consumed. With at least 12 non-overlapping intervals over three reset windows, fit:

`delta percentage points = a * noncached input + b * cached input + c * output`

Features are scaled by 100,000 tokens for numerical stability. Coefficients use ridge regularization around the observed pooled ratio (lambda 0.05), nonnegative bounds and six Huber reweighting passes based on residual MAD. These priors contain no API prices. A model/tier gets its own coefficients only with sufficient homogeneous observations; otherwise it uses the pooled fit. The latest 240 usable intervals adapt the fit; older raw observations remain persisted.

Use a chronological quota timeline. Interleaved reset observations, negative percentage movements, known plan changes and quota increases without local usage do not become spending intervals. Unknown-plan observations receive lower weight. Validation trains only on earlier, non-overlapping reset windows with a 30-second embargo; no in-sample error or invented confidence percentage is displayed. Empirical median/p90 relative errors need at least four held-out examples; the validated label needs at least eight. Typical error above 35% or p90 above 100% withholds capacity. Missing records or typical error above 20%/p90 above 50% downgrade a usable estimate to limited accuracy. Early ratio estimates are explicitly learning estimates, with no fabricated error percentage.

Displayed effective capacity is conditional on the observed model/tier/token mix. The approximate numerator maps reported account percentage onto that learned capacity; the separate `localTokens` field is the verified local subtotal. Neither is an official OpenAI token allowance. Cloud/other-device usage cannot be reconstructed exactly; consistent unobserved usage or new token mixes may bias estimates despite validation. Missing historical files cannot be recovered or replaced with invented zero usage.

## Changed files

- `src-tauri/src/pricing.rs`, `src-tauri/prices.json`: parser, model/tier matching, catalog/cache safety and quote quality.
- `src-tauri/src/data.rs`: response accounting, partial totals, subagent relations/rollout reads and scanner cursors.
- `src-tauri/src/limits.rs`, `src-tauri/src/lib.rs`: persistent token-based learner, validation and app-data integration.
- `src/main.ts`, `src/style.css`, `src/i18n.ts`: partial/approximate display, learning tooltips and compact settings.
- `tests/dashboard.spec.ts`, `tests/fixtures/openai-pricing-2026-10-26.md`: behavioral UI and official-price regressions; Rust tests live alongside the implementation.
- `README.md`, `PRODUCT.md`, `DESIGN.md`: updated behavior/accuracy documentation.

## Verification

- Windows and WSL Ubuntu 26.04 Rust tests: 41 passed on each platform, including price cache migration, official rates, effort/alias/context/tier, partial costs, independent hourly/weekly regressions, outliers, plan/reset changes, cursor restart and nested/missing/copied agents.
- Frontend type/build check passed; 17 Playwright UI tests passed. Settings were checked at 300/371px, English/Turkish, light/dark, with inline up-to-date state, update failures, no overflow and preserved author footer.
- Measured settings panel: **535px before, 358px after — 177px less height** on the Windows synthetic fixture. Other platforms/fonts can produce different pixel heights.
- Real-data diagnostics and reports remain in ignored private artifacts. Eight main sessions had calculable session and last-request USD; independent record comparison confirmed token components, last-response cost and context on all eight. Some archived/internal records are unavailable, so their sums stay partial. The real weekly calibration failed independent error thresholds and correctly withheld capacity while preserving local usage.
- Current official Markdown was fetched and 206 rates parsed. The versioned fixture verifies the actual Output/cached/30m/12h/long-context order. Sources: https://developers.openai.com/api/docs/pricing and https://developers.openai.com/api/docs/guides/prompt-caching .
- Local optimized Windows executable was built without creating or publishing installer/updater assets. Native tray/installer behavior still needs owner acceptance; synthetic UI tests do not prove every native interaction.
