import argparse
import datetime
import hashlib
import json
from pathlib import Path
from urllib.parse import quote

parser = argparse.ArgumentParser(description='Prepare reviewed binary-only release assets.')
parser.add_argument('folder', type=Path)
parser.add_argument('--version', required=True)
args = parser.parse_args()
root = args.folder.resolve()
files = sorted(p for p in root.iterdir() if p.is_file() and p.suffix in ['.exe', '.deb', '.AppImage', '.sig'])
for p in files:
    if ' ' in p.name:
        normalized = p.with_name(p.name.replace(' ', '.'))
        if normalized.exists():
            raise SystemExit(f'Asset name collision: {normalized.name}')
        p.rename(normalized)
files = sorted(p for p in root.iterdir() if p.is_file() and p.suffix in ['.exe', '.deb', '.AppImage', '.sig'])
expected = {f'Qufu.Codex.Dashboard_{args.version}{suffix}' for suffix in ['_x64-setup.exe', '_amd64.deb', '_amd64.AppImage']}
allowed = expected | {name + '.sig' for name in expected} | {f'QufuCodexDashboard-{args.version}.exe'}
extra = [p.name for p in files if p.name not in allowed]
if extra:
    raise SystemExit(f'Unexpected distribution assets: {extra}')
platforms = {}
for suffix, targets in [('_x64-setup.exe', ['windows-x86_64', 'windows-x86_64-nsis']), ('_amd64.AppImage', ['linux-x86_64', 'linux-x86_64-appimage']), ('_amd64.deb', ['linux-x86_64-deb'])]:
    packages = [p for p in files if p.name.endswith(suffix)]
    if len(packages) != 1:
        raise SystemExit(f'Expected one {suffix} package')
    package = packages[0]
    signature_file = package.with_name(package.name + '.sig')
    if signature_file not in files:
        raise SystemExit(f'Missing signature for {package.name}')
    signature = signature_file.read_text(encoding='utf-8-sig').strip()
    if not signature:
        raise SystemExit(f'Empty signature for {package.name}')
    info = {'url': f'https://github.com/qufu-io/qufu-codex-dashboard/releases/download/v{args.version}/{quote(package.name)}', 'signature': signature}
    for target in targets:
        platforms[target] = info
manifest = {'version': args.version, 'notes': f'Qufu Codex Dashboard {args.version} · alper@qufu.io', 'pub_date': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'platforms': platforms}
(root / 'latest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
checksummed = files + [root / 'latest.json']
(root / 'SHA256SUMS.txt').write_text(''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in checksummed), encoding='utf-8')
print(f'Prepared {len(platforms)} platform targets and {len(checksummed)} checksums. No upload performed.')
