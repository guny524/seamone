"""Check deletion syntax and the actual cache boundary without loading model libraries."""

import importlib.util
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from check_research_deletions import ENTRY_FILE, ROOT, violations

# The cache module and paths use only the standard library, unlike audio preparation.
spec = importlib.util.spec_from_file_location('cache_boundary', ROOT / ENTRY_FILE)
cache = importlib.util.module_from_spec(spec)
with patch.dict(sys.modules, {}):
    sys.path.insert(0, str(ROOT / 'research'))
    try:
        spec.loader.exec_module(cache)
    finally:
        sys.path.pop(0)


class DeletionSyntax(unittest.TestCase):
    def check(self, source, path='research/seamone_research/data.py'):
        return violations(source, path)

    def test_direct_modules_and_import_aliases_are_rejected(self):
        for source in [
            'import shutil\nshutil.rmtree(folder)',
            'import shutil as s\ns.move(source, destination)',
            'from shutil import rmtree as r\nr(folder)',
            'import os as operating\noperating.remove(file)',
            'from os import unlink as delete\ndelete(file)',
            'import os\nos.rmdir(folder)',
            'import os\nos.removedirs(folder)',
        ]:
            with self.subTest(source=source):
                errors = self.check(source)
                self.assertTrue(errors)
                self.assertIn('cache.remove_inside', errors[0])
                self.assertIn(ENTRY_FILE, errors[0])

    def test_paths_parameters_and_chained_constructors_are_rejected(self):
        for source in [
            'from pathlib import Path as P\nP("x").unlink()',
            'def clean(path):\n    path.rmdir()',
            'from pathlib import Path\nPath.unlink(path)',
            'def clean(module):\n    module.remove(path)',
        ]:
            with self.subTest(source=source):
                self.assertTrue(self.check(source))

    def test_callable_aliases_cannot_escape_as_arguments_or_return_values(self):
        for source in [
            'import shutil\nerase=shutil.rmtree\nerase(path)',
            'import os\nrun_callback(os.unlink)',
            'import shutil\ndef factory():\n    return shutil.rmtree',
            'import os\ndef f(value: os.remove(path)): pass',
        ]:
            with self.subTest(source=source):
                self.assertTrue(self.check(source))

    def test_reflection_and_import_loaders_are_rejected(self):
        for source in [
            'import shutil\ngetattr(shutil, "rmtree")(path)',
            'import shutil as s\nfrom builtins import getattr as g\ng(s,"rmtree")(path)',
            '__import__("shutil").rmtree(path)',
            'loader=__import__\nloader("os").unlink(path)',
            '__import__("os.path").remove(path)',
            'import importlib as i\ni.import_module("shutil").move(a,b)',
            'import os\ngetattr(os,"un"+"link")(path)',
            'import shutil\nshutil.__getattribute__("rmtree")(path)',
            'import os\nos.__dict__["unlink"](path)',
            'import os\nvars(os)["remove"](path)',
        ]:
            with self.subTest(source=source):
                self.assertTrue(self.check(source))

    def test_unknown_filesystem_lookup_and_wildcard_import_fail_closed(self):
        for source in [
            '__import__(module_name)',
            'import importlib\nimportlib.import_module(module_name)',
            'import shutil\ngetattr(shutil, method)(path)',
            'def cleanup(obj, name):\n    getattr(obj,name)(path)',
            'import os\nos.__dict__[name](path)',
            'from shutil import *',
        ]:
            with self.subTest(source=source):
                self.assertTrue(self.check(source))

    def test_only_the_exact_entry_function_can_delete(self):
        source = 'import shutil\ndef remove_inside(root,target):\n    shutil.rmtree(target)'
        self.assertEqual(self.check(source, ENTRY_FILE), [])
        self.assertTrue(self.check(source))
        nested = 'import shutil\ndef remove_inside(root,target):\n    def other():\n        shutil.rmtree(target)'
        self.assertTrue(self.check(nested, ENTRY_FILE))
        self.assertTrue(self.check(source.replace('remove_inside', 'other'), ENTRY_FILE))
        self.assertTrue(self.check(source + '\n' + source, ENTRY_FILE))
        self.assertTrue(self.check(source.replace('shutil.rmtree(target)', 'f=lambda: shutil.rmtree(target)'), ENTRY_FILE))

    def test_tests_have_no_direct_deletion_exemption(self):
        self.assertTrue(self.check('def test_cache(tmp_path):\n    tmp_path.unlink()', 'research/tests/test_cache.py'))

    def test_reads_collections_and_torch_dtype_lookup_are_allowed(self):
        source = '''import torch
from seamone_research import cache
value = getattr(torch, dtype)
cache.remove_inside(root, target)
items = [1,2]
items.remove(1)
def f(items: list[str]):
    items.remove("x")
text = "shutil.rmtree(path)"
# path.unlink()
'''
        self.assertEqual(self.check(source), [])

    def test_syntax_errors_do_not_skip_the_check(self):
        self.assertTrue(self.check('def invalid('))


class CacheBoundary(unittest.TestCase):
    def setUp(self):
        scratch = ROOT / '.tmp/research-deletion-tests'
        scratch.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.home = self.root / 'hf'
        self.home.mkdir()
        environment = patch.dict(os.environ, {'HF_HOME': str(self.home)})
        environment.start()
        self.addCleanup(environment.stop)

    def test_owned_directories_are_removed_without_touching_shared_hub(self):
        shared = self.home / 'hub/datasets--example/blobs/sentinel'
        shared.parent.mkdir(parents=True)
        shared.write_text('keep')
        raw = self.home / 'eval-raw/set'
        raw.mkdir(parents=True)
        (raw / 'piece').write_text('drop')
        cache.remove_inside(raw.parent, raw)
        self.assertFalse(raw.exists())
        self.assertEqual(shared.read_text(), 'keep')

    def test_root_outside_and_outward_symlinks_are_rejected(self):
        owned, outside = self.home / 'eval-raw', self.root / 'outside'
        owned.mkdir()
        outside.mkdir()
        (outside / 'sentinel').write_text('keep')
        link = owned / 'link'
        link.symlink_to(outside, target_is_directory=True)
        for target in (owned, outside, link):
            with self.subTest(target=target), self.assertRaises(ValueError):
                cache.remove_inside(owned, target)
        self.assertEqual((outside / 'sentinel').read_text(), 'keep')

    def test_a_caller_cannot_designate_an_arbitrary_directory_as_cache(self):
        unowned = self.root / 'unowned'
        unowned.mkdir()
        sentinel = unowned / 'sentinel'
        sentinel.write_text('keep')
        with self.assertRaises(ValueError):
            cache.remove_inside(unowned, sentinel, file=True)
        self.assertEqual(sentinel.read_text(), 'keep')

    def test_shared_home_and_hub_are_rejected_even_with_a_broad_declared_root(self):
        hub = self.home / 'hub'
        hub.mkdir()
        sentinel = hub / 'sentinel'
        sentinel.write_text('keep')
        for root, target in [(self.root, self.home), (self.home, hub), (hub, sentinel)]:
            with self.subTest(target=target), self.assertRaises(ValueError):
                cache.remove_inside(root, target, file=target == sentinel)
        self.assertEqual(sentinel.read_text(), 'keep')

    def test_file_removal_keeps_missing_and_directory_error_semantics(self):
        owned = self.home / 'seamone_eval/set'
        owned.mkdir(parents=True)
        manifest = owned / 'manifest.json'
        manifest.write_text('partial')
        cache.remove_inside(owned, manifest, file=True)
        self.assertFalse(manifest.exists())
        cache.remove_inside(owned, manifest, file=True, missing_ok=True)
        with self.assertRaises(FileNotFoundError):
            cache.remove_inside(owned, manifest, file=True)
        folder = owned / 'directory'
        folder.mkdir()
        with self.assertRaises(OSError):
            cache.remove_inside(owned, folder, file=True, missing_ok=True)
        self.assertTrue(folder.is_dir())


if __name__ == '__main__':
    unittest.main()
