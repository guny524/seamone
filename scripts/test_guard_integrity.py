"""Exercise the actual CI programs with branch, force-push and merge histories."""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from guard_integrity import ROOT, base_commit, workflow_program


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
        self.git('update-ref', 'refs/remotes/origin/develop', self.base)
        self.commit()

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, stderr=subprocess.PIPE).decode()

    def commit(self):
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-qam', 'fixture')

    def run_program(self, file, step, **variables):
        environment = os.environ.copy()
        environment.update(SOURCE_ROOT=str(self.root), **variables)
        return subprocess.run([sys.executable, '-I', '-c', workflow_program(file, step)],
                              cwd=self.root, env=environment, capture_output=True, text=True)

    def event_base(self, event, *, outputs=False):
        path, output = self.root / '.git/event.json', self.root / '.git/output'
        path.write_text(json.dumps(event))
        output.write_text('')
        result = self.run_program('.github/workflows/guard-base.yml', 'Select event base',
                                  GITHUB_EVENT_PATH=str(path), GITHUB_OUTPUT=str(output))
        self.assertEqual(result.returncode, 0, result.stderr)
        selected = dict(line.split('=', 1) for line in output.read_text().splitlines())
        return selected if outputs else selected['commit']

    def test_local_self_comparison_passes_but_a_pr_cannot_select_its_head(self):
        self.assertEqual(base_commit(self.root, 'HEAD', ci=False), self.git('rev-parse', 'HEAD').strip())
        with self.assertRaisesRegex(ValueError, 'differ from HEAD in CI'):
            base_commit(self.root, 'HEAD', ci=True, allow_self=False)
        path = self.root / '.git/event.json'
        path.write_text(json.dumps({'pull_request': {'base': {'sha': self.git('rev-parse', 'HEAD').strip()}}}))
        result = self.run_program('.github/workflows/guard-base.yml', 'Select event base',
                                  GITHUB_EVENT_PATH=str(path), GITHUB_OUTPUT=str(self.root / '.git/output'))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('guard base must differ from HEAD for a pull request', result.stderr)

    def test_pr_develop_and_feature_pushes_keep_the_reviewed_base(self):
        self.assertEqual(self.event_base({'pull_request': {'base': {'sha': self.base}}}), self.base)
        selected = self.event_base({'ref': 'refs/heads/develop', 'before': self.base}, outputs=True)
        self.assertEqual(selected['commit'], self.base)
        self.assertEqual(selected['policy-commit'], self.git('rev-parse', 'HEAD').strip())
        self.assertEqual(selected['allow-self'], 'true')
        previous = self.git('rev-parse', 'HEAD').strip()
        self.commit()
        self.assertEqual(self.event_base({'ref': 'refs/heads/topic', 'before': previous}), self.base)
        self.assertEqual(self.event_base({'ref': 'refs/heads/topic', 'before': '0' * 40}), self.base)

    def test_unavailable_before_falls_back_to_the_develop_merge_base(self):
        self.assertEqual(self.event_base({'ref': 'refs/heads/develop', 'before': '1' * 40}), self.base)

    def test_an_unchanged_branch_push_uses_its_existing_policy(self):
        head = self.git('rev-parse', 'HEAD').strip()
        self.git('update-ref', 'refs/remotes/origin/develop', head)
        selected = self.event_base({'ref': 'refs/heads/fresh', 'before': '0' * 40}, outputs=True)
        self.assertEqual(selected, {'commit': head, 'policy-commit': head, 'allow-self': 'true'})
        self.assertEqual(base_commit(self.root, head, ci=True, allow_self=True), head)
        result = self.run_program('.github/workflows/guard-integrity.yml', 'Run frozen base guards',
                                  LINT_BASE_REF=selected['policy-commit'], GUARD_ALLOW_SELF=selected['allow-self'])
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_an_approved_develop_merge_compares_its_parent_and_uses_merged_policy(self):
        import hashlib
        (self.root / 'guard.rs').write_text('old')
        policy = self.root / 'scripts/policy.json'
        policy.write_text(json.dumps({'sha': hashlib.sha256(b'old').hexdigest()}))
        (self.root / 'scripts/check_probe.py').write_text('''import hashlib,json
from pathlib import Path
root=Path(__file__).resolve().parent.parent
raise SystemExit(int(hashlib.sha256((root/'guard.rs').read_bytes()).hexdigest()!=json.loads((root/'scripts/policy.json').read_text())['sha']))
''')
        self.git('add', '.')
        self.commit()
        parent = self.git('rev-parse', 'HEAD').strip()
        self.git('checkout', '-qb', 'approved')
        (self.root / 'guard.rs').write_text('new')
        policy.write_text(json.dumps({'sha': hashlib.sha256(b'new').hexdigest()}))
        self.commit()
        self.git('checkout', '--detach', parent)
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'merge', '--no-ff', '--no-edit', 'approved')
        head = self.git('rev-parse', 'HEAD').strip()
        selected = self.event_base({'ref': 'refs/heads/develop', 'before': self.base}, outputs=True)
        self.assertEqual(selected, {'commit': parent, 'policy-commit': head, 'allow-self': 'true'})
        stale = self.run_program('.github/workflows/guard-integrity.yml', 'Run frozen base guards', LINT_BASE_REF=parent)
        self.assertEqual(stale.returncode, 1)
        approved = self.run_program('.github/workflows/guard-integrity.yml', 'Run frozen base guards',
                                    LINT_BASE_REF=head, GUARD_ALLOW_SELF='true')
        self.assertEqual(approved.returncode, 0, approved.stderr)

    def test_frozen_guard_ignores_disabled_head_script_and_expanded_head_policy(self):
        (self.root / 'scripts/check_probe.py').write_text('raise SystemExit(0)')
        (self.root / 'scripts/policy.json').write_text('{"marker":"head"}')
        result = self.run_program('.github/workflows/guard-integrity.yml', 'Run frozen base guards', LINT_BASE_REF=self.base)
        self.assertEqual(result.returncode, 0, result.stderr)
        (self.root / 'input.rs').write_text('bad')
        result = self.run_program('.github/workflows/guard-integrity.yml', 'Run frozen base guards', LINT_BASE_REF=self.base)
        self.assertEqual(result.returncode, 1, result.stderr)

    def test_ci_review_does_not_execute_a_disabled_head_integrity_helper(self):
        (self.root / 'scripts/guard_integrity.py').write_text('raise SystemExit(0)')
        result = self.run_program('.github/workflows/guard-review-trusted.yml', 'Require human review of guard changes', LINT_BASE_REF=self.base)
        self.assertEqual(result.returncode, 1)
        self.assertIn('scripts/guard_integrity.py: 검사와 규칙 파일이 바뀌었다', result.stdout)

    def test_a_behind_pr_only_reviews_its_own_diff_and_frozen_checks_use_the_merge(self):
        head = self.git('rev-parse', 'HEAD').strip()
        self.git('checkout', '-qb', 'develop-fixture', self.base)
        (self.root / 'approved.rs').write_text('approved')
        (self.root / 'scripts/check_probe.py').write_text('from pathlib import Path\nassert (Path(__file__).resolve().parent.parent/"approved.rs").read_text()=="approved"\n')
        self.git('add', '.')
        self.commit()
        base = self.git('rev-parse', 'HEAD').strip()
        self.git('checkout', '--detach', head)
        review = self.run_program('.github/workflows/guard-review-trusted.yml', 'Require human review of guard changes', LINT_BASE_REF=base, PR_HEAD_SHA=head)
        self.assertEqual(review.returncode, 0, review.stdout + review.stderr)
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'merge', '--no-edit', base)
        frozen = self.run_program('.github/workflows/guard-integrity.yml', 'Run frozen base guards', LINT_BASE_REF=base)
        self.assertEqual(frozen.returncode, 0, frozen.stdout + frozen.stderr)

    def test_package_metadata_is_allowed_and_lint_policy_needs_review(self):
        (self.root / 'Cargo.toml').write_text('[workspace.package]\nversion="2"\n[workspace.lints.rust]\nunsafe_code="deny"\n')
        self.commit()
        result = self.run_program('.github/workflows/guard-review-trusted.yml', 'Require human review of guard changes', LINT_BASE_REF=self.base)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        (self.root / 'Cargo.toml').write_text('[workspace.lints.rust]\nunsafe_code="allow"\n')
        self.commit()
        result = self.run_program('.github/workflows/guard-review-trusted.yml', 'Require human review of guard changes', LINT_BASE_REF=self.base)
        self.assertEqual(result.returncode, 1)
        self.assertIn('Cargo.toml [workspace.lints]', result.stdout)

    def test_cargo_configuration_toolchain_and_crate_lints_require_review(self):
        cargo = self.root / '.cargo/config.toml'
        cargo.parent.mkdir()
        cargo.write_text('[build]\nrustflags=["--cap-lints=allow"]\n')
        (self.root / 'rust-toolchain.toml').write_text('[toolchain]\nchannel="stable"\n')
        crate = self.root / 'crates/demo/Cargo.toml'
        crate.parent.mkdir(parents=True)
        crate.write_text('[lints.rust]\nunsafe_code="allow"\n')
        self.git('add', '.')
        self.commit()
        result = self.run_program('.github/workflows/guard-review-trusted.yml', 'Require human review of guard changes', LINT_BASE_REF=self.base)
        self.assertEqual(result.returncode, 1)
        for name in ('.cargo/config.toml', 'rust-toolchain.toml', 'crates/demo/Cargo.toml [lints]'):
            self.assertIn(name, result.stdout)


if __name__ == '__main__':
    unittest.main()
