#!/usr/bin/env python3
"""Install a verified Apps release, restoring files and services on failed cutover."""
import argparse
import hashlib
import json
import os
import pathlib
import pwd
import re
import shutil
import stat
import subprocess
import tarfile
import tempfile
import time
import urllib.error
import urllib.request

ROOT = pathlib.Path('/opt/silicon-apps')
ETC = pathlib.Path('/etc')
STATE = pathlib.Path('/var/lib')


def run(args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def capture(args):
    return subprocess.check_output(args, text=True)


def atomic(path, content, mode=0o600):
    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix='.' + path.name + '.', dir=path.parent)
    try:
        with os.fdopen(descriptor, 'wb') as output:
            output.write(content.encode() if isinstance(content, str) else content)
            output.flush()
            os.fsync(output.fileno())
        os.chmod(temporary, mode)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def account(name, home):
    try:
        pwd.getpwnam(name)
    except KeyError:
        run(['useradd', '--system', '--user-group', '--home-dir', str(home),
             '--no-create-home', '--shell', '/sbin/nologin', name])


def directory(path, user, mode=0o700):
    path.mkdir(parents=True, exist_ok=True)
    path.chmod(mode)
    identity = pwd.getpwnam(user)
    os.chown(path, identity.pw_uid, identity.pw_gid)


def environment_file(values):
    """Quote for systemd EnvironmentFile, including apostrophes and backslashes."""
    if not isinstance(values, dict):
        raise ValueError('Runtime secret must be a JSON object.')
    lines = []
    for key, value in values.items():
        if not isinstance(key, str) or not re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*', key):
            raise ValueError('Invalid environment variable name.')
        if not isinstance(value, str) or any(c in value for c in '\n\r\x00\ufeff'):
            raise ValueError('Runtime environment values must be single-line strings.')
        if any(0xd800 <= ord(c) <= 0xdfff or 0xfdd0 <= ord(c) <= 0xfdef or
               ord(c) & 0xffff in (0xfffe, 0xffff) for c in value):
            raise ValueError('Runtime environment contains invalid Unicode.')
        escaped = ''.join('\\' + c if c in '\\"`$' else c for c in value)
        lines.append(key + '="' + escaped + '"\n')
    return ''.join(lines)


def verify_release(path, revision, manifest):
    if manifest.get('revision') != revision or not isinstance(manifest.get('files'), dict):
        raise ValueError('Bundle revision or file manifest mismatch.')
    actual = set()
    for item in path.rglob('*'):
        if item.is_symlink() or not (item.is_dir() or item.is_file()):
            raise ValueError('Release contains a non-regular filesystem entry.')
        if item.is_file() and item.relative_to(path).as_posix() != 'build.json':
            actual.add(item.relative_to(path).as_posix())
    if actual != set(manifest['files']):
        raise ValueError('Release file inventory differs from the verified bundle.')
    for name, digest in manifest['files'].items():
        relative = pathlib.PurePosixPath(name)
        if relative.is_absolute() or '..' in relative.parts:
            raise ValueError('Unsafe bundle file path.')
        if hashlib.sha256((path / name).read_bytes()).hexdigest() != digest:
            raise ValueError('Release file checksum mismatch: ' + name)


def prepare_release(archive, expected_sha, revision, root=ROOT):
    if not re.fullmatch(r'[a-f0-9]{40}', revision):
        raise ValueError('Revision must be the exact forty-character commit.')
    if hashlib.sha256(archive.read_bytes()).hexdigest() != expected_sha:
        raise ValueError('Archive checksum mismatch; nothing installed.')
    releases = root / 'releases'
    releases.mkdir(parents=True, exist_ok=True)
    root.chmod(0o755)
    releases.chmod(0o755)
    release = releases / revision
    with tarfile.open(archive) as bundle:
        member = bundle.getmember('build.json')
        if not member.isfile() or member.size > 4 * 1024 * 1024:
            raise ValueError('Bundle manifest must be a regular, bounded file.')
        manifest_bytes = bundle.extractfile(member).read()
        manifest = json.loads(manifest_bytes)
        if release.exists():
            if release.is_symlink() or (release / 'build.json').read_bytes() != manifest_bytes:
                raise ValueError('Existing release differs from the verified bundle.')
            verify_release(release, revision, manifest)
        else:
            stage = pathlib.Path(tempfile.mkdtemp(prefix=revision + '.staging-', dir=releases))
            try:
                # Python 3.9.17+ includes the safe extraction filter backport.
                bundle.extractall(stage, filter='data')
                verify_release(stage, revision, manifest)
                # data_filter intentionally omits directory modes; normalize the verified
                # bundle so a restrictive caller umask cannot hide service files.
                stage.chmod(0o755)
                for directory_path in stage.rglob('*'):
                    if directory_path.is_dir():
                        directory_path.chmod(0o755)
                for entry in bundle.getmembers():
                    target = stage / entry.name
                    if target.is_dir():
                        target.chmod(0o755)
                    elif target.is_file():
                        target.chmod(0o755 if entry.mode & 0o111 else 0o644)
                stage.rename(release)
            finally:
                if stage.exists():
                    shutil.rmtree(stage)
    return release


def point_current(current, destination):
    temporary = current.with_name('current.new')
    if temporary.is_symlink():
        temporary.unlink()
    temporary.symlink_to(destination)
    temporary.replace(current)


class Cutover:
    """Keep original configs in memory; never serialize credentials to a rollback log."""
    def __init__(self, current, paths, services):
        self.current = current
        if current.exists() and not current.is_symlink():
            raise ValueError('Current release must be a managed symlink.')
        self.previous = current.resolve(strict=True) if current.is_symlink() else None
        self.files = {}
        for path in paths:
            if path.is_symlink():
                raise ValueError('Managed configuration cannot be a symlink.')
            info = path.stat() if path.exists() else None
            self.files[path] = (path.read_bytes(), stat.S_IMODE(info.st_mode), info.st_uid, info.st_gid) if info else None
        self.services = {}
        for service in services:
            enabled = subprocess.run(['systemctl', 'is-enabled', service], capture_output=True, text=True).stdout.strip()
            if enabled.startswith('masked'):
                raise ValueError('A deployment service is masked; resolve its policy first.')
            active = subprocess.run(['systemctl', 'is-active', '--quiet', service]).returncode == 0
            self.services[service] = (enabled, active)

    def rollback(self):
        failures = []
        def attempt(action):
            try:
                action()
            except Exception:
                failures.append(True)
        # Stop candidate processes and remove candidate enable links before restoring unit files.
        for service, (enabled, _) in self.services.items():
            if subprocess.run(['systemctl', 'is-active', '--quiet', service]).returncode == 0:
                attempt(lambda s=service: run(['systemctl', 'stop', s], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
            if enabled not in ('enabled', 'enabled-runtime'):
                subprocess.run(['systemctl', 'disable', service], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for path, original in self.files.items():
            if original is None:
                attempt(lambda p=path: p.unlink(missing_ok=True))
            else:
                content, mode, uid, gid = original
                attempt(lambda p=path, c=content, m=mode: atomic(p, c, m))
                attempt(lambda p=path, u=uid, g=gid: os.chown(p, u, g))
        if self.previous:
            attempt(lambda: point_current(self.current, self.previous))
        else:
            attempt(lambda: self.current.unlink(missing_ok=True))
        attempt(lambda: run(['systemctl', 'daemon-reload']))
        for service, (enabled, active) in self.services.items():
            if enabled in ('enabled', 'enabled-runtime'):
                command = ['systemctl', 'enable'] + (['--runtime'] if enabled == 'enabled-runtime' else []) + [service]
                attempt(lambda c=command: run(c, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
            if active:
                attempt(lambda s=service: run(['systemctl', 'start', s]))
        if failures:
            raise RuntimeError('Rollback encountered a service or filesystem error; inspect local service status.')


def wait_ready(service, role, timeout=60):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            if role == 'api':
                with opener.open('http://127.0.0.1:4310/health', timeout=2) as response:
                    ready = json.load(response).get('status') == 'ok'
            else:
                request = urllib.request.Request('http://127.0.0.1:4312/validate', data=b'{}')
                try:
                    opener.open(request, timeout=2).close()
                    ready = False
                except urllib.error.HTTPError as response:
                    ready = response.code == 401 and json.load(response).get('error') == 'Invalid runner credential.'
            if ready and subprocess.run(['systemctl', 'is-active', '--quiet', service]).returncode == 0:
                return
        except (OSError, ValueError):
            pass
        time.sleep(1)
    raise RuntimeError('Local service readiness timed out; no successful cutover recorded.')


def main():
    parser = argparse.ArgumentParser()
    for name in ('archive', 'sha256', 'revision', 'secret', 'bucket'):
        parser.add_argument('--' + name, required=True)
    parser.add_argument('--role', choices=['api', 'worker'], required=True)
    parser.add_argument('--region', default='us-east-2')
    args = parser.parse_args()
    if os.geteuid() != 0:
        raise SystemExit('Run as root on the designated Apps host.')
    if not re.fullmatch(r'[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]', args.bucket) or not re.fullmatch(r'[a-z]{2}-[a-z]+-\d+', args.region):
        raise ValueError('Invalid backup bucket or AWS region.')
    secret = json.loads(json.loads(capture(['aws', '--region', args.region, 'secretsmanager',
        'get-secret-value', '--secret-id', args.secret, '--output', 'json']))['SecretString'])
    environment_file(secret)  # Validate all values before changing a live service.
    token = secret.get('APPS_RUNNER_TOKEN', '')
    if len(token) < 32 or not token.isascii() or any(c.isspace() for c in token):
        raise ValueError('Production runner token must contain at least 32 ASCII non-space characters.')
    if args.role == 'api':
        required = ('APPS_ACCOUNTS_APP_SECRET', 'APPS_ACCOUNTS_SERVICE_TOKEN', 'APPS_RUNNER_URL')
        if any(not secret.get(key) for key in required) or secret.get('APPS_DEV_AUTH') != '0':
            raise ValueError('Production integrations are required and APPS_DEV_AUTH must be 0.')
    release = prepare_release(pathlib.Path(args.archive), args.sha256, args.revision)
    directory(ETC / 'silicon-apps', 'root', 0o700)
    files = {}
    if args.role == 'worker':
        name = 'silicon-apps-runner'
        account(name, STATE / name)
        directory(STATE / name, name)
        directory(STATE / name / 'jobs', name)
        run(['usermod', '-a', '-G', 'docker', name])
        run(['systemctl', 'enable', '--now', 'docker'])
        # Dependencies are revision-scoped so preparing an update cannot change the running worker.
        venv = ROOT / 'venvs' / args.revision
        venv.parent.mkdir(exist_ok=True)
        venv.parent.chmod(0o755)
        requirements = release / 'runner/requirements.txt'
        dependency_sha = hashlib.sha256(requirements.read_bytes()).hexdigest()
        if venv.exists():
            if not (venv / '.ready').is_file() or (venv / '.ready').read_text() != dependency_sha:
                raise ValueError('Existing worker environment is incomplete; remove that unused revision environment before retrying.')
        else:
            try:
                run(['python3', '-m', 'venv', str(venv)])
                venv.chmod(0o755)
                run([str(venv / 'bin/pip'), 'install', '--disable-pip-version-check', '-r', str(requirements)], stdout=subprocess.DEVNULL)
                # A restrictive caller umask must not make dependencies root-only.
                for path in venv.rglob('*'):
                    if not path.is_symlink():
                        path.chmod(0o755 if path.is_dir() or path.stat().st_mode & 0o111 else 0o644)
                atomic(venv / '.ready', dependency_sha, 0o644)
            except BaseException:
                shutil.rmtree(venv, ignore_errors=True)
                raise
        image = 'ubuntu:24.04'
        run(['docker', 'pull', image], stdout=subprocess.DEVNULL)
        digest = json.loads(capture(['docker', 'image', 'inspect', image]))[0]['RepoDigests'][0]
        env = {'APPS_RUNNER_TOKEN': token, 'APPS_RUNNER_HOST': '0.0.0.0', 'APPS_RUNNER_PORT': '4312',
               'TMPDIR': str(STATE / name / 'jobs'), 'APPS_RUNNER_IMAGES': json.dumps({'linux-x86_64': digest}, separators=(',', ':'))}
        unit = '''[Unit]\nDescription=Silicon Apps isolated Linux validator\nAfter=network-online.target docker.service\nRequires=docker.service\n[Service]\nUser=silicon-apps-runner\nGroup=silicon-apps-runner\nSupplementaryGroups=docker\nWorkingDirectory=/opt/silicon-apps/current\nEnvironmentFile=/etc/silicon-apps/worker.env\nExecStart=VENVPYTHON /opt/silicon-apps/current/runner/server.py\nRestart=on-failure\nRestartSec=3\nUMask=0077\nNoNewPrivileges=true\nProtectHome=true\nProtectSystem=strict\nReadWritePaths=/var/lib/silicon-apps-runner\n[Install]\nWantedBy=multi-user.target\n'''.replace('VENVPYTHON', str(venv / 'bin/python'))
        service = 'silicon-apps-runner'
        services = [service]
        files[ETC / 'systemd/system/silicon-apps-runner.service'] = (unit, 0o644)
        files[ETC / 'silicon-apps/worker-image.json'] = (json.dumps({'image': digest}), 0o600)
    else:
        name = 'silicon-apps'
        account(name, STATE / name)
        directory(STATE / name, name)
        account('caddy', STATE / 'caddy')
        directory(STATE / 'caddy', 'caddy')
        directory(ETC / 'caddy', 'root', 0o755)
        # Validate the candidate without replacing the live configuration.
        run([str(release / 'bin/caddy'), 'validate', '--config', str(release / 'deploy/Caddyfile')], stdout=subprocess.DEVNULL)
        env = secret
        service = 'silicon-apps-api'
        services = [service, 'caddy', 'silicon-apps-backup.timer']
        unit = (release / 'deploy/silicon-apps-api.service').read_text().replace('/opt/silicon-apps/bin/apps-server', '/opt/silicon-apps/current/bin/apps-server')
        files[ETC / 'systemd/system/silicon-apps-api.service'] = (unit, 0o644)
        files[ETC / 'caddy/Caddyfile'] = ((release / 'deploy/Caddyfile').read_text(), 0o644)
        caddy = '''[Unit]\nDescription=Silicon Apps HTTPS\nAfter=network-online.target\n[Service]\nUser=caddy\nGroup=caddy\nEnvironment=HOME=/var/lib/caddy\nExecStart=/opt/silicon-apps/current/bin/caddy run --config /etc/caddy/Caddyfile\nExecReload=/opt/silicon-apps/current/bin/caddy reload --config /etc/caddy/Caddyfile\nRestart=on-failure\nAmbientCapabilities=CAP_NET_BIND_SERVICE\nCapabilityBoundingSet=CAP_NET_BIND_SERVICE\nNoNewPrivileges=true\nProtectSystem=strict\nProtectHome=true\nReadWritePaths=/var/lib/caddy\n[Install]\nWantedBy=multi-user.target\n'''
        files[ETC / 'systemd/system/caddy.service'] = (caddy, 0o644)
        files[ETC / 'systemd/system/silicon-apps-backup.service'] = ('[Unit]\nDescription=Silicon Apps consistent catalog backup\n[Service]\nType=oneshot\nUser=silicon-apps\nUMask=0077\nExecStart=/usr/bin/python3 /opt/silicon-apps/current/deploy/backup.py --bucket ' + args.bucket + ' --region ' + args.region + '\n', 0o644)
        files[ETC / 'systemd/system/silicon-apps-backup.timer'] = ('[Unit]\nDescription=Hourly Silicon Apps backups\n[Timer]\nOnCalendar=hourly\nPersistent=true\nRandomizedDelaySec=120\n[Install]\nWantedBy=timers.target\n', 0o644)
    files[ETC / 'silicon-apps' / (args.role + '.env')] = (environment_file(env), 0o600)
    current = ROOT / 'current'
    transaction = Cutover(current, [*files, ROOT / 'previous-release', ROOT / 'deployment.json'], services)
    try:
        if args.role == 'api' and transaction.previous and (STATE / 'silicon-apps/apps.sqlite').exists():
            run(['systemctl', 'stop', service])
            run(['python3', str(release / 'deploy/backup.py'), '--bucket', args.bucket, '--region', args.region])
        for path, (content, mode) in files.items():
            atomic(path, content, mode)
        if transaction.previous:
            atomic(ROOT / 'previous-release', str(transaction.previous) + '\n')
        point_current(current, release)
        run(['systemctl', 'daemon-reload'])
        run(['systemctl', 'enable', service], stdout=subprocess.DEVNULL)
        run(['systemctl', 'restart', service])
        wait_ready(service, args.role)
        if args.role == 'api':
            # Restart to use the new Caddy binary as well as its configuration.
            run(['systemctl', 'enable', 'caddy', 'silicon-apps-backup.timer'], stdout=subprocess.DEVNULL)
            run(['systemctl', 'restart', 'caddy', 'silicon-apps-backup.timer'])
            run(['systemctl', 'is-active', '--quiet', 'caddy', 'silicon-apps-backup.timer'])
        atomic(ROOT / 'deployment.json', json.dumps({'revision': args.revision, 'sha256': args.sha256,
            'role': args.role, 'installed_at': time.time()}), 0o644)
    except BaseException:
        transaction.rollback()
        raise
    print(json.dumps({'role': args.role, 'revision': args.revision, 'service': 'active',
        'previous_release': str(transaction.previous) if transaction.previous else None}))


if __name__ == '__main__':
    main()
