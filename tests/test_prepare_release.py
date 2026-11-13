import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts' / 'prepare_release.py'
VERSION = '9.9.9'

class WindowsReleaseTests(unittest.TestCase):
    def run_script(self, folder):
        return subprocess.run([sys.executable, str(SCRIPT), str(folder), '--version', VERSION], capture_output=True, text=True)

    def fixture(self, folder, signature=True):
        name = f'Qufu Codex Dashboard_{VERSION}_x64-setup.exe'
        (folder / name).write_bytes(b'synthetic installer')
        if signature:
            (folder / (name + '.sig')).write_text('fixture-signature', encoding='utf-8')
        (folder / f'QufuCodexDashboard-{VERSION}.exe').write_bytes(b'synthetic portable')

    def test_windows_only_manifest_names_and_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root)
            result = self.run_script(root)
            self.assertEqual(result.returncode, 0, result.stderr)
            manifest = json.loads((root / 'latest.json').read_text())
            self.assertEqual(set(manifest['platforms']), {'windows-x86_64', 'windows-x86_64-nsis'})
            for platform in manifest['platforms'].values():
                self.assertEqual(platform['signature'], 'fixture-signature')
                self.assertTrue(platform['url'].endswith(f'/v{VERSION}/Qufu.Codex.Dashboard_{VERSION}_x64-setup.exe'))
            hashes = (root / 'SHA256SUMS.txt').read_text().splitlines()
            self.assertEqual(len(hashes), 4)
            self.assertEqual(len(list(root.iterdir())), 5)
            for row in hashes:
                digest, name = row.split('  ', 1)
                self.assertEqual(digest, hashlib.sha256((root / name).read_bytes()).hexdigest())

    def test_missing_installer_signature_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root, signature=False)
            result = self.run_script(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Missing signature', result.stderr)
            self.assertFalse((root / 'latest.json').exists())

    def test_linux_assets_are_rejected_in_future_releases(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root)
            (root / f'Qufu.Codex.Dashboard_{VERSION}_amd64.deb').write_bytes(b'synthetic Linux package')
            result = self.run_script(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Unexpected distribution assets', result.stderr)
            self.assertFalse((root / 'latest.json').exists())

if __name__ == '__main__':
    unittest.main()
