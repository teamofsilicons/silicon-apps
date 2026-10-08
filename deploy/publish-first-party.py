#!/usr/bin/env python3
"""Operator publication of our own native CI-verified CLI releases, with the API stopped.

The bundle is prepared from a completed GitHub build at an exact source revision. It
contains archives, their native execution reports, and a SHA-pinned release.json.
This is not an upload endpoint and cannot publish third-party apps.
"""
import argparse, hashlib, json, os, pathlib, re, sqlite3, tarfile, uuid
from datetime import datetime, timezone

TARGETS = {'linux-x86_64','linux-i686','linux-aarch64','linux-armv7hf','macos-x86_64','macos-aarch64','windows-x86_64','windows-i686','windows-aarch64'}
COMMANDS = [('--help',['--help']),('accounts --json',['accounts','--json']),('login status --json',['login','status','--json'])]
NAMES = {'silicon-apps':'Silicon Apps','silicon-accounts':'Silicon Accounts'}
DESCRIPTION = 'Silicon Accounts gives every Carbon and Silicon an account for the Silicon ecosystem. Sign in, create and manage Silicons, choose custodians, manage your profile and app authentication, and use app verification and user verification from one command line.'

def sha(data): return hashlib.sha256(data).hexdigest()
def uid(text): return str(uuid.uuid5(uuid.NAMESPACE_URL, 'https://apps.teamofsilicons.com/first-party/'+text))
def canonical(value): return json.dumps(value,sort_keys=True,separators=(',',':'))

def verified_packages(folder, metadata):
    app_id=metadata['app_id']; version=metadata['version']; source=metadata['source_commit']
    if app_id not in NAMES or not re.fullmatch(r'\d+\.\d+\.\d+',version) or not re.fullmatch('[a-f0-9]{40}',source): raise ValueError('Invalid first-party release identity')
    if not re.fullmatch(r'https://github.com/teamofsilicons/'+re.escape(app_id)+r'/actions/runs/[0-9]+',metadata['workflow']): raise ValueError('Expected the first-party GitHub build URL')
    reports=metadata['reports']
    if len(reports)!=9 or len(set(reports))!=9: raise ValueError('Exactly nine native reports are required')
    packages=[]; targets=set(); payloads={}
    now=datetime.now(timezone.utc).isoformat()
    for filename in reports:
        if filename in ('.','..') or pathlib.Path(filename).name!=filename: raise ValueError('Report filename must be a basename')
        report=json.loads((folder/filename).read_text())
        target=report['target']; archive=report['archive']
        if target not in TARGETS or target in targets or archive in ('.','..') or pathlib.Path(archive).name!=archive: raise ValueError('Invalid archive or duplicate target')
        targets.add(target)
        if report['source_commit']!=source or report['verifier_commit']!=source or report['version']!=f'{app_id} {version}' or report['app_id']!=app_id: raise ValueError('Native report identity mismatch')
        raw=(folder/archive).read_bytes(); digest=sha(raw)
        if digest!=report['sha256'] or len(raw)!=report['size']: raise ValueError('Native report does not describe these archive bytes')
        with tarfile.open(folder/archive,'r:gz') as tar:
            manifest=tar.extractfile('apps.yaml').read().decode()
        for key,value in [('app_id',app_id),('version',version),('command',app_id)]:
            if not re.search(r'^'+key+r': '+re.escape(value)+r'$',manifest,re.M): raise ValueError('Manifest mismatch')
        checks=[]
        for command,args in COMMANDS:
            matching=[c for c in report['command_results'] if c['argv']==args]
            if len(matching)!=1 or matching[0]['exit_code']!=0: raise ValueError('A required native command failed or is missing')
            result=matching[0]; output=result['stdout']
            if command=='--help' and not output.strip(): raise ValueError('Empty native help')
            if command=='accounts --json' and json.loads(output).get('app_id')!=app_id: raise ValueError('Discovery app ID mismatch')
            if command=='login status --json' and json.loads(output).get('authenticated') is not False: raise ValueError('Native verification must use a signed-out home')
            checks.append({'command':command,'passed':True,'exit_code':0,'stdout':output,'stderr':result['stderr'],'evidence':{'workflow':metadata['workflow'],'source_commit':source,'runtime':report['runtime'],'report_sha256':sha((folder/filename).read_bytes())}})
        packages.append({'id':uid(app_id+'/'+target+'/'+digest),'target':target,'sha256':digest,'size':len(raw),'command':app_id,'validation':checks,'created_at':now})
        payloads[digest]=raw
    return sorted(packages,key=lambda p:p['target']),payloads

def publish(root, folder, expected):
    metadata_bytes=(folder/'release.json').read_bytes()
    if sha(metadata_bytes)!=expected: raise ValueError('Release manifest checksum mismatch')
    metadata=json.loads(metadata_bytes)
    packages,payloads=verified_packages(folder,metadata)
    app_id=metadata['app_id'];version=metadata['version'];now=datetime.now(timezone.utc).isoformat()
    db=sqlite3.connect(root/'apps.sqlite',timeout=30)
    db.execute('BEGIN IMMEDIATE')
    try:
        catalog=json.loads(db.execute('select document from catalog where id=1').fetchone()[0])
        original=catalog['apps'].get(app_id)
        owner=catalog['apps']['silicon-apps']
        if original and (original['name']!=NAMES[app_id] or original['admin_uuid']!=owner['admin_uuid']): raise ValueError('Existing identity belongs to another app or admin')
        if original is None:
            app={'app_id':app_id,'name':NAMES[app_id],'description':DESCRIPTION,'logo':'','logo_alt':'','banner':'','banner_alt':'','tags':['developer-tools','authentication','identity'],'visibility':'public','domains':[],'account_ids':[],'access_uuids':[],'links':{'website':'https://accounts.teamofsilicons.com','docs':'https://developers.teamofsilicons.com/docs/accounts'},'carousel':[],'published':False,'setup_step':7,'created_at':now,'updated_at':now,'authors':owner['authors'],'admin_uuid':owner['admin_uuid'],'packages':[],'releases':[],'reviews':[],'installs':0,'history':[],'secret_hash':''}
        else: app=original
        ids=[p['id'] for p in packages]
        for channel in ('development','production'):
            existing=next((r for r in app['releases'] if r['channel']==channel and r['version']==version),None)
            if existing and existing['package_ids']!=ids: raise ValueError('Immutable release already has different packages')
        package_dir=root/'packages';package_dir.mkdir(exist_ok=True)
        for package in packages:
            destination=package_dir/package['sha256']
            if destination.exists():
                if sha(destination.read_bytes())!=package['sha256']: raise ValueError('Existing package storage is corrupt')
            else:
                with destination.open('xb') as file: file.write(payloads[package['sha256']])
                destination.chmod(0o600)
            if not any(p['id']==package['id'] for p in app['packages']):app['packages'].append(package)
        changed=False
        for channel in ('development','production'):
            if any(r['channel']==channel and r['version']==version for r in app['releases']):continue
            app['releases'].append({'id':uid(app_id+'/'+channel+'/'+version),'app_id':app_id,'channel':channel,'version':version,'package_ids':ids,'notes':f'{NAMES[app_id]} {version}. Native packages verified on all nine supported targets.','created_at':now,'promoted_from':uid(app_id+'/development/'+version) if channel=='production' else None})
            changed=True
        if changed:
            app['published']=True;app['setup_step']=7;app['updated_at']=now
            app['history'].append({'id':uid(app_id+'/publication/'+version),'at':now,'actor_uuid':'system','kind':'app.first_party_published','data':{'version':version,'source_commit':metadata['source_commit'],'workflow':metadata['workflow'],'release_manifest_sha256':expected,'targets':sorted(TARGETS)},'idempotency_key':None})
        catalog['apps'][app_id]=app
        db.execute('update catalog set document=? where id=1',(canonical(catalog),));db.commit()
        return {'app_id':app_id,'version':version,'targets':len(packages),'changed':changed,'release_id':uid(app_id+'/production/'+version)}
    except Exception:
        db.rollback();raise
    finally: db.close()

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--data-dir',type=pathlib.Path,required=True);parser.add_argument('--bundle',type=pathlib.Path,required=True);parser.add_argument('--sha256',required=True)
    args=parser.parse_args();print(json.dumps(publish(args.data_dir,args.bundle,args.sha256)))
