#!/usr/bin/env python3
"""Create an auditable native production bundle from a clean source revision."""
import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import tarfile
import tempfile

p = argparse.ArgumentParser()
p.add_argument('--caddy', required=True, type=pathlib.Path)
p.add_argument('--output', default='.dev/production', type=pathlib.Path)
a = p.parse_args()
root = pathlib.Path(__file__).resolve().parent.parent


def git(*args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


if git('status', '--porcelain', '--untracked-files=no'):
    raise SystemExit('Commit reviewed tracked changes before making a production bundle.')
revision = git('rev-parse', 'HEAD')
expected_caddy = 'd8fc6d179a5d283028a472a5618564f6ad8a86fed513e64f032b3b0b7cc45e42'
if sha(a.caddy) != expected_caddy:
    raise SystemExit('Caddy 2.11.7 Linux ARM64 archive checksum mismatch.')
api = root / 'target/aarch64-unknown-linux-gnu/release/apps-server'
web = root / 'web/dist'
if not api.is_file() or not (web / 'index.html').is_file():
    raise SystemExit('Build the Linux ARM64 API and production web assets first.')
for name in ('install.sh', 'install.ps1'):
    if (web / name).read_bytes() != (root / 'scripts' / name).read_bytes():
        raise SystemExit('Web installers do not match the reviewed scripts; rebuild web.')
a.output.mkdir(parents=True, exist_ok=True)
archive = a.output / (revision + '.tar.gz')
if archive.exists():
    raise SystemExit('An immutable archive already exists for this revision.')
with tempfile.TemporaryDirectory(prefix='apps-bundle-') as directory:
    stage = pathlib.Path(directory)
    (stage / 'bin').mkdir()
    shutil.copy2(api, stage / 'bin/apps-server')
    with tarfile.open(a.caddy) as tar:
        member = tar.getmember('caddy')
        if not member.isfile():
            raise SystemExit('Caddy binary is not a regular archive member.')
        with tar.extractfile(member) as source, (stage / 'bin/caddy').open('wb') as target:
            shutil.copyfileobj(source, target)
    (stage / 'bin/caddy').chmod(0o755)
    shutil.copytree(web, stage / 'web')
    for folder in ('runner', 'deploy'):
        for filename in git('ls-files', folder).splitlines():
            source = root / filename
            if not source.is_file() or source.is_symlink():
                raise SystemExit('Bundle source must be a regular file: ' + filename)
            destination = stage / filename
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)
    files = {}
    for path in sorted(stage.rglob('*')):
        if path.is_dir():
            path.chmod(0o755)
        elif path.is_file():
            path.chmod(0o755 if path.stat().st_mode & 0o111 else 0o644)
            files[str(path.relative_to(stage))] = sha(path)
    manifest = {'revision': revision, 'api_target': 'aarch64-unknown-linux-gnu',
                'caddy_version': '2.11.7', 'caddy_archive_sha256': expected_caddy,
                'files': files}
    (stage / 'build.json').write_text(json.dumps(manifest, indent=2) + '\n')
    with tarfile.open(archive, 'w:gz') as tar:
        for path in sorted(stage.iterdir()):
            tar.add(path, arcname=path.name)
receipt = {'revision': revision, 'archive': str(archive.resolve()),
           'sha256': sha(archive), 'size': archive.stat().st_size,
           'api_sha256': sha(api)}
(a.output / (revision + '.json')).write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
