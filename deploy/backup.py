#!/usr/bin/env python3
"""Back up a consistent SQLite snapshot with its immutable package/media objects."""
import argparse,datetime,pathlib,sqlite3,subprocess,tarfile,tempfile
p=argparse.ArgumentParser();p.add_argument('--bucket',required=True);p.add_argument('--region',default='us-east-2');a=p.parse_args()
root=pathlib.Path('/var/lib/silicon-apps');stamp=datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')
if not (root/'apps.sqlite').is_file():raise SystemExit('Catalog database is missing; refusing to create an empty backup.')
with tempfile.TemporaryDirectory(prefix='backup-',dir=root) as folder:
 folder=pathlib.Path(folder);snapshot=folder/'apps.sqlite'
 with sqlite3.connect('file:'+str(root/'apps.sqlite')+'?mode=ro',uri=True) as source,sqlite3.connect(snapshot) as target:source.backup(target)
 with sqlite3.connect(snapshot) as db:
  if db.execute('PRAGMA integrity_check').fetchone()[0]!='ok':raise SystemExit('Backup integrity check failed.')
 archive=folder/(stamp+'.tar.gz')
 with tarfile.open(archive,'w:gz') as tar:
  tar.add(snapshot,arcname='apps.sqlite')
  for name in ['packages','media']:
   if (root/name).exists():tar.add(root/name,arcname=name)
 destination='s3://'+a.bucket+'/backups/'+stamp+'.tar.gz'
 subprocess.run(['aws','--region',a.region,'s3','cp',str(archive),destination,'--only-show-errors'],check=True)
 print('Verified SQLite snapshot and immutable artifacts uploaded:',destination)
