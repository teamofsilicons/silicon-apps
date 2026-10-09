#!/usr/bin/env python3
"""Create an auditable native production bundle from a clean source revision.

The bundle holds the Linux ARM64 API, Caddy, Node.js, the server-rendered store
(Next.js standalone output, pnpm links preserved), the installers, the worker
and the deployment tools, with a file-by-file hash manifest in build.json.
"""
import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
CADDY_VERSION = '2.11.7'
CADDY_SHA256 = 'd8fc6d179a5d283028a472a5618564f6ad8a86fed513e64f032b3b0b7cc45e42'
NODE_VERSION = '24.21.0'
NODE_DIR = 'node-v' + NODE_VERSION + '-linux-arm64'
NODE_ARCHIVE = NODE_DIR + '.tar.xz'
NODE_URL = 'https://nodejs.org/dist/v' + NODE_VERSION + '/' + NODE_ARCHIVE
# From https://nodejs.org/dist/v24.21.0/SHASUMS256.txt
NODE_SHA256 = '6ad1325edbdb5649c379b75a237147a666c95d4f9ae8d340fef2d1575d289ad2'
INSTALLERS = ('install.sh', 'install.ps1')
# Read by the build only; the store reads its runtime settings at request time.
STORE_ENV_DROP = ('APPS_API_URL', 'STORE_PUBLIC_URL', 'STORE_DEV_API_PASSTHROUGH', 'DEVELOPERS_URL')


def git(*args):
    return subprocess.check_output(['git', '-C', str(ROOT), *args], text=True).strip()


def sha(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def node_archive(explicit, cache):
    """The official Node.js Linux ARM64 archive, reused when already downloaded, always checksum-verified."""
    candidates = [explicit] if explicit else [
        cache / NODE_ARCHIVE,
        ROOT.parent / 'silicon-accounts/.dev/production' / NODE_ARCHIVE,
    ]
    for candidate in candidates:
        if candidate and candidate.is_file():
            if sha(candidate) != NODE_SHA256:
                raise SystemExit('Node.js ' + NODE_VERSION + ' archive checksum mismatch: ' + str(candidate))
            return candidate
    if explicit:
        raise SystemExit('Node.js archive not found: ' + str(explicit))
    cache.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix='.' + NODE_ARCHIVE + '.', dir=cache)
    try:
        with os.fdopen(descriptor, 'wb') as output, urllib.request.urlopen(NODE_URL, timeout=120) as response:
            shutil.copyfileobj(response, output)
        if sha(temporary) != NODE_SHA256:
            raise SystemExit('Downloaded Node.js archive checksum mismatch; nothing kept.')
        os.replace(temporary, cache / NODE_ARCHIVE)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    return cache / NODE_ARCHIVE


def extract_node(archive, destination):
    """Copy only the runtime (bin/node) and its licence from the verified archive."""
    wanted = {NODE_DIR + '/bin/node': 'bin/node', NODE_DIR + '/LICENSE': 'LICENSE'}
    with tarfile.open(archive) as tar:
        for name, target in wanted.items():
            member = tar.getmember(name)
            if not member.isfile():
                raise SystemExit('Node.js archive member is not a regular file: ' + name)
            path = destination / target
            path.parent.mkdir(parents=True, exist_ok=True)
            with tar.extractfile(member) as source, path.open('wb') as output:
                shutil.copyfileobj(source, output)
            path.chmod(0o755 if target == 'bin/node' else 0o644)


def extract_caddy(archive, destination):
    with tarfile.open(archive) as tar:
        member = tar.getmember('caddy')
        if not member.isfile():
            raise SystemExit('Caddy binary is not a regular archive member.')
        with tar.extractfile(member) as source, destination.open('wb') as target:
            shutil.copyfileobj(source, target)
    destination.chmod(0o755)


def build_store(store):
    """Install the locked dependencies and run the production build."""
    stray = [p.name for p in store.glob('.env*') if p.name != '.env.example']
    if stray:
        raise SystemExit('Remove local environment files from store/ before a production build: ' + ', '.join(sorted(stray)))
    env = {k: v for k, v in os.environ.items() if k not in STORE_ENV_DROP}
    env.update(NODE_ENV='production', NEXT_TELEMETRY_DISABLED='1')
    subprocess.run(['pnpm', 'install', '--frozen-lockfile'], cwd=store, env=env, check=True)
    subprocess.run(['pnpm', 'run', 'build'], cwd=store, env=env, check=True)


def check_links(tree):
    """Every symlink must be relative and resolve to an existing path inside the tree."""
    top = tree.resolve()
    for current, directories, files in os.walk(tree):
        for name in directories + files:
            path = pathlib.Path(current) / name
            if not path.is_symlink():
                continue
            relative = path.relative_to(tree).as_posix()
            if os.path.isabs(os.readlink(path)):
                raise SystemExit('Absolute symlink in bundle: ' + relative)
            try:
                resolved = path.resolve(strict=True)
            except (OSError, RuntimeError):
                raise SystemExit('Broken symlink in bundle (a pnpm link without its target?): ' + relative)
            if resolved != top and top not in resolved.parents:
                raise SystemExit('Symlink leaves the bundle: ' + relative)


def stage_store(store, destination):
    """Copy the standalone server with its pnpm links, then its static files and public assets."""
    standalone = store / '.next/standalone'
    build_id = store / '.next/BUILD_ID'
    if not (standalone / 'server.js').is_file() or not build_id.is_file() or not (store / '.next/static').is_dir():
        raise SystemExit('Build the store first: store/.next/standalone, store/.next/static and store/.next/BUILD_ID are required.')
    identifier = build_id.read_text().strip()
    if (standalone / '.next/BUILD_ID').read_text().strip() != identifier:
        raise SystemExit('store/.next/standalone is from a different build than store/.next/static.')
    shutil.copytree(standalone, destination, symlinks=True)
    shutil.copytree(store / '.next/static', destination / '.next/static', symlinks=True, dirs_exist_ok=True)
    if (store / 'public').is_dir():
        shutil.copytree(store / 'public', destination / 'public', symlinks=True, dirs_exist_ok=True)
    # Next copies .env files next to server.js; runtime settings come from the service unit instead.
    secrets = [p for p in destination.glob('.env*') if p.name != '.env.example']
    if secrets:
        raise SystemExit('The store build contains environment files; remove them and rebuild: '
                         + ', '.join(sorted(p.relative_to(destination).as_posix() for p in secrets)))
    check_links(destination)
    return identifier


def inventory(stage):
    """Normalize modes and list every regular file (by hash) and symlink (by target), without following links."""
    files, links = {}, {}
    stage.chmod(0o755)
    for current, directories, names in os.walk(stage):
        for name in sorted(directories + names):
            path = pathlib.Path(current) / name
            relative = path.relative_to(stage).as_posix()
            if path.is_symlink():
                links[relative] = os.readlink(path)
            elif path.is_dir():
                path.chmod(0o755)
            elif path.is_file():
                if path.stat().st_nlink != 1:
                    raise SystemExit('Bundle files must not be hard links: ' + relative)
                path.chmod(0o755 if path.stat().st_mode & 0o111 else 0o644)
                files[relative] = sha(path)
            else:
                raise SystemExit('Bundle entry is not a file, directory or symlink: ' + relative)
    return dict(sorted(files.items())), dict(sorted(links.items()))


def write_archive(stage, archive, metadata):
    """Record the file-by-file manifest in build.json and write the archive, links kept as links."""
    files, links = inventory(stage)
    manifest = dict(metadata, files=files, symlinks=links)
    (stage / 'build.json').write_text(json.dumps(manifest, indent=2) + '\n')
    (stage / 'build.json').chmod(0o644)
    with tarfile.open(archive, 'w:gz') as tar:
        for path in sorted(stage.iterdir()):
            tar.add(path, arcname=path.name)
    return manifest


def copy_tracked(folder, stage):
    for filename in git('ls-files', folder).splitlines():
        source = ROOT / filename
        if not source.is_file() or source.is_symlink():
            raise SystemExit('Bundle source must be a regular file: ' + filename)
        destination = stage / filename
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument('--caddy', required=True, type=pathlib.Path, help='caddy_2.11.7_linux_arm64.tar.gz')
    p.add_argument('--node', type=pathlib.Path, help=NODE_ARCHIVE + ' (default: reuse a verified download or fetch it)')
    p.add_argument('--api', type=pathlib.Path, default=ROOT / 'target/aarch64-unknown-linux-gnu/release/apps-server')
    p.add_argument('--prebuilt-store', action='store_true',
                   help='package the existing store/.next build instead of running pnpm install and pnpm run build')
    p.add_argument('--output', default=ROOT / '.dev/production', type=pathlib.Path)
    a = p.parse_args()

    if git('status', '--porcelain', '--untracked-files=no'):
        raise SystemExit('Commit reviewed tracked changes before making a production bundle.')
    if git('status', '--porcelain', '--untracked-files=all', '--', 'store', 'deploy', 'runner', 'scripts'):
        raise SystemExit('Untracked files under store/, deploy/, runner/ or scripts/ would not be part of the revision; commit or remove them.')
    revision = git('rev-parse', 'HEAD')
    if sha(a.caddy) != CADDY_SHA256:
        raise SystemExit('Caddy ' + CADDY_VERSION + ' Linux ARM64 archive checksum mismatch.')
    if not a.api.is_file():
        raise SystemExit('Build the Linux ARM64 API first (cargo zigbuild ... --bin apps-server) or pass --api.')
    node = node_archive(a.node, a.output)
    a.output.mkdir(parents=True, exist_ok=True)
    archive = a.output / (revision + '.tar.gz')
    if archive.exists():
        raise SystemExit('An immutable archive already exists for this revision.')
    store = ROOT / 'store'
    if not a.prebuilt_store:
        build_store(store)

    with tempfile.TemporaryDirectory(prefix='apps-bundle-') as directory:
        stage = pathlib.Path(directory)
        (stage / 'bin').mkdir()
        shutil.copy2(a.api, stage / 'bin/apps-server')
        (stage / 'bin/apps-server').chmod(0o755)
        extract_caddy(a.caddy, stage / 'bin/caddy')
        extract_node(node, stage / 'node')
        build_id = stage_store(store, stage / 'store')
        # The installers keep their served paths: /install.sh and /install.ps1 from web/.
        (stage / 'web').mkdir()
        for name in INSTALLERS:
            git('ls-files', '--error-unmatch', 'scripts/' + name)
            shutil.copyfile(ROOT / 'scripts' / name, stage / 'web' / name)
        for folder in ('runner', 'deploy'):
            copy_tracked(folder, stage)
        manifest = write_archive(stage, archive, {
            'revision': revision, 'api_target': 'aarch64-unknown-linux-gnu',
            'caddy_version': CADDY_VERSION, 'caddy_archive_sha256': CADDY_SHA256,
            'node_version': NODE_VERSION, 'node_archive_sha256': NODE_SHA256,
            'store_build_id': build_id, 'store_build': 'prebuilt' if a.prebuilt_store else 'built'})
    receipt = {'revision': revision, 'archive': str(archive.resolve()),
               'sha256': sha(archive), 'size': archive.stat().st_size,
               'api_sha256': sha(a.api), 'node_version': NODE_VERSION,
               'store_build_id': build_id, 'store_build': manifest['store_build'],
               'files': len(manifest['files']), 'symlinks': len(manifest['symlinks'])}
    (a.output / (revision + '.json')).write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt, indent=2))


if __name__ == '__main__':
    main()
