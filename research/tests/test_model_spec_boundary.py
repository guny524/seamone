from dataclasses import replace
from unittest import TestCase
from unittest.mock import patch

from seamone_research.models import MODEL_SPECS, ModelSpec, resolve_model_path


class ModelSpecRevisionTests(TestCase):
    def test_constructor_rejects_floating_or_invalid_pins(self):
        for revision in ("main", "v1.2", "", "a" * 39, "A" * 40, "z" * 40, "a" * 40 + "\n"):
            with self.subTest(revision=revision), self.assertRaisesRegex(ValueError, "revision"):
                ModelSpec("fixture", "example/fixture", "whisper", revision, batch_size=1)

    def test_replacing_a_valid_spec_cannot_unpin_it(self):
        spec = MODEL_SPECS["whisper-small"]
        with self.assertRaisesRegex(ValueError, "revision"):
            replace(spec, revision="main")

    def test_full_lowercase_pin_reaches_the_loader_unchanged(self):
        revision = "a0" * 20
        spec = ModelSpec("fixture", "example/fixture", "whisper", revision, batch_size=1)
        with patch(
            "seamone_research.models.snapshot_download", return_value="/synthetic/snapshot"
        ) as download:
            self.assertEqual(resolve_model_path(spec), "/synthetic/snapshot")
        self.assertEqual(download.call_args.kwargs["revision"], revision)
