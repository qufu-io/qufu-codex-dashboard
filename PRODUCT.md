# Qufu Codex Dashboard
<!-- impeccable:product-schema 1 -->

## Platform
web

Windows and Linux native Tauri 2 host; local WebView UI, no browser or web server at runtime.

## Stack
User preference: Tauri 2. Rust data reader and plain TypeScript/CSS UI.

## Users
Codex Desktop user monitoring sessions while working on Windows.

## Product Purpose
Compact, accurate per-session usage with a session picker that mirrors local Codex Desktop threads.

## Capabilities and Constraints
Manual selection by default; optional safe Auto Follow; main and descendant usage separated; no invented missing values; cached input and reasoning are subsets. Local read-only SQLite and rollout access. Account limit snapshots separate from session metrics. Tray, topmost, remembered geometry and opacity. Turkish/English. Refresh official prices on startup with last-good cache. User approved 300 px wide compact, content-fitted height with expanding session/detail panels.

## Brand Commitments
Qufu name, minimal dark theme. User-provided transparent Qufu logo is included. The sidebar reference screenshot has not been supplied.

## Evidence on Hand
Local Codex metadata and logs, upstream source references. No screenshot/logo fixture available.

## Product Principles
Correct discovery before decoration. Unknown is not zero. No data mutation or remote transcript upload. Running session is not selected session.

The main display shows total tokens, Input/Output/Cache, last completed model response tokens with its own Input/Output/Cache, last measured context usage, and session USD. Context uses the session-reported capacity without guessing from the subscription. Details show the current account subscription when reported by Codex. Tokens/sec is shown at the upper right.

Current 1.0 layout: session USD alongside total tokens, last-response USD alongside last-response tokens, Input/Output/Cache under each row. The workspace line shows only the path. Subagent totals and records appear only in Details. System icons use the approved black-frame Lucide square-activity adaptation; the in-app Qufu logo is unchanged. Icon license: [Lucide ISC](docs/third-party/LUCIDE-LICENSE).

Approved 1.0 update: Square Activity system icon with black outer frame; in-app Qufu logo unchanged. The extra hide-to-tray button is removed. Displayed percentages use a leading percent sign and USD values use a trailing dollar sign. Context displays measured used/capacity tokens. Quota token fractions are approximate equivalents from local price-weighted calibration, not official token quotas or measured account-wide token spending. Their numerator is mapped from the reported account percentage; local reference usage, held-out error and sample counts are in the tooltip. Calibration runs in the background, scans incrementally at 30-second intervals, and stays unavailable when pricing coverage or validation is insufficient. Internal guardian/auto-review records remain in session details but are excluded from the public-model reference mix for quota estimates.
