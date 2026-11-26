"""Check SQL writes even when labels are deserialized rather than constructed."""

import json
import re
import sqlite3
import unittest
from unittest.mock import patch

import check_review_writes
from check_review_writes import ROOT, SNAPSHOT, database, functions, violations

PATH = 'crates/seamone-runtime/src/store.rs'


class SQLiteWithoutOptionalFunctions(sqlite3.Connection):
    """Simulate preparation on a build missing SQL functions until a user function is registered."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.registered = set()

    def create_function(self, name, *args, **kwargs):
        super().create_function(name, *args, **kwargs)
        self.registered.add(name)

    def check_functions(self, sql):
        for name in ('unixepoch', 'json_extract'):
            if re.search(r'\b' + name + r'\s*\(', sql, re.I) and name not in self.registered:
                raise sqlite3.OperationalError('no such function: ' + name)

    def execute(self, sql, *args, **kwargs):
        self.check_functions(sql)
        return super().execute(sql, *args, **kwargs)

    def executescript(self, sql):
        self.check_functions(sql)
        return super().executescript(sql)


class ReviewWrites(unittest.TestCase):
    def setUp(self):
        self.db = database()
        self.addCleanup(self.db.close)
        self.policy = {'authorized': [{'path': PATH, 'function': 'Store::mutate'}], 'unverified': []}

    def errors(self, source, policy=None):
        # Small examples need not define the production writer unless they test that signature.
        return '\n'.join(violations(source, PATH, policy or {'authorized': [], 'unverified': []}, self.db))

    def test_inspection_is_identical_with_and_without_optional_sqlite_functions(self):
        connect = sqlite3.connect
        with connect(':memory:', factory=SQLiteWithoutOptionalFunctions) as unsupported:
            for sql in ('EXPLAIN SELECT unixepoch()', 'EXPLAIN SELECT json_extract(?, ?)'):
                with self.subTest(sql=sql), self.assertRaisesRegex(sqlite3.OperationalError, 'no such function'):
                    unsupported.execute(sql)
        unsupported.close()
        source = (ROOT / PATH).read_text(encoding='utf-8')
        policy = json.loads((ROOT / SNAPSHOT).read_text(encoding='utf-8'))
        native = violations(source, PATH, policy, self.db)
        self.assertEqual(native, [])
        with patch.object(check_review_writes.sqlite3, 'connect',
                          side_effect=lambda *args, **kwargs: connect(*args, factory=SQLiteWithoutOptionalFunctions, **kwargs)):
            older = database()
        self.addCleanup(older.close)
        self.assertEqual(violations(source, PATH, policy, older), native)
        forbidden = '''fn bypass() {
            db.execute("UPDATE segments SET body=json_extract(?1,'$.body') WHERE revision<=unixepoch()", params);
            db.execute("INSERT INTO training_queue(segment_id,revision) VALUES (?1,unixepoch())", params);
        }'''
        for db in (self.db, older):
            diagnostic = violations(forbidden, PATH, {'authorized': [], 'unverified': []}, db)
            self.assertEqual(len(diagnostic), 2)
            self.assertTrue(all('outside the authorized review gate' in error for error in diagnostic))

    def test_inspection_functions_override_existing_host_implementations(self):
        connect = sqlite3.connect
        def host_connection(*args, **kwargs):
            db = connect(*args, **kwargs)
            db.create_function('unixepoch', -1, lambda *_: 123)
            db.create_function('json_extract', -1, lambda *_: 'host-specific')
            return db
        with patch.object(check_review_writes.sqlite3, 'connect', side_effect=host_connection):
            db = database()
        self.addCleanup(db.close)
        self.assertEqual(db.execute("SELECT unixepoch(), json_extract('{}','$.a')").fetchone(), (0, None))

    def test_direct_body_write_and_queue_insertion_are_rejected(self):
        source = '''impl Store { pub fn confirm_all(&mut self) {
            let label: VerifiedText = serde_json::from_str(body).unwrap();
            self.db.execute("UPDATE segments SET body=?1 WHERE id=?2", params);
            self.db.execute("INSERT INTO training_queue(segment_id,revision) VALUES (?1,?2)", params);
        }}'''
        diagnostic = self.errors(source)
        self.assertIn('Store::confirm_all writes segments', diagnostic)
        self.assertIn('Store::confirm_all writes training_queue', diagnostic)
        self.assertIn('Store::mutate(AuthorizedReview)', diagnostic)

    def test_authorized_writer_requires_the_evidence_type(self):
        source = '''impl Store { fn mutate(&mut self, authorization: crate::playback_evidence::AuthorizedReview<'_>) {
            self.writes.review(&tx, authorization, &current, &next)?;
            self.db.execute("UPDATE segments SET body=?1 WHERE id=?2", params);
            self.db.execute("INSERT INTO training_queue(segment_id,revision) VALUES (?1,?2)", params);
        }}'''
        self.assertEqual(self.errors(source, self.policy), '')
        self.assertIn('must take AuthorizedReview by value', self.errors(source.replace('authorization: crate::playback_evidence::AuthorizedReview', 'request: ReviewRequest'), self.policy))

    def test_sqlite_handles_quotes_aliases_ctes_and_replacement(self):
        for sql in ['UPDATE "segments" AS s SET "body"=?1 WHERE id=?2',
                    'WITH target AS (SELECT 1) UPDATE segments SET body=?1',
                    'INSERT OR REPLACE INTO "training_queue"(segment_id,revision) VALUES (?1,?2)',
                    '/* leading */ insert\n into training_queue(segment_id,revision) values (?1,?2)']:
            with self.subTest(sql=sql):
                source = 'fn bypass() { db.execute(r###"' + sql + '"###, params); }'
                self.assertIn('outside the authorized review gate', self.errors(source))

    def test_constant_sql_outside_a_function_cannot_hide_a_write(self):
        self.assertIn('<outside function> writes segments', self.errors('const SQL: &str = "UPDATE segments SET body=?1";'))

    def test_nested_function_does_not_inherit_authorization(self):
        source = '''impl Store { fn mutate(&mut self, authorization: crate::playback_evidence::AuthorizedReview<'_>) {
            fn bypass() { db.execute("UPDATE segments SET body=?1", params); }
        }}'''
        self.assertIn('Store::mutate::bypass writes segments', self.errors(source, self.policy))

    def test_reviewed_writer_body_cannot_bypass_admission_after_a_check(self):
        source = '''impl Store { fn mutate(&mut self, authorization: crate::playback_evidence::AuthorizedReview<'_>) {
            self.writes.review(&tx, authorization, &current, &next)?;
            self.db.execute("UPDATE segments SET body=?1 WHERE id=?2", params);
        }}'''
        _, scopes = functions(source)
        policy = {'authorized': [{'path': PATH, 'function': 'Store::mutate', 'body_sha256': scopes[0].digest()}], 'unverified': []}
        self.assertEqual(self.errors(source, policy), '')
        changed = source.replace('self.db.execute', 'next.verified = forged; self.db.execute')
        self.assertIn('changed its reviewed authorization and write body', self.errors(changed, policy))

    def test_trigger_and_view_definitions_are_rejected(self):
        for sql in ("CREATE TRIGGER t AFTER UPDATE ON training_queue BEGIN UPDATE segments SET body='forged'; END",
                    "CREATE TEMP TRIGGER t AFTER UPDATE ON training_queue BEGIN UPDATE segments SET body='forged'; END",
                    "CREATE VIEW v AS SELECT body FROM segments"):
            with self.subTest(sql=sql):
                self.assertIn('defines a trigger or view', self.errors('fn bypass() { db.execute_batch(r###"' + sql + '"###); }'))

    def test_ufcs_and_additional_sql_methods_cannot_hide_concat_sql(self):
        for call in ['Connection::execute(&tx, Q, params)', 'tx.query_one(Q, callback)',
                     'tx.query_row_and_then(Q, params, callback)', 'tx.prepare_with_flags(concat!("UPD","ATE segments SET body=?1"), flags)']:
            with self.subTest(call=call):
                self.assertIn('uses dynamic SQL', self.errors('fn bypass() {' + call + ';}'))

    def test_dynamic_sql_is_rejected_before_it_can_hide_a_write(self):
        for call in ['db.execute(sql, params)', 'db.prepare(&query)', 'db.execute_batch(format!("{}", query))']:
            with self.subTest(call=call):
                self.assertIn('uses dynamic SQL', self.errors('fn bypass() {' + call + ';}'))

    def test_unverified_writer_body_is_frozen_and_cannot_enqueue(self):
        source = 'fn clear() { segment.verified = None; db.execute("UPDATE segments SET body=?1", params); }'
        _, scopes = functions(source)
        policy = {'authorized': [], 'unverified': [{'path': PATH, 'function': 'clear', 'body_sha256': scopes[0].digest()}]}
        self.assertEqual(self.errors(source, policy), '')
        self.assertIn('changed its reviewed unverified-only body', self.errors(source.replace('None', 'Some(label)'), policy))
        changed = source.replace('UPDATE segments SET body=?1', 'INSERT INTO training_queue(segment_id) VALUES (?1)')
        _, scopes = functions(changed)
        policy['unverified'][0]['body_sha256'] = scopes[0].digest()
        self.assertIn('writes training_queue', self.errors(changed, policy))

    def test_reads_state_updates_comments_and_literal_examples_remain_allowed(self):
        source = '''fn work() {
            // db.execute("UPDATE segments SET body=?1", params);
            db.query_row("SELECT body FROM segments WHERE id=?1", params, callback);
            db.execute("UPDATE training_queue SET status='running' WHERE segment_id=?1", params);
            db.execute_batch("DROP TRIGGER fixture;");
            let text = "an example: UPDATE segments SET body=?1";
        }'''
        self.assertEqual(self.errors(source), '')

    def test_multiple_statements_and_unparseable_sql_fail_closed(self):
        self.assertIn('writes segments', self.errors('fn f() { db.execute_batch("SELECT 1; UPDATE segments SET body=?1;"); }'))
        self.assertIn('cannot be inspected', self.errors('fn f() { db.execute("UPDATE segments SET", params); }'))

    def test_rust_escapes_in_non_sql_literals_do_not_affect_sql_inspection(self):
        source = r'''fn normal() {
            let bom = "\u{FEFF}";
            let control = "\x7f";
            let zero = "\0";
            let quote = "\'";
            db.query_row("SELECT 1", [], callback);
        }'''
        self.assertEqual(self.errors(source), '')
        escaped = r'''fn bypass() { db.execute("\x55PDATE segments SET body=?1", params); }'''
        self.assertIn('writes segments', self.errors(escaped))
        bad = r'''fn bypass() { db.execute("UPDATE segments SET body='\q'", params); }'''
        diagnostic = self.errors(bad)
        self.assertIn(PATH + ':1:', diagnostic)
        self.assertIn('invalid Rust SQL literal', diagnostic)

    def test_only_known_migration_includes_are_allowed(self):
        source = 'impl Store { fn open() { db.execute_batch(include_str!("schema-v4.sql")); } }'
        policy = {'authorized': [], 'unverified': [], 'migrations': json.loads((ROOT / SNAPSHOT).read_text())['migrations']}
        self.assertEqual(self.errors(source, policy), '')
        self.assertIn('uses dynamic SQL', self.errors(source.replace('schema-v4.sql', 'other.sql'), policy))


if __name__ == '__main__':
    unittest.main()
