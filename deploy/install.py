#!/usr/bin/env python3
"""Install a verified Apps release, restoring files and services on failed cutover.

Runs on Amazon Linux 2023 (Python 3.9): keep the syntax 3.9-compatible.
"""
import argparse
import base64
import binascii
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
API_URL = 'http://127.0.0.1:4310'
STORE_USER = 'silicon-apps-store'
STORE_SERVICE = 'silicon-apps-store'
STORE_PORT = 4320
# The candidate store answers here before any live change; nothing else uses this port.
PREFLIGHT_PORT = 4321
PREFLIGHT_UNIT = 'silicon-apps-store-preflight'
STORE_PUBLIC_URL = 'https://apps.teamofsilicons.com'
# Text the server-rendered home page always contains, even when the API is unreachable.
STORE_READY_TEXT = ('<main', 'Silicon Apps')
# Mirrors PINNED_KEYS in crates/client/src/signing.rs (a test keeps them equal). The CLI
# trusts only these keys and keys they endorse, so a wrong seed here breaks every install.
PINNED_SIGNING_KEYS = {'apps-2026-10': '5WC06dtS61w+mPv3E0xNVz5ZoNQNMpX5/8YutuLZ3DU='}

# Linux validation images, one immutable per-platform manifest per target. All four come from
# the official python:3.14-slim-trixie multi-platform index the worker has pinned for
# linux-x86_64 since its first install (WORKER_IMAGE_INDEX, Python 3.14.8), resolved through
# the registry API on 2026-10-09; linux-x86_64 is that index's linux/amd64 manifest, so its bytes
# are unchanged. The minimal Ubuntu image has no CA store, and even an offline Rust TLS client
# constructor can need one; this runtime includes CA certificates and a glibc compatible with
# Ubuntu 24.04 release builds. Platforms are the runner's PLATFORMS (a test keeps them equal).
WORKER_IMAGE_INDEX = 'python@sha256:f85c5697265c178cc6887276c55fe16cf3d14ca35c3df6a5eab3b360534a55d2'
WORKER_IMAGES = {
    'linux-x86_64': ('linux/amd64', 'python@sha256:cfe2e24a75302a15934d37c2d86412893c0aa934dc3a97cbb439d04c01890ca9'),
    'linux-i686': ('linux/386', 'python@sha256:952db3a8cdafdd8c635a18f1a30e4b6a0cfb5f3e9737741e634c25f6699a908f'),
    'linux-aarch64': ('linux/arm64', 'python@sha256:7f47c8ffd4e70e88f7326b68e7df95109597ef66023a1052b7d1ec18390d2604'),
    'linux-armv7hf': ('linux/arm/v7', 'python@sha256:9d2a631d044769963ee27e56ed1d2de1adf64f108529e3d2de7d829cb2967806'),
}
# What `uname -m` and the pointer width must report inside each image on the x86_64 worker.
# linux-i686 runs natively as a 32-bit process, which may see the 64-bit kernel's machine name;
# qemu-arm reports armv7l, or armv8l for a CPU model that runs 32-bit code on ARMv8.
WORKER_SELF_CHECK = {
    'linux-x86_64': (('x86_64',), 64),
    'linux-i686': (('i386', 'i486', 'i586', 'i686', 'x86_64'), 32),
    'linux-aarch64': (('aarch64',), 64),
    'linux-armv7hf': (('armv7l', 'armv8l'), 32),
}
SELF_CHECK_CODE = "import os,struct;print(os.uname().machine,struct.calcsize('P')*8)"
# QEMU user-mode emulation for the two ARM targets only, registered by the official
# tonistiigi/binfmt image (tag qemu-v10.2.3-68, source e29e7d72c967): the linux/amd64 manifest
# of index sha256:400a4873b838d1b89194d982c45e5fb3cda4593fbfd7e08a02e76b03b21166f0. It
# registers with the F (fix binary) flag, so the kernel opens the interpreter once and every
# container can use it without the emulator in its image. It exits 0 even when a registration
# fails, so the entries are checked after every run.
BINFMT_IMAGE = 'tonistiigi/binfmt@sha256:465d3fdd28d0f2b871ba4b4ec98bd183292e96167f00d9fd40bd249f8632d705'
BINFMT_PLATFORM = 'linux/amd64'
EMULATORS = ('qemu-aarch64', 'qemu-arm')  # binfmt_misc entries for linux/arm64 and linux/arm/v7
BINFMT_MISC = pathlib.Path('/proc/sys/fs/binfmt_misc')
BINFMT_SERVICE = 'silicon-apps-binfmt'
RUNNER_SERVICE = 'silicon-apps-runner'

# Ed25519 public key derivation (RFC 8032, section 5.1.5), so the seed in the runtime
# secret can be checked against the pinned public key without third-party modules.
_P = 2 ** 255 - 19
_D = -121665 * pow(121666, _P - 2, _P) % _P
_SQRT_M1 = pow(2, (_P - 1) // 4, _P)


def _point_add(a, b):
    p, q = (a[1] - a[0]) * (b[1] - b[0]) % _P, (a[1] + a[0]) * (b[1] + b[0]) % _P
    r, s = 2 * a[3] * b[3] * _D % _P, 2 * a[2] * b[2] % _P
    e, f, g, h = q - p, s - r, s + r, q + p
    return (e * f % _P, g * h % _P, f * g % _P, e * h % _P)


def _base_point():
    y = 4 * pow(5, _P - 2, _P) % _P
    x2 = (y * y - 1) * pow(_D * y * y + 1, _P - 2, _P)
    x = pow(x2, (_P + 3) // 8, _P)
    if (x * x - x2) % _P:
        x = x * _SQRT_M1 % _P
    if x & 1:
        x = _P - x
    return (x, y, 1, x * y % _P)


def ed25519_public_key(seed):
    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], 'little')
    scalar = (scalar & ((1 << 254) - 8)) | (1 << 254)
    result, point = (0, 1, 1, 0), _base_point()
    while scalar:
        if scalar & 1:
            result = _point_add(result, point)
        point = _point_add(point, point)
        scalar >>= 1
    inverse = pow(result[2], _P - 2, _P)
    x, y = result[0] * inverse % _P, result[1] * inverse % _P
    return base64.b64encode((y | ((x & 1) << 255)).to_bytes(32, 'little')).decode()


def check_signing_keys(secret):
    """Fail before any change unless APPS_SIGNING_KEYS is usable. Never echo key material."""
    raw = secret.get('APPS_SIGNING_KEYS') or ''
    if not raw.strip():
        raise ValueError('The runtime secret has no APPS_SIGNING_KEYS. The API refuses to start without it: add the '
                         'production release signing key (key_id:base64-seed, newest first) to the secret and retry. '
                         'Nothing was changed.')
    ids = []
    for index, entry in enumerate([e.strip() for e in re.split(r'[,\n]', raw) if e.strip()], 1):
        key_id, separator, seed = entry.partition(':')
        if not separator or not re.fullmatch(r'[A-Za-z0-9._-]{1,64}', key_id):
            raise ValueError('APPS_SIGNING_KEYS entry %d is not key_id:base64-seed.' % index)
        try:
            seed_bytes = base64.b64decode(seed.strip(), validate=True)
        except (binascii.Error, ValueError):
            seed_bytes = b''
        if len(seed_bytes) != 32 or base64.b64encode(seed_bytes).decode() != seed.strip():
            raise ValueError('APPS_SIGNING_KEYS key %s is not a base64 32-byte Ed25519 seed.' % key_id)
        if key_id in ids:
            raise ValueError('APPS_SIGNING_KEYS lists key %s twice.' % key_id)
        pinned = PINNED_SIGNING_KEYS.get(key_id)
        if pinned and ed25519_public_key(seed_bytes) != pinned:
            raise ValueError('APPS_SIGNING_KEYS key %s does not match the public key pinned in the CLI; every install '
                             'would fail verification. Use the production key from the release key file.' % key_id)
        ids.append(key_id)
    revoked = [v.strip() for v in (secret.get('APPS_REVOKED_SIGNING_KEYS') or '').split(',') if v.strip()]
    if ids[0] in revoked:
        raise ValueError('The active signing key %s is listed in APPS_REVOKED_SIGNING_KEYS.' % ids[0])
    if not any(key_id in PINNED_SIGNING_KEYS for key_id in ids):
        raise ValueError('APPS_SIGNING_KEYS holds no key pinned in the CLI (%s); the CLI could not trust its '
                         'signatures.' % ', '.join(sorted(PINNED_SIGNING_KEYS)))
    return ids[0]


def check_api_secret(secret):
    """Validate the API runtime secret before anything is installed; returns the active signing key ID."""
    required = ('APPS_ACCOUNTS_APP_SECRET', 'APPS_ACCOUNTS_SERVICE_TOKEN', 'APPS_RUNNER_URL')
    if any(not secret.get(key) for key in required) or secret.get('APPS_DEV_AUTH') != '0':
        raise ValueError('Production integrations are required and APPS_DEV_AUTH must be 0.')
    origins = [o.strip().rstrip('/') for o in (secret.get('APPS_ALLOWED_ORIGINS') or '').split(',')]
    if STORE_PUBLIC_URL not in origins:
        raise ValueError('APPS_ALLOWED_ORIGINS must include ' + STORE_PUBLIC_URL + ': the store sends that Origin '
                         'with every visitor write.')
    return check_signing_keys(secret)


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


def file_sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def walk(path):
    """Yield (entry, relative name) for every entry below path without following symlinks."""
    for current, directories, files in os.walk(path):
        for name in directories + files:
            item = pathlib.Path(current) / name
            yield item, item.relative_to(path).as_posix()


def verify_release(path, revision, manifest):
    """Regular files match their hashes; symlinks (the store's pnpm links) match their recorded,
    relative targets and resolve inside the release. Nothing else may exist."""
    links = manifest.get('symlinks', {})
    if manifest.get('revision') != revision or not isinstance(manifest.get('files'), dict) or not isinstance(links, dict):
        raise ValueError('Bundle revision or file manifest mismatch.')
    for name in [*manifest['files'], *links]:
        relative = pathlib.PurePosixPath(name)
        if relative.is_absolute() or '..' in relative.parts:
            raise ValueError('Unsafe bundle file path.')
    files, actual_links = set(), {}
    for item, name in walk(path):
        if item.is_symlink():
            actual_links[name] = os.readlink(item)
        elif item.is_file():
            if name != 'build.json':
                files.add(name)
        elif not item.is_dir():
            raise ValueError('Release contains a non-regular filesystem entry.')
    if files != set(manifest['files']) or actual_links != links:
        raise ValueError('Release file inventory differs from the verified bundle.')
    top = path.resolve()
    for name, target in links.items():
        try:
            resolved = (path / name).resolve(strict=True)
        except (OSError, RuntimeError):
            raise ValueError('Release symlink is broken: ' + name)
        if os.path.isabs(target) or (resolved != top and top not in resolved.parents):
            raise ValueError('Release symlink leaves the release: ' + name)
    for name, digest in manifest['files'].items():
        if file_sha256(path / name) != digest:
            raise ValueError('Release file checksum mismatch: ' + name)


def prepare_release(archive, expected_sha, revision, root=ROOT):
    if not re.fullmatch(r'[a-f0-9]{40}', revision):
        raise ValueError('Revision must be the exact forty-character commit.')
    if file_sha256(archive) != expected_sha:
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
                # bundle so a restrictive caller umask cannot hide service files. chmod
                # follows symlinks, so links are skipped (their targets are entries too).
                stage.chmod(0o755)
                for directory_path, _ in walk(stage):
                    if directory_path.is_dir() and not directory_path.is_symlink():
                        directory_path.chmod(0o755)
                for entry in bundle.getmembers():
                    target = stage / entry.name
                    if entry.issym() or entry.islnk() or target.is_symlink():
                        continue
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


def store_renders(opener, port):
    """GET / answers 200 with server-rendered HTML (the page text, not an empty client shell)."""
    request = urllib.request.Request('http://127.0.0.1:%d/' % port, headers={
        # A monitor user agent, so the store does not count the check as a page view.
        'Accept': 'text/html', 'User-Agent': 'silicon-apps-install-monitor (python)'})
    with opener.open(request, timeout=10) as response:
        body = response.read(4 * 1024 * 1024).decode('utf-8', 'replace')
        return (response.status == 200 and response.headers.get_content_type() == 'text/html'
                and all(text in body for text in STORE_READY_TEXT))


def wait_ready(service, role, timeout=60):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            if role == 'api':
                with opener.open(API_URL + '/health', timeout=2) as response:
                    ready = json.load(response).get('status') == 'ok'
            elif role == 'store':
                ready = store_renders(opener, STORE_PORT)
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
    raise RuntimeError(service + ' readiness timed out; no successful cutover recorded.')


def unit_properties(unit, release):
    """The [Service] settings of a reviewed unit as systemd-run properties, pointed at one release."""
    properties, section = [], None
    for line in unit.splitlines():
        line = line.strip()
        if line.startswith('[') and line.endswith(']'):
            section = line
        elif section == '[Service]' and '=' in line and not line.startswith('#'):
            key, value = line.split('=', 1)
            if key in ('ExecStart', 'Restart', 'RestartSec'):
                continue
            properties += ['-p', key + '=' + value.replace('/opt/silicon-apps/current', str(release))]
    return properties


def preflight_store(release, timeout=90):
    """Run the candidate store from its release directory, with the store unit's user and
    sandbox, on a spare port against the running API. It must render before anything live changes."""
    unit = (release / 'deploy' / (STORE_SERVICE + '.service')).read_text()
    subprocess.run(['systemctl', 'stop', PREFLIGHT_UNIT], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    subprocess.run(['systemctl', 'reset-failed', PREFLIGHT_UNIT], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    run(['systemd-run', '--quiet', '--collect', '--unit', PREFLIGHT_UNIT, '--description',
         'Silicon Apps candidate store check', *unit_properties(unit, release),
         '-p', 'Environment=PORT=%d' % PREFLIGHT_PORT, '--',
         str(release / 'node/bin/node'), str(release / 'store/server.js')], stdout=subprocess.DEVNULL)
    try:
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if subprocess.run(['systemctl', 'is-active', '--quiet', PREFLIGHT_UNIT]).returncode != 0:
                raise RuntimeError('The candidate store exited before it rendered; see journalctl -u '
                                   + PREFLIGHT_UNIT + '. Nothing was changed.')
            try:
                if store_renders(opener, PREFLIGHT_PORT):
                    return
            except (OSError, ValueError):
                pass
            time.sleep(1)
        raise RuntimeError('The candidate store did not render GET / in time; see journalctl -u '
                           + PREFLIGHT_UNIT + '. Nothing was changed.')
    finally:
        subprocess.run(['systemctl', 'stop', PREFLIGHT_UNIT], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def verify_signing(active_key_id):
    """The started API publishes the configured active key and the pinned keys unchanged."""
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(API_URL + '/.well-known/silicon-apps-keys.json', timeout=10) as response:
        document = json.load(response)
    published = {key.get('key_id'): key.get('public_key') for key in document.get('keys', [])}
    if document.get('active_key_id') != active_key_id or active_key_id not in published:
        raise RuntimeError('The API does not publish the configured active signing key.')
    for key_id, public_key in PINNED_SIGNING_KEYS.items():
        if key_id in published and published[key_id] != public_key:
            raise RuntimeError('The API publishes signing key ' + key_id + ' with a different public key than the CLI pins.')


def pull_worker_images():
    """Pull every pinned image for its own platform; the runner and the boot unit use --pull=never."""
    for platform, image in [*WORKER_IMAGES.values(), (BINFMT_PLATFORM, BINFMT_IMAGE)]:
        run(['docker', 'pull', '--platform', platform, image], stdout=subprocess.DEVNULL, timeout=900)


def binfmt_command(docker='docker'):
    """Replace only our two entries, then register arm64 and arm (nothing else) from the pinned image."""
    return [docker, 'run', '--rm', '--pull=never', '--privileged', '--network=none', '--platform', BINFMT_PLATFORM,
            BINFMT_IMAGE, '--uninstall', ','.join(EMULATORS), '--install', 'arm64,arm']


def emulator_registered(name):
    """The binfmt_misc entry is enabled and carries the F flag (for example `flags: POCF`)."""
    try:
        lines = (BINFMT_MISC / name).read_text().splitlines()
    except OSError:
        return False
    flags = [line.split(':', 1)[1].strip() for line in lines if line.startswith('flags:')]
    return bool(lines) and lines[0].strip() == 'enabled' and bool(flags) and 'F' in flags[0]


def register_emulators(attempts=10):
    """Register QEMU for linux-aarch64 and linux-armv7hf now, as the boot unit does at every boot.
    Entries that are already enabled with the F flag may be serving a live worker, so they are
    left alone; the cutover registers again through the unit while the worker is stopped.
    Returns whether the binfmt image ran."""
    if all(emulator_registered(name) for name in EMULATORS):
        return False
    try:
        result = subprocess.run(binfmt_command(), capture_output=True, text=True, timeout=120)
    except subprocess.TimeoutExpired:
        raise RuntimeError('The pinned binfmt image did not finish within 120 seconds; the worker was not changed.')
    # Some kernels expose a new entry only after a short delay.
    for attempt in range(attempts):
        missing = [name for name in EMULATORS if not emulator_registered(name)]
        if not missing:
            return True
        if attempt + 1 < attempts:
            time.sleep(1)
    log = ' '.join(line.strip() for line in (result.stderr or '').splitlines() if line.strip())[-600:]
    raise RuntimeError('QEMU emulation is not registered with the F flag for %s (docker exit %d; binfmt said: %s). '
                       'Check %s; the worker was not changed.' % (', '.join(missing), result.returncode,
                                                                  log or 'nothing', BINFMT_MISC))


def binfmt_unit():
    """Oneshot unit that re-registers ARM emulation at boot, before the worker starts. Registrations
    are kernel state that a reboot clears. `$$` is systemd's escape for a literal dollar sign."""
    check = ('for n in %s; do i=0; until head -n 1 %s/$$n 2>/dev/null | grep -qx enabled && '
             'grep -Eq "^flags: [A-Z]*F" %s/$$n; do i=$$((i+1)); if [ $$i -ge 10 ]; then '
             'echo "$$n is not registered with the F flag" >&2; exit 1; fi; sleep 1; done; done'
             % (' '.join(EMULATORS), BINFMT_MISC, BINFMT_MISC))
    return ('[Unit]\nDescription=Silicon Apps QEMU emulation for linux-aarch64 and linux-armv7hf validation\n'
            'After=docker.service\nRequires=docker.service\nBefore=' + RUNNER_SERVICE + '.service\n'
            '[Service]\nType=oneshot\nRemainAfterExit=yes\nTimeoutStartSec=180\n'
            'ExecStart=' + ' '.join(binfmt_command('/usr/bin/docker')) + '\n'
            "ExecStartPost=/bin/sh -c '" + check + "'\n"
            '[Install]\nWantedBy=multi-user.target\n')


def self_check_command(target):
    """The runner's container isolation, with python3 in the image instead of a package binary."""
    platform, image = WORKER_IMAGES[target]
    return ['docker', 'run', '--rm', '--pull=never', '--platform', platform, '--network=none', '--read-only',
            '--cap-drop=ALL', '--security-opt=no-new-privileges', '--pids-limit=32', '--memory=256m', '--cpus=1',
            '--user=65534:65534', '--tmpfs=/tmp:rw,noexec,nosuid,size=16m', '--entrypoint', '/usr/local/bin/python3',
            image, '-I', '-c', SELF_CHECK_CODE]


SELF_CHECK_HINTS = {
    'linux-i686': 'linux-i686 runs natively and needs a kernel with 32-bit x86 support (ia32_emulation)',
    'linux-aarch64': 'linux-aarch64 needs the qemu-aarch64 binfmt_misc entry with the F flag',
    'linux-armv7hf': 'linux-armv7hf needs the qemu-arm binfmt_misc entry with the F flag',
}


def self_check():
    """Every pinned image starts under its platform and reports the expected machine and word
    size. Returns {target: 'machine bits'}; raises once, naming every target that does not run."""
    results, failures = {}, []
    for target in WORKER_IMAGES:
        machines, bits = WORKER_SELF_CHECK[target]
        platform = WORKER_IMAGES[target][0]
        try:
            result = subprocess.run(self_check_command(target), capture_output=True, text=True, timeout=120)
        except subprocess.TimeoutExpired:
            failures.append('%s (%s) did not finish within 120 seconds' % (target, platform))
            continue
        lines = (result.stdout or '').strip().splitlines()
        words = lines[-1].split() if lines else []
        if result.returncode == 0 and len(words) == 2 and words[0] in machines and words[1] == str(bits):
            results[target] = ' '.join(words)
            continue
        errors = [line.strip() for line in (result.stderr or '').splitlines() if line.strip()]
        failures.append('%s (%s) exited %d and reported %r, expected %s and %d-bit (%s)' % (
            target, platform, result.returncode, ' '.join(words), ' or '.join(machines), bits,
            errors[-1][:300] if errors else 'no error output'))
    if failures:
        hints = [SELF_CHECK_HINTS[t] for t in WORKER_IMAGES if t in SELF_CHECK_HINTS and t not in results]
        raise RuntimeError('Worker platform self-check failed: ' + '; '.join(failures) + '. '
                           + ('; '.join(hints) + '.' if hints else ''))
    return results


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
    # Includes APPS_SIGNING_KEYS: the API refuses to start without it, so fail before any change.
    active_key = check_api_secret(secret) if args.role == 'api' else None
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
        # Pull the four pinned images and the emulator image, register ARM emulation where it is
        # missing, and prove every platform runs before anything live changes. Registration only
        # touches the two binfmt_misc entries this worker owns.
        pull_worker_images()
        register_emulators()
        self_check()
        images = {target: image for target, (_, image) in WORKER_IMAGES.items()}
        env = {'APPS_RUNNER_TOKEN': token, 'APPS_RUNNER_HOST': '0.0.0.0', 'APPS_RUNNER_PORT': '4312',
               'TMPDIR': str(STATE / name / 'jobs'), 'APPS_RUNNER_IMAGES': json.dumps(images, separators=(',', ':'))}
        unit = '''[Unit]\nDescription=Silicon Apps isolated Linux validator\nAfter=network-online.target docker.service BINFMT.service\nRequires=docker.service BINFMT.service\n[Service]\nUser=silicon-apps-runner\nGroup=silicon-apps-runner\nSupplementaryGroups=docker\nWorkingDirectory=/opt/silicon-apps/current\nEnvironmentFile=/etc/silicon-apps/worker.env\nExecStart=VENVPYTHON /opt/silicon-apps/current/runner/server.py\nRestart=on-failure\nRestartSec=3\nUMask=0077\nNoNewPrivileges=true\nProtectHome=true\nProtectSystem=strict\nReadWritePaths=/var/lib/silicon-apps-runner\n[Install]\nWantedBy=multi-user.target\n'''.replace('VENVPYTHON', str(venv / 'bin/python')).replace('BINFMT', BINFMT_SERVICE)
        service = RUNNER_SERVICE
        services = [service, BINFMT_SERVICE]
        files[ETC / 'systemd/system/silicon-apps-runner.service'] = (unit, 0o644)
        files[ETC / 'systemd/system' / (BINFMT_SERVICE + '.service')] = (binfmt_unit(), 0o644)
        files[ETC / 'silicon-apps/worker-image.json'] = (json.dumps({'image': images['linux-x86_64'], 'images': images,
                                                                     'index': WORKER_IMAGE_INDEX, 'emulator': BINFMT_IMAGE}), 0o600)
    else:
        name = 'silicon-apps'
        account(name, STATE / name)
        directory(STATE / name, name)
        account('caddy', STATE / 'caddy')
        directory(STATE / 'caddy', 'caddy')
        directory(ETC / 'caddy', 'root', 0o755)
        # The store runs as its own user with no state and no access to the API's secrets.
        account(STORE_USER, STATE / STORE_USER)
        # Validate the candidate without replacing the live configuration.
        run([str(release / 'bin/caddy'), 'validate', '--config', str(release / 'deploy/Caddyfile')], stdout=subprocess.DEVNULL)
        preflight_store(release)
        env = secret
        service = 'silicon-apps-api'
        services = [service, STORE_SERVICE, 'caddy', 'silicon-apps-backup.timer']
        unit = (release / 'deploy/silicon-apps-api.service').read_text().replace('/opt/silicon-apps/bin/apps-server', '/opt/silicon-apps/current/bin/apps-server')
        files[ETC / 'systemd/system/silicon-apps-api.service'] = (unit, 0o644)
        files[ETC / 'systemd/system' / (STORE_SERVICE + '.service')] = ((release / 'deploy' / (STORE_SERVICE + '.service')).read_text(), 0o644)
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
        if args.role == 'worker':
            # Register again from the pinned image through the boot unit itself, so the next reboot
            # runs a proven unit. The worker requires the unit, so systemd stops it meanwhile.
            run(['systemctl', 'enable', BINFMT_SERVICE], stdout=subprocess.DEVNULL)
            run(['systemctl', 'restart', BINFMT_SERVICE])
        run(['systemctl', 'enable', service], stdout=subprocess.DEVNULL)
        run(['systemctl', 'restart', service])
        # The API listens only after reading and signing every stored package, which can take a while once.
        wait_ready(service, args.role, timeout=300 if args.role == 'api' else 60)
        if args.role == 'worker':
            # The worker is ready only once every platform runs on the registrations the unit made.
            platforms = self_check()
        if args.role == 'api':
            # The first start with a signing key signs every existing release (backed up above).
            verify_signing(active_key)
            run(['systemctl', 'enable', STORE_SERVICE], stdout=subprocess.DEVNULL)
            run(['systemctl', 'restart', STORE_SERVICE])
            wait_ready(STORE_SERVICE, 'store')
            # Restart to use the new Caddy binary as well as its configuration.
            run(['systemctl', 'enable', 'caddy', 'silicon-apps-backup.timer'], stdout=subprocess.DEVNULL)
            run(['systemctl', 'restart', 'caddy', 'silicon-apps-backup.timer'])
            run(['systemctl', 'is-active', '--quiet', 'caddy', 'silicon-apps-backup.timer'])
        atomic(ROOT / 'deployment.json', json.dumps({'revision': args.revision, 'sha256': args.sha256,
            'role': args.role, 'installed_at': time.time()}), 0o644)
    except BaseException:
        transaction.rollback()
        raise
    summary = {'role': args.role, 'revision': args.revision, 'service': 'active',
               'previous_release': str(transaction.previous) if transaction.previous else None}
    if args.role == 'worker':
        summary['platforms'] = platforms
    print(json.dumps(summary))


if __name__ == '__main__':
    main()
