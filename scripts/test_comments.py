"""Comments are lexed as comments, and a PR cannot approve its own additions."""

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from check_comments import inventory, python_comments, rust_comments, violations
from check_lint_exceptions import git
from snapshot_baselines import errors

ROOT = Path(__file__).resolve().parent.parent


class CommentSyntax(unittest.TestCase):
    def test_rust_strings_characters_docs_and_nested_block_comments(self):
        source = '''//! module docs
/// public docs
/** more docs */
/*! module docs */
let raw = r##"// text /* text */"##;
let escaped = "quote \\" // text";
let character = '/';
// explanation
/* outer /* inner */ end */
//// ordinary
/*** ordinary */
'''
        found = [comment for _, comment in rust_comments(source)]
        self.assertEqual(found, ['// explanation', '/* outer /* inner */ end */',
                                 '//// ordinary', '/*** ordinary */'])

    def test_python_strings_docstrings_shebang_and_encoding(self):
        source = '#!/usr/bin/env python3\n# coding: utf-8\n"""# module documentation"""\ntext = "# text"\nvalue = 1 # explanation\n'
        self.assertEqual(list(python_comments(source)), [(5, '# explanation')])

    def test_only_module_class_and_function_first_strings_are_documentation(self):
        source = '''"""module docs"""
class Work:
    """class docs"""
    async def run(self):
        """function docs"""
        value = "data"
        "hidden explanation"
        return value
if True:
    """conditional explanation"""
'''
        self.assertEqual(list(python_comments(source)), [
            (10, '"""conditional explanation"""'), (7, '"hidden explanation"')])

    def test_unclosed_rust_comment_fails_closed(self):
        with self.assertRaises(ValueError):
            list(rust_comments('/* unfinished'))


class CommentBaseline(unittest.TestCase):
    def setUp(self):
        scratch = ROOT / '.tmp/comment-tests'
        scratch.mkdir(parents=True, exist_ok=True)
        temp = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        (self.root / 'scripts').mkdir()
        self.path = self.root / 'scripts/comments.json'
        self.path.write_text(json.dumps({'comments': []}))
        git(self.root, 'init', '-q')
        git(self.root, 'add', '.')
        self.commit()

    def commit(self):
        git(self.root, '-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
            '-c', 'commit.gpgsign=false', 'commit', '-qm', 'baseline')

    def test_code_and_documentation_without_ordinary_comments_pass(self):
        (self.root / 'lib.rs').write_text('/// Contract\nfn work() {}\n')
        (self.root / 'code.py').write_text('"""Contract"""\nvalue = "# data"\n')
        self.assertEqual(violations(self.root, []), [])
        self.assertEqual(errors(self.root, 'scripts/comments.json', 'HEAD'), [])

    def test_source_comment_names_the_structural_fix(self):
        (self.root / 'lib.rs').write_text('// Preserve the exact request on retry.\nfn work() {}\n')
        diagnostics = '\n'.join(violations(self.root, []))
        self.assertIn('lib.rs:1', diagnostics)
        self.assertIn('이름과 구조로 드러내거나 검사로 옮기세요', diagnostics)

    def test_snapshot_addition_does_not_approve_the_source_comment(self):
        (self.root / 'code.py').write_text('# external constraint\n')
        entries = [{'path': path, 'sha256': digest, 'text': text}
                   for path, _, digest, text in inventory(self.root)]
        self.path.write_text(json.dumps({'comments': entries}))
        self.assertEqual(violations(self.root, entries), [])
        self.assertIn('unreviewed snapshot addition', '\n'.join(
            errors(self.root, 'scripts/comments.json', 'HEAD')))

    def test_reviewed_comment_cannot_be_edited_duplicated_or_moved(self):
        source = self.root / 'lib.rs'
        comment = '// External API constraint.'
        source.write_text(comment + '\n')
        entries = [{'path': 'lib.rs', 'sha256': hashlib.sha256(comment.encode()).hexdigest(),
                    'text': comment}]
        self.path.write_text(json.dumps({'comments': entries}))
        git(self.root, 'add', '.')
        self.commit()
        self.assertEqual(violations(self.root, entries), [])
        for changed in (comment + '\n' + comment, '// Changed constraint.'):
            source.write_text(changed + '\n')
            self.assertIn('unreviewed comment', '\n'.join(violations(self.root, entries)))
        source.rename(self.root / 'moved.rs')
        self.assertIn('unreviewed comment', '\n'.join(violations(self.root, entries)))

    def test_snapshot_text_must_match_its_hash_and_source(self):
        comment = '# external constraint'
        (self.root / 'code.py').write_text(comment + '\n')
        entry = {'path': 'code.py', 'sha256': hashlib.sha256(comment.encode()).hexdigest(),
                 'text': comment}
        self.assertEqual(violations(self.root, [entry]), [])
        for changed in ({**entry, 'text': '# different'}, {**entry, 'sha256': '0' * 64},
                        {'path': entry['path'], 'sha256': entry['sha256']}):
            self.assertIn('comment text and sha256 must match',
                          '\n'.join(violations(self.root, [changed])))

    def test_missing_base_or_snapshot_fails_closed(self):
        self.assertIn('cannot read reviewed snapshot', '\n'.join(
            errors(self.root, 'scripts/comments.json', 'missing')))
        git(self.root, 'rm', 'scripts/comments.json')
        self.commit()
        self.path.parent.mkdir(exist_ok=True)
        self.path.write_text(json.dumps({'comments': []}))
        self.assertIn('cannot read reviewed snapshot', '\n'.join(
            errors(self.root, 'scripts/comments.json', 'HEAD')))


if __name__ == '__main__':
    unittest.main()
