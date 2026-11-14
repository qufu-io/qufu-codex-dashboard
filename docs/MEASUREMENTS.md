# Measurements and history

Official session measurements, API-equivalent costs and learned account-quota projections have distinct scopes and units.

## Official measurements

Reference: [OpenAI Codex b6ad8ea](https://github.com/openai/codex/tree/b6ad8ea1fcd5562a7d3003887930be7d84824307), particularly `protocol/src/protocol.rs` (`TokenUsage`, `TokenUsageInfo`, `TokenUsageRecord`, turn lifecycle events), `core/src/state/session.rs`, and `app-server-protocol/src/protocol/v2/thread.rs`.

- Input includes cached input. Uncached input is `max(Input − Cache, 0)`.
- Output includes reasoning. Reasoning and cache are never added again.
- Raw Total is `Input + Output`; CLI Used is `Uncached Input + Output`.
- The latest official `token_count.info.total_token_usage` is retained separately from response accounting. When a newly completed response precedes its CLI snapshot, response accounting is the temporary fallback. Details expose CLI Used; the main Total keeps its raw session scope. Subagents are separate.
- Context uses the **last official context measurement**, not accumulated task usage. With `B=12000`, CLI remaining percent is `round(clamp((max(W−B−max(T−B,0),0)/(W−B))*100,0,100))`; for `W<=B` it is zero. Displayed used percent is `100 − remaining`. Raw measured context tokens and reported capacity are retained alongside this baseline-adjusted percentage; their simple ratio therefore need not equal the displayed percentage.
- Official account percentages/reset times come from the installed Codex `account/rateLimits/read`. They are not replaced by the learned model. An independent app-server is not assumed to have another process's live thread state; thread usage comes from read-only rollouts.
- Missing cache/reasoning fields are explicitly flagged. Missing cache excludes that response from quota training, makes CLI Used unavailable, and permits only a partial price subtotal for known components.

`scripts/codex_measurement_oracle.py <official-source-checkout>` extracts and compiles the unmodified official TokenUsage methods to generate `tests/fixtures/codex-measurements.json`. Normal tests use the committed 50-case fixture without network access or compiling Codex itself.

## Last Request is a task

`turn_id` identifies a user task, including all of its model responses. `turn_token_usage` is a cumulative snapshot and replaces the previous task snapshot. Response IDs deduplicate individual pricing records; each response keeps its own model/tier/cache-write/context tariff. A missing response price does not erase the known subtotal.

Legacy counters are attributed only across known task boundaries. A missing boundary or snapshot cannot establish a full task total. Completed, active, interrupted, failed and incomplete tasks remain distinguishable. Last completed task is separate from the current task. Context is saved at completion only if a fresh official measurement occurred during that task; quota-only updates do not erase it. Compaction invalidates stale context.

`root_turn_id` links known child tasks to their originating task. Linked descendant usage appears separately in Details and is not added to the main task. Missing root links stay unknown. Parent-child traversal deduplicates child IDs; copied foreign-thread records are rejected.

## Learned quota model

Since 1.0.3, each window learns its own three independent weights from its own percentage changes:

`Δpercentage points ≈ a·N + b·C + c·O`

Here N is uncached input, C cached input, and O output including reasoning. No USD price enters this calculation.

An early ratio supplies provisional equal weights, explicitly marked learning. At least 12 intervals across three resets and a full-rank feature Gram matrix are required for the three-variable fit. Proportional/collinear inputs cannot establish independent weights. The positive bounded ridge fit uses 100,000-token numerical scaling, a small ridge penalty (0.0001) and six Huber reweighting passes. The reference unit never changes: **one uncached-input-equivalent token**. Thus each window’s capacity is `100/a_window`, and observed account percent maps to an approximate numerator in that same unit. These are not raw token allowances.

The raw-token projection is a separate, **usage-mix-dependent** metric. Given the N/C/O token amounts observed in the current account/window and learned weights, `rawCapacity = 100 × (N+C+O) / (aN+bC+cO)`. The original `capacity` and `used` reference-unit values remain available and unchanged. Main quota cards prioritize the raw projection when supported, while the history selector can display either raw projections or the original reference capacity. Historical `initial-ratio` fits receive a read-only raw overlay only when all three saved positive finite weights are equal and their saved capacity agrees with `100/a`; independent fits without a saved observed mix must remain missing, **not** guessed from the reference capacity. Learning projections are explicitly labeled provisional on the main card and omitted from reliable history lines; unstable/insufficient capacities are withheld. Raw projections vary with usage mix and do not certify an official fixed backend limit. The raw numerator is the official percent mapped into the projected unit, not independently measured account-wide spending. Recorded local tokens remain a separate subtotal.

The five-hour and weekly windows have separate training intervals, coefficients, reset cohorts, validation errors and capacity estimates. Weekly calibration works without any five-hour observations and is unaffected by changes to five-hour percentages. There is no r ratio, 0.15 assumption, coefficient sharing or fallback to the other window. Both reuse the same safe fitting implementation and raw local token evidence, not one another’s learned model. If one window lacks evidence, only its capacity remains unavailable; measured local usage is retained.

Reset/group/known-plan changes, any intervening percentage decrease (including while a cohort is accumulating), saturation and quota rises with no local tokens do not form spending intervals. A minimum three-point increase limits integer-percent quantization noise; the persisted diagnostics retain a one-percentage-point difference uncertainty. Other-device/cloud usage can only be detected indirectly through inconsistency and errors; it cannot be reconstructed or reliably separated from real quota changes.

Validation uses earlier non-overlapping reset windows with a 30-second embargo. Each window is validated only against its own preceding reset windows, using the same fitting regime: initial equal-weight predictions cannot validate a later three-coefficient regression. Median/p90 relative errors are empirical held-out errors, not confidence percentages. At least four held-out errors are needed to display them; eight plus an identifiable model are needed for validated status. Median error over 35% or p90 over 100% withholds capacity. Missing records and weaker validation reduce reliability. Leave-one-window coefficient ranges describe sensitivity, not statistical confidence intervals. Each window’s own most recent 240 usable intervals adapt its fit; raw history is not deleted.

Capacity change diagnostics compare the fixed reference, model/tier/effort mix, within-window learned weight ratios and empirical error. They label estimated changes or recalibration effects. `backendChangeVerified` is always false: these observations cannot prove an unannounced backend policy change.

## Account isolation (1.0.4)

The authoritative `account/rateLimits/read` response optionally supplies `accountId`. Only this explicit ID is SHA-256 pseudonymized for local account scopes. No email, auth-file contents or percent/reset heuristics identify an account. A new read-only App Server is used each poll so the feed does not retain the previous account's credentials across login switches. If identity is unavailable, official percentages remain visible but account capacity is not calculated.

New additive `account_ticks`, `account_points` and `account_calibrations` tables preserve all older data. Points have a scope and continuity epoch. Each observed account change, unidentified read, or gap exceeding 90 seconds interrupts continuity; training never crosses that boundary. All verified intervals for the same account may contribute to that account's own hourly/weekly fit, even when the account returns later. Token subtotals include only local usage bracketed by that account's continuous observations. The initial N/C/O weights are equal; at least three observed percentage points are accumulated before fitting an interval to reduce integer-percent quantization noise. Reset/decreasing percentages invalidate the interval rather than count as spending.

Old snapshots and calibration fits without identity remain stored but cannot be attributed retrospectively to an account. Account quota charts use the currently verified scope. Session/turn totals remain the whole conversation and can span multiple accounts; they are not relabeled as per-account totals. Concurrent account use, a rapid switch-and-return between polls, cloud and other-device use cannot be excluded by local observations. Therefore local attributed usage is a partial observed-interval subtotal, and learned capacity remains approximate, never a certified backend allowance.

The dashboard hides the five-hour card for the explicit personal `pro`, `prolite` and `promax` plan values as a display preference; collection/calibration and official values are unchanged. This does not assert that Pro has no server-side five-hour limit. Context capacity always comes from the selected session’s `model_context_window`, never a fixed subscription-to-capacity mapping.

Weekly-only accounts use the weekly duration (10,080 minutes), wherever the backend places it in primary/secondary. No five-hour point is synthesized and the weekly model does not depend on five-hour observations.

## Storage and graphs

The private `quota-learning.sqlite3` schema migrates additively to version 4. Existing usage, points and cursors remain. Before migrating a populated database, SQLite `VACUUM INTO` creates a consistent `pre-v4-<timestamp>.sqlite3` backup. Older cursors are replayed to recover turn facts. One-to-one supersession excludes verified duplicate legacy rows from fitting without deleting the original records.

New tables retain quota identities, response/turn/root/effort metadata, task facts, usable observation intervals, rejection diagnostics and timestamped calibrations. Only IDs, timing, model metadata and numeric measurements are persisted; user/assistant messages and error-message text are not.

Legacy shared-model fits remain stored and are labeled as previous shared calibration. New fits carry `calibrationMode=independent-v1`; graph lines do not bridge different calibration modes, and a methodology change is not treated as a verified backend quota change. No schema migration or history deletion is needed for these additive JSON fields.

Click Last Request, Context, five-hour or weekly metrics to open a separate, screen-fitted 480×300 window. Task bars default to CLI Used; Raw Total and estimated USD are selectable. Context is measured at completed-task endpoints. Quota charts are global for the current verified account, offering mix-dependent raw projections, fixed reference capacities and observed percentages. Learning/unstable fits do not create a capacity line. Gaps, model/context changes and reset boundaries are not interpolated. Large histories reduce drawing using actual extrema/endpoints and retain missing-data breaks; underlying records remain intact. Keyboard focus and native tooltips expose individual details.

## Verification boundaries

Rust tests cover the compiled official oracle, subsets, mixed record formats, response/task snapshots, missing components, nested agents, learned weights/r, collinearity, resets, outliers, migration, supersession and restart. Playwright covers existing behavior plus four chart types, English/Turkish, dark/light, 480×300 and 320×240, missing data and read failures.

Synthetic UI tests do not prove native window-manager behavior. Native desktop automation and Linux compilation depend on available host tooling; report those checks separately. Real-record diagnostic reports belong only in ignored private artifacts, never in release inputs.


## Local verification on 2026-10-27

- Official-source oracle: 50 cases. Rust suite: 61 passing tests. Playwright: 29 passing tests.
- Read-only real-rollout comparison: nine sessions discovered; six accessible rollouts supplied 36 official session/turn/context checks, all matched in the final capture. Three unavailable rollouts were skipped. Diagnostics include the exact observed byte boundary to prevent later appends from changing the comparison instant.
- A consistent copy of the installed schema-0 database was migrated to schema 4. All 13,989 original usage rows and 32,790 original quota points survived; 2,536 tasks were recovered. A pre-migration SQLite backup was created. The original installed database stayed at schema 0 and was not migrated by verification.
- The local optimized Windows test executable was built. Native desktop automation was unavailable (native pipe not connected), and this WSL installation had no Rust toolchain; actual native window interactions and a Linux build are not claimed as verified. Windows window creation uses an async command as required by the Tauri/WebView2 thread constraint.

Standalone startup follow-up: after correcting a plain-Cargo packaging mistake, the Tauri CLI production build was checked through its actual WebView2 page with the Vite server unavailable. The embedded http://tauri.localhost/ dashboard rendered and the native IPC bridge was present. This establishes native main-window loading; other native window interactions and Linux compilation remain separate checks.


## Release verification — 1.0.2

The subsequent [source CI](https://github.com/qufu-io/qufu-codex-dashboard/actions/runs/40575838994) passed 61 Rust and 29 Playwright tests on each of Windows and Ubuntu 22.04. [Signed package CI](https://github.com/qufu-io/qufu-codex-dashboard/actions/runs/40575838624) successfully compiled and packaged Windows NSIS and Linux DEB/AppImage. This establishes Linux compilation in CI, separately from the local WSL tooling limitation above. Package signatures, signed version and tamper rejection passed; all nine published files were downloaded anonymously with matching SHA-256 digests. Native Linux desktop interaction and clean-host installation remain unverified.


## Task consumption notices (development)

The main task can show a local informational notice, never an official quota alarm or subscription charge. Raw Total includes cached input; CLI Used subtracts cached input but is not a quota-equivalent measure. Cached input can still consume quota. Output already includes reasoning.

For the selected session, compare the current task to up to 20 preceding completed tasks with full usage and known cache. Exclude the current task, incomplete/interrupted tasks and known future completions. At least five baseline tasks are required. A >=3x increase triggers a notice only with >=100K CLI Used or >=1M Raw Total; >=5M Raw Total separately flags high task volume even without a baseline. Partial usage can only trigger the absolute volume notice and is labeled a lower bound. Main-task and subagent accounting remain separate. No conversation text, external telemetry, settings migration or official percentage changes are involved.

Compact quantities always use English K/M/B suffixes (B = 1,000,000,000), including Turkish UI; exact values and uncertainty remain in tooltips. Quota fractions and Learning status have separate space so units remain visible at 300px.

Task-reader regressions cover legacy/new response overlap without cumulative turn snapshots and counter resets. Old persisted derived task history is not silently rewritten: previously saved graph measurements may retain prior parser results, while the main reader rebuilds the current task/baseline from rollouts after restart. Quota token evidence and fitted quota coefficients were not affected by these task-only bugs. A separate read-only historical recomputation is not implemented or claimed.

## Weekly Sol 6.1-equivalent replay

The optional API-priced weekly view uses a frozen bundled short-context API tariff (Standard GPT-6.1 Sol as the type-by-type denominator). Standard Astra is 5× uncached input, 10× cached input, 5× output; Standard Luna is 0.05×, 0.10×, 0.05×. A known higher API speed tier uses its own short rates. This is a modeled conversion, not a known subscription formula. The quota database's three token categories do not retain context length or cache-write breakdown, so this view does not assert actual API cost.

Each observation interval must have a known tariff for every nonzero component; otherwise its whole percent movement is excluded, rather than attributing unknown charges to known models. The ratio or three-coefficient fit is rerun on normalized observations for the same account, plan and quota key. The capacity is anchored to Sol uncached-input-equivalent tokens (not the current token mix). Daily historical boundary snapshots replay only observations available through their cutoff, include provisional values explicitly, and do not write to or replace stored usage, percentages, or original fits. Held-out errors and insufficient evidence remain visible; off-device/cloud/third-party usage can still affect account percentages.

The weekly card API-dollar estimate uses the **GPT-5.4 Auto Review scenario**, described in its tooltip and here. GPT-5.4 Standard short is used only as a proxy for the literal `codex-auto-review` Standard label, giving N/C/O ratios 1.25/2.5/1.5 vs. GPT-6.1 Sol. The local model label and samples are never changed. Other unpriced model/tier combinations are still excluded. This scenario refits one robust median scale through the latest real quota observations, with no independent model or token-type weights beyond the API basis. Chronological rollout tests use only earlier non-overlapping reset cohorts, and fewer than three eligible cohorts cannot receive verified/limited reliability status. OpenAI has not confirmed the underlying reviewer model, pricing or subscription accounting. The underlying strict replay and original quota history remain stored/available to the data layer. The simplified chart menu exposes only Raw Token and API $, plus official percentage for the weekly window.


## API-dollar quota display and simplified charts (1.0.9)

Quota percentages remain official backend measurements. Neither token nor dollar numerators are measured account-wide spending: they map the official percentage onto estimated capacity. No quota weights or historical measurements are changed by switching display units.

For the weekly scenario, take the normalized N/C/O mixture from the same accepted observations available through the fit cutoff (at most 240 intervals). Project the fitted capacity onto that mixture, then multiply by its bundled short-context Standard Sol API cost per normalized token: `USD_capacity = projected_capacity × dot(mix, [input_price, cache_price, output_price]) / sum(mix) / 1e6`; `USD_used = official_percent × USD_capacity / 100`. Applying the input price alone to the summed normalized categories is incorrect. The literal Standard auto-review proxy remains GPT-5.4; no other unknown model or tier is silently substituted.

For the independent five-hour raw projection, the API cost/token comes from the same account-attributed current-window samples used for its raw mixture, priced with each sample's model/tier. Missing prices withhold the dollar projection without hiding valid token capacity. Frozen bundled short-context rates keep the scenario consistent and historical replay reproducible; context length/cache-write breakdown and off-device consumption remain limitations. These are API-equivalent scenarios, not a subscription bill, confirmed OpenAI quota or a claim of actual API spend. Live session USD estimates retain their existing separate pricing engine.

The weekly card defaults to two-decimal API dollars and remembers its Token/API $ selection. Task and quota charts expose Raw Token and API $, with an additional official-percentage view only for weekly quota. Context history remains a direct measured-context view. Legacy CLI/reference/strict replay data is not deleted; only extra menu choices and task-spending warning UI are retired. Old five-hour fits without stored dollar evidence remain gaps rather than invented values.

Auto Follow no longer has a Beta label and is enabled once on settings schema 7 migration; explicit opt-out afterwards is preserved. A target still must be verified and present in the session list. Otherwise manual selection is kept. Main, Settings and Details all retain 300px width and content-fitted height.
