#!/usr/bin/env python3
"""Exercise public bootstrap and signed catalog installs in fresh, signed-out homes."""
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import urllib.request


def main():
    version = os.environ['RELEASE_VERSION']
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
        raise ValueError('Expected a release version')
    windows = os.name == 'nt'
    environment = os.environ.copy()
    for key in ('APPS_TOKEN', 'ACCOUNTS_URL', 'APPS_RELEASES_URL'):
        environment.pop(key, None)
    environment.update(SILICON_APPS_NO_DAEMON='1', APPS_URL='https://apps.teamofsilicons.com')

    def run(command):
        result = subprocess.run(command, env=environment, text=True,
                                capture_output=True, timeout=180)
        if result.returncode:
            raise RuntimeError(f'{command!r}: {result.stdout}\n{result.stderr}')
        return result.stdout

    with tempfile.TemporaryDirectory(prefix='apps-public-install-') as temporary:
        root = Path(temporary)
        bootstrap_home = root / 'bootstrap'
        catalog_home = root / 'catalog'
        bootstrap_home.mkdir()
        catalog_home.mkdir()
        environment['SILICON_HOME'] = str(bootstrap_home)
        installer = root / ('install.ps1' if windows else 'install.sh')
        with urllib.request.urlopen('https://apps.teamofsilicons.com/' + installer.name, timeout=30) as response:
            installer.write_bytes(response.read())
        if windows:
            run(['pwsh', '-NoProfile', '-File', str(installer), '-Version', version,
                 '-HomeDirectory', str(bootstrap_home), '-NoStartup', '-NoPath'])
        else:
            run(['sh', str(installer), '--version', version, '--home', str(bootstrap_home),
                 '--no-startup', '--no-path'])

        def cli(home, args, state_home=None):
            executable = home / '.apps' / 'bin' / ('silicon-apps.cmd' if windows else 'silicon-apps')
            prefix = ['cmd', '/d', '/c', str(executable)] if windows else [str(executable)]
            return run(prefix + ['--home', str(state_home or home)] + args)

        assert cli(bootstrap_home, ['--version']).strip() == 'silicon-apps ' + version
        # A second home exercises network resolution, signatures and managed installation.
        cli(bootstrap_home, ['install', 'silicon-apps@' + version], state_home=catalog_home)
        for home in (bootstrap_home, catalog_home):
            assert cli(home, ['--version']).strip() == 'silicon-apps ' + version
            assert json.loads(cli(home, ['accounts', '--json']))['app_id'] == 'silicon-apps'
            assert json.loads(cli(home, ['login', 'status', '--json']))['authenticated'] is False
            docs = json.loads(cli(home, ['docs', 'storage', '--json']))
            assert docs['topic'] == 'storage' and 'storage:write' in docs['content']
            assert 'storage' in cli(home, ['storage', '--help']).lower()
        print(json.dumps({'version': version, 'platform': os.name,
                          'public_bootstrap': True, 'signed_catalog_install': True,
                          'storage_documentation': True, 'signed_out': True}))


if __name__ == '__main__':
    main()
