#!/usr/bin/env python3
"""Back up a consistent SQLite snapshot with its immutable package/media objects."""
import argparse
import datetime
import pathlib
import re
import sqlite3
import stat
import subprocess
import tarfile
import tempfile


def add_objects(archive, root):
    """Ignore upload staging names before stat: they may disappear at any time."""
    def add_file(path):
        if not stat.S_ISREG(path.lstat().st_mode):
            raise ValueError('Committed object must be a regular file: ' + str(path))
        archive.add(path, arcname=str(path.relative_to(root)), recursive=False)

    packages = root / 'packages'
    if packages.exists():
        if packages.is_symlink() or not packages.is_dir():
            raise ValueError('Package storage must be a directory.')
        for path in packages.iterdir():
            if re.fullmatch(r'[0-9a-f]{64}', path.name):
                add_file(path)
    media = root / 'media'
    if media.exists():
        if media.is_symlink() or not media.is_dir():
            raise ValueError('Media storage must be a directory.')
        for app in media.iterdir():
            if app.is_symlink() or not app.is_dir():
                raise ValueError('App media storage must be a directory.')
            for path in app.iterdir():
                if re.fullmatch(r'[0-9a-f]{64}(?:\.mime)?', path.name):
                    add_file(path)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--bucket', required=True)
    parser.add_argument('--region', default='us-east-2')
    args = parser.parse_args()
    root = pathlib.Path('/var/lib/silicon-apps')
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')
    if not (root / 'apps.sqlite').is_file():
        raise SystemExit('Catalog database is missing; refusing to create an empty backup.')
    with tempfile.TemporaryDirectory(prefix='backup-', dir=root) as folder:
        folder = pathlib.Path(folder)
        snapshot = folder / 'apps.sqlite'
        with sqlite3.connect('file:' + str(root / 'apps.sqlite') + '?mode=ro', uri=True) as source, sqlite3.connect(snapshot) as target:
            source.backup(target)
        with sqlite3.connect(snapshot) as db:
            if db.execute('PRAGMA integrity_check').fetchone()[0] != 'ok':
                raise SystemExit('Backup integrity check failed.')
        archive = folder / (stamp + '.tar.gz')
        with tarfile.open(archive, 'w:gz') as tar:
            tar.add(snapshot, arcname='apps.sqlite')
            add_objects(tar, root)
        destination = 's3://' + args.bucket + '/backups/' + stamp + '.tar.gz'
        subprocess.run(['aws', '--region', args.region, 's3', 'cp', str(archive), destination, '--only-show-errors'], check=True)
        print('Verified SQLite snapshot and immutable artifacts uploaded:', destination)


if __name__ == '__main__':
    main()
