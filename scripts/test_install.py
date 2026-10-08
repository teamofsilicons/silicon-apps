"""Bootstrap regression checks using isolated, checksum-verified fake packages."""
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('install.sh').resolve()

class BootstrapTests(unittest.TestCase):
    def install(self, command='silicon-apps', options=()):
        temporary = tempfile.TemporaryDirectory(prefix='apps-installer-test-')
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        home = root / "home with 'quotes' and $(literal)"
        profile = root / 'zsh-settings'
        home.mkdir(); profile.mkdir()
        body = '''#!/bin/sh
set -eu
home=""
while [ "$#" -gt 0 ]; do
 case "$1" in
 --home) home="$2"; shift 2;;
 --server) shift 2;;
 --json) shift;;
 *) break;;
 esac
done
state="$home/.apps"
case "$1" in
 install)
 mkdir -p "$state/bin"
 cp "$0" "$state/bin/COMMAND"
 chmod +x "$state/bin/COMMAND"
 printf '0' > "$state/polls"
 ;;
 daemon)
 case "$2" in
 stop) printf '{"status":"stop_requested"}\\n';;
 status)
 n=$(cat "$state/polls"); n=$((n + 1)); printf '%s' "$n" > "$state/polls"
 if [ "$n" -le 2 ]; then printf '{"running":true}\\n'; else printf '{"running":false}\\n'; fi
 ;;
 install)
 test "$(cat "$state/polls")" -gt 2
 printf 'registered' > "$state/startup"
 ;;
 esac
 ;;
 --version) printf 'COMMAND test\\n';;
 *) exit 2;;
esac
'''.replace('COMMAND', command).encode()
        archive = root / 'package.tar.gz'
        with tarfile.open(archive, 'w:gz') as target:
            member = tarfile.TarInfo('bin/'+command); member.size=len(body); member.mode=0o755
            target.addfile(member, io.BytesIO(body))
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        env = os.environ.copy(); env.update(SHELL='/bin/zsh', ZDOTDIR=str(profile))
        args = ['sh', str(SCRIPT), '--archive',str(archive),'--sha256',digest,'--home',str(home),*options]
        result = subprocess.run(args, env=env, text=True,capture_output=True,timeout=20)
        self.assertEqual(result.returncode,0,result.stdout+'\n'+result.stderr)
        return home,profile,args,env,result

    def test_path_quotes_real_shell_directory_and_waits_for_updater(self):
        home,profile,args,env,result = self.install()
        self.assertTrue((home/'.apps/startup').exists())
        rc = profile/'.zshrc'
        self.assertTrue(rc.exists())
        path = subprocess.check_output(['sh','-c','. "$1"; printf "%s" "$PATH"','sh',str(rc)],text=True)
        self.assertEqual(path.split(':')[0],str(home/'.apps/bin'))
        self.assertIn('silicon-apps --help',result.stdout)
        again = subprocess.run(args,env=env,text=True,capture_output=True,timeout=20)
        self.assertEqual(again.returncode,0,again.stderr)
        self.assertEqual(rc.read_text().count('export PATH='),1)

    def test_legacy_archive_is_still_supported(self):
        home,profile,args,env,result = self.install(command='apps')
        self.assertTrue((home/'.apps/startup').exists())
        self.assertIn('apps --help',result.stdout)

    def test_no_path_and_no_startup_leave_user_configuration_alone(self):
        home,profile,args,env,result = self.install(options=('--no-path','--no-startup'))
        self.assertFalse((profile/'.zshrc').exists())
        self.assertFalse((home/'.apps/startup').exists())

if __name__ == '__main__': unittest.main()
