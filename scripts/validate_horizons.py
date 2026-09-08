#!/usr/bin/env python3
"""Download independent JPL Horizons observer fixtures and compare the local CLI."""

import csv
import json
import math
import subprocess
import time
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / "artifacts/horizons"
BODIES = [10, 301, 199, 299, 4, 5, 6, 7, 8]


def fetch(body):
    params = {
        "format": "json",
        "COMMAND": str(body),
        "EPHEM_TYPE": "OBSERVER",
        "CENTER": "coord@399",
        "COORD_TYPE": "GEODETIC",
        "SITE_COORD": "0,51.4779,0.046",
        "START_TIME": "2026-09-04 12:00",
        "STOP_TIME": "2026-09-04 12:01",
        "STEP_SIZE": "1m",
        "QUANTITIES": "4",
        "APPARENT": "AIRLESS",
        "ANG_FORMAT": "DEG",
        "EXTRA_PREC": "YES",
        "CSV_FORMAT": "YES",
    }
    url = "https://ssd.jpl.nasa.gov/api/horizons.api?" + urllib.parse.urlencode(
        {k: v if k == "format" else f"'{v}'" for k, v in params.items()}
    )
    path = CACHE / f"{body}.json"
    if not path.exists():
        for attempt in range(5):
            try:
                with urllib.request.urlopen(url, timeout=90) as response:
                    payload = json.load(response)
                if "$$SOE" not in payload.get("result", ""):
                    raise RuntimeError(payload)
                path.write_text(json.dumps(payload, indent=2) + "\n")
                break
            except Exception:
                if attempt == 4:
                    raise
                time.sleep(2**attempt)
    result = json.loads(path.read_text())["result"]
    first = result.split("$$SOE")[1].strip().splitlines()[0]
    row = next(csv.reader([first]))
    return {"body": body, "azimuth_deg": float(row[3]), "altitude_deg": float(row[4]), "url": url}


def vector(azimuth, altitude):
    azimuth, altitude = math.radians(azimuth), math.radians(altitude)
    return (
        math.sin(azimuth) * math.cos(altitude),
        math.cos(azimuth) * math.cos(altitude),
        math.sin(altitude),
    )


def main():
    CACHE.mkdir(parents=True, exist_ok=True)
    fixtures = []
    for body in BODIES:
        fixtures.append(fetch(body))
        print(f"Horizons body {body}: cached", flush=True)
    target = ROOT / "tests/fixtures/horizons-greenwich-2026.json"
    target.parent.mkdir(exist_ok=True)
    fixture = {
        "source": "NASA/JPL Horizons API, observer table, AIRLESS",
        "date_utc": "2026-09-04T12:00:00",
        "longitude_deg": 0,
        "latitude_deg": 51.4779,
        "height_m": 46,
        "tolerance_arcsec": 2.0,
        "bodies": fixtures,
    }
    target.write_text(json.dumps(fixture, indent=2) + "\n")
    output = ROOT / "artifacts/greenwich-2026.json"
    subprocess.run(
        [
            str(ROOT / "build/astra_cli"),
            "--date",
            fixture["date_utc"],
            "--site",
            "0,51.4779,46",
            "--scale",
            "UTC",
            "--no-atmosphere",
            "--output",
            str(output),
        ],
        check=True,
    )
    local = {body["body"]: body for body in json.loads(output.read_text())["bodies"]}
    residuals = []
    for ref in fixtures:
        result = local[ref["body"]]
        v = vector(ref["azimuth_deg"], ref["altitude_deg"])
        u = vector(result["azimuth_deg"], result["altitude_deg"])
        chord = math.sqrt(sum((a - b) ** 2 for a, b in zip(u, v)))
        error = math.degrees(2 * math.asin(min(1, chord / 2))) * 3600
        residuals.append({"body": ref["body"], "error_arcsec": error})
        print(f"Body {ref['body']:3}: {error:.6f} arcsec", flush=True)
        if error > fixture["tolerance_arcsec"]:
            raise RuntimeError(f"Horizons mismatch: {residuals[-1]}")
    (ROOT / "artifacts/horizons-comparison.json").write_text(json.dumps(residuals, indent=2) + "\n")


if __name__ == "__main__":
    main()
