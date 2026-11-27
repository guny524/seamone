"""A rejected policy must still expose independent compiler diagnostics."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class LintExecution(unittest.TestCase):
    def test_policy_failure_does_not_hide_clippy(self):
        scratch = ROOT / '.tmp/lint-execution'
        scratch.mkdir(parents=True, exist_ok=True)
        for target in ('lint', 'lint-native', 'lint-ios'):
            with self.subTest(target=target), tempfile.TemporaryDirectory(dir=scratch) as directory:
                root = Path(directory)
                (root / 'Makefile').write_text((ROOT / 'Makefile').read_text())
                for name, body in {
                    'python3': 'exit 1',
                    'node': 'exit 0',
                    'cargo': 'echo "$*" >> "$COMPILER_LOG"',
                }.items():
                    path = root / name
                    path.write_text('#!/bin/sh\n' + body + '\n')
                    path.chmod(0o755)
                log = root / 'compiler.log'
                result = subprocess.run(
                    ['make', '-f', str(ROOT / 'Makefile'), target],
                    cwd=root,
                    env={**os.environ, 'PATH': str(root) + os.pathsep + os.environ['PATH'],
                         'COMPILER_LOG': str(log)},
                    capture_output=True, text=True, check=False,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(log.exists(), result.stdout + result.stderr)
                self.assertIn('clippy', log.read_text())


if __name__ == '__main__':
    unittest.main()
