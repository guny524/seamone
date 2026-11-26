"""Check every public declaration shape, including fields and grouped exports."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import check_runtime_api
from check_runtime_api import inventory, surface


class RuntimeApi(unittest.TestCase):
    def test_public_shapes_and_fields(self):
        source = '''pub use store::{Store, Other as Alias};
        pub mod service;
        pub struct Bag { pub value: Option<Vec<u8>>, private: bool }
        pub enum State { Idle, Active }
        pub const READY: bool = true;
        impl Bag { pub fn open(path: impl AsRef<Path>) -> Result<Self, String> { todo!() } }
        pub(crate) fn internal() {}
        '''
        found = list(surface(source))
        self.assertEqual(len(found), 7)
        self.assertIn('pub value : Option < Vec < u8 > >', found)
        self.assertIn('pub fn open ( path : impl AsRef < Path > ) - > Result < Self , String >', found)
        self.assertIn('pub use store : : { Store , Other as Alias }', found)

    def test_comments_literals_and_restricted_items_do_not_export(self):
        source = '''// pub use store::Store;
        /* pub fn hidden() {} /* nested */ */
        const TEXT: &str = r###"pub fn forged() {}"###;
        pub(super) struct Hidden;
        '''
        self.assertEqual(list(surface(source)), [])

    def test_unsafe_async_and_inline_module_are_not_skipped(self):
        found = list(surface('pub mod api { pub async unsafe fn call() {} }'))
        self.assertEqual(found, ['pub mod api {', 'pub async unsafe fn call ( )'])

    def test_implicit_public_enum_variants_and_trait_members_are_frozen(self):
        initial = 'pub enum State { Idle } pub trait Trainer { fn train(); }'
        changed = initial.replace('Idle', 'Idle, RawStore(Store)').replace('fn train();', 'fn train(); fn mutate();')
        self.assertNotEqual(list(surface(initial)), list(surface(changed)))

    def test_public_file_modules_are_followed_recursively(self):
        scratch = check_runtime_api.ROOT / '.tmp/api-tests'
        scratch.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=scratch) as directory:
            root = Path(directory)
            runtime = root / 'src'
            (runtime / 'public').mkdir(parents=True)
            (runtime / 'lib.rs').write_text('pub mod public; mod private;')
            (runtime / 'public.rs').write_text('pub mod nested; pub struct View { pub raw: Store }')
            (runtime / 'public/nested.rs').write_text('pub fn mutate() {}')
            (runtime / 'private.rs').write_text('pub fn hidden() {}')
            with patch.object(check_runtime_api, 'ROOT', root), patch.object(check_runtime_api, 'RUNTIME', runtime):
                found = inventory()
            self.assertIn(['src/public/nested.rs', 'pub fn mutate ( )'], found)
            self.assertIn(['src/public.rs', 'pub raw : Store'], found)
            self.assertFalse(any('hidden' in declaration for _, declaration in found))


if __name__ == '__main__':
    unittest.main()
