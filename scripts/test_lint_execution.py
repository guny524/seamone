"""Workflow prerequisites and compiler diagnostics remain independently checked."""

import os
import re
import shlex
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def workflow_jobs(source):
    start = re.search(r'^jobs:[ \t]*$', source, re.MULTILINE)
    if start is None:
        raise ValueError('workflow must declare a jobs mapping')
    body = source[start.end():]
    following = re.search(r'^\S[^\n]*:', body, re.MULTILINE)
    if following:
        body = body[:following.start()]
    jobs = list(re.finditer(r'^  ([\w-]+):[ \t]*$', body, re.MULTILINE))
    if not jobs:
        raise ValueError('workflow jobs must use named block mappings')
    return [(job[1], body[job.end():jobs[index + 1].start() if index + 1 < len(jobs) else len(body)])
            for index, job in enumerate(jobs)]


def workflow_steps(job):
    steps = re.split(r'^      - ', job, flags=re.MULTILINE)[1:]
    if not steps and re.search(r'^\s+(?:- )?run:', job, re.MULTILINE):
        raise ValueError('workflow run steps must use the repository step indentation')
    return steps


def setup_python(step):
    action = re.search(r'^[ \t]*uses: (actions/setup-python@[^\s#]+)', step, re.MULTILINE)
    if action is None:
        return None
    version = re.search(r'^[ \t]*python-version:[ \t]*[\'"]?([0-9.]+)[\'"]?[ \t]*(?:#.*)?$', step, re.MULTILINE)
    return action[1], version[1] if version else None


def make_step(step):
    run = re.search(r'^[ \t]*run:[ \t]*(.*)', step, re.MULTILINE)
    if run is None:
        return False
    command = step[run.end():] if run[1].startswith(('|', '>')) else run[1].strip()
    if len(command) > 1 and command[0] in {"'", '"'} and command[-1] == command[0]:
        command = command[1:-1]
    for line in command.replace('\\\n', ' ').splitlines():
        lexer = shlex.shlex(line, posix=True, punctuation_chars=';&|()')
        lexer.whitespace_split = True
        first = True
        try:
            for word in lexer:
                if all(char in ';&|()' for char in word):
                    first = True
                elif first and (word in {'env', 'command', 'sudo', 'time', 'if', 'then', 'do', '!'}
                                or re.match(r'^[A-Za-z_]\w*=', word)):
                    continue
                elif first:
                    if word.rsplit('/', 1)[-1] in {'make', 'gmake', 'make.exe'}:
                        return True
                    first = False
        except ValueError as error:
            raise ValueError(f'cannot inspect workflow run command: {line}') from error
    return False


def workflow_python_errors(source, reference):
    errors = []
    for job, body in workflow_jobs(source):
        active = None
        for step in workflow_steps(body):
            if make_step(step) and active != reference:
                errors.append(f'{job}: make requires {reference[0]} with Python {reference[1]} before the step')
            configured = setup_python(step)
            if configured:
                active = None if re.search(r'^[ \t]*if:', step, re.MULTILINE) else configured
    return errors


class LintExecution(unittest.TestCase):
    def python_reference(self):
        jobs = dict(workflow_jobs((ROOT / '.github/workflows/ci.yml').read_text()))
        configured = [setup_python(step) for step in workflow_steps(jobs['lint-test'])]
        reference = next(value for value in configured if value is not None)
        self.assertRegex(reference[0], r'^actions/setup-python@[a-f0-9]{40}$')
        self.assertRegex(reference[1], r'^3\.[0-9]+(?:\.[0-9]+)?$')
        self.assertGreaterEqual(int(reference[1].split('.')[1]), 11)
        return reference

    def test_every_workflow_make_job_pins_the_ci_python_before_make(self):
        reference = self.python_reference()
        for path in sorted((ROOT / '.github/workflows').iterdir()):
            if path.suffix in {'.yml', '.yaml'}:
                with self.subTest(workflow=path.name):
                    self.assertEqual(workflow_python_errors(path.read_text(), reference), [])

    def test_python_setup_rejects_removal_wrong_version_unpinned_action_and_late_setup(self):
        action, version = self.python_reference()
        setup = f"      - uses: {action}\n        with:\n          python-version: '{version}'\n"
        make = '      - run: make build\n'
        header = 'jobs:\n  installers:\n    steps:\n'
        self.assertEqual(workflow_python_errors(header + setup + make, (action, version)), [])
        for steps in [make, make + setup, setup.replace(version, '3.10') + make,
                      setup.replace(action, 'actions/setup-python@v6') + make,
                      setup.replace('        with:', "        if: runner.os == 'macOS'\n        with:") + make]:
            with self.subTest(steps=steps):
                self.assertTrue(workflow_python_errors(header + steps, (action, version)))
        self.assertTrue(workflow_python_errors(header + setup + setup.replace(version, '3.10') + make, (action, version)))

    def test_make_detection_covers_shell_blocks_and_ignores_installer_arguments(self):
        for step in ['run: make check-imports', 'run: "make build"',
                     'name: installer\n        run: |\n          make build ARGS=x\n',
                     'run: SEAMONE_CHROME="browser path" make ui-test',
                     'run: echo ready; make build', 'run: env FLAGS=1 make -C research test-offline']:
            with self.subTest(step=step):
                self.assertTrue(make_step(step))
        for step in ['run: choco install make --version=4.4.1', 'run: echo "make build"',
                     'name: make build\nrun: echo ready', 'run: |\n  # make build\n  echo ready\n']:
            with self.subTest(step=step):
                self.assertFalse(make_step(step))

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
