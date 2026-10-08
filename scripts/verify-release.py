#!/usr/bin/env python3
"""Verify our just-built release archive and its three required discovery commands."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', required=True)
    parser.add_argument('--version', help='Select one version when the directory contains older archives')
    parser.add_argument('--directory', type=Path, default=Path('dist'))
    args = parser.parse_args()
    archives = list(args.directory.glob(f'apps-{args.version or "*"}-{args.target}.tar.gz'))
    if len(archives) != 1:
        raise SystemExit(f'Expected one archive for {args.target}; found {len(archives)}')
    archive = archives[0].resolve()
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    expected = archive.with_name(archive.name + '.sha256').read_text().split()[0]
    if digest != expected:
        raise SystemExit('Archive SHA-256 differs from its sidecar; no binary executed')
    binary_name = 'apps.exe' if args.target.startswith('windows-') else 'apps'
    runner = json.loads(os.environ.get('APPS_VERIFY_RUNNER', '[]'))
    with tempfile.TemporaryDirectory(prefix='apps-release-verify-') as temporary:
        home = Path(temporary)
        binary = home / binary_name
        with tarfile.open(archive, mode='r:gz') as package:
            member = package.getmember(f'bin/{binary_name}')
            if not member.isfile():
                raise SystemExit('Release command must be a regular file')
            with package.extractfile(member) as source:
                binary.write_bytes(source.read())
        binary.chmod(0o755)
        environment = os.environ.copy()
        for name in ('APPS_TOKEN', 'ACCOUNTS_URL', 'APPS_TELEMETRY_KEY', 'APPS_TELEMETRY_TABLE_KEY'):
            environment.pop(name, None)
        environment['SILICON_HOME'] = str(home)
        environment['APPS_URL'] = 'http://127.0.0.1:9'
        command_results = []
        def run(arguments):
            result = subprocess.run(runner + [str(binary)] + arguments, env=environment,
                                    capture_output=True, text=True, timeout=45, check=False)
            command_results.append({'argv': arguments, 'exit_code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
            if result.returncode != 0:
                raise SystemExit(f'{arguments!r} failed ({result.returncode}): {result.stderr}\n{result.stdout}')
            return result.stdout
        if not run(['--help']).strip():
            raise SystemExit('--help returned empty output')
        account = json.loads(run(['accounts', '--json']))
        status = json.loads(run(['login', 'status', '--json']))
        validation = json.loads(run(['validate', str(archive), '--json']))
        if account.get('app_id') != 'apps' or status.get('authenticated') is not False or validation.get('valid') is not True:
            raise SystemExit('Release discovery/validation contract failed')
        evidence = {'archive': archive.name, 'sha256': digest, 'size': archive.stat().st_size,
                    'target': args.target, 'version': run(['--version']).strip(),
                    'commands': ['--help', 'accounts --json', 'login status --json'],
                    'authenticated': status['authenticated'], 'app_id': account['app_id'],
                    'runtime': runner or ['native-or-host-compatible'],
                    'source_commit': os.environ.get('APPS_SOURCE_COMMIT', os.environ.get('GITHUB_SHA')),
                    'verifier_commit': os.environ.get('GITHUB_SHA'),
                    'command_results': command_results}
    output = archive.with_name(archive.name.removesuffix('.tar.gz') + '.verification.json')
    output.write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps(evidence, indent=2))


if __name__ == '__main__':
    main()
