#!/usr/bin/env python3
"""Offline Accounts UUID export consumer. Stop Apps API/workers/CLI updater and back up first.
Preview by default; --apply commits the identical CSV. Never translates authenticated subjects.
"""
import argparse
import base64
import csv
import datetime
import io
import json
import os
from pathlib import Path
import re
import sqlite3
import uuid


def parse_mapping(text):
    reader = csv.reader(io.StringIO(text))
    if next(reader, None) != ['old_uuid', 'new_uuid', 'kind']:
        raise ValueError('Expected exact header old_uuid,new_uuid,kind')
    mapping, targets = {}, set()
    for fields in reader:
        if not fields:
            continue
        if len(fields) != 3:
            raise ValueError('Expected three CSV fields')
        old, new, kind = fields
        parsed = uuid.UUID(new)
        if str(parsed) != new or parsed.version != 4 or not re.fullmatch(r'[A-Za-z0-9]{1,64}|[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}', old):
            raise ValueError('Expected legacy/canonical source and canonical lowercase UUIDv4 target')
        if kind not in ('carbon', 'silicon') or old == new or old in mapping or new in targets:
            raise ValueError('Invalid kind, duplicate mapping or account merge')
        mapping[old] = (new, kind)
        targets.add(new)
    if not mapping or mapping.keys() & targets:
        raise ValueError('Empty, chained and cyclic mappings are forbidden')
    return mapping


def fresh_rows(mapping, ledger):
    for old, link in mapping.items():
        if old in ledger and tuple(ledger[old]) != tuple(link):
            raise ValueError('Mapping conflicts with immutable migration history')
        if any(source != old and (prior[0] == link[0] or prior[0] == old or source == link[0]) for source, prior in ledger.items()):
            raise ValueError('Mapping conflicts with a previous source or target')
    return {old: link for old, link in mapping.items() if old not in ledger}


def compact(value):
    return json.dumps(value, separators=(',', ':'), ensure_ascii=False)


class Rewriter:
    """Only declared identity fields; user strings, provider payloads and signatures stay exact."""
    def __init__(self, mapping):
        self.mapping = mapping
        self.references = set()
        self.kinds = {}

    def account(self, value, public_id=None):
        if value:
            self.references.add(value)
            if public_id and public_id.startswith(('c:', 'si:')):
                kind = 'carbon' if public_id.startswith('c:') else 'silicon'
                self.kinds.setdefault(value, set()).add(kind)
        return self.mapping.get(value, (value, None))[0]

    def field(self, value, key, public_id=None):
        if isinstance(value, dict) and isinstance(value.get(key), str):
            value[key] = self.account(value[key], public_id)

    def package(self, value):
        if isinstance(value, dict) and isinstance(value.get('author_signature'), dict):
            sig = value['author_signature']
            self.field(sig, 'signer_uuid', sig.get('signer_id'))

    def release(self, value):
        if isinstance(value, dict) and isinstance(value.get('withdrawn'), dict):
            withdrawn = value['withdrawn']
            self.field(withdrawn, 'by_uuid', withdrawn.get('by_id'))

    def event_data(self, kind, value):
        if kind in ('author.joined', 'author.invite_accepted', 'author.admin_transferred', 'author.left', 'author.removed'):
            self.field(value, 'uuid')
        elif kind == 'author.invited':
            self.field(value, 'account_uuid', value.get('to'))
        elif kind == 'package.accepted':
            self.package(value)
        elif kind.startswith('release.'):
            self.release(value)

    def app(self, value):
        self.field(value, 'admin_uuid')
        if 'access_uuids' in value:
            value['access_uuids'] = [self.account(x) for x in value['access_uuids']]
        for key in ('authors', 'reviews'):
            for row in value.get(key, []):
                self.field(row, 'uuid', row.get('id'))
        for row in value.get('packages', []):
            self.package(row)
        for row in value.get('releases', []):
            self.release(row)
        for row in value.get('history', []):
            self.field(row, 'actor_uuid')
            self.event_data(row['kind'], row['data'])

    def response(self, value):
        if not isinstance(value, dict):
            return
        if 'authors' in value and 'app_id' in value:
            self.app(value)
        elif 'uuid' in value and ('rating' in value or 'display_name' in value):
            self.field(value, 'uuid', value.get('id'))
        self.field(value, 'account_uuid', value.get('to'))
        self.field(value, 'owner_uuid', value.get('owner_id'))
        self.field(value, 'actor_uuid')
        self.package(value)
        self.release(value)
        for key in ('app', 'release', 'package'):
            self.response(value.get(key))
        for row in value.get('items', []):
            self.response(row)

    def catalog(self, value):
        for app in value['apps'].values():
            self.app(app)
        for invite in value['invites']:
            self.field(invite, 'account_uuid', invite.get('to'))
        for report in value['reports']:
            self.field(report, 'actor_uuid')
        value['platforms'] = {self.account(key): targets for key, targets in value.get('platforms', {}).items()}


def token_subject(token):
    # Inspection only to delete revoked session material, never authentication.
    try:
        part = token.split('.')[1]
        return json.loads(base64.urlsafe_b64decode(part + '=' * (-len(part) % 4)))['sub']
    except (IndexError, ValueError, KeyError, TypeError):
        return None


def migrate(database, mapping, apply=False):
    database = Path(database)
    if not database.is_file() or database.is_symlink():
        raise ValueError('Database must be an existing regular file')
    db = sqlite3.connect(f'{database.resolve().as_uri()}?mode=rw', uri=True, isolation_level=None)
    db.execute('PRAGMA foreign_keys=ON')
    db.execute('BEGIN IMMEDIATE')
    try:
        # Normal additive SQL steps are part of the same rollback/commit boundary.
        # executescript() is deliberately avoided: it would commit before running DDL.
        migrations = sorted((Path(__file__).resolve().parents[1] / 'migrations').glob('*.sql'))
        if not migrations:
            raise ValueError('Run this tool from a complete candidate checkout with migrations/')
        for migration in migrations:
            statement = ''
            for line in migration.read_text().splitlines(keepends=True):
                statement += line
                if sqlite3.complete_statement(statement):
                    db.execute(statement)
                    statement = ''

        ledger = {old: (new, kind) for old, new, kind in db.execute('SELECT old_uuid,new_uuid,kind FROM account_uuid_migrations')}
        fresh = fresh_rows(mapping, ledger)
        rewrite = Rewriter(fresh)
        catalog = json.loads(db.execute('SELECT document FROM catalog WHERE id=1').fetchone()[0])
        rewrite.catalog(catalog)
        blobs = []
        for rowid, kind, data in db.execute('SELECT rowid,type,data FROM events'):
            value = json.loads(data)
            rewrite.event_data(kind, value)
            blobs.append(('events', rowid, 'data', compact(value)))
        for rowid, recipients in db.execute('SELECT rowid,recipient_uuids FROM events'):
            blobs.append(('events', rowid, 'recipient_uuids', compact([rewrite.account(x) for x in json.loads(recipients)])))
        for rowid, kind, body in db.execute('SELECT rowid,kind,body FROM outbox'):
            value = json.loads(body)
            if kind == 'mail.invite':
                rewrite.field(value, 'account_uuid', value.get('to'))
            elif kind == 'mail.report':
                rewrite.field(value, 'actor_uuid')
            blobs.append(('outbox', rowid, 'body', compact(value)))
        for rowid, response in db.execute('SELECT rowid,response FROM idempotency'):
            value = json.loads(response)
            rewrite.response(value)
            blobs.append(('idempotency', rowid, 'response', compact(value)))
        columns = [('idempotency', 'actor'), ('pending_secrets', 'actor'), ('events', 'actor_uuid'), ('subscriptions', 'owner_uuid'), ('author_keys', 'owner_uuid')]
        for table, column in columns:
            for (value,) in db.execute(f'SELECT {column} FROM {table}'):
                rewrite.account(value)
        for table in ('subscriptions', 'author_keys'):
            for owner, public_id in db.execute(f'SELECT owner_uuid,owner_id FROM {table}'):
                rewrite.account(owner, public_id)
        if any(new in rewrite.references for new, _kind in fresh.values()):
            raise ValueError('A target already owns or is referenced by data; refusing merge')
        if any(rewrite.kinds.get(old, {kind}) != {kind} for old, (_new, kind) in fresh.items()):
            raise ValueError('Mapping kind conflicts with stored account identity')
        # The append-only guard is suspended only inside this offline transaction and restored verbatim.
        trigger = db.execute("SELECT sql FROM sqlite_master WHERE type='trigger' AND name='events_are_append_only_update'").fetchone()
        if trigger:
            db.execute('DROP TRIGGER events_are_append_only_update')
        if fresh:
            db.execute('UPDATE catalog SET document=? WHERE id=1', (compact(catalog),))
            for table, rowid, column, value in blobs:
                db.execute(f'UPDATE {table} SET {column}=? WHERE rowid=? AND {column}<>?', (value, rowid, value))
            for old, (new, kind) in fresh.items():
                for table, column in columns:
                    db.execute(f'UPDATE {table} SET {column}=? WHERE {column}=?', (new, old))
                db.execute('INSERT INTO account_uuid_migrations VALUES(?,?,?,?)', (old, new, kind, datetime.datetime.now(datetime.timezone.utc).isoformat()))
        if trigger:
            db.execute(trigger[0])
        tables = {name for (name,) in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        expired = 0
        if fresh and 'sessions' in tables:
            for session, access in db.execute('SELECT id,access_token FROM sessions').fetchall():
                subject = token_subject(access)
                if subject is None or subject in fresh:
                    db.execute('DELETE FROM sessions WHERE id=?', (session,))
                    expired += 1
        pending = db.execute('DELETE FROM oauth_pending').rowcount if fresh and 'oauth_pending' in tables else 0
        result = {'apply': apply, 'mapping_rows': len(mapping), 'new_mappings': len(fresh), 'already_applied': len(mapping) - len(fresh), 'browser_sessions_expired': expired, 'oauth_attempts_expired': pending, 'linked_accounts_seen': len(rewrite.references)}
        db.execute('COMMIT' if apply else 'ROLLBACK')
        return result
    except Exception:
        db.execute('ROLLBACK')
        raise
    finally:
        db.close()


def migrate_cli(home, mapping, apply=False):
    """Explicit home only. Private signing seeds, trust pins and installed packages are untouched."""
    root = Path(home) / '.apps'
    if not root.is_dir() or root.is_symlink():
        raise ValueError('CLI home must contain an existing regular .apps directory')
    ledger_path = root / 'account-uuid-migrations.json'
    session_path = root / 'session.json'
    for path in (ledger_path, session_path):
        if path.is_symlink():
            raise ValueError('CLI state must not be a symbolic link')
    (root / 'locks').mkdir(exist_ok=True, mode=0o700)
    with (root / 'locks' / 'auth').open('a+b') as lock:
        if os.name == 'nt':
            import msvcrt
            lock.seek(0)
            msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        ledger = json.loads(ledger_path.read_text()) if ledger_path.exists() else {}
        fresh = fresh_rows(mapping, ledger)
        session = json.loads(session_path.read_text()) if session_path.exists() else None
        subject = token_subject(session.get('tokens', {}).get('access_token', '')) if session else None
        expired = bool(session and (subject is None or subject in mapping))
        if apply:
            if fresh:
                temporary = root / ('.uuid-mapping-' + uuid.uuid4().hex)
                fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                try:
                    with os.fdopen(fd, 'w') as output:
                        output.write(compact({**ledger, **fresh}))
                        output.flush()
                        os.fsync(output.fileno())
                    os.replace(temporary, ledger_path)
                finally:
                    temporary.unlink(missing_ok=True)
            if expired:
                session_path.unlink()
        return {'apply': apply, 'new_mappings': len(fresh), 'already_applied': len(mapping) - len(fresh), 'sign_in_expired': expired}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mapping', type=Path)
    target = parser.add_mutually_exclusive_group(required=True)
    target.add_argument('--database', type=Path)
    target.add_argument('--cli-home', type=Path)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    try:
        mapping = parse_mapping(args.mapping.read_text())
        result = migrate(args.database, mapping, args.apply) if args.database else migrate_cli(args.cli_home, mapping, args.apply)
    except (ValueError, OSError, sqlite3.Error) as error:
        parser.error(str(error))
    print(json.dumps(result))


if __name__ == '__main__':
    main()
