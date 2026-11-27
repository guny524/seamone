"""Exercise Cargo's interpretation of dependency aliases, platforms and kinds without compiling."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from check_crate_boundaries import ui_output_errors, ui_html_errors

CHECK = Path(__file__).with_name("check_crate_boundaries.py").resolve()


class DependencyBoundaries(unittest.TestCase):
    def setUp(self):
        scratch = CHECK.parent.parent / ".tmp" / "architecture-tests"
        scratch.mkdir(parents=True, exist_ok=True)
        temp = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        (self.root / "Cargo.toml").write_text('[workspace]\nmembers = ["*layer"]\nresolver = "3"\n')
        for layer in ("core", "asr", "ui", "runtime"):
            self.crate(f"{layer}-layer", f"seamone-{layer}")
        self.crate("host-layer", "seamone")
        self.add("runtime-layer", '[features]\ntest-support = []\n')

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

    def test_ui_cannot_substitute_the_registry_for_the_workspace_domain(self):
        self.add("ui-layer", '[dependencies]\ndomain = {package="seamone-core", version="0.1"}\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("depend on the workspace copy of seamone-core", result.stderr)

    def test_new_workspace_crate_needs_an_explicit_boundary(self):
        self.crate("extra-layer", "bridge")
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("classify workspace crate bridge", result.stderr)

    def test_host_uses_runtime_instead_of_engine(self):
        self.add("host-layer", '[dependencies]\nengine = {package="seamone-asr", path="../asr-layer"}\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("seamone_runtime::LibraryService", result.stderr)

    def test_host_cannot_own_sqlite_or_wav_io(self):
        for dependency in ("rusqlite", "hound"):
            with self.subTest(dependency=dependency):
                manifest = self.root / "host-layer/Cargo.toml"
                original = manifest.read_text()
                try:
                    self.add("host-layer", f'[dependencies]\n{dependency} = "*"\n')
                    result = self.check()
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("seamone-runtime", result.stderr)
                    self.assertIn("seamone_runtime::LibraryService", result.stderr)
                finally:
                    manifest.write_text(original)

    def test_runtime_cannot_import_tauri_even_as_a_gated_alias(self):
        self.add("runtime-layer", '[target.\'cfg(target_os="ios")\'.build-dependencies]\nshell = {package="tauri", version="2"}\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("import tauri in seamone", result.stderr)

    def test_host_can_use_runtime_and_runtime_can_use_engine(self):
        self.add("host-layer", '[dependencies]\nservice = {package="seamone-runtime", path="../runtime-layer"}\n')
        self.add("runtime-layer", '[dependencies]\nengine = {package="seamone-asr", path="../asr-layer"}\n')
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_ui_can_decode_runtime_playback_wav(self):
        self.add("ui-layer", '[dependencies]\nhound = "3.5.1"\n')
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_host_cannot_use_an_unlisted_native_dependency(self):
        self.add("host-layer", '[dependencies]\nsqlite = {package="libsqlite3-sys", version="*"}\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("put native work in seamone-runtime", result.stderr)

    def test_support_is_only_allowed_on_the_host_dev_edge(self):
        for kind in ("dependencies", "build-dependencies", "dev-dependencies"):
            with self.subTest(kind=kind):
                path = self.root / "host-layer/Cargo.toml"
                original = path.read_text()
                try:
                    self.add("host-layer", f'[{kind}]\nservice = {{package="seamone-runtime", path="../runtime-layer", features=["test-support"]}}\n')
                    result = self.check()
                    self.assertEqual(result.returncode == 0, kind == "dev-dependencies", result.stderr)
                finally:
                    path.write_text(original)

    def test_support_cannot_be_forwarded_by_a_product_feature(self):
        self.add("host-layer", '[dependencies]\nservice = {package="seamone-runtime", path="../runtime-layer"}\n[features]\ndefault=["service/test-support"]\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("test-support belongs only", result.stderr)

    def test_support_cannot_be_a_runtime_default(self):
        self.add("runtime-layer", 'default=["test-support"]\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("test-support belongs only", result.stderr)

    def test_ui_cannot_enable_browser_output_through_gated_aliased_dependencies(self):
        for kind in ['dependencies', 'build-dependencies', 'target.\'cfg(target_os="ios")\'.dev-dependencies']:
            for feature in ['HtmlAudioElement', 'HtmlMediaElement']:
                with self.subTest(kind=kind, feature=feature):
                    path = self.root / 'ui-layer/Cargo.toml'
                    original = path.read_text()
                    try:
                        self.add('ui-layer', f'[{kind}]\nbrowser = {{package="web-sys",version="0.3",features=["{feature}"]}}\n')
                        result = self.check()
                        self.assertNotEqual(result.returncode, 0)
                        self.assertIn('UI web-sys output feature', result.stderr)
                    finally:
                        path.write_text(original)

    def test_ui_cannot_forward_optional_web_output_but_can_render_waveforms(self):
        self.add('ui-layer', '[dependencies]\nbrowser = {package="web-sys",version="0.3",features=["CanvasRenderingContext2d"],optional=true}\n')
        self.assertEqual(self.check().returncode, 0)
        self.add('ui-layer', '[features]\nplayback=["browser?/HtmlMediaElement"]\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('UI web-sys output feature', result.stderr)

    def test_ui_media_tags_types_and_javascript_are_rejected_even_if_transitive_bindings_exist(self):
        for source in ['view! { <audio autoplay=true/> }', 'view! { <video/> }',
                       'use web_sys::HtmlAudioElement as Player;', 'leptos::html::Audio',
                       '#[wasm_bindgen(inline_js = "new Audio()")]',
                       'element.set_inner_html(r#"<audio src=x>"#)']:
            with self.subTest(source=source):
                self.assertTrue(list(ui_output_errors(source)))
        self.assertEqual(list(ui_output_errors('// HtmlAudioElement\nlet caption = "audio recording"; view! { <canvas/> }')), [])

    def test_ui_source_scan_is_part_of_the_cargo_boundary_command(self):
        source = self.root / 'ui-layer/src/lib.rs'
        source.write_text('fn player() { let _ = web_sys::HtmlMediaElement::play; }')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('UI media API is forbidden', result.stderr)

    def test_static_dom_literals_urls_and_embedded_documents_are_rejected(self):
        for source in ['document.create_element("audio")', 'document.create_element("video")',
                       'node.set_attribute("src", "data:audio/wav;base64,AAA")',
                       'format!("data:video/mp4;base64,{wav}")',
                       'let name = r#"audio"#;', r'let name = "\x61udio";',
                       'view! { <iframe/> }', 'view! { <embed/> }', 'view! { <object/> }']:
            with self.subTest(source=source):
                self.assertTrue(list(ui_output_errors(source)))
        self.assertEqual(list(ui_output_errors('let caption = "audio recording"; view! { <canvas/> }')), [])

    def test_trunk_html_is_checked_and_normal_html_still_passes(self):
        page = self.root / 'ui-layer/index.html'
        for markup in ['<audio autoplay></audio>', '<IFRAME src="song.wav"></IFRAME>',
                       '<embed src="data:audio/wav;base64,AAA"/>', '<object></object>',
                       '<a href="data:video/mp4;base64,AAA">open</a>', '<script>new Audio()</script>']:
            with self.subTest(markup=markup):
                self.assertTrue(ui_html_errors(markup))
                page.write_text(markup)
                result = self.check()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('index.html: UI media', result.stderr)
        page.write_text('<!doctype html><html><head><title>seamone</title></head><body><p>audio recording</p></body></html>')
        self.assertEqual(self.check().returncode, 0)


if __name__ == "__main__":
    unittest.main()
