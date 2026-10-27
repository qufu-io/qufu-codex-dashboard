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
