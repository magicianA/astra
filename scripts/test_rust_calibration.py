"""Check independent calibration regeneration without scientific dependencies."""

import re
import tempfile
import unittest
from pathlib import Path

from rust_calibration import TARGET, write_background, write_twilight


def block(source, name):
    return source.split(f"// BEGIN GENERATED {name}\n")[1].split(f"// END GENERATED {name}")[0]


class RustCalibrationTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.target = Path(self.directory.name) / "calibration.rs"
        self.target.write_text(TARGET.read_text())

    def test_current_cpp_and_rust_outputs_match(self):
        root = TARGET.parent.parent
        rust = self.target.read_text()
        header = (root / "include/astro/twilight_calibration.hpp").read_text()
        expected = re.findall(r"-?\d+\.\d+", header.split("= {")[1])
        actual = re.findall(r"-?\d+\.\d+", block(rust, "TWILIGHT"))
        self.assertEqual(len(expected), 401)
        self.assertEqual(actual, expected)
        background = (root / "include/astro/background_calibration.hpp").read_text()
        expected = re.search(r"background_radiance_scale = ([\d.]+)f", background)[1]
        self.assertIn(f"= {expected};", block(rust, "BACKGROUND"))

    def test_background_update_preserves_twilight(self):
        previous = self.target.read_text()
        write_background(0.1234567890123, self.target)
        updated = self.target.read_text()
        self.assertEqual(block(updated, "TWILIGHT"), block(previous, "TWILIGHT"))
        self.assertIn("pub const BACKGROUND_RADIANCE_SCALE: f32 = 0.123456789012;", updated)

    def test_twilight_update_preserves_background(self):
        previous = self.target.read_text()
        values = [index / 80 - 2.5 for index in range(401)]
        write_twilight(values, "1" * 64, self.target)
        updated = self.target.read_text()
        self.assertEqual(block(updated, "BACKGROUND"), block(previous, "BACKGROUND"))
        self.assertIn(f"Clear LUT SHA-256: {'1' * 64}", updated)
        actual = re.findall(r"-?\d+\.\d+", block(updated, "TWILIGHT"))
        self.assertEqual(actual, [f"{value:.12f}" for value in values])

    def test_bad_input_preserves_file(self):
        previous = self.target.read_text()
        for scale in [0, -1, float("inf"), float("nan")]:
            with self.assertRaises(ValueError):
                write_background(scale, self.target)
        for values in [[0.0] * 400, [float("nan")] * 401]:
            with self.assertRaises(ValueError):
                write_twilight(values, "0" * 64, self.target)
        with self.assertRaises(ValueError):
            write_twilight([0.0] * 401, "invalid", self.target)
        self.assertEqual(self.target.read_text(), previous)


if __name__ == "__main__":
    unittest.main()
