"""Exercise Cargo's interpretation of dependency aliases, platforms and kinds without compiling."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

CHECK = Path(__file__).with_name("check_crate_boundaries.py").resolve()


class DependencyBoundaries(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        (self.root / "Cargo.toml").write_text('[workspace]\nmembers = ["*layer"]\nresolver = "3"\n')
        for layer in ("core", "asr", "ui"):
            self.crate(f"{layer}-layer", f"seamone-{layer}")

    def crate(self, folder, name):
        path = self.root / folder
        (path / "src").mkdir(parents=True)
        (path / "src/lib.rs").touch()
        (path / "Cargo.toml").write_text(f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2024"\n')

    def add(self, folder, toml):
        with (self.root / folder / "Cargo.toml").open("a") as out:
            out.write(toml)

    def check(self):
        return subprocess.run([sys.executable, CHECK], cwd=self.root, capture_output=True, text=True)

    def test_ui_can_use_domain(self):
        self.add("ui-layer", '[dependencies]\ndomain = {package="seamone-core", path="../core-layer"}\n')
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_ui_cannot_alias_engine_on_another_platform(self):
        self.add("ui-layer", """[target.'cfg(target_os = "ios")'.dependencies]
engine = {package="seamone-asr", path="../asr-layer", optional=true}
""")
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("seamone-ui -> seamone-asr", result.stderr)
        self.assertIn("ios", result.stderr)

    def test_domain_cannot_hide_io_in_build_or_dev_dependencies(self):
        self.add("core-layer", '[build-dependencies]\nserde_json = "1"\n[dev-dependencies]\ndb = {package="rusqlite", version="0.39"}\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("seamone-core -> serde_json (build", result.stderr)
        self.assertIn("seamone-core -> rusqlite (dev", result.stderr)

    def test_ui_cannot_access_native_plugin_directly(self):
        self.add("ui-layer", '[dependencies]\nfilesystem = {package="tauri-plugin-fs", version="2"}\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("import tauri-plugin-fs in seamone", result.stderr)

    def test_new_workspace_crate_needs_an_explicit_boundary(self):
        self.crate("extra-layer", "bridge")
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("classify workspace crate bridge", result.stderr)


if __name__ == "__main__":
    unittest.main()
