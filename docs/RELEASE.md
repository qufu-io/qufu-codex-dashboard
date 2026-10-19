# Release process

## Visibility and ownership

- Private source: `qufu-io/qufu-codex-dashboard`.
- Public downloads: `qufu-io/qufu-codex-dashboard-releases`.
- Freeware, closed source. Author: alper@qufu.io.
- Only README, product screenshot, freeware license, third-party notices and signed release assets belong in the public repository. Never push application sources there.

## Acceptance gate

The owner accepted the updated 1.0 package and requested the supplied screenshot. Portable Windows download is retained. Publication is authorized after the signed platform build gates pass.

## Build and publication

1. Build and test Windows and Ubuntu 22.04 in the private source workflow. The workflow uploads build artifacts only and never publishes source or binaries to the public repository automatically.
2. The updater private signing key is stored locally under ignored `artifacts/private/signing/updater.key`. Keep a secure backup. Configure `TAURI_SIGNING_PRIVATE_KEY` as a private source repository Actions secret before CI signed builds; do not upload it to the distribution repository or embed it in the app.
3. `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is empty for the current key. Only the public verification key is included in application configuration. Package signatures bind the application version.
4. Collect the Windows installer, Linux deb/AppImage, signature files and checksums. Run `scripts/prepare_release.py` to create `latest.json`; target URLs must point to the public distribution release.
5. Wait for owner acceptance, then publish release `v1.0.0` in the public distribution repository with the bilingual notes, signed packages, `latest.json`, and SHA256SUMS.txt. Do not rewrite an existing source tag or force push. The first source tag predates the updater refinement; future release provenance must record the actual source commit separately.
6. Verify anonymous access, asset checksums, manifest platform URLs, and repository visibility. Never describe a draft as a downloadable public release.

## Updates

Settings checks `https://github.com/qufu-io/qufu-codex-dashboard-releases/releases/latest/download/latest.json`. No login/token is needed. Installation requires an explicit click and a matching package signature and signed version. Only newer versions are offered; republishing 1.0 does not trigger an update for an installed 1.0. Network/missing manifest failures are not reported as up-to-date.

## Boundaries

Local Linux packages use the WSL build distribution baseline. Ubuntu 22.04 CI builds provide wider compatibility. Installing a new version on another machine and testing actual replacement/restart remain separate acceptance checks; fixture-based UI tests do not prove real installation.

Windows distribution includes the recommended installer and an optional portable executable. The NSIS installer uses downloadBootstrapper: it checks HKLM/HKCU WebView2 registration, downloads the official Microsoft bootstrapper when missing, runs /silent /install, and aborts on failure. A clean Windows host without WebView2 has not yet been tested.
