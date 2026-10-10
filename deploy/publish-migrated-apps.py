#!/usr/bin/env python3
"""Offline publication of the eight official Accounts migration releases.

Stop the Apps API and take a consistent backup before applying. Native CI reports
must be reviewed against their exact successful workflow and source revisions.
This operator tool is deliberately absent from the public upload API.
"""
import argparse
import hashlib
import json
import pathlib
import re
import sqlite3
import tarfile
import uuid
from datetime import datetime, timezone

NAMES = {app: 'Silicon ' + name for app, name in {
    'briefcase': 'Briefcase', 'remind': 'Remind', 'dm': 'DM',
    'extend': 'Extend', 'commit': 'Commit', 'hook': 'Hook',
    'waveform': 'Waveform', 'mcport': 'MCPort',
}.items()}
TARGETS = {system + '-' + arch for system in ('linux', 'macos', 'windows')
           for arch in ('x86_64', 'aarch64')}
COMMANDS = [('--help', ['--help']), ('accounts --json', ['accounts', '--json']),
            ('login status --json', ['login', 'status', '--json'])]

def sha(data):
    return hashlib.sha256(data).hexdigest()

def uid(text):
    return str(uuid.uuid5(uuid.NAMESPACE_URL, 'https://apps.teamofsilicons.com/first-party/' + text))

def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'))

def basename(name):
    if not isinstance(name, str) or name in ('.', '..') or pathlib.Path(name).name != name:
        raise ValueError('Evidence paths must be basenames')
    return name

def verified_packages(folder, metadata):
    app = metadata['app_id']
    version = metadata['version']
    source = metadata['source_commit']
    verifier = metadata['verifier_commit']
    if app not in NAMES or not re.fullmatch(r'\d+\.\d+\.\d+', version):
        raise ValueError('Unknown official app or invalid release version')
    if not all(re.fullmatch('[a-f0-9]{40}', value) for value in (source, verifier)):
        raise ValueError('Exact source and verifier commits are required')
    if not re.fullmatch(r'https://github.com/teamofsilicons/silicon-' + re.escape(app)
                        + r'/actions/runs/[0-9]+', metadata['workflow']):
        raise ValueError('Native workflow must belong to this official app repository')
    reports = metadata['reports']
    if len(reports) != 6 or len(set(reports)) != 6:
        raise ValueError('Exactly six distinct native reports are required')
    packages, payloads, seen = [], {}, set()
    now = datetime.now(timezone.utc).isoformat()
    for filename in reports:
        raw_report = (folder / basename(filename)).read_bytes()
        report = json.loads(raw_report)
        target = report['target']
        if target not in TARGETS or target in seen:
            raise ValueError('Unsupported or duplicate target')
        seen.add(target)
        if (report['app_id'], report['source_commit'], report['verifier_commit']) != (app, source, verifier):
            raise ValueError('Native report identity differs from the reviewed release')
        expected_version = app + ' ' + version
        actual_version = report['version'].strip()
        # Extend includes its Rust client/API contract in its documented version output.
        if actual_version != expected_version and not (
                app == 'extend' and actual_version.startswith(expected_version + ' (silicon-extend-client ' + version + ')')):
            raise ValueError('Native executable version differs from the manifest')
        runtime = report['runtime']
        system, arch = target.split('-')
        expected_system = {'linux': 'Linux', 'macos': 'Darwin', 'windows': 'Windows'}[system]
        expected_machine = {'x86_64': {'x86_64', 'amd64'}, 'aarch64': {'aarch64', 'arm64'}}[arch]
        if (runtime.get('native') is not True or runtime.get('system') != expected_system
                or runtime.get('machine', '').lower() not in expected_machine):
            raise ValueError('Native runner does not match the package target')
        archive = folder / basename(report['archive'])
        raw = archive.read_bytes()
        digest = sha(raw)
        if digest != report['sha256'] or len(raw) != report['size']:
            raise ValueError('Archive bytes differ from the native execution evidence')
        with tarfile.open(archive, 'r:gz') as tar:
            manifests = [m for m in tar.getmembers() if m.name in ('apps.yaml', './apps.yaml')]
            if len(manifests) != 1 or not manifests[0].isfile():
                raise ValueError('Archive must contain one regular apps.yaml')
            manifest = tar.extractfile(manifests[0]).read().decode()
        for key, value in [('app_id', app), ('version', version), ('command', app)]:
            if not re.search(r'^' + key + r':\s*' + re.escape(value) + r'\s*$', manifest, re.M):
                raise ValueError('Packaged Apps identity differs from the native report')
        checks = []
        for command, args in COMMANDS:
            found = [c for c in report['command_results'] if c['argv'] == args]
            if len(found) != 1 or found[0]['exit_code'] != 0:
                raise ValueError('Missing or failed required native command')
            result = found[0]
            if command == '--help' and not result['stdout'].strip():
                raise ValueError('Native help output is empty')
            if command == 'accounts --json':
                discovery = json.loads(result['stdout'])
                if discovery.get('app_id') != app or discovery.get('version') != version:
                    raise ValueError('Native account discovery identity differs')
                if discovery.get('api_url') != 'https://api.' + app + '.teamofsilicons.com':
                    raise ValueError('Package still targets a legacy API origin')
            if command == 'login status --json' and json.loads(result['stdout']).get('authenticated') is not False:
                raise ValueError('Native verification must use a signed-out home')
            checks.append({'command': command, 'passed': True, 'exit_code': 0,
                           'stdout': result['stdout'], 'stderr': result['stderr'],
                           'evidence': {'workflow': metadata['workflow'], 'source_commit': source,
                                        'verifier_commit': verifier, 'runtime': runtime,
                                        'report_sha256': sha(raw_report)}})
        packages.append({'id': uid(app + '/' + target + '/' + digest), 'target': target,
                         'sha256': digest, 'size': len(raw), 'command': app,
                         'validation': checks, 'created_at': now})
        payloads[digest] = raw
    return sorted(packages, key=lambda p: p['target']), payloads


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
        if original is None: raise ValueError('Register the official app through Apps before importing a release')
        app=original
        if not 200 <= len(app['description']) <= 600: raise ValueError('App description does not satisfy publishing readiness')
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
            app['releases'].append({'id':uid(app_id+'/'+channel+'/'+version),'app_id':app_id,'channel':channel,'version':version,'package_ids':ids,'notes':f'{NAMES[app_id]} {version}. Native packages verified on all six supported targets.','created_at':now,'promoted_from':uid(app_id+'/development/'+version) if channel=='production' else None})
            changed=True
        if changed:
            app['published']=True;app['setup_step']=7;app['updated_at']=now
            app['history'].append({'id':uid(app_id+'/publication/'+version),'at':now,'actor_uuid':'system','kind':'app.native_release_published','data':{'version':version,'source_commit':metadata['source_commit'],'verifier_commit':metadata['verifier_commit'],'workflow':metadata['workflow'],'release_manifest_sha256':expected,'targets':sorted(TARGETS)},'idempotency_key':None})
        catalog['apps'][app_id]=app
        db.execute('update catalog set document=? where id=1',(canonical(catalog),));db.commit()
        return {'app_id':app_id,'version':version,'targets':len(packages),'changed':changed,'release_id':uid(app_id+'/production/'+version)}
    except Exception:
        db.rollback();raise
    finally: db.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', type=pathlib.Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--data-dir', type=pathlib.Path)
    parser.add_argument('--writers-stopped', action='store_true')
    args = parser.parse_args()
    if sha((args.bundle / 'release.json').read_bytes()) != args.sha256:
        parser.error('Release manifest checksum mismatch')
    if args.data_dir:
        if not args.writers_stopped:
            parser.error('Stop all Apps writers and back up before publication')
        result = publish(args.data_dir, args.bundle, args.sha256)
    else:
        packages, _ = verified_packages(args.bundle, json.loads((args.bundle / 'release.json').read_text()))
        result = {'verified': True, 'targets': len(packages)}
    print(json.dumps(result))
