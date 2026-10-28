# Release process

## Visibility and ownership

- Public MIT source: `qufu-io/qufu-codex-dashboard`.
- Public downloads: `qufu-io/qufu-codex-dashboard-releases`.
- MIT-licensed open source since 2026-10-24. Author: alper@qufu.io.
- Keep application sources in the source repository; the separate downloads repository contains documentation, license/notices and signed packages. Never publish local Codex data, signing keys, credentials, build caches or private diagnostics. See [licensing transition](LICENSING.md) for legacy 1.0 packages.

## Acceptance gate

The owner explicitly authorized publication of the measurement, quota calibration and history update on 2026-10-27. Publish it as 1.0.2 so existing 1.0.1 installations receive an update. Portable Windows download is retained. Publication is authorized after the signed platform build gates pass.

## Build and publication

1. Build and test Windows and Ubuntu 22.04 in the source release workflow. The workflow uploads build artifacts only and never publishes source or binaries to the public repository automatically.
2. The updater private signing key is stored locally under ignored `artifacts/private/signing/updater.key`. Keep a secure backup. Configure `TAURI_SIGNING_PRIVATE_KEY` as a source repository Actions secret before CI signed builds; do not upload it to the distribution repository or embed it in the app.
3. `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is empty for the current key. Only the public verification key is included in application configuration. Package signatures bind the application version.
4. Collect the Windows installer, Linux deb/AppImage, signature files and checksums. Run `scripts/prepare_release.py` to create `latest.json`; target URLs must point to the public distribution release.
5. Wait for owner acceptance, then publish the accepted release tag (currently `v1.0.2`) in the public distribution repository with the bilingual notes, signed packages, `latest.json`, and SHA256SUMS.txt. Do not rewrite an existing source tag or force push. The first source tag predates the updater refinement; future release provenance must record the actual source commit separately.
6. Verify anonymous access, asset checksums, manifest platform URLs, and repository visibility. Never describe a draft as a downloadable public release.

## Updates

Settings checks `https://github.com/qufu-io/qufu-codex-dashboard-releases/releases/latest/download/latest.json`. No login/token is needed. Installation requires an explicit click and a matching package signature and signed version. Only newer versions are offered; republishing 1.0 does not trigger an update for an installed 1.0. Network/missing manifest failures are not reported as up-to-date.

## Boundaries

Local Linux packages use the WSL build distribution baseline. Ubuntu 22.04 CI builds provide wider compatibility. Installing a new version on another machine and testing actual replacement/restart remain separate acceptance checks; fixture-based UI tests do not prove real installation.

Windows distribution includes the recommended installer and an optional portable executable. The NSIS installer uses downloadBootstrapper: it checks HKLM/HKCU WebView2 registration, downloads the official Microsoft bootstrapper when missing, runs /silent /install, and aborts on failure. A clean Windows host without WebView2 has not yet been tested.

Published 2026-10-19: public release v1.0.0, source CI run 39227168273, application source e1b1383fa984f0f68dc0ae61703281df7bbf5889. All platform builds/tests passed. Uploaded digests and anonymous installer/manifest access were verified. The public tag covers only the distribution documentation, not the private application source.

## Public contributions

`ci.yml` runs Windows/Linux source checks on main pushes and pull requests, with read-only permissions and no signing secrets. `release.yml` is maintainer-triggered or tag-triggered and builds signed artifacts only; it does not publish releases automatically. Signing secrets must never be exposed to fork PRs. Repository visibility and source licensing changes do not authorize publishing a new binary version.

For a local standalone test executable without an installer, run `powershell -ExecutionPolicy Bypass -File scripts/build-test.ps1`. Use the Tauri CLI, not plain `cargo build --release`: Tauri production builds require its custom-protocol feature to embed the frontend instead of requesting devUrl. Backend diagnostic commands alone do not verify that the native webview can load the packaged UI.


## Published 1.0.2 — 2026-10-27

Source tag `v1.0.2` identifies `357b5290abd253ff936fb1f725712701d765599d`. [Source checks](https://github.com/qufu-io/qufu-codex-dashboard/actions/runs/40575838994) passed on Windows/Ubuntu 22.04: 61 Rust and 29 Playwright tests per platform. [Signed builds](https://github.com/qufu-io/qufu-codex-dashboard/actions/runs/40575838624) passed for Windows NSIS and Linux DEB/AppImage. All three updater signatures, signed versions and tamper rejection were verified against the application public key. All nine [public release assets](https://github.com/qufu-io/qufu-codex-dashboard-releases/releases/tag/v1.0.2) were downloaded anonymously and matched local SHA-256 digests. The latest updater manifest serves 1.0.2 with five platform aliases. Distribution documentation/tag points to `27e3047` rather than the source commit; release notes record both repositories' roles. Native Windows standalone loading was checked before the version bump; a second CI-binary launch was prevented by an existing installed instance, which was preserved. Clean-host installation and every native chart interaction are not claimed as verified.


## Authorized same-version refresh — 2026-10-27

The owner explicitly requested removal of Compact view, corrected startup sizing, a non-personal README screenshot and republication without changing 1.0.2. Keep published tags/history intact. Build the new main commit through workflow_dispatch and record its exact SHA/run IDs in both releases. Attach its source archive to the source release, since the original v1.0.2 source tag remains the first build. Back up existing distribution assets before replacement, verify fresh signatures, replace packages/signatures/checksums and upload latest.json last. If replacement fails, restore the saved original asset set. Same-version clients intentionally require a manual reinstall; do not bypass the updater's version checks.


Refresh published successfully from source `1f663fc75e9436aa746356d3b7a9d68674447a0c`: [source CI 40580240768](https://github.com/qufu-io/qufu-codex-dashboard/actions/runs/40580240768) passed 61 Rust and 31 Playwright tests per platform; [signed builds 40580240762](https://github.com/qufu-io/qufu-codex-dashboard/actions/runs/40580240762) passed. Actual Windows WebView2 startup, Settings expansion/collapse, correction of an intentionally wrong native size and absence of Compact view were verified. After the user voluntarily closed the installed app, only the newly started helper was stopped/restarted. All three package signatures, signed versions and tamper rejection passed. All nine replacement assets, the refreshed source archive and the new illustrative README screenshot were downloaded anonymously with matching hashes. The old asset set remains backed up privately. Original tags remain unchanged; releases identify the refreshed source SHA/archive. Application and manifest version remain 1.0.2, so existing 1.0.2 users reinstall manually.


## Authorized 1.0.3 — 2026-10-28

The owner authorized independent weekly calibration, an always-visible Version/update/author footer, tests and publication as 1.0.3. Use a new immutable source tag, pass both platform CI jobs, verify signatures and publish new signed packages/manifest. Normal in-app updates from 1.0.2 are supported by the increased version.


## Published 1.0.3 — 2026-10-28

Source/tag: `8fe154eaee941ac64b6c6258154d3aa7bd645c13` / `v1.0.3`. [Source CI 40645911296](https://github.com/qufu-io/qufu-codex-dashboard/actions/runs/40645911296) passed 61 Rust and 32 Playwright tests on each of Windows and Ubuntu 22.04. [Signed package CI 40645911526](https://github.com/qufu-io/qufu-codex-dashboard/actions/runs/40645911526) passed. All three updater signatures, signed version 1.0.3 and tamper rejection were verified. All nine [public assets](https://github.com/qufu-io/qufu-codex-dashboard-releases/releases/tag/v1.0.3) were downloaded without authentication and matched local SHA-256 digests. The latest updater manifest serves 1.0.3 with five platform aliases; distribution documentation/tag identifies `a18bc99`. A scoped independent review found no release blockers. Native 1.0.3 desktop interactions and clean-host installation were not separately exercised; do not equate mocked-IPC UI tests with those checks.


## Authorized 1.0.4 consolidation — 2026-10-29

The owner authorized per-account quota isolation and consolidation into `qufu-io/qufu-codex-dashboard`, including a new 1.0.4 release. Publish all signed installers, signatures, checksums and latest.json in the source repository release. Runtime trusted URLs and updater endpoint now point only there. Preserve old tags/releases. Do not delete or publish into `qufu-io/qufu-codex-dashboard-releases`; the owner will remove it after updating. Older binaries retain that old updater endpoint and must manually install 1.0.4. The public key is unchanged. Continue to require Windows/Ubuntu source CI and signed build gates, signature/version/tamper verification and anonymous asset/manifest verification.


## Published 1.0.4 in the consolidated source repository — 2026-10-29

Source/tag: `891d90e3b9ff289c739fe1531bf3ee4a2805fc8f` / `v1.0.4`. [Source CI 40770229210](https://github.com/qufu-io/qufu-codex-dashboard/actions/runs/40770229210) passed 65 Rust and 34 Playwright tests on each of Windows and Ubuntu 22.04. [Signed CI 40770229385](https://github.com/qufu-io/qufu-codex-dashboard/actions/runs/40770229385) passed for Windows NSIS/portable and Linux DEB/AppImage. All three package signatures, signed version and tamper rejection passed. All nine [public assets](https://github.com/qufu-io/qufu-codex-dashboard/releases/tag/v1.0.4) were downloaded anonymously and matched local SHA-256 digests. The latest manifest serves 1.0.4 and five platform aliases from this repository. The CI-built EXE read a live account-scoped quota snapshot in diagnostic mode without exposing raw identity. Real A/B login switching and clean-host installation remain unverified; synthetic regressions cover isolation and continuity. The old distribution repository was not modified or deleted. Existing old-endpoint installations require a manual 1.0.4 install before owner-managed removal of that repository.
