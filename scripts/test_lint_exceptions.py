"""Check nested Rust attributes, inline Python directives and configuration suppressions."""

import unittest
from pathlib import Path

from check_lint_exceptions import (
    additions, configuration_exceptions, python_exceptions, rust_exceptions,
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


if __name__ == "__main__":
    unittest.main()
