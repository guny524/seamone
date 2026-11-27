"""A rejected policy must still expose independent compiler diagnostics."""

import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class LintExecution(unittest.TestCase):
    def test_ci_compiler_steps_remain_independent_after_failures(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        for job, required in (
            ('lint-test', ('lint-runtime-minimal', 'lint-format', 'lint-workspace-code',
                           'lint-wasm-code', 'test-runtime-doc')),
            ('macos-native-lint', ('lint-runtime-minimal', 'lint-native-code', 'lint-ios-code')),
        ):
            start = workflow.index('  ' + job + ':\n')
            following = re.search(r'^  [a-z][a-z-]*:', workflow[start + 1:], re.MULTILINE)
            end = start + 1 + following.start() if following else len(workflow)
            steps = re.split(r'^      - ', workflow[start:end], flags=re.MULTILINE)
            for target in required:
                with self.subTest(job=job, target=target):
                    matches = [step for step in steps
                               if re.search(r'run: make ' + target + r'(?:\s|$)', step)]
                    self.assertTrue(matches, (job, target))
                    for step in matches:
                        self.assertIn('if: ${{ !cancelled() }}', step)

    def test_policy_and_minimal_or_native_failures_do_not_hide_other_compilers(self):
        scratch = ROOT / '.tmp/lint-execution'
        scratch.mkdir(parents=True, exist_ok=True)
        for target, failure in (('lint', 'policy'), ('lint-code', '--no-default-features'),
                                ('lint-code', '--workspace'), ('lint-native', 'policy'),
                                ('lint-ios', 'policy')):
            with self.subTest(target=target, failure=failure), tempfile.TemporaryDirectory(dir=scratch) as directory:
                root = Path(directory)
                (root / 'Makefile').write_text((ROOT / 'Makefile').read_text())
                for name, body in {
                    'python3': 'exit 1',
                    'node': 'exit 0',
                    'cargo': 'echo "$*" >> "$COMPILER_LOG"\ncase " $* " in *" $FAIL_COMPILER "*) exit 1;; esac',
                }.items():
                    path = root / name
                    path.write_text('#!/bin/sh\n' + body + '\n')
                    path.chmod(0o755)
                log = root / 'compiler.log'
                result = subprocess.run(
                    ['make', '-f', str(ROOT / 'Makefile'), target],
                    cwd=root,
                    env={**os.environ, 'PATH': str(root) + os.pathsep + os.environ['PATH'],
                         'COMPILER_LOG': str(log), 'FAIL_COMPILER': failure},
                    capture_output=True, text=True, check=False,
                )
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertTrue(log.exists(), result.stdout + result.stderr)
                commands = log.read_text().splitlines()
                self.assertTrue(any(command.startswith('clippy ') for command in commands))
                if target in ('lint', 'lint-code'):
                    self.assertTrue(any('--workspace' in command for command in commands), commands)
                    self.assertTrue(any('--target wasm32-unknown-unknown' in command
                                        for command in commands), commands)


if __name__ == '__main__':
    unittest.main()
