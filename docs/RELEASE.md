# 1.0 release process

Status: owner approved local 1.0 testing and authorized private publication to `qufu-io/qufu-codex-dashboard`. Publish only after both GitHub build jobs succeed.

## Prepared packages

- Windows x64 NSIS installer and portable exe.
- Linux amd64 Debian package and AppImage.
- Windows and Linux GitHub Actions build matrix; release creation defaults to draft.

## Approval gate

1. The owner tests the local Windows build and confirms the requested UI and settings.
2. Confirm the destination GitHub account and private repository name.
3. Review source files and exclusions. Never upload local Codex data, authentication, logs, build caches or diagnostic reports.
4. Create the private repository and push the reviewed source only after explicit approval.
5. Tag `v1.0.0`, wait for both Windows and Ubuntu 22.04 builds and tests, inspect artifacts, then publish the private release.

## Downloads and sharing

Invited repository collaborators can sign in, open Releases and download setup/Linux packages. A private release link is not an anonymous download link. The repository owner manages access through Settings → Collaborators. Personal-repository collaborators have write access; use an organization repository if separate read-only roles are needed.

Source is MIT-licensed with alper@qufu.io attribution. Private visibility and the source license are separate choices.

## Verification boundaries

The local Linux package uses the local Ubuntu build baseline. The GitHub Ubuntu 22.04 job builds release artifacts for wider compatibility. Do not label a local build as a successful GitHub CI run. Installing the Windows package and testing it on another machine remain part of the owner's acceptance test.
