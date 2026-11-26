"""Check base absence, self-edits, duplicate allowances and unreadable snapshots."""

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from check_runtime_api import ROOT
from snapshot_baselines import SNAPSHOTS, errors


class ReviewedSnapshots(unittest.TestCase):
    def setUp(self):
        scratch = ROOT / '.tmp/snapshot-tests'
        scratch.mkdir(parents=True, exist_ok=True)
        directory = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / 'scripts').mkdir()
        self.data = {path: {key: ['reviewed'] for key in keys} for path, keys in SNAPSHOTS.items()}
        for path, data in self.data.items():
            self.save(path, data)
        self.git('init', '-q')
        self.commit()

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, stderr=subprocess.PIPE)

    def commit(self):
        self.git('add', '.')
        self.git('-c', 'user.name=Architecture Test', '-c', 'user.email=test@invalid',
                 '-c', 'commit.gpgsign=false', 'commit', '-qm', 'reviewed snapshots')

    def save(self, path, data):
        (self.root / path).parent.mkdir(exist_ok=True)
        (self.root / path).write_text(json.dumps(data))

    def test_unchanged_lists_pass_and_removals_are_allowed(self):
        for path, keys in SNAPSHOTS.items():
            with self.subTest(path=path):
                self.assertEqual(errors(self.root, path, 'HEAD'), [])
                self.save(path, {key: [] for key in keys})
                self.assertEqual(errors(self.root, path, 'HEAD'), [])

    def test_every_list_rejects_self_approval_and_duplicate_counts(self):
        for path, keys in SNAPSHOTS.items():
            for key in keys:
                for addition in ('forged', 'reviewed'):
                    with self.subTest(path=path, key=key, addition=addition):
                        data = json.loads(json.dumps(self.data[path]))
                        data[key].append(addition)
                        self.save(path, data)
                        diagnostic = '\n'.join(errors(self.root, path, 'HEAD'))
                        self.assertIn('unreviewed snapshot addition', diagnostic)
                        self.assertIn('이 PR의 CI는 사람이 보고 판단', diagnostic)

    def test_missing_base_file_never_bootstraps_from_current_code(self):
        for path in SNAPSHOTS:
            self.git('rm', path)
        self.commit()
        for path, data in self.data.items():
            self.save(path, data)
            self.assertIn('cannot read reviewed snapshot', '\n'.join(errors(self.root, path, 'HEAD')))

    def test_unknown_ref_and_broken_current_json_fail_closed(self):
        for path in SNAPSHOTS:
            with self.subTest(path=path):
                self.assertIn('cannot read reviewed snapshot', '\n'.join(errors(self.root, path, 'missing-ref')))
                (self.root / path).write_text('{')
                self.assertIn('cannot read reviewed snapshot', '\n'.join(errors(self.root, path, 'HEAD')))


if __name__ == '__main__':
    unittest.main()
