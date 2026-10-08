"""Regression tests for offline first-party publication, using synthetic fixtures."""
import copy
import importlib.util
import io
import json
import pathlib
import sqlite3
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('publisher', pathlib.Path(__file__).with_name('publish-first-party.py'))
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.bundle = self.root / 'bundle'
        self.bundle.mkdir()
        self.owner = {'app_id': 'silicon-apps', 'name': 'Silicon Apps', 'admin_uuid': 'test-admin', 'authors': [{'uuid': 'test-admin'}], 'installs': 17, 'packages': [], 'releases': [], 'history': [], 'custom_field': 'preserve'}
        self.initial = {'apps': {'silicon-apps': self.owner}, 'invites': [{'id': 'preserve'}]}
        with sqlite3.connect(self.root / 'apps.sqlite') as db:
            db.execute('create table catalog(id integer primary key, document text)')
            db.execute('insert into catalog values(1, ?)', (json.dumps(self.initial),))
        self.metadata = {'app_id': 'silicon-accounts', 'version': '0.3.1', 'source_commit': 'a' * 40, 'workflow': 'https://github.com/teamofsilicons/silicon-accounts/actions/runs/123', 'reports': []}
        for target in sorted(publisher.TARGETS):
            archive = self.bundle / (target + '.tar.gz')
            manifest = f'app_id: silicon-accounts\nversion: 0.3.1\ncommand: silicon-accounts\n# fixture {target}\n'.encode()
            with tarfile.open(archive, 'w:gz') as tar:
                info = tarfile.TarInfo('apps.yaml')
                info.size = len(manifest)
                tar.addfile(info, io.BytesIO(manifest))
            report = {'app_id': 'silicon-accounts', 'version': 'silicon-accounts 0.3.1', 'source_commit': 'a' * 40, 'verifier_commit': 'a' * 40, 'target': target, 'archive': archive.name, 'sha256': publisher.sha(archive.read_bytes()), 'size': archive.stat().st_size, 'runtime': 'synthetic-test-fixture', 'command_results': [{'argv': args, 'exit_code': 0, 'stdout': output, 'stderr': ''} for args, output in [(['--help'], 'fixture help'), (['accounts', '--json'], '{"app_id":"silicon-accounts"}'), (['login', 'status', '--json'], '{"authenticated":false}')]]}
            filename = target + '.json'
            (self.bundle / filename).write_text(json.dumps(report))
            self.metadata['reports'].append(filename)
        self.save_manifest()

    def save_manifest(self):
        raw = json.dumps(self.metadata).encode()
        (self.bundle / 'release.json').write_bytes(raw)
        self.digest = publisher.sha(raw)

    def read_catalog(self):
        with sqlite3.connect(self.root / 'apps.sqlite') as db:
            return json.loads(db.execute('select document from catalog').fetchone()[0])

    def publish(self):
        return publisher.publish(self.root, self.bundle, self.digest)

    def test_publication_preserves_catalog_and_is_idempotent(self):
        result = self.publish()
        self.assertTrue(result['changed'])
        after = self.read_catalog()
        self.assertEqual(after['apps']['silicon-apps'], self.owner)
        self.assertEqual(after['invites'], self.initial['invites'])
        app = after['apps']['silicon-accounts']
        self.assertEqual(len(app['packages']), 9)
        self.assertEqual(len(app['releases']), 2)
        self.assertTrue(app['published'])
        self.assertFalse(self.publish()['changed'])
        self.assertEqual(self.read_catalog(), after)

    def test_changed_immutable_release_rolls_back(self):
        self.publish()
        with sqlite3.connect(self.root / 'apps.sqlite') as db:
            catalog = self.read_catalog()
            catalog['apps']['silicon-accounts']['releases'][0]['package_ids'] = ['another-package']
            db.execute('update catalog set document=?', (json.dumps(catalog),))
        with self.assertRaisesRegex(ValueError, 'Immutable release'):
            self.publish()
        self.assertEqual(self.read_catalog(), catalog)

    def test_invalid_evidence_never_changes_catalog(self):
        original = self.read_catalog()
        name = self.metadata['reports'][0]
        path = self.bundle / name
        valid = json.loads(path.read_text())
        for field, value in [('source_commit', 'b' * 40), ('sha256', '0' * 64), ('target', 'unsupported')]:
            report = copy.deepcopy(valid)
            report[field] = value
            path.write_text(json.dumps(report))
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.publish()
            self.assertEqual(self.read_catalog(), original)
        for index in range(3):
            report = copy.deepcopy(valid)
            report['command_results'][index]['exit_code'] = 1
            path.write_text(json.dumps(report))
            with self.subTest(command=index), self.assertRaises(ValueError):
                self.publish()
            self.assertEqual(self.read_catalog(), original)

    def test_manifest_checksum_and_first_party_identity(self):
        with self.assertRaisesRegex(ValueError, 'checksum'):
            publisher.publish(self.root, self.bundle, '0' * 64)
        self.metadata['app_id'] = 'third-party'
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'identity'):
            self.publish()
        self.assertEqual(self.read_catalog(), self.initial)


if __name__ == '__main__':
    unittest.main()
