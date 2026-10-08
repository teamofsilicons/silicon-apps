"""Failure-path tests; never invoke real systemd, AWS, Docker or host installation."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import tarfile
import tempfile
import unittest
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

    def bundle(self):
        contents = {'bin/apps-server': b'candidate executable', 'web/index.html': b'production frontend'}
        manifest = json.dumps({'revision': REVISION, 'files': {
            name: hashlib.sha256(data).hexdigest() for name, data in contents.items()}}).encode()
        archive = self.root / 'release.tar.gz'
        with tarfile.open(archive, 'w:gz') as output:
            for name, data in {**contents, 'build.json': manifest}.items():
                member = tarfile.TarInfo(name)
                member.size = len(data)
                member.mode = 0o755 if name.startswith('bin/') else 0o644
                output.addfile(member, io.BytesIO(data))
        return archive, hashlib.sha256(archive.read_bytes()).hexdigest()

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
        (release / 'web/index.html').write_bytes(b'tampered')
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


if __name__ == '__main__':
    unittest.main()
