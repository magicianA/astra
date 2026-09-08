#!/usr/bin/env python3
"""Reproducible, resumable ESA TAP job for the G<=12 runtime catalogue."""

import csv
import gzip
import json
import pathlib
import time
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parents[1]
BASE = "https://gea.esac.esa.int/tap-server/tap"
QUERY = """SELECT g.source_id, g.ref_epoch, g.ra, g.dec, g.parallax,
g.pmra, g.pmdec, g.radial_velocity, g.phot_g_mean_mag, g.bp_rp,
g.ra_error, g.dec_error, g.pmra_error, g.pmdec_error,
g.parallax_error, g.radial_velocity_error, g.ruwe,
g.astrometric_params_solved, h.original_ext_source_id AS hip_id,
g.ra_dec_corr, g.ra_parallax_corr, g.ra_pmra_corr, g.ra_pmdec_corr,
g.dec_parallax_corr, g.dec_pmra_corr, g.dec_pmdec_corr,
g.parallax_pmra_corr, g.parallax_pmdec_corr, g.pmra_pmdec_corr
FROM gaiadr3.gaia_source AS g
LEFT OUTER JOIN gaiadr3.hipparcos2_best_neighbour AS h
ON g.source_id = h.source_id
WHERE g.phot_g_mean_mag <= 12"""


def request(url, data=None):
    body = urllib.parse.urlencode(data).encode() if data else None
    return urllib.request.urlopen(urllib.request.Request(url, body), timeout=120)


def main():
    raw = ROOT / "data/catalog/raw"
    raw.mkdir(parents=True, exist_ok=True)
    target = raw / "gaia-dr3-g12.csv.gz"
    state_path = raw / "gaia-job.json"
    if target.exists():
        print(f"Already downloaded: {target}")
        return
    if state_path.exists():
        state = json.loads(state_path.read_text())
        if state["query"] != QUERY:
            raise RuntimeError("Existing job has a different query")
    else:
        with request(
            BASE + "/async",
            {
                "REQUEST": "doQuery",
                "LANG": "ADQL",
                "FORMAT": "csv",
                "MAXREC": "5000000",
                "QUERY": QUERY,
                "PHASE": "RUN",
            },
        ) as response:
            url = response.geturl().rstrip("/")
        state = {"url": url, "query": QUERY, "maxrec": 5000000}
        state_path.write_text(json.dumps(state, indent=2) + "\n")
    url = state["url"]
    while True:
        with request(url + "/phase") as response:
            phase = response.read().decode().strip()
        print(f"Gaia job: {phase}", flush=True)
        if phase == "COMPLETED":
            break
        if phase == "PENDING":
            request(url + "/phase", {"PHASE": "RUN"}).close()
        if phase in ("ERROR", "ABORTED"):
            with request(url + "/error") as response:
                raise RuntimeError(response.read().decode())
        time.sleep(15)
    with request(url + "/results") as response:
        tree = ET.fromstring(response.read())
    results = [e for e in tree.iter() if e.tag.split("}")[-1] == "result"]
    if not results:
        raise RuntimeError("Completed job has no result")
    # ESA may emit an href with duplicated /results/results/. The UWS
    # canonical result endpoint is defined by the returned result identifier.
    link = url + "/results/" + urllib.parse.quote(results[0].attrib["id"], safe="")
    with request(link) as response, gzip.open(str(target) + ".partial", "wb") as out:
        total = 0
        while block := response.read(1024 * 1024):
            out.write(block)
            total += len(block)
            if total % (32 * 1024 * 1024) == 0:
                print(f"Downloaded {total // 1024 // 1024} MiB CSV", flush=True)
    partial = pathlib.Path(str(target) + ".partial")
    with gzip.open(partial, "rt") as stream:
        reader = csv.DictReader(stream)
        if not reader.fieldnames or "source_id" not in reader.fieldnames:
            raise RuntimeError("TAP did not return the requested CSV")
        count = sum(1 for _ in reader)
    # Independent count from the same table, without the cross-match join.
    with request(
        BASE
        + "/sync?"
        + urllib.parse.urlencode(
            {
                "REQUEST": "doQuery",
                "LANG": "ADQL",
                "FORMAT": "csv",
                "QUERY": "SELECT COUNT(*) AS n FROM gaiadr3.gaia_source WHERE phot_g_mean_mag <= 12",
            }
        )
    ) as response:
        expected = int(list(csv.DictReader(response.read().decode().splitlines()))[0]["n"])
    if count < expected or count >= state["maxrec"]:
        raise RuntimeError(f"Incomplete/overflow result: {count}, expected >= {expected}")
    state.update(rows=count, expected_unique_sources=expected, complete=True)
    state_path.write_text(json.dumps(state, indent=2) + "\n")
    partial.rename(target)
    print(f"Gaia complete: {count:,} rows", flush=True)


if __name__ == "__main__":
    main()
