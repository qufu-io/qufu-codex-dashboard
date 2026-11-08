# Quota projection review — PR #1

## Decision and scope
Reviewed the raw-token display proposal independently against the existing estimator. The conversion is dimensionally correct and does not change the three-category learning objective. Corrections below are required for safe interpretation. No user quota database migration, deletion, backfill write, version bump or binary release is included.

## Mathematics
For public weights a, b, c in percentage points/token, predicted delta is aN+bC+cO. For a measured local mix, projected raw capacity is 100(N+C+O)/(aN+bC+cO). Internally coefficients are scaled to percentage points per 100,000 tokens, which explains the additional 100,000 factor in the helper. N+C equals input; output already includes reasoning. There is no cache or reasoning double count. Scaling all mix amounts equally leaves capacity unchanged; changing their proportions can change capacity even with unchanged coefficients.

The fixed reference remains 100/a uncached-input-equivalent tokens. Initial equal weights do not identify three independent weights. Ridge/Huber fitting requires diverse intervals and reset windows; coefficients and reference capacity remain statistical estimates. API prices are not inputs. Five-hour and weekly fits remain independent.

## Findings corrected
- **P1:** Raw-mix changes could masquerade as quota changes. Change analysis again compares the fixed reference; raw projection/mix changes are separate diagnostics. Backend changes are never asserted.
- **P2:** Validation could credit the independent regression with errors from earlier equal-weight models. Only chronological validation folds using the same fitting regime now count.
- **P2:** Current mix brackets could include another plan/reset within the same account epoch. Both endpoints must now match the current plan/reset.
- **P2:** Learning projections lacked a visible status. Main cards now show Learning; unstable/insufficient capacities are withheld. Official percentages remain untouched.
- **P2:** The PR changed task-history defaults to Raw Total. CLI Used is restored for task history; quota history defaults to raw projection.
- Historical equal-weight raw overlays require verified saved equal weights and capacity consistency. Missing independent-fit mix remains absent; incompatible bases are not joined. Historical reliability labels describe the saved fit and are not retrospective validation by the corrected method.

## Account scope and historical comparisons
All local catalog sessions, including concurrent main/subagent sessions, contribute deduplicated response evidence. Account ID scopes and observed continuity epochs isolate known switches. Different CODEX_HOME directories, other devices and cloud usage are not reconstructed. Concurrent different-account activity or unobserved switch-and-return cannot be attributed definitively without per-response account identity.

A 367M to 284M raw projection change (about 22.6%) alone cannot identify backend quota changes. Compare the same account, plan, limit group, fitting regime, N/C/O mix and chronological error evidence first. A provisional initial-ratio estimate cannot be treated as a validated baseline against an independent regression. Safe future retrospective analysis can recompute a separately labeled, read-only series from each historical cutoff's account-scoped points and token evidence; it must not apply future weights/mix to old dates or overwrite original fits. No such bulk backfill is claimed here.

## Pro and context
Personal Pro/Pro Lite/Pro Max hide the five-hour card only. Weekly percentages and both backend fits are preserved. Context capacity uses the selected session's Codex-provided capacity; subscription alone does not establish a universal context size. Missing usage remains missing even when capacity is known.

## Verification
Local Windows: Rust unit tests **70 PASS**; TypeScript/Vite production build **PASS**; Playwright **45 PASS** (English/Turkish, dark/light, 300 px main layout, small history windows, missing data, confidence states, Pro variants, mix/history boundaries). Rust formatting passed. Linux exact-revision CI remains a separate merge gate; results are recorded on the PR. Playwright uses mocked native IPC; native desktop/clean installation and real simultaneous multi-account attribution were not exercised. No signed package or new release was requested.
