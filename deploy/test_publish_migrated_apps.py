"""Synthetic evidence tests; these fixtures are never production release reports."""
import copy
import importlib.util
import io
import json
import pathlib
import sqlite3
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('publisher', pathlib.Path(__file__).with_name('publish-migrated-apps.py'))
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)

class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.bundle = self.root / 'bundle'
        self.bundle.mkdir()
        app = dict(app_id='commit', name='Silicon Commit', admin_uuid='owner', description='Description ' * 25,
                   packages=[], releases=[], history=[], authors=[{'uuid': 'owner'}], installs=42, published=False)
        self.before = {'apps': {'commit': app, 'silicon-apps': {'admin_uuid': 'owner', 'untouched': True}}, 'invites': [42]}
        with sqlite3.connect(self.root / 'apps.sqlite') as db:
            db.execute('CREATE TABLE catalog(id INTEGER PRIMARY KEY,document TEXT)')
            db.execute('INSERT INTO catalog VALUES(1,?)', (json.dumps(self.before),))
        self.meta = dict(app_id='commit', version='0.5.0', source_commit='a' * 40, verifier_commit='b' * 40,
                         workflow='https://github.com/teamofsilicons/silicon-commit/actions/runs/1', reports=[])
        for target in sorted(p.TARGETS):
            archive = self.bundle / (target + '.tar.gz')
            with tarfile.open(archive, 'w:gz') as tar:
                raw = b'app_id: commit\nversion: 0.5.0\ncommand: commit\n'
                info = tarfile.TarInfo('apps.yaml'); info.size = len(raw)
                tar.addfile(info, io.BytesIO(raw))
            system, arch = target.split('-')
            report = dict(app_id='commit', version='commit 0.5.0', source_commit='a' * 40, verifier_commit='b' * 40,
                          target=target, archive=archive.name, sha256=p.sha(archive.read_bytes()), size=archive.stat().st_size,
                          runtime={'system': {'linux': 'Linux', 'macos': 'Darwin', 'windows': 'Windows'}[system],
                                   'machine': arch, 'native': True},
                          command_results=[{'argv': args, 'exit_code': 0, 'stdout': out, 'stderr': ''} for args, out in [
                              (['--help'], 'Synthetic test help'),
                              (['accounts', '--json'], json.dumps({'app_id': 'commit', 'version': '0.5.0', 'api_url': 'https://api.commit.teamofsilicons.com'})),
                              (['login', 'status', '--json'], '{"authenticated":false}'),
                              (['--version'], 'commit 0.5.0\n')]])
            name = target + '.json'; (self.bundle / name).write_text(json.dumps(report)); self.meta['reports'].append(name)
        self.save()

    def save(self):
        raw = json.dumps(self.meta).encode(); (self.bundle / 'release.json').write_bytes(raw); self.digest = p.sha(raw)

    def catalog(self):
        with sqlite3.connect(self.root / 'apps.sqlite') as db:
            return json.loads(db.execute('SELECT document FROM catalog').fetchone()[0])

    def publish(self):
        return p.publish(self.root, self.bundle, self.digest)

    def test_preservation_and_exact_replay(self):
        self.assertTrue(self.publish()['changed'])
        after = self.catalog()
        self.assertEqual(after['apps']['silicon-apps'], self.before['apps']['silicon-apps'])
        self.assertEqual(after['invites'], self.before['invites'])
        for key in ['authors', 'admin_uuid', 'description', 'name', 'installs']:
            self.assertEqual(after['apps']['commit'][key], self.before['apps']['commit'][key])
        self.assertEqual(len(after['apps']['commit']['packages']), 6)
        self.assertEqual(len(after['apps']['commit']['releases']), 2)
        self.assertFalse(self.publish()['changed'])
        self.assertEqual(self.catalog(), after)

    def test_evidence_tampering_cannot_publish(self):
        path = self.bundle / self.meta['reports'][0]; original = json.loads(path.read_text())
        cases = [dict(source_commit='c' * 40), dict(verifier_commit='c' * 40), dict(sha256='0' * 64),
                 dict(runtime={'native': False, 'system': 'Linux', 'machine': 'aarch64'}),
                 dict(runtime={'native': True, 'system': 'Windows', 'machine': 'aarch64'})]
        for patch in cases:
            report = copy.deepcopy(original); report.update(patch); path.write_text(json.dumps(report))
            with self.subTest(patch=patch), self.assertRaises(ValueError): self.publish()
            self.assertEqual(self.catalog(), self.before)
        for index, output in [(1, '{"app_id":"commit","version":"0.5.0","api_url":"https://backend.commit.teamofsilicons.com"}'),
                              (2, '{"authenticated":true}'), (3, 'commit 0.4.0')]:
            report = copy.deepcopy(original); report['command_results'][index]['stdout'] = output
            path.write_text(json.dumps(report))
            with self.subTest(index=index), self.assertRaises(ValueError): self.publish()
            self.assertEqual(self.catalog(), self.before)
        report = copy.deepcopy(original); report['command_results'].pop()
        path.write_text(json.dumps(report))
        with self.assertRaises(ValueError): self.publish()
        self.assertEqual(self.catalog(), self.before)

    def test_registration_and_repository_are_required(self):
        self.meta['workflow'] = self.meta['workflow'].replace('teamofsilicons', 'third-party'); self.save()
        with self.assertRaises(ValueError): self.publish()
        self.assertEqual(self.catalog(), self.before)

    def test_existing_release_cannot_be_replaced(self):
        self.publish(); changed = self.catalog(); changed['apps']['commit']['releases'][0]['package_ids'] = ['other']
        with sqlite3.connect(self.root / 'apps.sqlite') as db:
            db.execute('UPDATE catalog SET document=?', (json.dumps(changed),))
        with self.assertRaisesRegex(ValueError, 'Immutable release'): self.publish()
        self.assertEqual(self.catalog(), changed)


    def browser_bundle(self):
        self.meta.update(app_id='browser', version='1.1.1', workflow='https://github.com/unlikefraction/silicon-browser/actions/runs/1')
        catalog = self.catalog()
        browser = catalog['apps'].pop('commit')
        browser.update(app_id='browser', name='Silicon Browser', admin_uuid=p.BROWSER_OWNER_UUID,
                       authors=[{'uuid': p.BROWSER_OWNER_UUID, 'id': 'si:tos'}])
        catalog['apps']['browser'] = browser
        with sqlite3.connect(self.root / 'apps.sqlite') as db:
            db.execute('UPDATE catalog SET document=?', (json.dumps(catalog),))
        for filename in self.meta['reports']:
            path = self.bundle / filename; report = json.loads(path.read_text())
            report.update(app_id='browser', version='browser 1.1.1')
            archive = self.bundle / report['archive']
            with tarfile.open(archive, 'w:gz') as tar:
                raw = b'app_id: browser\nversion: 1.1.1\ncommand: browser\n'
                info = tarfile.TarInfo('apps.yaml'); info.size = len(raw); tar.addfile(info, io.BytesIO(raw))
            report.update(sha256=p.sha(archive.read_bytes()), size=archive.stat().st_size)
            report['command_results'][1]['stdout'] = json.dumps({'app_id': 'browser', 'version': '1.1.1', 'api_url': 'https://backend.browser.teamofsilicons.com'})
            report['command_results'][3]['stdout'] = 'browser 1.1.1\n'
            path.write_text(json.dumps(report))
        self.save()
        return catalog

    def test_browser_preserves_existing_owner_and_replay(self):
        before = self.browser_bundle()
        self.assertTrue(self.publish()['changed'])
        after = self.catalog()
        self.assertEqual(after['apps']['silicon-apps'], before['apps']['silicon-apps'])
        for key in ['authors', 'admin_uuid', 'description', 'name', 'installs']:
            self.assertEqual(after['apps']['browser'][key], before['apps']['browser'][key])
        self.assertFalse(self.publish()['changed'])
        self.assertEqual(self.catalog(), after)

    def test_briefcase_exact_api_path_passes_and_legacy_fails(self):
        self.meta.update(app_id='briefcase', workflow='https://github.com/teamofsilicons/silicon-briefcase/actions/runs/1')
        for filename in self.meta['reports']:
            path = self.bundle / filename; report = json.loads(path.read_text())
            report.update(app_id='briefcase', version='briefcase 0.5.0')
            archive = self.bundle / report['archive']
            with tarfile.open(archive, 'w:gz') as tar:
                raw = b'app_id: briefcase\nversion: 0.5.0\ncommand: briefcase\n'
                info = tarfile.TarInfo('apps.yaml'); info.size = len(raw); tar.addfile(info, io.BytesIO(raw))
            report.update(sha256=p.sha(archive.read_bytes()), size=archive.stat().st_size)
            report['command_results'][1]['stdout'] = json.dumps({'app_id': 'briefcase', 'version': '0.5.0', 'api_url': p.API_ORIGINS['briefcase']})
            report['command_results'][3]['stdout'] = 'briefcase 0.5.0\n'
            path.write_text(json.dumps(report))
        self.save()
        self.assertEqual(len(p.verified_packages(self.bundle, self.meta)[0]), 6)
        path = self.bundle / self.meta['reports'][0]; report = json.loads(path.read_text())
        original = report['command_results'][1]['stdout']
        for invalid in ['https://backend.briefcase.teamofsilicons.com/api/v1/', 'https://api.briefcase.teamofsilicons.com', 'https://api.briefcase.teamofsilicons.com/api/v1/other']:
            report['command_results'][1]['stdout'] = original.replace(p.API_ORIGINS['briefcase'], invalid)
            path.write_text(json.dumps(report))
            with self.subTest(invalid=invalid), self.assertRaises(ValueError): p.verified_packages(self.bundle, self.meta)

    def test_browser_cannot_be_reassigned_to_apps_owner(self):
        before = self.browser_bundle()
        before['apps']['browser']['admin_uuid'] = before['apps']['silicon-apps']['admin_uuid']
        with sqlite3.connect(self.root / 'apps.sqlite') as db:
            db.execute('UPDATE catalog SET document=?', (json.dumps(before),))
        with self.assertRaisesRegex(ValueError, 'another app or admin'): self.publish()
        self.assertEqual(self.catalog(), before)

    def test_browser_repository_and_origin_are_exact(self):
        before = self.browser_bundle()
        self.meta['workflow'] = self.meta['workflow'].replace('unlikefraction', 'teamofsilicons'); self.save()
        with self.assertRaises(ValueError): self.publish()
        self.meta['workflow'] = self.meta['workflow'].replace('teamofsilicons', 'unlikefraction'); self.save()
        path = self.bundle / self.meta['reports'][0]; report = json.loads(path.read_text())
        report['command_results'][1]['stdout'] = report['command_results'][1]['stdout'].replace('backend.browser', 'api.browser')
        path.write_text(json.dumps(report))
        with self.assertRaises(ValueError): self.publish()
        self.assertEqual(self.catalog(), before)

if __name__ == '__main__':
    unittest.main()
