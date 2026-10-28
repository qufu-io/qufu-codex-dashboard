# Qufu Codex Dashboard
<!-- impeccable:product-schema 1 -->

## Platform
web

Windows and Linux native Tauri 2 host; local WebView UI, no browser or web server at runtime.

## Stack
User preference: Tauri 2. Rust data reader and plain TypeScript/CSS UI.

## Users
Codex Desktop users monitoring local sessions on Windows and Linux.

## Product Purpose
Compact, accurate per-session usage with a session picker that mirrors local Codex Desktop threads.

## Capabilities and Constraints
Manual selection by default; optional safe Auto Follow; main and descendant usage separated; no invented missing values; cached input and reasoning are subsets. Local read-only SQLite and rollout access. Account limit snapshots separate from session metrics. Tray, topmost, remembered geometry and opacity. Turkish/English. Refresh official prices on startup with last-good cache. A 300 px wide compact, content-fitted window with expanding session/detail panels.

## Brand Commitments
Qufu name, minimal dark theme. User-provided transparent Qufu logo is included. Published product screenshots use example workspace/session names.

## Evidence on Hand
Read-only local metadata and logs were used during development; only synthetic fixtures and the approved example product screenshot are versioned.

## Product Principles
Correct discovery before decoration. Unknown is not zero. No data mutation or remote transcript upload. Running session is not selected session.

The main display shows total tokens, Input/Output/Cache, last completed model response tokens with its own Input/Output/Cache, last measured context usage, and session USD. Context uses the session-reported capacity without guessing from the subscription. Details show the current account subscription when reported by Codex. Tokens/sec is shown at the upper right.

Current 1.0 layout: session USD alongside total tokens, last-response USD alongside last-response tokens, Input/Output/Cache under each row. The workspace line shows only the path. Subagent totals and records appear only in Details. System icons use the approved black-frame Lucide square-activity adaptation; the in-app Qufu logo is unchanged. Icon license: [Lucide ISC](docs/third-party/LUCIDE-LICENSE).

Approved 1.0 update: Square Activity system icon with black outer frame; in-app Qufu logo unchanged. The extra hide-to-tray button is removed. Displayed percentages use a leading percent sign and USD values use a trailing dollar sign. Context displays measured used/capacity tokens. Quota token fractions are approximate equivalents from observed token usage and quota-percentage changes, not official token quotas or measured account-wide token spending. Their numerator is mapped from the reported account percentage; local reference usage, held-out error and sample counts are in the tooltip. Calibration runs in the background, scans incrementally at 30-second intervals, and keeps measured local usage visible and withholds capacity when independent validation fails. Historical usage from all local threads seeds the price-independent learner. Token observations and quota points persist in a private local SQLite database. Partial subagent and cost totals remain visible with a small partial label. Settings use compact spacing and inline update status; Qufu/author styling is preserved.


Development measurement update: Last Request is a whole user task keyed by turn_id, with separate active/completed state and per-response model pricing. Session Total keeps its raw main-thread scope; CLI Used is separately exposed. Context uses the official 12,000-token baseline and remaining-percent rounding. Only hourly N/C/O weights are fit; weekly capacity reuses them with an assumed or learned r (initially 0.15). Capacity is in fixed uncached-input-equivalent units, never an official raw-token quota. Numeric histories persist additively without conversation text. Functional 480×300 history windows open on the relevant metric, preserve missing-data gaps and follow theme/language; the main compact layout is retained. See [measurement contract](docs/MEASUREMENTS.md).


Same-version 1.0.2 refresh: one automatic content-fitted layout (300px main, 371px expanded panel), no Compact view preference. Startup measures the intended content width and fonts, and resize events reconcile restored native geometry. Other saved preferences persist. Public screenshots use illustrative, non-personal data.


1.0.3 supersedes the shared weekly model: each quota window independently fits its own N/C/O weights and validates against its own resets; no hourly/weekly ratio or coefficient sharing. The main window always shows a compact Version/update row above the centered qufu.io and alper@qufu.io author footer, including when Settings is closed.
