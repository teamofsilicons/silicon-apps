#!/usr/bin/env python3
"""Durable SQLite/CLI backfill regression: run directly or with unittest."""
import base64
import copy
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
from migrate_account_uuids import compact, migrate, migrate_cli, parse_mapping

A = 'f858d0b5-98ba-4a4d-8ce5-114e93136f23'
B = '9ab26444-1f74-46bb-a734-235e98cb6f2d'
CSV = f'old_uuid,new_uuid,kind\nAda,{A},carbon\nBot,{B},silicon\n'

def token(subject):
    return 'header.' + base64.urlsafe_b64encode(compact({'sub': subject}).encode()).decode().rstrip('=') + '.signature'

class Backfill(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.database = self.root / 'apps.sqlite'
        self.db = sqlite3.connect(self.database, isolation_level=None)
        for file in sorted((Path(__file__).resolve().parents[1] / 'migrations').glob('*.sql')):
            self.db.executescript(file.read_text())
        self.mapping = parse_mapping(CSV)
        self.catalog = {'apps': {'test': {'app_id': 'test', 'admin_uuid': 'Ada', 'authors': [{'uuid': 'Ada', 'id': 'c:ada'}], 'access_uuids': ['Bot'], 'reviews': [{'uuid': 'Bot', 'id': 'si:bot', 'text': 'Ada', 'rating': 5}], 'packages': [{'id': 'package-id', 'sha256': 'same-bytes', 'author_signature': {'signer_uuid': 'Ada', 'signer_id': 'c:ada', 'signature': 'unchanged-signature', 'public_key': 'same-public-key'}}], 'releases': [{'id': 'release-id', 'signatures': {'package-id': {'signature': 'unchanged-release-signature'}}, 'withdrawn': {'by_uuid': 'Ada', 'by_id': 'c:ada', 'reason': 'Ada'}}], 'history': [{'id': 'event-id', 'actor_uuid': 'Ada', 'kind': 'author.admin_transferred', 'data': {'uuid': 'Bot'}}, {'id': 'opaque-event', 'actor_uuid': 'Ada', 'kind': 'app.details_changed', 'data': {'links': {'uuid': 'Ada'}}}], 'secret_hash': 'unchanged-secret-hash', 'links': {'uuid': 'Ada'}}}, 'invites': [{'id': 'invite-id', 'app_id': 'test', 'account_uuid': 'Bot', 'to': 'si:bot'}], 'reports': [{'actor_uuid': 'Ada', 'message': 'Ada'}], 'platforms': {'Ada': ['macos-aarch64']}}
        self.db.execute('UPDATE catalog SET document=?', (compact(self.catalog),))
        self.db.execute("INSERT INTO events(id,type,app_id,actor_uuid,visibility,recipient_uuids,data,occurred_at) VALUES('event-id','author.invited','test','Ada','authors','[\"Bot\"]','{\"account_uuid\":\"Bot\"}','time')")
        self.db.execute("INSERT INTO subscriptions(id,owner_uuid,owner_id,delivery,url,secret,types,status,created_at,updated_at) VALUES('sub','Ada','c:ada','webhook','https://example.test/hook','private-webhook-secret','[]','active','time','time')")
        self.db.execute("INSERT INTO subscription_deliveries(id,subscription_id,event_seq,status,next_attempt_ms,created_ms,created_at) VALUES('delivery','sub',1,'pending',0,0,'time')")
        self.db.execute("INSERT INTO author_keys(key_id,owner_uuid,owner_id,public_key,created_at) VALUES('key-id','Ada','c:ada','unchanged-author-public','time')")
        self.db.execute('INSERT INTO idempotency VALUES(?,?,?,?,?)', ('Ada', 'retry-key', 'fingerprint', compact({'app': self.catalog['apps']['test'], 'app_secret': 'one-time-private-secret'}), 'time'))
        self.db.execute("INSERT INTO pending_secrets VALUES('Ada','pending','fingerprint','new-app','private-app-secret','time')")
        self.db.execute("INSERT INTO outbox(id,kind,body,created_at) VALUES('mail','mail.invite','{\"account_uuid\":\"Bot\",\"to\":\"si:bot\"}','time')")
        self.db.executescript('CREATE TABLE sessions(id TEXT PRIMARY KEY,access_token TEXT,refresh_token TEXT,expires_at INTEGER); CREATE TABLE oauth_pending(state TEXT); INSERT INTO oauth_pending VALUES("pending");')
        self.db.execute('INSERT INTO sessions VALUES(?,?,?,?)', ('old', token('Ada'), 'old-refresh', 99999999))
        self.db.execute('INSERT INTO sessions VALUES(?,?,?,?)', ('new', token(A), 'new-refresh', 99999999))

    def tearDown(self):
        self.db.close()
        self.temp.cleanup()

    def dump(self):
        return '\n'.join(self.db.iterdump())

    def test_dry_apply_replay_and_preservation(self):
        before = self.dump()
        original_events = self.db.execute('SELECT * FROM events').fetchall()
        self.assertEqual(migrate(self.database, self.mapping)['new_mappings'], 2)
        self.assertEqual(self.dump(), before)
        result = migrate(self.database, self.mapping, True)
        self.assertEqual(result['browser_sessions_expired'], 1)
        self.assertEqual(result['oauth_attempts_expired'], 1)
        value = json.loads(self.db.execute('SELECT document FROM catalog').fetchone()[0])
        app = value['apps']['test']
        self.assertEqual(app['admin_uuid'], A)
        self.assertEqual(app['authors'][0]['uuid'], A)
        self.assertEqual(app['access_uuids'], [B])
        self.assertEqual(app['reviews'][0]['uuid'], B)
        self.assertEqual(app['reviews'][0]['text'], 'Ada')
        self.assertEqual(app['history'][0]['data']['uuid'], B)
        self.assertEqual(app['history'][1]['data']['links']['uuid'], 'Ada')
        self.assertEqual(app['packages'][0]['author_signature']['signature'], 'unchanged-signature')
        self.assertEqual(app['packages'][0]['author_signature']['signer_uuid'], A)
        self.assertEqual(app['releases'][0]['signatures'], self.catalog['apps']['test']['releases'][0]['signatures'])
        self.assertEqual(app['releases'][0]['withdrawn']['by_uuid'], A)
        self.assertEqual(value['platforms'], {A: ['macos-aarch64']})
        self.assertEqual(self.db.execute('SELECT * FROM events').fetchall(), original_events)
        self.assertEqual(result['retired_events'], 1)
        self.assertEqual(result['cancelled_deliveries'], 1)
        self.assertEqual(self.db.execute('SELECT event_seq,reason FROM event_identity_retirements').fetchall(), [(1, 'account_uuid_migrated')])
        self.assertEqual(self.db.execute('SELECT status FROM subscription_deliveries').fetchone()[0], 'failed')
        self.assertEqual(self.db.execute('SELECT owner_uuid,secret FROM subscriptions').fetchone(), (A, 'private-webhook-secret'))
        self.assertEqual(self.db.execute('SELECT owner_uuid,public_key FROM author_keys').fetchone(), (A, 'unchanged-author-public'))
        self.assertEqual(self.db.execute('SELECT actor,secret FROM pending_secrets').fetchone(), (A, 'private-app-secret'))
        replay = self.db.execute('SELECT actor,response FROM idempotency').fetchone()
        self.assertEqual(replay[0], A)
        self.assertEqual(json.loads(replay[1])['app_secret'], 'one-time-private-secret')
        self.assertEqual(self.db.execute('SELECT id FROM sessions').fetchall(), [('new',)])
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE events SET actor_uuid='changed'")
        applied = self.dump()
        self.assertEqual(migrate(self.database, self.mapping, True)['new_mappings'], 0)
        self.assertEqual(self.dump(), applied)
        with self.assertRaises(ValueError):
            migrate(self.database, parse_mapping(CSV.replace(A, '0e2c66f4-fc2f-4b85-a956-701b60570f12')), True)
        self.assertEqual(self.dump(), applied)

    def test_target_and_kind_conflicts_roll_back(self):
        self.db.execute("INSERT INTO author_keys(key_id,owner_uuid,owner_id,public_key,created_at) VALUES('other',?,'c:other','another-public','time')", (A,))
        before = self.dump()
        with self.assertRaises(ValueError):
            migrate(self.database, self.mapping, True)
        self.assertEqual(self.dump(), before)
        self.db.execute("DELETE FROM author_keys WHERE key_id='other'")
        with self.assertRaises(ValueError):
            migrate(self.database, parse_mapping(CSV.replace('carbon', 'silicon')), True)

    def test_partial_export_fails_and_unrelated_canonical_events_stay_deliverable(self):
        before = self.dump()
        with self.assertRaisesRegex(ValueError, 'absent'):
            migrate(self.database, {'Ada': self.mapping['Ada']}, True)
        self.assertEqual(self.dump(), before)
        current = '16e8f0c1-425c-4e76-bb05-17a13579233c'
        self.db.execute("INSERT INTO events(id,type,app_id,actor_uuid,visibility,data,occurred_at) VALUES('current','app.details_changed','test',?,'authors','{\"text\":\"Ada\"}','time')", (current,))
        events = self.db.execute('SELECT * FROM events').fetchall()
        migrate(self.database, self.mapping, True)
        self.assertEqual(self.db.execute('SELECT * FROM events').fetchall(), events)
        self.assertEqual(self.db.execute('SELECT event_seq FROM event_identity_retirements').fetchall(), [(1,)])

    def test_cli_keeps_signing_seeds_installations_and_new_sign_in(self):
        root = self.root / '.apps'
        (root / 'keys').mkdir(parents=True)
        (root / 'keys' / 'private.key').write_bytes(b'private-signing-seed')
        (root / 'installed.json').write_text('{"package":"unchanged"}')
        (root / 'trusted-keys.json').write_text('{"keys":"unchanged"}')
        session = root / 'session.json'
        session.write_text(compact({'tokens': {'access_token': token('Ada'), 'refresh_token': 'old'}}))
        before = session.read_bytes()
        self.assertTrue(migrate_cli(self.root, self.mapping)['sign_in_expired'])
        self.assertEqual(session.read_bytes(), before)
        self.assertTrue(migrate_cli(self.root, self.mapping, True)['sign_in_expired'])
        self.assertFalse(session.exists())
        self.assertEqual((root / 'keys' / 'private.key').read_bytes(), b'private-signing-seed')
        self.assertEqual((root / 'installed.json').read_text(), '{"package":"unchanged"}')
        session.write_text(compact({'tokens': {'access_token': token(A), 'refresh_token': 'fresh'}}))
        self.assertFalse(migrate_cli(self.root, self.mapping, True)['sign_in_expired'])
        self.assertTrue(session.exists())
        with self.assertRaises(ValueError):
            migrate_cli(self.root, parse_mapping(CSV.replace('carbon', 'silicon')), True)

    def test_parser_rejects_ambiguity(self):
        for text in [CSV.replace('old_uuid', 'old'), CSV.replace('4a4d', '1a4d'), CSV.replace(A, A.upper()), CSV + f'New,{A},carbon\n']:
            with self.assertRaises(ValueError):
                parse_mapping(text)

if __name__ == '__main__':
    unittest.main()
