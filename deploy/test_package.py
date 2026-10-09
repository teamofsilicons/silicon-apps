"""Packager tests with synthetic inputs: no network, no build tools and no real release archives."""
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import stat
import tarfile
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


package = load('apps_deploy_package', 'package.py')
installer = load('apps_deploy_install_for_package', 'install.py')
REVISION = 'c' * 40


def write(path, data=b'', mode=0o644):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    path.chmod(mode)


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()

    def store(self, build_id='build-1', standalone_id=None, name='store'):
        """A Next standalone build laid out the way pnpm leaves it: packages in .pnpm, relative links to them."""
        store = self.root / name
        standalone = store / '.next/standalone'
        write(store / '.next/BUILD_ID', build_id.encode())
        write(store / '.next/static/chunks/app-1.js', b'chunk')
        write(store / 'public/favicon.ico', b'icon')
        write(standalone / 'server.js', b'require("next")')
        write(standalone / '.next/BUILD_ID', (standalone_id or build_id).encode())
        write(standalone / 'node_modules/.pnpm/next@16/node_modules/next/package.json', b'{}')
        write(standalone / 'node_modules/.pnpm/react@19/node_modules/react/index.js', b'react')
        (standalone / 'node_modules/next').symlink_to('.pnpm/next@16/node_modules/next')
        (standalone / 'node_modules/.pnpm/next@16/node_modules/react').symlink_to('../../react@19/node_modules/react')
        return store

    def test_store_is_staged_with_links_static_files_and_public_assets(self):
        destination = self.root / 'stage/store'
        self.assertEqual(package.stage_store(self.store(), destination), 'build-1')
        self.assertTrue((destination / 'node_modules/next').is_symlink())
        self.assertEqual(os.readlink(destination / 'node_modules/next'), '.pnpm/next@16/node_modules/next')
        self.assertEqual((destination / 'node_modules/.pnpm/next@16/node_modules/react/index.js').read_bytes(), b'react')
        self.assertEqual((destination / '.next/static/chunks/app-1.js').read_bytes(), b'chunk')
        self.assertEqual((destination / 'public/favicon.ico').read_bytes(), b'icon')

    def test_store_from_another_build_or_with_environment_files_is_refused(self):
        with self.assertRaisesRegex(SystemExit, 'different build'):
            package.stage_store(self.store(standalone_id='build-2'), self.root / 'stage-a/store')
        store = self.store(name='store-with-env')
        write(store / '.next/standalone/.env.production', b'SECRET=1')
        with self.assertRaisesRegex(SystemExit, 'environment files'):
            package.stage_store(store, self.root / 'stage-b/store')
        with self.assertRaisesRegex(SystemExit, 'Build the store first'):
            package.stage_store(self.root / 'missing', self.root / 'stage-c/store')

    def test_broken_absolute_or_escaping_links_are_refused(self):
        for name, target, message in [('gone', '.pnpm/gone@1/node_modules/gone', 'Broken symlink'),
                                      ('absolute', '/usr/lib', 'Absolute symlink'),
                                      ('outside', '../../outside', 'leaves the bundle')]:
            with self.subTest(name=name):
                (self.root / 'outside').mkdir(exist_ok=True)
                tree = self.root / ('tree-' + name)
                write(tree / 'node_modules/.pnpm/ok/index.js', b'ok')
                (tree / 'node_modules' / name).symlink_to(target)
                with self.assertRaisesRegex(SystemExit, message):
                    package.check_links(tree)

    def test_inventory_hashes_files_records_links_and_normalizes_modes(self):
        stage = self.root / 'stage'
        write(stage / 'bin/apps-server', b'api', 0o700)
        write(stage / 'web/install.sh', b'installer', 0o600)
        (stage / 'web/install-link').symlink_to('install.sh')
        files, links = package.inventory(stage)
        self.assertEqual(files, {'bin/apps-server': hashlib.sha256(b'api').hexdigest(),
                                 'web/install.sh': hashlib.sha256(b'installer').hexdigest()})
        self.assertEqual(links, {'web/install-link': 'install.sh'})
        self.assertEqual(stat.S_IMODE((stage / 'bin/apps-server').stat().st_mode), 0o755)
        self.assertEqual(stat.S_IMODE((stage / 'web/install.sh').stat().st_mode), 0o644)
        os.link(stage / 'web/install.sh', stage / 'web/hard')
        with self.assertRaisesRegex(SystemExit, 'hard links'):
            package.inventory(stage)

    def test_packaged_store_passes_the_installer_verification(self):
        stage = self.root / 'stage'
        write(stage / 'bin/apps-server', b'api', 0o755)
        write(stage / 'node/bin/node', b'node', 0o755)
        write(stage / 'web/install.sh', b'installer')
        package.stage_store(self.store(), stage / 'store')
        archive = self.root / (REVISION + '.tar.gz')
        manifest = package.write_archive(stage, archive, {'revision': REVISION})
        self.assertIn('store/node_modules/next', manifest['symlinks'])
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        release = installer.prepare_release(archive, digest, REVISION, self.root / 'install')
        self.assertEqual((release / 'store/node_modules/next/package.json').read_bytes(), b'{}')
        self.assertEqual(stat.S_IMODE((release / 'node/bin/node').stat().st_mode), 0o755)
        self.assertEqual(installer.prepare_release(archive, digest, REVISION, self.root / 'install'), release)

    def node_tarball(self, path):
        with tarfile.open(path, 'w:xz') as tar:
            for name, data, mode in [('bin/node', b'ELF node', 0o755), ('LICENSE', b'MIT', 0o644),
                                     ('lib/node_modules/npm/index.js', b'npm', 0o644)]:
                member = tarfile.TarInfo(package.NODE_DIR + '/' + name)
                member.size, member.mode = len(data), mode
                tar.addfile(member, io.BytesIO(data))
            link = tarfile.TarInfo(package.NODE_DIR + '/bin/npm')
            link.type, link.linkname = tarfile.SYMTYPE, '../lib/node_modules/npm/index.js'
            tar.addfile(link)
        return hashlib.sha256(path.read_bytes()).hexdigest()

    def test_node_archive_is_verified_reused_or_downloaded(self):
        cache = self.root / 'cache'
        cache.mkdir()
        fake = self.root / 'fake.tar.xz'
        digest = self.node_tarball(fake)
        with patch.object(package, 'NODE_SHA256', digest), patch.object(package, 'ROOT', self.root / 'repo'):
            self.assertEqual(package.node_archive(fake, cache), fake)
            with self.assertRaisesRegex(SystemExit, 'not found'):
                package.node_archive(self.root / 'absent.tar.xz', cache)
            accounts = self.root / 'silicon-accounts/.dev/production' / package.NODE_ARCHIVE
            write(accounts, fake.read_bytes())
            self.assertEqual(package.node_archive(None, cache), accounts)
            accounts.unlink()
            with patch.object(package.urllib.request, 'urlopen', return_value=io.BytesIO(fake.read_bytes())) as urlopen:
                self.assertEqual(package.node_archive(None, cache), cache / package.NODE_ARCHIVE)
            self.assertEqual(urlopen.call_args[0][0], package.NODE_URL)
            self.assertEqual(package.node_archive(None, cache), cache / package.NODE_ARCHIVE)
            (cache / package.NODE_ARCHIVE).unlink()
            with patch.object(package.urllib.request, 'urlopen', return_value=io.BytesIO(b'tampered')):
                with self.assertRaisesRegex(SystemExit, 'checksum mismatch'):
                    package.node_archive(None, cache)
            self.assertEqual(list(cache.iterdir()), [])
        with self.assertRaisesRegex(SystemExit, 'checksum mismatch'):
            package.node_archive(fake, cache)

    def test_only_the_node_runtime_and_licence_are_bundled(self):
        fake = self.root / 'fake.tar.xz'
        self.node_tarball(fake)
        package.extract_node(fake, self.root / 'node')
        found = sorted(p.relative_to(self.root / 'node').as_posix() for p in (self.root / 'node').rglob('*') if p.is_file())
        self.assertEqual(found, ['LICENSE', 'bin/node'])
        self.assertEqual(stat.S_IMODE((self.root / 'node/bin/node').stat().st_mode), 0o755)

    def test_pinned_versions(self):
        self.assertEqual(package.NODE_ARCHIVE, 'node-v24.21.0-linux-arm64.tar.xz')
        self.assertEqual(package.NODE_URL, 'https://nodejs.org/dist/v24.21.0/node-v24.21.0-linux-arm64.tar.xz')
        self.assertEqual(package.CADDY_VERSION, '2.11.7')


if __name__ == '__main__':
    unittest.main()
