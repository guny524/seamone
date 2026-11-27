"""Check SQL writes even when labels are deserialized rather than constructed."""

import json
import re
import sqlite3
import unittest
from unittest.mock import patch

import check_review_writes
from check_review_writes import ROOT, SNAPSHOT, database, persistence_digest, violations

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

    def errors(self, source, policy=None):

        return '\n'.join(violations(source, PATH, policy or {'modules': []}, self.db))

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
            diagnostic = violations(forbidden, PATH, {'modules': []}, db)
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
        self.assertIn('WritePermit', diagnostic)

    def test_reviewed_writer_owns_fixed_sql(self):
        source = '''impl Permit { fn review() {
            db.execute("UPDATE segments SET body=?1 WHERE id=?2", params);
            db.execute("INSERT INTO training_queue(segment_id,revision) VALUES (?1,?2)", params);
        }}'''
        policy = {'modules': [{'path': PATH, 'sha256': persistence_digest(source)}]}
        self.assertEqual(self.errors(source, policy), '')
        self.assertIn('reviewed persistence module changed', self.errors(source.replace('body=?1', 'body=?3'), policy))

    def test_persistence_imports_types_and_visibility_are_reviewed_but_test_bodies_can_grow(self):
        source = '''use actual::Authority;
        struct Permit;
        impl Permit { fn review() { db.execute("UPDATE segments SET body=?1", params); } }
        #[cfg(test)] mod tests { fn normal() {} }'''
        policy = {'modules': [{'path': PATH, 'sha256': persistence_digest(source)}]}
        self.assertEqual(self.errors(source, policy), '')
        for changed in (source.replace('actual::Authority', 'fake::Authority'),
                        source.replace('struct Permit', 'pub(super) struct Permit')):
            self.assertIn('reviewed persistence module changed', self.errors(changed, policy))
        self.assertEqual(self.errors(source.replace('fn normal() {}', 'fn another_case() { hidden_sql(); }'), policy), '')

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

    def test_a_nested_write_changes_the_reviewed_persistence_module(self):
        source = 'impl Permit { fn review() {} }'
        policy = {'modules': [{'path': PATH, 'sha256': persistence_digest(source)}]}
        changed = source.replace('fn review() {}', 'fn review() { fn bypass() { db.execute("UPDATE segments SET body=?1", params); } }')
        self.assertIn('Permit::review::bypass writes segments', self.errors(changed, policy))

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

    def test_queue_identity_updates_cannot_retarget_an_existing_training_job(self):
        for column in ('segment_id', 'revision', 'recording_revision'):
            source = f'fn retarget() {{ db.execute("UPDATE training_queue SET {column}=?1", params); }}'
            with self.subTest(column=column):
                self.assertIn('writes training_queue', self.errors(source))

    def test_connection_opening_sql_references_and_hidden_source_are_rejected(self):
        for source in (
            'fn bypass() { let c = rusqlite::Connection::open(path); }',
            'fn bypass() { <rusqlite::Connection>::open(path); }',
            'type Db = rusqlite::Connection; fn bypass() { Db::open(path); }',
            'fn bypass() { db.authorizer(None::<fn(rusqlite::hooks::AuthContext)>); }',
            'use rusqlite::Connection as Db; fn bypass() { Db::open_in_memory(); }',
            'fn bypass() { let run = Connection::execute; }',
            'fn bypass() { stringify!(UPDATE segments SET body = ?); }',
            'fn bypass() { include_str!("hidden.sql"); }',
            '#[path="../hidden.rs"] mod ext;',
            'macro_rules! dispatch { ($x:expr,$m:ident) => { $x.$m() } }',
        ):
            with self.subTest(source=source):
                self.assertTrue(self.errors(source))
        for source in (
            'impl Store { fn open() { Connection::open(path); } }',
            '#[cfg(test)] #[path="../fixture.rs"] mod fixture;',
            '#[cfg(test)] mod fixtures { #[path="../fixture.rs"] mod child; }',
            '#[cfg(test)] fn fixture() { Connection::open_in_memory(); include_str!("fixture.sql"); }',
        ):
            with self.subTest(source=source):
                self.assertEqual(self.errors(source), '')

    def test_conditional_module_paths_require_test_only_items(self):
        for attribute in (
            '#[cfg_attr(all(), path="../store_ext.rs")]',
            '#[cfg_attr(feature="external", path="../store_ext.rs")]',
        ):
            source = attribute + ' mod external;'
            self.assertIn('cannot use #[path]', self.errors(source))
            self.assertEqual(self.errors('#[cfg(test)] ' + source), '')
        self.assertEqual(self.errors('fn normal() { let path = "audio"; }'), '')

    def test_reads_state_updates_comments_and_literal_examples_remain_allowed(self):
        source = '''fn work() {
            // db.execute("UPDATE segments SET body=?1", params);
            db.query_row("SELECT body FROM segments WHERE id=?1", params, callback);
            db.execute("UPDATE training_queue SET status='running' WHERE segment_id=?1", params);
            db.execute_batch("DROP TRIGGER fixture;");
            let text = "an example: UPDATE segments SET body=?1";
        }'''
        self.assertEqual(self.errors(source), '')

    def test_prepared_statement_parameters_are_data_and_local_shadowing_is_respected(self):
        source = '''fn marks() {
            let mut s = tx.prepare_cached("INSERT INTO settings(key,value) VALUES (?1,?2)")?;
            for mark in marks { s.execute(params![mark.id, "UPDATE segments SET body='example'"])?; }
            let mut read = tx.prepare("SELECT value FROM settings WHERE key=?1")?;
            read.query_row(["key"], callback)?;
            { let s = connection; s.execute(sql, params); }
            s.execute(params)?;
        }'''
        diagnostic = self.errors(source)
        self.assertEqual(diagnostic.count('uses dynamic SQL'), 1, diagnostic)
        self.assertNotIn('writes segments', diagnostic)
        self.assertIn('uses dynamic SQL', self.errors('fn bypass() { let s = choose(prepare); s.execute(sql, params); }'))
        self.assertIn('writes segments', self.errors('fn bypass() { let mut s = tx.prepare("UPDATE segments SET body=?1")?; s.execute(params)?; }'))

    def test_sql_arguments_are_inspected_after_transaction_control_or_empty_prefixes(self):
        for prefix in ('SAVEPOINT a;', 'VALUES(1);', ';'):
            source = f'fn bypass() {{ db.execute_batch("{prefix} UPDATE segments SET body=?1;"); }}'
            with self.subTest(prefix=prefix):
                self.assertIn('writes segments', self.errors(source))
        self.assertEqual(self.errors('fn normal() { db.execute_batch("; SAVEPOINT a; SELECT 1; RELEASE a;"); }'), '')

    def test_raw_statement_apis_require_the_reviewed_module(self):
        for source in (
            'fn bypass() { rusqlite::Batch::new(&tx, &sql); }',
            'fn bypass() { statement.raw_execute(); }',
            'fn bypass() { statement.raw_bind_parameter(1, value); }',
        ):
            with self.subTest(source=source):
                self.assertIn('raw SQL API', self.errors(source))

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
        policy = {'modules': [], 'migrations': json.loads((ROOT / SNAPSHOT).read_text())['migrations']}
        self.assertEqual(self.errors(source, policy), '')
        self.assertIn('uses dynamic SQL', self.errors(source.replace('schema-v4.sql', 'other.sql'), policy))


if __name__ == '__main__':
    unittest.main()
