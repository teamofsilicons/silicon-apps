#!/usr/bin/env python3
"""Exercise a deployed validator with exact release bytes; never print credentials."""
import argparse
import base64
import hashlib
import json
import pathlib
import shlex
import tarfile
import urllib.error
import urllib.request

import yaml

p = argparse.ArgumentParser()
p.add_argument('archive', type=pathlib.Path)
p.add_argument('--target', default='linux-x86_64')
p.add_argument('--env', default='/etc/silicon-apps/worker.env', type=pathlib.Path)
p.add_argument('--url', default='http://127.0.0.1:4312/validate')
p.add_argument('--output', required=True, type=pathlib.Path)
a = p.parse_args()
env = {}
for line in a.env.read_text().splitlines():
    if not line or line.startswith('#'):
        continue
    key, value = line.split('=', 1)
    env[key] = shlex.split(value)[0]
token = env['APPS_RUNNER_TOKEN']
data = a.archive.read_bytes()
with tarfile.open(a.archive) as tar:
    manifest = yaml.safe_load(tar.extractfile('apps.yaml').read())
body = {'target': a.target, 'app_id': manifest['app_id'], 'manifest': manifest,
        'package_base64': base64.b64encode(data).decode(),
        'package_sha256': hashlib.sha256(data).hexdigest()}
encoded = json.dumps(body).encode()
request = urllib.request.Request(a.url, data=encoded, headers={
    'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json'})
with urllib.request.urlopen(request, timeout=120) as response:
    result = json.load(response)
assert result.get('isolated') is True and result.get('target') == a.target, result
assert len(result.get('validation', [])) == 3, result
assert all(check['passed'] is True for check in result['validation']), result
try:
    urllib.request.urlopen(urllib.request.Request(a.url, data=b"{}"), timeout=10)
except urllib.error.HTTPError as error:
    assert error.code == 401, error.code
else:
    raise AssertionError('Worker accepted an unauthenticated request.')
evidence = {'package_sha256': body['package_sha256'], 'size': len(data),
            'target': a.target, 'unauthenticated_status': 401, 'result': result}
a.output.write_text(json.dumps(evidence, indent=2) + '\n')
a.output.chmod(0o600)
print(json.dumps({'target': a.target, 'sha256': body['package_sha256'],
                  'required_commands_passed': 3, 'unauthenticated_status': 401}))
