"""Check SQL writes even when labels are deserialized rather than constructed."""

import unittest

from check_review_writes import database, functions, violations

PATH = 'crates/seamone-runtime/src/store.rs'


class ReviewWrites(unittest.TestCase):
    def setUp(self):
        self.db = database()
        self.addCleanup(self.db.close)
        self.policy = {'authorized': [{'path': PATH, 'function': 'Store::mutate'}], 'unverified': []}

    def errors(self, source, policy=None):
        # Small examples need not define the production writer unless they test that signature.
        return '\n'.join(violations(source, PATH, policy or {'authorized': [], 'unverified': []}, self.db))

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

    def test_only_known_migration_includes_are_allowed(self):
        source = 'impl Store { fn open() { db.execute_batch(include_str!("schema-v4.sql")); } }'
        self.assertEqual(self.errors(source), '')
        self.assertIn('uses dynamic SQL', self.errors(source.replace('schema-v4.sql', 'other.sql')))


if __name__ == '__main__':
    unittest.main()
