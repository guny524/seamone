"""Check nested Rust attributes, inline Python directives and configuration suppressions."""

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from check_lint_exceptions import (
    additions, check, clippy_commands, configuration, configuration_exceptions,
    python_exceptions, rust_exceptions,
)


class LintExceptions(unittest.TestCase):
    def test_cfg_attr_and_multiline_expect_are_detected(self):
        source = '#[cfg_attr(mobile, expect(unused_variables, reason = "platform"))]\nfn a() {}'
        self.assertEqual(len(list(rust_exceptions(source))), 1)
        self.assertEqual(len(list(rust_exceptions('#![allow(dead_code)]'))), 1)
        self.assertEqual(list(rust_exceptions('#[derive(Clone)]')), [])

    def test_duplicate_exception_and_changed_reason_need_review(self):
        entry = ("file.rs", '#[expect(dead_code, reason = "bounded")]')
        self.assertFalse(additions([entry], [entry]))
        self.assertEqual(additions([entry, entry], [entry])[entry], 1)
        self.assertTrue(additions([("other.rs", entry[1])], [entry]))

    def test_python_directives_are_comments_not_string_contents(self):
        source = 'value = "# noqa"\nvalue = 1  # noqa: F401\nitem = 2  # type: ignore\n'
        self.assertEqual(list(python_exceptions(source)), ['# noqa: F401', '# type: ignore'])

    def test_configuration_allow_and_ruff_ignores_are_detected(self):
        cargo = '[lints.clippy]\ndead_code = {level="allow"}\n'
        self.assertEqual(list(configuration_exceptions(Path("Cargo.toml"), cargo)), ["clippy.dead_code=allow"])
        ruff = '[tool.ruff.lint]\nignore = ["F401"]\n'
        self.assertTrue(list(configuration_exceptions(Path("pyproject.toml"), ruff)))

    def test_inline_parameters_closures_and_nested_comments(self):
        source = '''fn work(#[expect(unused_variables)] arg: u8) {
        let closure = |#[allow(unused)] x: u8| 0;
        let string = r##"#[expect(unused)]"##;
        /* #[allow(dead_code)] /* nested */ */
        }'''
        self.assertEqual(list(rust_exceptions(source)), [
            '#[expect(unused_variables)]', '#[allow(unused)]',
        ])


class BaselineProtection(unittest.TestCase):
    def setUp(self):
        scratch = Path(__file__).resolve().parent.parent / '.tmp/lint-tests'
        scratch.mkdir(parents=True, exist_ok=True)
        temp = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        (self.root / 'scripts').mkdir()
        self.baseline = self.root / 'scripts/lint_exceptions.json'
        self.cargo = '[workspace.lints.clippy]\npedantic={level="warn",priority=-1}\n'
        self.ruff = '[tool.ruff.lint]\nselect=["E","F"]\n[tool.ruff.lint.mccabe]\nmax-complexity=15\n'
        self.make = 'lint:\n\tcargo clippy --workspace -- -D warnings\n'
        for name, content in [('Cargo.toml', self.cargo), ('pyproject.toml', self.ruff), ('Makefile', self.make)]:
            (self.root / name).write_text(content)
        self.data = {'existing': [], 'configuration': {
            'Cargo.toml': configuration(Path('Cargo.toml'), self.cargo),
            'pyproject.toml': configuration(Path('pyproject.toml'), self.ruff),
        }, 'clippy_commands': clippy_commands(self.make)}
        self.save()
        self.git('init', '-q')
        self.git('add', '.')
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '-qm', 'baseline')

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, stderr=subprocess.PIPE)

    def save(self):
        self.baseline.parent.mkdir(exist_ok=True)
        self.baseline.write_text(json.dumps(self.data))

    def errors(self):
        return '\n'.join(check(self.root, 'HEAD'))

    def test_unchanged_snapshot_passes(self):
        self.assertEqual(self.errors(), '')

    def test_unreadable_ref_and_missing_json_fail_closed(self):
        self.assertIn('cannot read lint baseline', '\n'.join(check(self.root, 'missing-ref')))
        self.git('rm', 'scripts/lint_exceptions.json')
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '-qm', 'remove')
        self.save()
        self.assertIn('cannot read lint baseline', self.errors())

    def test_baseline_cannot_grow_alongside_the_source(self):
        (self.root / 'lib.rs').write_text('fn f(#[expect(unused)] a: u8) {}')
        self.data['existing'].append(['lib.rs', '#[expect(unused)]'])
        self.save()
        self.assertIn('new lint suppression', self.errors())

    def test_duplicate_count_cannot_grow(self):
        self.data['existing'] = [['lib.rs', '#[expect(unused)]']]
        self.save()
        self.git('add', '.')
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '-qm', 'reviewed entry')
        self.data['existing'] *= 2
        self.save()
        self.assertIn('1 new lint suppression', self.errors())

    def test_deleted_cargo_rule_and_workspace_inheritance_are_detected(self):
        for content in ('', '[lints]\nworkspace=true\n'):
            with self.subTest(content=content):
                (self.root / 'Cargo.toml').write_text(content)
                self.assertIn('lint tables changed', self.errors())

    def test_ruff_selection_and_threshold_cannot_weaken(self):
        for content in (self.ruff.replace('["E","F"]', '["E"]'),
                        self.ruff.replace('=15', '=99'), ''):
            with self.subTest(content=content):
                (self.root / 'pyproject.toml').write_text(content)
                self.assertIn('lint tables changed', self.errors())

    def test_changing_the_configuration_snapshot_does_not_bypass_base(self):
        (self.root / 'Cargo.toml').write_text('')
        self.data['configuration']['Cargo.toml'] = configuration(Path('Cargo.toml'), '')
        self.save()
        self.assertIn('configuration vs HEAD', self.errors())

    def test_enabling_documentation_and_names_lints_preserves_the_other_rules(self):
        self.cargo += 'missing_errors_doc="allow"\nmissing_panics_doc="allow"\nmodule_name_repetitions="allow"\n'
        (self.root / 'Cargo.toml').write_text(self.cargo)
        self.data['configuration']['Cargo.toml'] = configuration(Path('Cargo.toml'), self.cargo)
        self.data['existing'] = list(configuration_exceptions(Path('Cargo.toml'), self.cargo))
        self.data['existing'] = [['Cargo.toml', item] for item in self.data['existing']]
        self.save()
        self.git('add', '.')
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '-qm', 'documented baseline')
        tightened = self.cargo.replace('"allow"', '"warn"')
        (self.root / 'Cargo.toml').write_text(tightened)
        self.data['existing'] = []
        self.data['configuration']['Cargo.toml'] = configuration(Path('Cargo.toml'), tightened)
        self.save()
        self.assertEqual(self.errors(), '')
        weakened = tightened.replace('priority=-1', 'priority=-2')
        (self.root / 'Cargo.toml').write_text(weakened)
        self.data['configuration']['Cargo.toml'] = configuration(Path('Cargo.toml'), weakened)
        self.save()
        self.assertIn('lint tables changed', self.errors())

    def test_removing_ruff_ignores_does_not_allow_weaker_selection(self):
        original = self.ruff.replace('select=["E","F"]', 'select=["E","F"]\nignore=["E501"]')
        (self.root / 'pyproject.toml').write_text(original)
        self.data['configuration']['pyproject.toml'] = configuration(Path('pyproject.toml'), original)
        self.save()
        self.git('add', '.')
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '-qm', 'ruff baseline')
        (self.root / 'pyproject.toml').write_text(self.ruff)
        self.data['configuration']['pyproject.toml'] = configuration(Path('pyproject.toml'), self.ruff)
        self.save()
        self.assertEqual(self.errors(), '')
        changed = self.ruff.replace('["E","F"]', '["E"]')
        (self.root / 'pyproject.toml').write_text(changed)
        self.data['configuration']['pyproject.toml'] = configuration(Path('pyproject.toml'), changed)
        self.save()
        self.assertIn('lint tables changed', self.errors())

    def test_alternate_configuration_files_are_forbidden(self):
        for name in ('clippy.toml', '.clippy.toml', 'ruff.toml', '.ruff.toml'):
            with self.subTest(name=name):
                path = self.root / name
                path.write_text('')
                self.assertIn(name + ': alternate lint configuration is forbidden', self.errors())
                path.unlink()

    def test_cargo_rustflags_allow_is_forbidden(self):
        (self.root / '.cargo').mkdir()
        for name in ('config', 'config.toml'):
            path = self.root / '.cargo' / name
            for flags in ('[build]\nrustflags=["-A", "warnings"]\n',
                          '[target.x86_64-unknown-linux-gnu]\nrustflags="--allow=warnings"\n',
                          '[build]\nrustflags=["--cap-lints=allow"]\n'):
                with self.subTest(name=name, flags=flags):
                    path.write_text(flags)
                    self.assertIn('-A/--allow/--cap-lints rustflags are forbidden', self.errors())
            path.unlink()

    def test_clippy_flags_cannot_be_removed_or_suppressed(self):
        for command in (self.make.replace(' -D warnings', ''), self.make.replace('-D warnings', '-D warnings -A unused'), self.make.replace('-D warnings', '-D warnings --cap-lints=allow'), ''):
            with self.subTest(command=command):
                (self.root / 'Makefile').write_text(command)
                self.assertIn('Makefile: clippy', self.errors())

    def test_additional_strict_mobile_command_preserves_existing_recipes(self):
        mobile = '\tcargo clippy --locked -p seamone --target aarch64-apple-ios-sim -- -D warnings\n'
        (self.root / 'Makefile').write_text(self.make + 'lint-ios:\n' + mobile)
        self.assertEqual(self.errors(), '')
        for weakened in (mobile.replace(' -D warnings', ''),
                         mobile.replace(' -D warnings', ' -D warnings -A unused')):
            (self.root / 'Makefile').write_text(self.make + 'lint-ios:\n' + weakened)
            self.assertIn('clippy must keep -D warnings', self.errors())
        (self.root / 'Makefile').write_text('lint-ios:\n' + mobile)
        self.assertIn('clippy recipes differ', self.errors())

    def test_move_preserves_exception_scope_and_copy_does_not(self):
        entry = '#[expect(unused)]'
        source = 'fn f(' + entry + ' arg: u8) {}'
        (self.root / 'old.rs').write_text(source)
        self.data['existing'] = [['old.rs', entry]]
        self.save()
        self.git('add', '.')
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '-qm', 'old source')
        self.git('mv', 'old.rs', 'moved.rs')
        self.data['existing'] = [['moved.rs', entry]]
        self.save()
        self.assertEqual(self.errors(), '')
        (self.root / 'old.rs').write_text(source)
        self.git('add', 'old.rs')
        self.data['existing'].append(['old.rs', entry])
        self.save()
        self.assertIn('new lint suppression', self.errors())

    def test_new_cargo_member_must_inherit_frozen_workspace_rules(self):
        manifest = self.root / 'member/Cargo.toml'
        manifest.parent.mkdir()
        source = '[lints]\nworkspace=true\n'
        manifest.write_text(source)
        self.data['configuration']['member/Cargo.toml'] = configuration(manifest, source)
        self.save()
        self.assertEqual(self.errors(), '')
        source = '[lints.clippy]\npedantic="warn"\n'
        manifest.write_text(source)
        self.data['configuration']['member/Cargo.toml'] = configuration(manifest, source)
        self.save()
        self.assertIn('lint tables changed', self.errors())

    def test_runtime_scope_cannot_be_removed_after_the_split(self):
        original = self.make + 'native:\n\tcargo clippy -p seamone -p seamone-runtime --all-targets -- -D warnings\n'
        (self.root / 'Makefile').write_text(original)
        self.data['clippy_commands'] = clippy_commands(original)
        self.save()
        self.git('add', '.')
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '-qm', 'runtime lint scope')
        weakened = original.replace(' -p seamone-runtime', '')
        (self.root / 'Makefile').write_text(weakened)
        self.data['clippy_commands'] = clippy_commands(weakened)
        self.save()
        self.assertIn('clippy recipes differ', self.errors())

    def test_pre_split_baseline_requires_the_expanded_runtime_scope(self):
        original = self.make + 'native:\n\tcargo clippy -p seamone --all-targets $(CARGO_FEATURE_ARGS) -- -D warnings\n'
        (self.root / 'Makefile').write_text(original)
        self.data['clippy_commands'] = clippy_commands(original)
        self.save()
        self.git('add', '.')
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '-qm', 'pre-split native lint')
        expanded = original.replace('-p seamone ', '-p seamone -p seamone-runtime ').replace('$(CARGO_FEATURE_ARGS)', '$(WORKSPACE_FEATURE_ARGS)')
        (self.root / 'Makefile').write_text(expanded)
        self.data['clippy_commands'] = clippy_commands(expanded)
        self.save()
        self.assertEqual(self.errors(), '')
        weakened = expanded.replace(' -p seamone-runtime', '')
        (self.root / 'Makefile').write_text(weakened)
        self.data['clippy_commands'] = clippy_commands(weakened)
        self.save()
        self.assertIn('clippy recipes differ', self.errors())


if __name__ == "__main__":
    unittest.main()
