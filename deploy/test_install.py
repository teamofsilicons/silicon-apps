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
import sys
import tarfile
import tempfile
import types
from contextlib import redirect_stdout
import unittest
import unittest.mock
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('apps_deploy_install', Path(__file__).with_name('install.py'))
installer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(installer)
REVISION = 'a' * 40
RUNNER_PATH = Path(__file__).resolve().parent.parent / 'runner/server.py'


def load_runner():
    """runner/server.py itself; a stand-in yaml module when PyYAML is absent (only its config helpers are used)."""
    spec = importlib.util.spec_from_file_location('apps_runner_server', RUNNER_PATH)
    module = importlib.util.module_from_spec(spec)
    stub = {} if importlib.util.find_spec('yaml') else {'yaml': types.ModuleType('yaml')}
    with patch.dict(sys.modules, stub):
        spec.loader.exec_module(module)
    return module


def read_environment(path):
    """Decode the systemd EnvironmentFile lines environment_file() writes."""
    values = {}
    for line in path.read_text().splitlines():
        key, value = line.split('=', 1)
        values[key] = re.sub(r'\\(.)', r'\1', value[1:-1])
    return values


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


LINUX_TARGETS = {'linux-x86_64', 'linux-i686', 'linux-aarch64', 'linux-armv7hf'}
# What each platform reports on a healthy x86_64 worker (a 32-bit process may see x86_64).
HEALTHY = {'linux/amd64': 'x86_64 64', 'linux/386': 'x86_64 32', 'linux/arm64': 'aarch64 64', 'linux/arm/v7': 'armv7l 32'}


def binfmt_entry(name, flags='POCF', state='enabled'):
    return '%s\ninterpreter /usr/bin/%s\nflags: %s\noffset 0\nmagic 7f454c46\n' % (state, name, flags)


class WorkerPlatformTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.binfmt = Path(self.temporary.name)
        patcher = patch.object(installer, 'BINFMT_MISC', self.binfmt)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.commands = []

    def test_pins_cover_the_four_linux_targets_with_the_runners_platforms(self):
        runner = load_runner()
        self.assertEqual(set(installer.WORKER_IMAGES), LINUX_TARGETS)
        self.assertEqual({t: p for t, (p, _) in installer.WORKER_IMAGES.items()}, runner.PLATFORMS)
        self.assertEqual(set(installer.WORKER_SELF_CHECK), LINUX_TARGETS)
        images = {t: image for t, (_, image) in installer.WORKER_IMAGES.items()}
        for image in images.values():
            self.assertRegex(image, r'^python@sha256:[0-9a-f]{64}$')
        # Per-platform manifests: four different digests, none of them the multi-platform index.
        self.assertEqual(len(set(images.values())), 4)
        self.assertNotIn(installer.WORKER_IMAGE_INDEX, images.values())
        self.assertEqual(runner.image_config(json.loads(json.dumps(images))), images)
        self.assertRegex(installer.BINFMT_IMAGE, r'^tonistiigi/binfmt@sha256:[0-9a-f]{64}$')
        self.assertEqual(installer.EMULATORS, ('qemu-aarch64', 'qemu-arm'))

    def test_emulator_entry_needs_enabled_and_the_f_flag(self):
        for content, expected in [(binfmt_entry('qemu-arm'), True), (binfmt_entry('qemu-arm', 'F'), True),
                                  (binfmt_entry('qemu-arm', 'POC'), False), (binfmt_entry('qemu-arm', 'OC'), False),
                                  (binfmt_entry('qemu-arm', state='disabled'), False), ('', False)]:
            with self.subTest(content=content[:30]):
                (self.binfmt / 'qemu-arm').write_text(content)
                self.assertEqual(installer.emulator_registered('qemu-arm'), expected)
        (self.binfmt / 'qemu-arm').unlink()
        self.assertFalse(installer.emulator_registered('qemu-arm'))

    def fake_binfmt(self, flags='POCF', register=True):
        def fake(command, **kwargs):
            self.commands.append(command)
            if register:
                for name in installer.EMULATORS:
                    (self.binfmt / name).write_text(binfmt_entry(name, flags))
            return subprocess.CompletedProcess(command, 0, '{}', 'installing: arm64 OK\ninstalling: arm OK\n')
        return patch.object(installer.subprocess, 'run', side_effect=fake)

    def test_registration_runs_only_the_pinned_arm_emulators(self):
        with self.fake_binfmt(), patch.object(installer.time, 'sleep') as sleep:
            self.assertTrue(installer.register_emulators())
        sleep.assert_not_called()
        self.assertEqual(self.commands, [['docker', 'run', '--rm', '--pull=never', '--privileged', '--network=none',
                                          '--platform', 'linux/amd64', installer.BINFMT_IMAGE,
                                          '--uninstall', 'qemu-aarch64,qemu-arm', '--install', 'arm64,arm']])

    def test_registration_without_the_f_flag_or_an_entry_fails_clearly(self):
        for flags, register in (('POC', True), ('POCF', False)):
            with self.subTest(flags=flags, register=register):
                for entry in self.binfmt.iterdir():
                    entry.unlink()
                with self.fake_binfmt(flags, register), patch.object(installer.time, 'sleep') as sleep:
                    with self.assertRaisesRegex(RuntimeError, 'F flag for qemu-aarch64, qemu-arm .*arm64 OK') as caught:
                        installer.register_emulators(attempts=3)
                self.assertEqual(sleep.call_count, 2)
                self.assertIn('worker was not changed', str(caught.exception))

    def test_live_registrations_are_left_for_the_cutover_and_others_replaced(self):
        for name in installer.EMULATORS:
            (self.binfmt / name).write_text(binfmt_entry(name))
        with self.fake_binfmt():
            self.assertFalse(installer.register_emulators())
        self.assertEqual(self.commands, [])
        # An entry without the F flag (say, from a distribution package) is replaced.
        (self.binfmt / 'qemu-arm').write_text(binfmt_entry('qemu-arm', 'OC'))
        with self.fake_binfmt(), patch.object(installer.time, 'sleep'):
            self.assertTrue(installer.register_emulators())
        self.assertEqual(self.commands, [installer.binfmt_command()])
        self.assertTrue(installer.emulator_registered('qemu-arm'))

    def test_registration_waits_for_an_entry_the_kernel_shows_late(self):
        def appear(_seconds):
            for name in installer.EMULATORS:
                (self.binfmt / name).write_text(binfmt_entry(name))
        with self.fake_binfmt(register=False), patch.object(installer.time, 'sleep', side_effect=appear):
            installer.register_emulators()

    def test_boot_unit_reregisters_before_the_worker(self):
        unit = installer.binfmt_unit()
        lines = unit.splitlines()
        for line in ('Type=oneshot', 'RemainAfterExit=yes', 'After=docker.service', 'Requires=docker.service',
                     'Before=silicon-apps-runner.service', 'WantedBy=multi-user.target'):
            self.assertIn(line, lines)
        self.assertIn('ExecStart=' + ' '.join(['/usr/bin/docker'] + installer.binfmt_command()[1:]), lines)
        check = next(line for line in lines if line.startswith('ExecStartPost=/bin/sh -c '))
        self.assertIn('for n in qemu-aarch64 qemu-arm;', check)
        self.assertIn('^flags: [A-Z]*F', check)
        # systemd expands $NAME and %x: every dollar is escaped and no specifier is used.
        self.assertNotIn('$', unit.replace('$$', ''))
        self.assertNotIn('%', unit)

    def fake_containers(self, outputs):
        def fake(command, **kwargs):
            self.commands.append(command)
            platform = command[command.index('--platform') + 1]
            result = outputs.get(platform, HEALTHY[platform])
            if isinstance(result, Exception):
                raise result
            if isinstance(result, tuple):
                return subprocess.CompletedProcess(command, result[0], result[1], result[2])
            return subprocess.CompletedProcess(command, 0, result + '\n', '')
        return patch.object(installer.subprocess, 'run', side_effect=fake)

    def test_self_check_runs_each_image_under_its_platform_with_the_runners_isolation(self):
        with self.fake_containers({'linux/386': 'i686 32', 'linux/arm/v7': 'armv8l 32'}):
            results = installer.self_check()
        self.assertEqual(results, {'linux-x86_64': 'x86_64 64', 'linux-i686': 'i686 32',
                                   'linux-aarch64': 'aarch64 64', 'linux-armv7hf': 'armv8l 32'})
        runner = load_runner()
        with tempfile.TemporaryDirectory() as folder:
            images = {t: image for t, (_, image) in installer.WORKER_IMAGES.items()}
            for target, command in zip(installer.WORKER_IMAGES, self.commands):
                launched = []
                with patch.object(runner, 'bounded_run', side_effect=lambda c, *a, **k: launched.append(c) or (0, '', '')), \
                        patch.object(runner.subprocess, 'run'), patch.object(runner, 'native_target', return_value=None):
                    runner.execute(Path(folder), 'bin/app', ['version'], target, images)
                runner_command = launched[0]
                isolation = [a for a in runner_command if a.startswith('--') and not a.startswith(('--env=', '--workdir=', '--name', '--mount', '--entrypoint', '--platform'))]
                self.assertTrue(isolation)
                for flag in isolation:
                    self.assertIn(flag, command)
                platform = runner_command[runner_command.index('--platform') + 1]
                self.assertEqual(command[command.index('--platform') + 1], platform)
                self.assertEqual(command[command.index('--entrypoint') + 2], runner_command[runner_command.index('--entrypoint') + 2])
                self.assertEqual(command[-3:], ['-I', '-c', installer.SELF_CHECK_CODE])

    def test_self_check_names_every_platform_that_does_not_run(self):
        outputs = {'linux/386': 'x86_64 64',  # a 64-bit userland is not linux-i686
                   'linux/arm64': (126, '', 'exec /usr/local/bin/python3: exec format error\n'),
                   'linux/arm/v7': subprocess.TimeoutExpired(['docker'], 120)}
        with self.fake_containers(outputs):
            with self.assertRaises(RuntimeError) as caught:
                installer.self_check()
        message = str(caught.exception)
        self.assertNotIn('linux-x86_64', message)
        for text in ("linux-i686 (linux/386) exited 0 and reported 'x86_64 64'", 'linux-aarch64 (linux/arm64) exited 126',
                     'exec format error', 'linux-armv7hf (linux/arm/v7) did not finish', 'ia32_emulation',
                     'qemu-aarch64 binfmt_misc entry with the F flag', 'qemu-arm binfmt_misc entry'):
            self.assertIn(text, message)
        self.assertEqual(len(self.commands), 4)


OLD_RUNNER_UNIT = '[Unit]\nDescription=old worker\n[Service]\nExecStart=/old\n'


class WorkerMainTests(unittest.TestCase):
    """main() for the worker with every host effect replaced, from pulls to the post-ready self-check."""
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        base = Path(self.temporary.name).resolve()
        self.root, self.etc, self.state, self.binfmt = base / 'opt', base / 'etc', base / 'state', base / 'binfmt_misc'
        self.binfmt.mkdir()
        self.previous = self.root / 'releases' / ('b' * 40)
        self.previous.mkdir(parents=True)
        (self.root / 'current').symlink_to(self.previous)
        self.release = self.root / 'releases' / REVISION
        (self.release / 'runner').mkdir(parents=True)
        requirements = b'PyYAML==6.0.2\n'
        (self.release / 'runner/requirements.txt').write_bytes(requirements)
        venv = self.root / 'venvs' / REVISION
        venv.mkdir(parents=True)
        (venv / '.ready').write_text(hashlib.sha256(requirements).hexdigest())
        self.unit_path = self.etc / 'systemd/system/silicon-apps-runner.service'
        self.unit_path.parent.mkdir(parents=True)
        self.unit_path.write_text(OLD_RUNNER_UNIT)
        self.env_path = self.etc / 'silicon-apps/worker.env'
        self.env_path.parent.mkdir(parents=True)
        self.env_path.write_text('APPS_RUNNER_TOKEN="old"\n')
        self.secret = {'APPS_RUNNER_TOKEN': 'r' * 40}
        self.states = {'silicon-apps-runner': ('enabled', True), 'docker': ('enabled', True)}
        self.events, self.outputs, self.broken_after_restart, self.flags = [], {}, {}, 'POCF'

    def fake_run(self, command, **kwargs):
        self.events.append(' '.join(str(c) for c in command))
        if command[0] == 'systemctl':
            action, service = command[1], command[-1]
            enabled, active = self.states.get(service, ('not-found', False))
            if action == 'is-enabled':
                return subprocess.CompletedProcess(command, 0 if enabled == 'enabled' else 1, enabled + '\n')
            if action == 'is-active':
                return subprocess.CompletedProcess(command, 0 if active else 3)
            if action in ('start', 'restart', 'stop'):
                self.states[service] = (enabled, action != 'stop')
            if action in ('enable', 'disable'):
                self.states[service] = ('enabled' if action == 'enable' else 'disabled', active)
            return subprocess.CompletedProcess(command, 0)
        if command[:2] == ['docker', 'run'] and installer.BINFMT_IMAGE in command:
            for name in installer.EMULATORS:
                (self.binfmt / name).write_text(binfmt_entry(name, self.flags))
            return subprocess.CompletedProcess(command, 0, '{}', 'installing: arm64 OK\n')
        if command[:2] == ['docker', 'run']:
            platform = command[command.index('--platform') + 1]
            outputs = self.outputs
            if 'systemctl restart silicon-apps-runner' in self.events:
                outputs = dict(outputs, **self.broken_after_restart)
            code, out, err = outputs.get(platform, (0, HEALTHY[platform] + '\n', ''))
            return subprocess.CompletedProcess(command, code, out, err)
        return subprocess.CompletedProcess(command, 0, '', '')

    def install(self):
        record = lambda name: (lambda *args, **kwargs: self.events.append(name + ' ' + ' '.join(str(a) for a in args)))
        argv = ['install.py', '--archive', 'release.tar.gz', '--sha256', '0' * 64, '--revision', REVISION,
                '--role', 'worker', '--secret', 'arn:aws:secretsmanager:us-east-2:1:secret:worker', '--bucket', 'apps-artifacts']
        output = io.StringIO()
        with patch.multiple(installer, ROOT=self.root, ETC=self.etc, STATE=self.state, BINFMT_MISC=self.binfmt,
                            account=record('account'), directory=record('directory'),
                            wait_ready=lambda service, role, timeout=60: self.events.append('ready ' + service),
                            prepare_release=unittest.mock.Mock(return_value=self.release),
                            capture=unittest.mock.Mock(return_value=json.dumps({'SecretString': json.dumps(self.secret)}))), \
                patch.object(installer.os, 'geteuid', return_value=0), patch.object(installer.os, 'chown'), \
                patch.object(installer.subprocess, 'run', side_effect=self.fake_run), \
                patch.object(installer.time, 'sleep'), patch('sys.argv', argv), redirect_stdout(output):
            installer.main()
        return json.loads(output.getvalue())

    def positions(self, prefix):
        return [i for i, event in enumerate(self.events) if event.startswith(prefix)]

    def checks(self):
        return self.positions('docker run --rm --pull=never --platform')

    def test_pins_registers_and_checks_every_platform(self):
        summary = self.install()
        self.assertEqual(summary['platforms'], {'linux-x86_64': 'x86_64 64', 'linux-i686': 'x86_64 32',
                                                'linux-aarch64': 'aarch64 64', 'linux-armv7hf': 'armv7l 32'})
        pulls = [' '.join(['docker pull --platform', platform, image])
                 for platform, image in [*installer.WORKER_IMAGES.values(), ('linux/amd64', installer.BINFMT_IMAGE)]]
        self.assertEqual([e for e in self.events if e.startswith('docker pull')], pulls)
        self.assertFalse([e for e in self.events if 'slim-trixie' in e or 'docker image inspect' in e])
        checks = self.checks()
        self.assertEqual(len(checks), 8)
        order = [max(self.positions('docker pull')), self.positions('docker run --rm --pull=never --privileged')[0],
                 checks[0], checks[3], self.positions('systemctl daemon-reload')[0],
                 self.positions('systemctl enable silicon-apps-binfmt')[0], self.positions('systemctl restart silicon-apps-binfmt')[0],
                 self.positions('systemctl restart silicon-apps-runner')[0], self.positions('ready silicon-apps-runner')[0], checks[4]]
        self.assertEqual(order, sorted(order))
        environment = read_environment(self.env_path)
        images = {target: image for target, (_, image) in installer.WORKER_IMAGES.items()}
        self.assertEqual(json.loads(environment.pop('APPS_RUNNER_IMAGES')), images)
        self.assertEqual(environment, {'APPS_RUNNER_TOKEN': 'r' * 40, 'APPS_RUNNER_HOST': '0.0.0.0', 'APPS_RUNNER_PORT': '4312',
                                       'TMPDIR': str(self.state / 'silicon-apps-runner/jobs')})
        self.assertEqual(stat.S_IMODE(self.env_path.stat().st_mode), 0o600)
        venv_python = self.root / 'venvs' / REVISION / 'bin/python'
        self.assertEqual(self.unit_path.read_text(), (
            '[Unit]\nDescription=Silicon Apps isolated Linux validator\n'
            'After=network-online.target docker.service silicon-apps-binfmt.service\n'
            'Requires=docker.service silicon-apps-binfmt.service\n[Service]\nUser=silicon-apps-runner\n'
            'Group=silicon-apps-runner\nSupplementaryGroups=docker\nWorkingDirectory=/opt/silicon-apps/current\n'
            'EnvironmentFile=/etc/silicon-apps/worker.env\nExecStart=%s /opt/silicon-apps/current/runner/server.py\n'
            'Restart=on-failure\nRestartSec=3\nUMask=0077\nNoNewPrivileges=true\nProtectHome=true\nProtectSystem=strict\n'
            'ReadWritePaths=/var/lib/silicon-apps-runner\n[Install]\nWantedBy=multi-user.target\n') % venv_python)
        with patch.object(installer, 'BINFMT_MISC', self.binfmt):
            boot_unit = installer.binfmt_unit()
        self.assertEqual((self.etc / 'systemd/system/silicon-apps-binfmt.service').read_text(), boot_unit)
        record = json.loads((self.etc / 'silicon-apps/worker-image.json').read_text())
        self.assertEqual(record, {'image': images['linux-x86_64'], 'images': images,
                                  'index': installer.WORKER_IMAGE_INDEX, 'emulator': installer.BINFMT_IMAGE})
        self.assertEqual(self.states['silicon-apps-binfmt'], ('enabled', True))
        self.assertEqual((self.root / 'current').resolve(), self.release)
        self.assertEqual(json.loads((self.root / 'deployment.json').read_text())['role'], 'worker')

    def assert_unchanged(self):
        self.assertEqual((self.root / 'current').resolve(), self.previous)
        self.assertEqual(self.unit_path.read_text(), OLD_RUNNER_UNIT)
        self.assertEqual(self.env_path.read_text(), 'APPS_RUNNER_TOKEN="old"\n')
        self.assertFalse((self.etc / 'systemd/system/silicon-apps-binfmt.service').exists())
        self.assertFalse((self.etc / 'silicon-apps/worker-image.json').exists())
        self.assertFalse((self.root / 'deployment.json').exists())

    def test_a_platform_that_does_not_run_stops_before_any_live_change(self):
        self.outputs = {'linux/arm/v7': (126, '', 'exec /usr/local/bin/python3: exec format error\n')}
        with self.assertRaisesRegex(RuntimeError, r'linux-armv7hf \(linux/arm/v7\) exited 126.*exec format error'):
            self.install()
        self.assert_unchanged()
        self.assertFalse([e for e in self.events if e.startswith(('systemctl daemon-reload', 'systemctl restart', 'systemctl stop'))])
        self.assertEqual(self.states['silicon-apps-runner'], ('enabled', True))

    def test_emulation_without_the_f_flag_stops_before_the_self_check(self):
        self.flags = 'POC'
        with self.assertRaisesRegex(RuntimeError, 'F flag for qemu-aarch64, qemu-arm'):
            self.install()
        self.assert_unchanged()
        self.assertEqual(self.checks(), [])

    def test_an_upgrade_leaves_live_emulation_alone_until_the_unit_restart(self):
        for name in installer.EMULATORS:
            (self.binfmt / name).write_text(binfmt_entry(name))
        self.install()
        self.assertEqual(self.positions('docker run --rm --pull=never --privileged'), [])
        self.assertLess(self.checks()[3], self.positions('systemctl restart silicon-apps-binfmt')[0])
        self.assertLess(self.positions('systemctl restart silicon-apps-binfmt')[0], self.checks()[4])

    def test_a_failed_check_after_the_restart_rolls_everything_back(self):
        self.broken_after_restart = {'linux/arm64': (1, '', 'exec format error\n')}
        with self.assertRaisesRegex(RuntimeError, 'linux-aarch64'):
            self.install()
        self.assert_unchanged()
        self.assertEqual(len(self.checks()), 8)
        self.assertEqual(self.states['silicon-apps-runner'], ('enabled', True))
        self.assertEqual(self.states['silicon-apps-binfmt'], ('disabled', False))
        self.assertGreater(self.positions('systemctl start silicon-apps-runner')[-1], self.checks()[-1])


if __name__ == '__main__':
    unittest.main()
