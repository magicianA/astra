#!/usr/bin/env python3
"""Compare Rust CLI numerical results against the independently built original C++ CLI.

Build the baseline from the original main C++ source before removing it. No Rust
branch or implementation is accessed by this validation. Example:
  python3 scripts/validate_rust_parity.py --cpp build/astra_cli
"""

import argparse
import json
import math
import subprocess
import tempfile
from pathlib import Path

CASES = [
    ("modern", ["--date", "2026-09-04T22:00:00", "--geometric"]),
    ("ancient", ["--date", "-3000-01-02T12:00:00", "--no-atmosphere"]),
    ("year-zero", ["--date", "0-01-02T12:00:00", "--no-atmosphere"]),
    ("far-future", ["--date", "6999-01-02T12:00:00", "--no-atmosphere"]),
    ("UTC", ["--date", "2026-09-04T12:00:00", "--scale", "UTC"]),
    ("UTC-leap", ["--date", "2016-12-31T23:59:60", "--scale", "UTC"]),
    ("north-pole", ["--date", "2026-09-04T22:00:00", "--site", "0,90,0"]),
    ("south-pole", ["--date", "2026-09-04T22:00:00", "--site", "0,-90,0"]),
    (
        "solar-totality",
        ["--date", "2024-04-08T18:42:00", "--scale", "UTC", "--site", "-96.7,32.28333333333333,0"],
    ),
    (
        "lunar-totality",
        ["--date", "2025-03-14T06:58:41", "--scale", "UTC", "--site", "-122.4194,37.7749,0"],
    ),
    (
        "solar-events",
        [
            "--date",
            "2024-04-08T00:00:00",
            "--scale",
            "UTC",
            "--site",
            "-96.7,32.28333333333333,0",
            "--events",
            "solar",
            "--days",
            "1",
        ],
    ),
    (
        "lunar-events",
        [
            "--date",
            "2025-03-14T00:00:00",
            "--scale",
            "UTC",
            "--site",
            "-122.4194,37.7749,0",
            "--events",
            "lunar",
            "--days",
            "1",
        ],
    ),
    (
        "equatorial-plan",
        [
            "--date",
            "2024-03-20T00:00:00",
            "--site",
            "0,0,0",
            "--no-atmosphere",
            "--plan",
            "--target",
            "10",
        ],
    ),
]


def compare(expected, actual, path, differences, maxima):
    if isinstance(expected, bool) or expected is None or isinstance(expected, str):
        if expected != actual:
            differences.append((path, expected, actual))
    elif isinstance(expected, (int, float)):
        if not isinstance(actual, (int, float)) or not math.isfinite(actual):
            differences.append((path, expected, actual))
            return
        delta = abs(expected - actual)
        field = path.rsplit(".", 1)[-1]
        maxima[field] = max(maxima.get(field, 0.0), delta)
        # The exported degrees and scalar states are compared well below the
        # original model's accuracy. Event contacts are solved to 0.1 seconds.
        tolerance = max(1e-10, abs(expected) * 2e-14)
        if delta > tolerance:
            differences.append((path, expected, actual))
    elif isinstance(expected, list):
        if not isinstance(actual, list) or len(expected) != len(actual):
            differences.append(
                (
                    path + ".length",
                    len(expected),
                    len(actual) if isinstance(actual, list) else actual,
                )
            )
            return
        for i, (a, b) in enumerate(zip(expected, actual)):
            compare(a, b, f"{path}[{i}]", differences, maxima)
    elif isinstance(expected, dict):
        expected_keys = set(expected) - {"compute_ms"}
        actual_keys = set(actual) - {"compute_ms"} if isinstance(actual, dict) else set()
        if expected_keys != actual_keys:
            differences.append((path + ".keys", sorted(expected_keys), sorted(actual_keys)))
            return
        for key in sorted(expected_keys):
            compare(expected[key], actual[key], f"{path}.{key}", differences, maxima)
    else:
        raise TypeError(path)


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpp", type=Path, default=root / "build/astra_cli")
    parser.add_argument("--rust", type=Path, default=root / "target/release/astra_cli")
    parser.add_argument("--data", type=Path, default=root / "data")
    args = parser.parse_args()
    cpp, rust, data = args.cpp.resolve(), args.rust.resolve(), args.data.resolve()
    failures = []
    maxima = {}
    with tempfile.TemporaryDirectory(prefix="astra-rust-parity-") as directory:
        directory = Path(directory)
        for name, flags in CASES:
            reports = []
            for implementation, binary in [("C++", cpp), ("Rust", rust)]:
                output = directory / f"{name}-{implementation}.json"
                run = subprocess.run(
                    [str(binary), "--data", str(data), *flags, "--output", str(output)],
                    cwd=root,
                    capture_output=True,
                    text=True,
                )
                if run.returncode:
                    failure = (name + ".exit", implementation, run.stderr.strip())
                    failures.append(failure)
                    print(
                        f"FAIL {name}: {implementation} exited {run.returncode}: {run.stderr.strip()}"
                    )
                    break
                reports.append(json.loads(output.read_text()))
            if len(reports) != 2:
                continue
            differences = []
            compare(reports[0], reports[1], name, differences, maxima)
            if differences:
                failures.extend(differences)
                print(f"FAIL {name}: {len(differences)} differences")
                for path, expected, actual in differences[:8]:
                    print(f"  {path}: C++={expected!r}, Rust={actual!r}")
            else:
                print(f"PASS {name}")
    print("Maximum absolute differences by numerical field:")
    for field, delta in sorted(maxima.items()):
        if delta:
            print(f"  {field}: {delta:.12g}")
    if failures:
        raise SystemExit(f"{len(failures)} parity differences across {len(CASES)} cases")
    print(f"All {len(CASES)} original-main/Rust parity cases passed.")


if __name__ == "__main__":
    main()
