"""Check event bases, human review boundaries and running frozen code against head."""

import json
import os
import sys
import subprocess
import tempfile
import unittest
from pathlib import Path

from guard_integrity import ROOT, base_commit, protected_changes, resolve_base, run_base_guards


class GuardIntegrity(unittest.TestCase):
    def setUp(self):
        scratch = ROOT / '.tmp/integrity-tests'
        scratch.mkdir(parents=True, exist_ok=True)
        directory = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / 'scripts').mkdir()
        (self.root / 'Cargo.toml').write_text('[workspace.lints.rust]\nunsafe_code="deny"\n')
        (self.root / 'input.rs').write_text('good')
        (self.root / 'scripts/policy.json').write_text('{"marker":"base"}')
        (self.root / 'scripts/check_probe.py').write_text('''import json
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
def main():
    assert json.loads((ROOT/'scripts/policy.json').read_text())['marker']=='base'
    return int((ROOT/'input.rs').read_text()=='bad')
if __name__ == '__main__': raise SystemExit(main())
''')
        self.git('init', '-q')
        self.git('add', '.')
        self.commit()
        self.base = self.git('rev-parse', 'HEAD').strip()
        self.commit()

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, stderr=subprocess.PIPE).decode()

    def commit(self):
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-qm', 'fixture')

    def test_pull_request_and_push_use_their_own_base_commits(self):
        self.assertEqual(resolve_base(self.root, {'pull_request': {'base': {'sha': self.base}}}), self.base)
        self.assertEqual(resolve_base(self.root, {'before': self.base}), self.base)
        self.git('update-ref', 'refs/remotes/origin/develop', self.base)
        self.assertEqual(resolve_base(self.root, {'before': '0' * 40}), self.base)
        with self.assertRaisesRegex(ValueError, 'differ from HEAD'):
            base_commit(self.root, 'HEAD')

    def test_ci_base_program_emits_a_commit_line_and_rejects_head(self):
        workflow = json.loads((ROOT / '.github/workflows/guard-integrity.yml').read_text())
        step = workflow['jobs']['guard-base']['steps'][-1]['run']
        program = step.split("\n", 1)[1].rsplit("\nPY", 1)[0]
        event, output = self.root / 'event.json', self.root / 'output'
        environment = os.environ.copy()
        environment.update(GITHUB_EVENT_PATH=str(event), GITHUB_OUTPUT=str(output))
        event.write_text(json.dumps({'before': self.base}))
        result = subprocess.run([sys.executable, '-I', '-c', program], cwd=self.root,
                                env=environment, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output.read_text(), 'commit=' + self.base + '\n')
        event.write_text(json.dumps({'before': self.git('rev-parse', 'HEAD').strip()}))
        result = subprocess.run([sys.executable, '-I', '-c', program], cwd=self.root,
                                env=environment, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('guard base must differ from HEAD', result.stderr)

    def test_frozen_guard_ignores_disabled_head_script_and_expanded_head_policy(self):
        (self.root / 'scripts/check_probe.py').write_text('def main(): return 0')
        (self.root / 'scripts/policy.json').write_text('{"marker":"head"}')
        self.assertEqual(run_base_guards(self.root, self.base), 0)
        (self.root / 'input.rs').write_text('bad')
        self.assertEqual(run_base_guards(self.root, self.base), 1)

    def test_ci_review_does_not_execute_a_disabled_head_integrity_helper(self):
        (self.root / 'scripts/guard_integrity.py').write_text('raise SystemExit(0)')
        workflow = json.loads((ROOT / '.github/workflows/guard-integrity.yml').read_text())
        step = workflow['jobs']['guard-rules-review']['steps'][-1]['run']
        program = step.split("\n", 1)[1].rsplit("\nPY", 1)[0]
        environment = os.environ.copy()
        environment.update(LINT_BASE_REF=self.base, SOURCE_ROOT=str(self.root))
        result = subprocess.run([sys.executable, '-I', '-c', program], env=environment, capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn('scripts/guard_integrity.py: 검사와 규칙 파일이 바뀌었다', result.stdout)

    def test_protected_paths_require_human_review_but_package_metadata_does_not(self):
        self.assertEqual(protected_changes(self.root, self.base), [])
        (self.root / 'Cargo.toml').write_text('[workspace.package]\nversion="2"\n[workspace.lints.rust]\nunsafe_code="deny"\n')
        self.assertEqual(protected_changes(self.root, self.base), [])
        (self.root / 'Cargo.toml').write_text('[workspace.lints.rust]\nunsafe_code="allow"\n')
        (self.root / 'Makefile').write_text('lint:\n\ttrue\n')
        found = protected_changes(self.root, self.base)
        self.assertIn('Cargo.toml [workspace.lints]', found)
        self.assertIn('Makefile', found)


if __name__ == '__main__':
    unittest.main()
