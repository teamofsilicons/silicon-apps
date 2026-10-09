"""Failure-path tests; never invoke real systemd, AWS, Docker or host installation."""
import base64
import email.message
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tarfile
import tempfile
from contextlib import redirect_stdout
import unittest
import unittest.mock
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('apps_deploy_install', Path(__file__).with_name('install.py'))
installer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(installer)
REVISION = 'a' * 40


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()

    def bundle(self, links=None, manifest_links=None, contents=None):
        contents = contents or {'bin/apps-server': b'candidate executable', 'web/install.sh': b'installer'}
        links = links or {}
        manifest = json.dumps({'revision': REVISION, 'files': {
            name: hashlib.sha256(data).hexdigest() for name, data in contents.items()},
            'symlinks': links if manifest_links is None else manifest_links}).encode()
        archive = self.root / 'release.tar.gz'
        with tarfile.open(archive, 'w:gz') as output:
            for name, data in {**contents, 'build.json': manifest}.items():
                member = tarfile.TarInfo(name)
                member.size = len(data)
                member.mode = 0o755 if name.startswith('bin/') else 0o644
                output.addfile(member, io.BytesIO(data))
            for name, target in links.items():
                member = tarfile.TarInfo(name)
                member.type = tarfile.SYMTYPE
                member.linkname = target
                member.mode = 0o777
                output.addfile(member)
        return archive, hashlib.sha256(archive.read_bytes()).hexdigest()

    PNPM = {'store/server.js': b'require("next")',
            'store/node_modules/.pnpm/next@16/node_modules/next/index.js': b'module.exports = 1',
            'store/node_modules/.pnpm/react@19/node_modules/react/index.js': b'module.exports = 2'}
    PNPM_LINKS = {'store/node_modules/next': '.pnpm/next@16/node_modules/next',
                  'store/node_modules/.pnpm/next@16/node_modules/react': '../../react@19/node_modules/react'}

    def test_store_pnpm_links_are_preserved_and_verified(self):
        archive, digest = self.bundle(self.PNPM_LINKS, contents=self.PNPM)
        old = os.umask(0o077)
        try:
            release = installer.prepare_release(archive, digest, REVISION, self.root / 'install')
        finally:
            os.umask(old)
        link = release / 'store/node_modules/next'
        self.assertTrue(link.is_symlink())
        self.assertEqual(os.readlink(link), '.pnpm/next@16/node_modules/next')
        self.assertEqual((link / 'index.js').read_bytes(), b'module.exports = 1')
        self.assertEqual((release / 'store/node_modules/.pnpm/next@16/node_modules/react/index.js').read_bytes(), b'module.exports = 2')
        # The link's 0777 mode must not leak onto its target.
        self.assertEqual(stat.S_IMODE((release / 'store/node_modules/.pnpm/next@16/node_modules/next/index.js').stat().st_mode), 0o644)
        self.assertEqual(stat.S_IMODE((release / 'store/node_modules/.pnpm/next@16/node_modules/next').stat().st_mode), 0o755)
        self.assertEqual(installer.prepare_release(archive, digest, REVISION, self.root / 'install'), release)
        link.unlink()
        link.symlink_to('.pnpm/react@19/node_modules/react')
        with self.assertRaisesRegex(ValueError, 'inventory'):
            installer.prepare_release(archive, digest, REVISION, self.root / 'install')

    def test_unlisted_dangling_or_escaping_links_are_rejected(self):
        cases = [
            (dict(self.PNPM_LINKS), {}, 'inventory'),  # links the manifest does not record
            ({'store/node_modules/gone': '.pnpm/gone@1/node_modules/gone'}, None, 'broken'),
        ]
        for links, manifest_links, message in cases:
            with self.subTest(message=message):
                archive, digest = self.bundle(links, manifest_links, contents=self.PNPM)
                with self.assertRaisesRegex(ValueError, message):
                    installer.prepare_release(archive, digest, REVISION, self.root / ('install-' + message))
                self.assertFalse((self.root / ('install-' + message) / 'releases' / REVISION).exists())
        for target in ('/etc/passwd', '../../../../etc'):
            with self.subTest(target=target):
                archive, digest = self.bundle({'store/node_modules/evil': target}, contents=self.PNPM)
                with self.assertRaises(Exception):
                    installer.prepare_release(archive, digest, REVISION, self.root / 'install-evil')
                self.assertFalse((self.root / 'install-evil/releases' / REVISION).exists())

    def test_restrictive_umask_and_existing_release_verification(self):
        archive, digest = self.bundle()
        old = os.umask(0o077)
        try:
            release = installer.prepare_release(archive, digest, REVISION, self.root / 'install')
        finally:
            os.umask(old)
        for name in ('', 'bin', 'web'):
            self.assertEqual(stat.S_IMODE((release / name).stat().st_mode), 0o755)
        self.assertEqual(stat.S_IMODE((release / 'bin/apps-server').stat().st_mode), 0o755)
        self.assertEqual(installer.prepare_release(archive, digest, REVISION, self.root / 'install'), release)
        (release / 'web/install.sh').write_bytes(b'tampered')
        with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
            installer.prepare_release(archive, digest, REVISION, self.root / 'install')

    def test_corrupt_archive_never_creates_release(self):
        archive, _ = self.bundle()
        with self.assertRaisesRegex(ValueError, 'Archive checksum'):
            installer.prepare_release(archive, '0' * 64, REVISION, self.root / 'install')
        self.assertFalse((self.root / 'install').exists())

    def test_untracked_nested_manifest_is_rejected(self):
        archive, digest = self.bundle()
        release = installer.prepare_release(archive, digest, REVISION, self.root / 'install')
        (release / 'web/build.json').write_text('{}')
        with self.assertRaisesRegex(ValueError, 'inventory'):
            installer.prepare_release(archive, digest, REVISION, self.root / 'install')

    def test_environment_round_trip_uses_one_quoted_value(self):
        value = '  alpha\'" \\ $HOME `command` # trailing  '
        encoded = installer.environment_file({'TEST': value})
        # Decode the documented EnvironmentFile double-quote escape grammar.
        body = encoded[len('TEST="'):-2]
        decoded = ''
        index = 0
        while index < len(body):
            char = body[index]
            if char == '\\' and index + 1 < len(body) and body[index + 1] in '\\"`$':
                index += 1
                char = body[index]
            decoded += char
            index += 1
        self.assertEqual(decoded, value)
        self.assertEqual(installer.environment_file({'EMPTY': ''}), 'EMPTY=""\n')
        for invalid in ({'0BAD': 'safe'}, {'VALID': 'line\nbreak'}, {'VALID': None}, {'VALID': '\ufeff'}):
            with self.assertRaises(ValueError):
                installer.environment_file(invalid)

    def systemd(self, states):
        self.calls = []
        def fake(command, **kwargs):
            self.calls.append(command)
            action = command[1]
            service = command[-1]
            enabled, active = states.get(service, ('not-found', False))
            if action == 'is-enabled':
                return subprocess.CompletedProcess(command, 0 if enabled == 'enabled' else 1, enabled + '\n')
            if action == 'is-active':
                return subprocess.CompletedProcess(command, 0 if active else 3)
            if action in ('start', 'restart', 'stop'):
                states[service] = (enabled, action != 'stop')
            if action in ('enable', 'disable'):
                states[service] = ('enabled' if action == 'enable' else 'disabled', active)
            return subprocess.CompletedProcess(command, 0)
        return patch.object(installer.subprocess, 'run', side_effect=fake)

    def test_upgrade_rollback_restores_configs_pointer_permissions_and_services(self):
        previous, candidate = self.root / 'previous', self.root / 'candidate'
        previous.mkdir(); candidate.mkdir()
        current = self.root / 'current'
        current.symlink_to(previous)
        env, unit, caddy, new_file = [self.root / name for name in ('api.env', 'api.service', 'Caddyfile', 'new.timer')]
        for path in (env, unit, caddy):
            path.write_text('old-' + path.name)
            path.chmod(0o600 if path == env else 0o644)
        states = {'api': ('enabled', True), 'caddy': ('enabled', True), 'timer': ('disabled', False)}
        with self.systemd(states), patch.object(installer.os, 'chown'):
            transaction = installer.Cutover(current, [env, unit, caddy, new_file], list(states))
            for path in (env, unit, caddy, new_file):
                installer.atomic(path, 'candidate', 0o644)
            installer.point_current(current, candidate)
            states['timer'] = ('enabled', True)
            transaction.rollback()
        self.assertEqual(current.resolve(), previous)
        for path in (env, unit, caddy):
            self.assertEqual(path.read_text(), 'old-' + path.name)
        self.assertEqual(stat.S_IMODE(env.stat().st_mode), 0o600)
        self.assertFalse(new_file.exists())
        self.assertEqual(states, {'api': ('enabled', True), 'caddy': ('enabled', True), 'timer': ('disabled', False)})

    def test_failed_initial_install_removes_candidate_configs_and_stops_service(self):
        current, env = self.root / 'current', self.root / 'api.env'
        candidate = self.root / 'candidate'
        candidate.mkdir()
        states = {'api': ('not-found', False)}
        with self.systemd(states):
            transaction = installer.Cutover(current, [env], ['api'])
            installer.atomic(env, 'new secret')
            installer.point_current(current, candidate)
            states['api'] = ('enabled', True)
            transaction.rollback()
        self.assertFalse(current.is_symlink())
        self.assertFalse(env.exists())
        self.assertEqual(states['api'], ('disabled', False))

    def test_backup_failure_before_file_changes_restarts_previous_service(self):
        previous = self.root / 'previous'
        previous.mkdir()
        current = self.root / 'current'
        current.symlink_to(previous)
        states = {'api': ('enabled', True)}
        with self.systemd(states):
            transaction = installer.Cutover(current, [], ['api'])
            installer.run(['systemctl', 'stop', 'api'])
            transaction.rollback()  # A backup exception reaches this same guarded path.
        self.assertTrue(states['api'][1])
        self.assertEqual(current.resolve(), previous)

    def test_readiness_retries_a_slow_start(self):
        response = io.BytesIO(b'{"status":"ok"}')
        with patch.object(installer.urllib.request, 'build_opener') as build, self.systemd({'api': ('enabled', True)}), patch.object(installer.time, 'sleep'):
            build.return_value.open.side_effect = [ConnectionRefusedError(), response]
            installer.wait_ready('api', 'api', timeout=5)
            self.assertEqual(build.return_value.open.call_count, 2)


# RFC 8032 section 7.1, tests 1 and 2: (secret seed, public key).
RFC8032 = [('9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60',
            'd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a'),
           ('4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb',
            '3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c')]
SEED = base64.b64encode(bytes.fromhex(RFC8032[0][0])).decode()
PUBLIC = base64.b64encode(bytes.fromhex(RFC8032[0][1])).decode()
OTHER_SEED = base64.b64encode(bytes.fromhex(RFC8032[1][0])).decode()


class SigningKeyTests(unittest.TestCase):
    def test_public_key_derivation_matches_rfc_8032(self):
        for seed, public in RFC8032:
            self.assertEqual(installer.ed25519_public_key(bytes.fromhex(seed)), base64.b64encode(bytes.fromhex(public)).decode())

    def test_pins_mirror_the_cli(self):
        source = (Path(__file__).resolve().parent.parent / 'crates/client/src/signing.rs').read_text()
        block = source[source.index('pub const PINNED_KEYS'):]
        block = block[:block.index('];')]
        pins = dict(re.findall(r'key_id:\s*"([^"]+)",\s*public_key:\s*"([^"]+)"', block))
        self.assertTrue(pins)
        self.assertEqual(pins, installer.PINNED_SIGNING_KEYS)

    def check(self, value, **extra):
        with patch.dict(installer.PINNED_SIGNING_KEYS, {'test-2026': PUBLIC}, clear=True):
            return installer.check_signing_keys({'APPS_SIGNING_KEYS': value, **extra})

    def test_valid_keys_return_the_active_key(self):
        self.assertEqual(self.check('test-2026:' + SEED), 'test-2026')
        # A newer, unpinned key first, endorsed by the pinned key listed after it.
        self.assertEqual(self.check('test-2027:' + OTHER_SEED + ',test-2026:' + SEED), 'test-2027')
        self.assertEqual(self.check('test-2027:' + OTHER_SEED + '\ntest-2026:' + SEED, APPS_REVOKED_SIGNING_KEYS=''), 'test-2027')

    def test_unusable_keys_fail_without_echoing_key_material(self):
        for value, message in [
            ('', 'has no APPS_SIGNING_KEYS'),
            ('   ', 'has no APPS_SIGNING_KEYS'),
            (SEED, 'not key_id:base64-seed'),
            ('bad id:' + SEED, 'not key_id:base64-seed'),
            ('test-2026:' + SEED[:-4], 'base64 32-byte'),
            ('test-2026:' + SEED.rstrip('='), 'base64 32-byte'),
            ('test-2026:' + SEED + ',test-2026:' + SEED, 'twice'),
            ('test-2026:' + OTHER_SEED, 'does not match the public key pinned'),
            ('test-2027:' + OTHER_SEED, 'no key pinned'),
        ]:
            with self.subTest(message=message, value=value[:12]):
                with self.assertRaisesRegex(ValueError, message) as caught:
                    self.check(value)
                for secret in (SEED, OTHER_SEED, SEED[:-4]):
                    self.assertNotIn(secret, str(caught.exception))
        with self.assertRaisesRegex(ValueError, 'REVOKED'):
            self.check('test-2026:' + SEED, APPS_REVOKED_SIGNING_KEYS='test-2026')
        with self.assertRaisesRegex(ValueError, 'has no APPS_SIGNING_KEYS'):
            installer.check_signing_keys({})

    def test_api_secret_requires_the_store_origin(self):
        secret = {'APPS_ACCOUNTS_APP_SECRET': 's', 'APPS_ACCOUNTS_SERVICE_TOKEN': 't', 'APPS_RUNNER_URL': 'http://w',
                  'APPS_DEV_AUTH': '0', 'APPS_SIGNING_KEYS': 'test-2026:' + SEED,
                  'APPS_ALLOWED_ORIGINS': 'https://developers.teamofsilicons.com'}
        with patch.dict(installer.PINNED_SIGNING_KEYS, {'test-2026': PUBLIC}, clear=True):
            with self.assertRaisesRegex(ValueError, 'APPS_ALLOWED_ORIGINS'):
                installer.check_api_secret(secret)
            secret['APPS_ALLOWED_ORIGINS'] = 'https://apps.teamofsilicons.com/, https://developers.teamofsilicons.com'
            self.assertEqual(installer.check_api_secret(secret), 'test-2026')
            with self.assertRaisesRegex(ValueError, 'APPS_DEV_AUTH'):
                installer.check_api_secret(dict(secret, APPS_DEV_AUTH='1'))


class Page(io.BytesIO):
    def __init__(self, body, status=200, content_type='text/html; charset=utf-8'):
        super().__init__(body)
        self.status = status
        self.headers = email.message.Message()
        self.headers['Content-Type'] = content_type


HOME = b'<!DOCTYPE html><html><head><title>Silicon Apps: apps for Carbons and Silicons</title></head><body><main>Find an app</main></body></html>'


class StoreTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.release = Path(self.temporary.name) / 'releases' / REVISION
        (self.release / 'deploy').mkdir(parents=True)
        unit = Path(__file__).with_name('silicon-apps-store.service')
        (self.release / 'deploy' / unit.name).write_text(unit.read_text())
        self.commands = []

    def fake_run(self, active=True):
        def fake(command, **kwargs):
            self.commands.append(command)
            if command[:2] == ['systemctl', 'is-active']:
                return subprocess.CompletedProcess(command, 0 if active else 3)
            return subprocess.CompletedProcess(command, 0)
        return patch.object(installer.subprocess, 'run', side_effect=fake)

    def test_store_renders_only_server_rendered_html(self):
        opener = unittest.mock.Mock()
        for page, expected in [(Page(HOME), True), (Page(b'<html><body><div id="root"></div></body></html>'), False),
                               (Page(HOME, content_type='application/json'), False), (Page(HOME, status=203), False)]:
            opener.open.return_value = page
            self.assertEqual(installer.store_renders(opener, 4320), expected)
        request = opener.open.call_args[0][0]
        self.assertEqual(request.full_url, 'http://127.0.0.1:4320/')
        # The store does not count a monitor as a page view.
        self.assertRegex(request.get_header('User-agent'), 'monitor|python')

    def test_store_readiness_waits_for_the_page(self):
        with patch.object(installer.urllib.request, 'build_opener') as build, self.fake_run(), patch.object(installer.time, 'sleep'):
            build.return_value.open.side_effect = [ConnectionRefusedError(), Page(b'<html>Loading</html>'), Page(HOME)]
            installer.wait_ready('silicon-apps-store', 'store', timeout=5)
        self.assertEqual(build.return_value.open.call_count, 3)

    def test_preflight_runs_the_candidate_with_the_unit_sandbox_and_always_stops_it(self):
        with self.fake_run(), patch.object(installer, 'store_renders', side_effect=[False, True]), patch.object(installer.time, 'sleep'):
            installer.preflight_store(self.release)
        launch = next(c for c in self.commands if c[0] == 'systemd-run')
        properties = [launch[i + 1] for i, value in enumerate(launch) if value == '-p']
        self.assertIn('User=silicon-apps-store', properties)
        self.assertIn('ProtectSystem=strict', properties)
        self.assertIn('NoNewPrivileges=true', properties)
        self.assertIn('WorkingDirectory=%s/store' % self.release, properties)
        self.assertIn('Environment=APPS_API_URL=http://127.0.0.1:4310', properties)
        # The spare port is set after the unit's own PORT, so it wins.
        self.assertGreater(properties.index('Environment=PORT=4321'), properties.index('Environment=PORT=4320'))
        self.assertFalse([p for p in properties if p.split('=')[0] in ('ExecStart', 'Restart', 'RestartSec')])
        self.assertFalse([p for p in properties if '/opt/silicon-apps/current' in p])
        self.assertEqual(launch[launch.index('--') + 1:], [str(self.release / 'node/bin/node'), str(self.release / 'store/server.js')])
        self.assertEqual(self.commands[-1], ['systemctl', 'stop', 'silicon-apps-store-preflight'])

    def test_preflight_failure_changes_nothing_and_stops_the_candidate(self):
        with self.fake_run(active=False), patch.object(installer.time, 'sleep'):
            with self.assertRaisesRegex(RuntimeError, 'exited before it rendered'):
                installer.preflight_store(self.release)
        self.assertEqual(self.commands[-1], ['systemctl', 'stop', 'silicon-apps-store-preflight'])
        with self.fake_run(), patch.object(installer, 'store_renders', return_value=False), patch.object(installer.time, 'sleep'), \
                patch.object(installer.time, 'monotonic', side_effect=[0, 0, 0, 100]):
            with self.assertRaisesRegex(RuntimeError, 'did not render'):
                installer.preflight_store(self.release, timeout=5)
        self.assertEqual(self.commands[-1], ['systemctl', 'stop', 'silicon-apps-store-preflight'])


class MainTests(unittest.TestCase):
    """main() with every host effect replaced: the order of the cutover and its rollback."""
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        base = Path(self.temporary.name).resolve()
        self.root, self.etc, self.state = base / 'opt', base / 'etc', base / 'state'
        self.previous = self.root / 'releases' / ('b' * 40)
        self.previous.mkdir(parents=True)
        (self.root / 'current').symlink_to(self.previous)
        (self.state / 'silicon-apps').mkdir(parents=True)
        (self.state / 'silicon-apps/apps.sqlite').write_bytes(b'catalog')
        self.release = self.root / 'releases' / REVISION
        (self.release / 'deploy').mkdir(parents=True)
        for name in ('Caddyfile', 'silicon-apps-api.service', 'silicon-apps-store.service'):
            (self.release / 'deploy' / name).write_text(Path(__file__).with_name(name).read_text())
        self.secret = {'APPS_ACCOUNTS_APP_SECRET': 'app', 'APPS_ACCOUNTS_SERVICE_TOKEN': 'service',
                       'APPS_RUNNER_URL': 'http://worker:4312', 'APPS_RUNNER_TOKEN': 'r' * 40, 'APPS_DEV_AUTH': '0',
                       'APPS_ALLOWED_ORIGINS': 'https://apps.teamofsilicons.com,https://developers.teamofsilicons.com',
                       'APPS_SIGNING_KEYS': 'test-2026:' + SEED}
        self.events = []
        self.states = {'silicon-apps-api': ('enabled', True), 'caddy': ('enabled', True),
                       'silicon-apps-backup.timer': ('enabled', True), 'silicon-apps-store': ('', False)}

    def systemctl(self, command, **kwargs):
        self.events.append(' '.join(str(c) for c in command))
        if command[0] != 'systemctl':
            return subprocess.CompletedProcess(command, 0)
        action, service = command[1], command[-1]
        enabled, active = self.states.get(service, ('', False))
        if action == 'is-enabled':
            return subprocess.CompletedProcess(command, 0 if enabled == 'enabled' else 1, enabled + '\n')
        if action == 'is-active':
            return subprocess.CompletedProcess(command, 0 if active else 3)
        if action in ('start', 'restart', 'stop'):
            for name in command[2:]:
                self.states[name] = (self.states.get(name, ('', False))[0], action != 'stop')
        if action in ('enable', 'disable'):
            for name in command[2:]:
                self.states[name] = ('enabled' if action == 'enable' else 'disabled', self.states.get(name, ('', False))[1])
        return subprocess.CompletedProcess(command, 0)

    def install(self, store_ready=None):
        record = lambda name: (lambda *args, **kwargs: self.events.append(name + ' ' + ' '.join(str(a) for a in args)))
        argv = ['install.py', '--archive', 'release.tar.gz', '--sha256', '0' * 64, '--revision', REVISION, '--role', 'api',
                '--secret', 'arn:aws:secretsmanager:us-east-2:1:secret:apps', '--bucket', 'apps-artifacts']
        def ready(service, role, timeout=60):
            self.events.append('ready ' + service)
            if service == 'silicon-apps-store' and store_ready:
                store_ready()
        with patch.multiple(installer, ROOT=self.root, ETC=self.etc, STATE=self.state,
                            account=record('account'), directory=record('directory'),
                            preflight_store=record('preflight'), verify_signing=record('signing'),
                            wait_ready=ready, prepare_release=unittest.mock.Mock(return_value=self.release),
                            capture=unittest.mock.Mock(return_value=json.dumps({'SecretString': json.dumps(self.secret)}))), \
                patch.dict(installer.PINNED_SIGNING_KEYS, {'test-2026': PUBLIC}, clear=True), \
                patch.object(installer.os, 'geteuid', return_value=0), patch.object(installer.os, 'chown'), \
                patch.object(installer.subprocess, 'run', side_effect=self.systemctl), \
                patch('sys.argv', argv), \
                redirect_stdout(io.StringIO()):
            installer.main()

    def position(self, prefix):
        return next(i for i, event in enumerate(self.events) if event.startswith(prefix))

    def test_missing_signing_key_fails_before_anything_is_installed(self):
        del self.secret['APPS_SIGNING_KEYS']
        prepare = unittest.mock.Mock()
        with patch.object(installer, 'prepare_release', prepare), \
                patch.object(installer, 'capture', return_value=json.dumps({'SecretString': json.dumps(self.secret)})), \
                patch.object(installer.os, 'geteuid', return_value=0), \
                patch('sys.argv', ['install.py', '--archive', 'a', '--sha256', '0' * 64, '--revision', REVISION, '--role', 'api',
                                   '--secret', 's', '--bucket', 'apps-artifacts']):
            with self.assertRaisesRegex(ValueError, 'APPS_SIGNING_KEYS'):
                installer.main()
        prepare.assert_not_called()

    def test_cutover_order(self):
        self.install()
        order = ['preflight', 'systemctl stop silicon-apps-api', 'python3 ' + str(self.release / 'deploy/backup.py'),
                 'systemctl daemon-reload', 'systemctl restart silicon-apps-api', 'ready silicon-apps-api', 'signing test-2026',
                 'systemctl enable silicon-apps-store', 'systemctl restart silicon-apps-store', 'ready silicon-apps-store',
                 'systemctl restart caddy silicon-apps-backup.timer']
        self.assertEqual([self.position(p) for p in order], sorted(self.position(p) for p in order))
        self.assertLess(self.position('account silicon-apps-store'), self.position('preflight'))
        self.assertEqual((self.root / 'current').resolve(), self.release)
        unit = (self.etc / 'systemd/system/silicon-apps-store.service').read_text()
        self.assertIn('User=silicon-apps-store', unit)
        self.assertIn('APPS_SIGNING_KEYS=', (self.etc / 'silicon-apps/api.env').read_text())
        self.assertEqual(json.loads((self.root / 'deployment.json').read_text())['revision'], REVISION)

    def test_store_that_never_renders_rolls_everything_back(self):
        def fail():
            raise RuntimeError('silicon-apps-store readiness timed out')
        with self.assertRaisesRegex(RuntimeError, 'readiness'):
            self.install(store_ready=fail)
        self.assertEqual((self.root / 'current').resolve(), self.previous)
        self.assertFalse((self.etc / 'systemd/system/silicon-apps-store.service').exists())
        self.assertFalse((self.root / 'deployment.json').exists())
        self.assertEqual(self.states['silicon-apps-store'][1], False)
        self.assertTrue(self.states['silicon-apps-api'][1] and self.states['caddy'][1])
        self.assertLess(self.position('ready silicon-apps-store'), len(self.events) - 1)
        self.assertNotIn('systemctl restart caddy silicon-apps-backup.timer', self.events)


if __name__ == '__main__':
    unittest.main()
