$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if (Test-Path .tools/cargo/bin/cargo.exe) {
    $env:CARGO_HOME = "$PWD\.tools\cargo"
    $env:RUSTUP_HOME = "$PWD\.tools\rustup"
    $env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
}
npm.cmd run tauri -- build --no-bundle
if ($LASTEXITCODE -ne 0) { throw 'Standalone test build failed' }
$destination = 'artifacts/test-build-history/QufuCodexDashboard-test.exe'
New-Item -ItemType Directory -Force (Split-Path $destination) | Out-Null
Copy-Item src-tauri/target/release/qufu-codex-dashboard.exe $destination -Force
Get-FileHash $destination -Algorithm SHA256
