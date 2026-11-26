"""Check unsafe forgery rejection and base-protected adapter allowances."""

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from check_unsafe_files import BASELINE, ROOT, check, unsafe_lines
from check_review_writes import functions


class UnsafeSyntax(unittest.TestCase):
    def test_blocks_functions_impls_and_traits_are_detected(self):
        source = 'unsafe { read(p); }\nunsafe fn f() {}\nunsafe impl Send for X {}\nunsafe trait T {}'
        self.assertEqual(list(unsafe_lines(source)), [1, 2, 3, 4])

    def test_comments_and_rust_literals_do_not_count(self):
        source = '''// unsafe {}
        /* unsafe fn f() {} /* unsafe impl Send for X {} */ */
        const A: &str = "unsafe {}";
        const B: &str = r###"unsafe {}"###;
        const C: char = 'u';'''
        self.assertEqual(list(unsafe_lines(source)), [])


class UnsafeBoundary(unittest.TestCase):
    def setUp(self):
        scratch = ROOT / '.tmp/unsafe-tests'
        scratch.mkdir(parents=True, exist_ok=True)
        directory = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / 'scripts').mkdir()
        (self.root / 'capture.rs').write_text('fn capture() { unsafe { native(); } }')
        self.save(['capture.rs'])
        self.git('init', '-q')
        self.commit()

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, stderr=subprocess.PIPE)

    def commit(self):
        self.git('add', '.')
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '-qm', 'baseline')

    def save(self, files):
        (self.root / BASELINE).parent.mkdir(exist_ok=True)
        records = []
        for name in files:
            _, scopes = functions((self.root / name).read_text())
            records.extend({'path': name, 'function': scope.name, 'body_sha256': scope.digest()} for scope in scopes)
        (self.root / BASELINE).write_text(json.dumps({'functions': records}))

    def errors(self):
        return '\n'.join(check(self.root, 'HEAD'))

    def test_existing_adapter_passes(self):
        self.assertEqual(self.errors(), '')

    def test_untracked_unsafe_forgery_is_rejected(self):
        (self.root / 'evidence.rs').write_text('unsafe fn forge() {}')
        self.assertIn('evidence.rs:1: unsafe is forbidden', self.errors())

    def test_an_allowance_cannot_be_added_alongside_the_forgery(self):
        (self.root / 'evidence.rs').write_text('fn forge() { unsafe { read(p) } }')
        self.save(['capture.rs', 'evidence.rs'])
        self.assertIn('unreviewed snapshot addition', self.errors())

    def test_unknown_ref_missing_and_malformed_snapshot_fail_closed(self):
        self.assertIn('cannot read reviewed snapshot', '\n'.join(check(self.root, 'missing-ref')))
        self.git('rm', BASELINE)
        self.assertIn('cannot read reviewed snapshot', self.errors())
        self.save(['capture.rs'])
        (self.root / BASELINE).write_text('{')
        self.assertIn('cannot read reviewed snapshot', self.errors())

    def test_missing_base_snapshot_cannot_bootstrap_from_the_source(self):
        self.git('rm', BASELINE)
        self.commit()
        self.save(['capture.rs'])
        self.assertIn('cannot read reviewed snapshot', self.errors())

    def test_a_file_move_needs_a_reviewed_base_allowance(self):
        self.git('mv', 'capture.rs', 'native.rs')
        self.save(['native.rs'])
        self.assertIn('unreviewed snapshot addition', self.errors())

    def test_another_function_in_the_same_file_cannot_inherit_unsafe_permission(self):
        source = self.root / 'capture.rs'
        source.write_text(source.read_text() + '\nunsafe fn forge() {}')
        self.assertIn('unsafe is forbidden here', self.errors())
        source.write_text('fn capture() { unsafe { changed_native_call(); } }')
        self.assertIn('reviewed unsafe function capture changed', self.errors())

    def test_capture_evidence_dependencies_and_runtime_include_are_rejected(self):
        path = self.root / 'crates/seamone-runtime/src/capture.rs'
        path.parent.mkdir(parents=True)
        path.write_text('use crate::playback_evidence; use crate::store::Store; fn f() { include!("hidden.inc"); std::mem::transmute_copy(p); }')
        diagnostic = self.errors()
        self.assertIn('runtime include! is forbidden', diagnostic)
        self.assertIn('capture cannot access store/playback evidence', diagnostic)

    def test_removing_an_allowance_does_not_allow_remaining_unsafe(self):
        self.save([])
        self.assertIn('capture.rs:1: unsafe is forbidden', self.errors())


if __name__ == '__main__':
    unittest.main()
