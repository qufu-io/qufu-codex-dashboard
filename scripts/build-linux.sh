#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")/.." && pwd)"
build_dir="${QUFU_BUILD_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/qufu-codex-build}"
mkdir -p "$build_dir/tools" "$build_dir/source"
export CARGO_HOME="$build_dir/tools/cargo"
export RUSTUP_HOME="$build_dir/tools/rustup"
export PATH="$CARGO_HOME/bin:$build_dir/tools/node/bin:$PATH"
export CARGO_BUILD_JOBS=2
if ! command -v cargo >/dev/null; then
  curl --fail --silent --show-error https://sh.rustup.rs -o "$build_dir/tools/rustup-init.sh"
  sh "$build_dir/tools/rustup-init.sh" -y --no-modify-path --profile minimal
fi
if ! command -v node >/dev/null; then
  curl --fail --silent --show-error https://nodejs.org/dist/latest-v22.x/SHASUMS256.txt -o "$build_dir/tools/SHASUMS256.txt"
  node_archive="$(awk '/linux-x64.tar.xz$/{print $2;exit}' "$build_dir/tools/SHASUMS256.txt")"
  curl --fail --silent --show-error "https://nodejs.org/dist/latest-v22.x/$node_archive" -o "$build_dir/tools/$node_archive"
  (cd "$build_dir/tools"; grep " $node_archive$" SHASUMS256.txt | sha256sum --check)
  mkdir -p "$build_dir/tools/node"
  tar -xJf "$build_dir/tools/$node_archive" --strip-components=1 -C "$build_dir/tools/node"
fi
tar -C "$project_dir" --exclude=.tools --exclude=.research --exclude=node_modules --exclude=artifacts --exclude=src-tauri/target --exclude=.git --exclude=test-results --exclude=playwright-report -cf - . | tar -C "$build_dir/source" -xf -
cd "$build_dir/source"
npm ci
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
npm run tauri -- build --bundles deb,appimage
mkdir -p "$project_dir/artifacts/1.0/linux"
find src-tauri/target/release/bundle -type f \( -name '*.deb' -o -name '*.AppImage' \) -exec cp {} "$project_dir/artifacts/1.0/linux/" \;
cp src-tauri/target/release/qufu-codex-dashboard "$project_dir/artifacts/1.0/linux/qufu-codex-dashboard"
