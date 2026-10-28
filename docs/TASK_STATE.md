# Task checkpoint — 2026-10-29

## Current outcome
Qufu Codex Dashboard **1.0.4 is public** in the single source/download repository, https://github.com/qufu-io/qufu-codex-dashboard/releases/tag/v1.0.4 . User authorized implementation, tests, source commit/push and publication. The legacy qufu-codex-dashboard-releases repository was not modified or deleted; owner will remove it after updating. Existing binaries use the old endpoint and require one manual install of 1.0.4. Future updates use the source repository. No work remains pending.

## Source and verification
- Immutable tag v1.0.4: `891d90e3b9ff289c739fe1531bf3ee4a2805fc8f`.
- Source CI `40770229210`: **65 Rust + 34 Playwright tests passed on each of Windows and Ubuntu 22.04**, plus frontend production builds.
- Signed package CI `40770229385`: Windows NSIS/portable and Linux DEB/AppImage builds passed.
- All three updater signatures, signed version 1.0.4 and tamper rejection verified against the unchanged public key.
- All nine public assets downloaded without authentication and matched local SHA-256 digests. Latest manifest serves 1.0.4 and five platform aliases from the source repository.
- Actual CI-built Windows EXE read live quota data in diagnostic mode and produced the expected SHA-256 account pseudonym without raw identity. No login change or running-dashboard shutdown was performed.
- Independent review identified dropped identity events during background scans and missing graph epoch boundaries; both fixed and regression-tested. Follow-up review found no remaining material blockers.
- Private diagnostics remain ignored under artifacts/private; no real account identifiers, auth files, conversations or database files were published.

## Implemented account isolation
- `quota.rs`: official optional root accountId only; SHA-256 pseudonym; no email/auth-file reading. Fresh read-only App Server each 30-second poll avoids retained login state. No ID means no capacity calibration, while official percentages remain visible.
- `limits.rs`: additive account_ticks/account_points/account_calibrations; all legacy rows retained. Points have account scope + epoch. Observed switches, unidentified reads and gaps >90 seconds break training continuity. Every observed identity event is queued as Job::Observe independently of slower Job::Scan. Histories show only current verified account and never connect different epochs.
- Only bracketed active-account intervals contribute local subtotals. Same-account historical intervals can be reused on return. Unidentified legacy data cannot be retrospectively assigned to an account.
- Hourly/weekly models stay independent. Equal initial weights for uncached Input, Cache and Output; accumulate >=3 percentage points for quantization tolerance, then fit nonnegative robust ridge when identifiable. Reset/decreasing percentages invalidate intervals.
- `lib.rs`: live feed takes precedence over session-associated quota snapshots. `main.ts`/i18n: short pseudonymous Quota account code in Details. Existing duration matching supports weekly-only accounts with no invented five-hour quota.
- `updates.rs`, tauri.conf.json, prepare_release.py: trusted URLs, manifest and release asset paths point to the source repository. Signing public key unchanged. README, privacy/methodology/release/changelog documents and illustrative screenshot updated.

## Preserved measurement contract
Input includes Cache; Output includes Reasoning. Raw Total = Input + Output. CLI Used = Input - Cache + Output. Official context uses 12,000-token baseline and remaining-percent rounding. Last Request is a whole user task with cumulative snapshots replaced, not summed, and per-response model/tier costs. Session/turn totals retain conversation scope across accounts; they are not per-account spending. Missing values remain unknown/partial. Account percentages are official; USD is an API-equivalent estimate and quota capacity is an inferred uncached-input-equivalent reference. Existing tasks, history, settings, compact content fitting, separate history windows and persistent Version/author footer remain.

## Limits / not verified
Real two-account login switching, concurrent-account attribution and clean-host installation were not exercised. Concurrent users, rapid switch-and-return between polls, cloud/other-device usage cannot be reliably attributed from local records; estimates remain approximate. UI tests use mocked native IPC, not proof of every native window-manager interaction. Official account identity availability depends on installed CLI/backend support; unknown identity disables training rather than guessing. See MEASUREMENTS.md for equations/source references and RELEASE.md for historical provenance. Do not repeat reference-source research or passing test suites without a new change/failure. No raw external source code/logs belong in this checkpoint.
