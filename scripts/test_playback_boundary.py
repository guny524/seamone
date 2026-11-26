"""Check sibling constructors, widened visibility and trait-based evidence forgery."""

import subprocess
import tempfile
import unittest

from pathlib import Path

from check_playback_boundary import ROOT, caller_errors, declarations, errors


class PlaybackBoundary(unittest.TestCase):
    def test_a_new_sibling_constructor_is_rejected(self):
        initial = 'impl Playbacks { pub(super) fn issue(&self, store: &Store) -> Reply {} }'
        changed = initial + 'pub(super) fn forge() -> PlaybackProof { todo!() }'
        self.assertIn('unreviewed evidence declaration', '\n'.join(errors(declarations(changed), declarations(initial))))

    def test_visibility_and_signatures_cannot_change_unnoticed(self):
        initial = 'pub(super) struct Proof { request: Request } impl Proof { pub(super) fn request(self) -> Request {} }'
        for changed in [initial.replace('request: Request', 'pub(super) request: Request'),
                        initial.replace('pub(super) fn', 'pub(crate) fn'),
                        initial.replace('self)', '&self)')]:
            with self.subTest(changed=changed):
                self.assertTrue(errors(declarations(changed), declarations(initial)))

    def test_trait_and_derived_constructors_are_frozen(self):
        initial = 'pub(super) struct Playbacks { state: State }'
        for changed in ['#[derive(Default)]' + initial,
                        initial + 'impl Default for Playbacks { fn default() -> Self { todo!() } }',
                        initial + 'impl From<Request> for Proof { fn from(r: Request) -> Self { todo!() } }']:
            with self.subTest(changed=changed):
                self.assertTrue(errors(declarations(changed), declarations(initial)))

    def test_test_helpers_cannot_become_production_constructors(self):
        source = '#[cfg(test)] pub(super) fn mutate_after_playback() {}'
        self.assertTrue(errors(declarations(source.replace('#[cfg(test)]', '')), declarations(source)))

    def test_only_service_entry_points_and_test_scopes_may_call_evidence(self):
        scratch = ROOT / '.tmp/caller-tests'
        scratch.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=scratch) as directory:
            root = Path(directory)
            path = root / 'crates/seamone-runtime/src/lib.rs'
            path.parent.mkdir(parents=True)
            subprocess.run(['git', 'init', '-q'], cwd=root, check=True)
            source = '''impl LibraryService { fn playback() { self.listening.issue_playback_token(store,id,rev); } }
            #[cfg(test)] mod tests { fn helper() { p.issue_playback_token(store,id,rev); p.mark_playback_ended(token); p.authorize_review(req,token); } }
            fn bypass() { p.issue_playback_token(store,id,rev); p.mark_playback_ended(token); p.authorize_review(req,token); }
            '''
            path.write_text(source)
            policy = [{'path': str(path.relative_to(root)), 'function': 'LibraryService::playback', 'method': 'issue_playback_token'}]
            errors = caller_errors(root, policy)
            self.assertEqual(len(errors), 3)
            self.assertTrue(all('bypass' in error for error in errors))
            path.write_text(source.replace('fn bypass()', '#[cfg(test)] fn bypass()'))
            self.assertEqual(caller_errors(root, policy), [])
            path.write_text('fn normal() { let _ = self.ended; other.issue(); other.authorize(); }')
            self.assertEqual(caller_errors(root, policy), [])
            path.write_text('fn bypass() { call!(p, mark_playback_ended); }')
            self.assertIn('cannot call mark_playback_ended', '\n'.join(caller_errors(root, policy)))

    def test_private_helpers_comments_and_literals_do_not_expand_access(self):
        initial = 'pub(super) fn issue() {}'
        changed = initial + 'fn begin() {} // pub(super) fn forge() {}\n const TEXT: &str = "impl Default for Proof {}";'
        self.assertEqual(errors(declarations(changed), declarations(initial)), [])

    def test_duplicate_declarations_are_rejected_across_cfg_branches(self):
        declaration = 'pub(super) fn issue() {}'
        self.assertTrue(errors(declarations(declaration * 2), declarations(declaration)))


if __name__ == '__main__':
    unittest.main()
