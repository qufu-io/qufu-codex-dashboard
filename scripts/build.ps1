$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if (Test-Path .tools/cargo/bin/cargo.exe) {
    $env:CARGO_HOME = "$PWD\.tools\cargo"
    $env:RUSTUP_HOME = "$PWD\.tools\rustup"
    $env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
}
npm.cmd run tauri -- build --bundles nsis
if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
New-Item -ItemType Directory -Force artifacts/1.0/windows | Out-Null
Copy-Item src-tauri/target/release/qufu-codex-dashboard.exe artifacts/1.0/windows/QufuCodexDashboard-1.0.exe
Get-ChildItem src-tauri/target/release/bundle/nsis | Where-Object { $_.Name -match '\.(exe|sig)$' } | Copy-Item -Destination artifacts/1.0/windows -Force
Get-ChildItem artifacts/1.0/windows -File | Get-FileHash -Algorithm SHA256
