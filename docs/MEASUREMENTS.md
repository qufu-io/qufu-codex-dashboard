# Measurements and history (development build)

These changes are not part of the published 1.0.1 installer. Application version metadata and releases are unchanged.

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

The five-hour and weekly windows have separate training intervals, coefficients, reset cohorts, validation errors and capacity estimates. Weekly calibration works without any five-hour observations and is unaffected by changes to five-hour percentages. There is no r ratio, 0.15 assumption, coefficient sharing or fallback to the other window. Both reuse the same safe fitting implementation and raw local token evidence, not one another’s learned model. If one window lacks evidence, only its capacity remains unavailable; measured local usage is retained.

Reset/group/known-plan changes, any intervening percentage decrease (including while a cohort is accumulating), saturation and quota rises with no local tokens do not form spending intervals. A minimum three-point increase limits integer-percent quantization noise; the persisted diagnostics retain a one-percentage-point difference uncertainty. Other-device/cloud usage can only be detected indirectly through inconsistency and errors; it cannot be reconstructed or reliably separated from real quota changes.

Validation uses earlier non-overlapping reset windows with a 30-second embargo. Each window is validated only against its own preceding reset windows. Median/p90 relative errors are empirical held-out errors, not confidence percentages. At least four held-out errors are needed to display them; eight plus an identifiable model are needed for validated status. Median error over 35% or p90 over 100% withholds capacity. Missing records and weaker validation reduce reliability. Leave-one-window coefficient ranges describe sensitivity, not statistical confidence intervals. Each window’s own most recent 240 usable intervals adapt its fit; raw history is not deleted.

Capacity change diagnostics compare the fixed reference, model/tier/effort mix, within-window learned weight ratios and empirical error. They label estimated changes or recalibration effects. `backendChangeVerified` is always false: these observations cannot prove an unannounced backend policy change.

## Storage and graphs

The private `quota-learning.sqlite3` schema migrates additively to version 4. Existing usage, points and cursors remain. Before migrating a populated database, SQLite `VACUUM INTO` creates a consistent `pre-v4-<timestamp>.sqlite3` backup. Older cursors are replayed to recover turn facts. One-to-one supersession excludes verified duplicate legacy rows from fitting without deleting the original records.

New tables retain quota identities, response/turn/root/effort metadata, task facts, usable observation intervals, rejection diagnostics and timestamped calibrations. Only IDs, timing, model metadata and numeric measurements are persisted; user/assistant messages and error-message text are not.

Legacy shared-model fits remain stored and are labeled as previous shared calibration. New fits carry `calibrationMode=independent-v1`; graph lines do not bridge different calibration modes, and a methodology change is not treated as a verified backend quota change. No schema migration or history deletion is needed for these additive JSON fields.

Click Last Request, Context, five-hour or weekly metrics to open a separate, screen-fitted 480×300 window. Task bars default to CLI Used; Raw Total and estimated USD are selectable. Context is measured at completed-task endpoints. Quota charts are global, offer observed percentages, and use the fixed reference capacity. Learning/unstable fits do not create a capacity line. Gaps, model/context changes and reset boundaries are not interpolated. Large histories reduce drawing using actual extrema/endpoints and retain missing-data breaks; underlying records remain intact. Keyboard focus and native tooltips expose individual details.

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
