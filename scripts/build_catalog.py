#!/usr/bin/env python3
"""Build a portable, little-endian star pack; preserve original source files."""

import csv
import gzip
import hashlib
import json
import math
import pathlib
import struct

ROOT = pathlib.Path(__file__).resolve().parents[1]
DATA = ROOT / "data/catalog"
RESEARCH = ROOT / "docs/research"
MAS = math.pi / (180 * 3600 * 1000)
RECORD = struct.Struct("<QII7d6f")
RV_UNKNOWN, DIST_UNKNOWN, PM_UNKNOWN, MULTIPLE, GAIA, LOW_QUALITY = 1, 2, 4, 8, 16, 32
POSITION_MATCH = 128


def sha256(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(4 * 1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def number(text, default=math.nan):
    try:
        v = float(text)
        return v if math.isfinite(v) else default
    except (ValueError, TypeError):
        return default


def build():
    DATA.mkdir(parents=True, exist_ok=True)
    hip = {}
    names = {}
    buckets = {}
    epoch_buckets = {}
    local_matches = {}
    with gzip.open(RESEARCH / "hip2.dat.gz", "rt") as stream:
        for line in stream:
            ident = int(line[:6])
            ra, dec = float(line[15:28]), float(line[29:42])
            plx, pmra, pmde = map(float, (line[43:50], line[51:59], line[60:68]))
            flags = RV_UNKNOWN
            if plx <= 0 or plx < 5 * float(line[83:89]):
                flags |= DIST_UNKNOWN
            if int(line[13:14]) > 1 or int(line[7:10]) % 10 != 5:
                flags |= MULTIPLE
            hip[ident] = [
                ident,
                ident,
                flags,
                ra,
                dec,
                pmra * MAS,
                pmde * MAS,
                max(plx, 0) / 1000,
                0.0,
                1991.25,
                float(line[129:136]),
                number(line[152:158], 0.65),
                float(line[69:75]),
                float(line[76:82]),
                float(line[90:96]),
                float(line[97:103]),
            ]
            ra2000 = (ra + pmra * MAS * 8.75 / max(math.cos(dec), 1e-8)) % (2 * math.pi)
            de2000 = dec + pmde * MAS * 8.75
            cell = (int(math.degrees(ra2000)) % 360, int(math.degrees(de2000) + 90))
            buckets.setdefault(cell, []).append((ident, ra2000, de2000))
            # Sparse index at Gaia's epoch for missing official cross-matches.
            ra2016 = (ra + pmra * MAS * 24.75 / max(math.cos(dec), 1e-8)) % (2 * math.pi)
            de2016 = dec + pmde * MAS * 24.75
            key = (int(math.degrees(ra2016) * 20) % 7200, int((math.degrees(de2016) + 90) * 20))
            epoch_buckets.setdefault(key, []).append((ident, ra2016, de2016))
    matched_bsc = 0
    with gzip.open(RESEARCH / "bsc5-catalog.gz", "rt") as stream:
        for line in stream:
            line = line.rstrip("\n").ljust(197)
            try:
                ra = (
                    (int(line[75:77]) + int(line[77:79]) / 60 + float(line[79:83]) / 3600)
                    * math.pi
                    / 12
                )
                dec = (
                    (int(line[84:86]) + int(line[86:88]) / 60 + int(line[88:90]) / 3600)
                    * math.pi
                    / 180
                )
                if line[83] == "-":
                    dec = -dec
            except ValueError:
                continue
            candidates = []
            cell = int(math.degrees(ra)) % 360, int(math.degrees(dec) + 90)
            # Widen longitude near the poles; angular acceptance remains 30 arcsec.
            for dx in range(-2, 3):
                for dy in (-1, 0, 1):
                    for ident, ar, de in buckets.get(((cell[0] + dx) % 360, cell[1] + dy), []):
                        dra = (ar - ra + math.pi) % (2 * math.pi) - math.pi
                        dist = math.hypot(dra * math.cos(dec), de - dec)
                        candidates.append((dist, ident))
            candidates.sort()
            if not candidates or candidates[0][0] > math.radians(30 / 3600):
                continue
            if len(candidates) > 1 and candidates[1][0] < max(
                2 * candidates[0][0], math.radians(2 / 3600)
            ):
                continue  # ambiguous binary/component match: do not guess
            ident = candidates[0][1]
            row = hip[ident]
            name = line[4:14].strip()
            names[str(ident)] = {
                "name": name or f"HIP {ident}",
                "hr": int(line[:4]),
                "hd": line[25:31].strip(),
                "bsc_match_arcsec": candidates[0][0] / MAS / 1000,
            }
            mag, bv = number(line[102:107]), number(line[109:114])
            if math.isfinite(mag):
                row[10] = mag
            if math.isfinite(bv):
                row[11] = bv
            rv = number(line[166:170])
            if math.isfinite(rv) and not line[170:174].strip():
                row[2] &= ~RV_UNKNOWN
                row[8] = rv
                names[str(ident)]["rv_source"] = "BSC5 heliocentric; formal error unknown"
            matched_bsc += 1
    proper = {
        32349: "天狼星 Sirius",
        30438: "老人星 Canopus",
        71683: "南门二 Alpha Centauri",
        69673: "大角星 Arcturus",
        91262: "织女星 Vega",
        24608: "五车二 Capella",
        24436: "参宿七 Rigel",
        37279: "南河三 Procyon",
        27989: "参宿四 Betelgeuse",
        7588: "水委一 Achernar",
        68702: "马腹一 Hadar",
        97649: "牛郎星 Altair",
        21421: "毕宿五 Aldebaran",
        80763: "心宿二 Antares",
        65474: "角宿一 Spica",
        102098: "天津四 Deneb",
        113368: "北落师门 Fomalhaut",
        11767: "北极星 Polaris",
        87937: "巴纳德星 Barnard's Star",
    }
    for ident, name in proper.items():
        if ident in hip:
            names.setdefault(str(ident), {})["name"] = name
    gaia_path = DATA / "raw/gaia-dr3-g12.csv.gz"
    replaced = set()
    seen_gaia = set()
    counts = {"gaia": 0, "hipparcos": 0, "bsc_matches": matched_bsc, "duplicate_gaia_rows": 0}
    target = DATA / "stars.bin"
    with open(str(target) + ".partial", "wb") as out:
        out.write(b"ASTARS01" + struct.pack("<QQ", 0, RECORD.size))
        if gaia_path.exists():
            with gzip.open(gaia_path, "rt") as stream:
                for g in csv.DictReader(stream):
                    sid = int(g["source_id"])
                    if sid in seen_gaia:
                        counts["duplicate_gaia_rows"] += 1
                        continue
                    seen_gaia.add(sid)
                    h = int(number(g.get("hip_id"), 0))
                    position_match = False
                    if not h:
                        ra = math.radians(float(g["ra"]))
                        dec = math.radians(float(g["dec"]))
                        cosdec = math.cos(dec)
                        cell = int(math.degrees(ra) * 20) % 7200, int((math.degrees(dec) + 90) * 20)
                        reach = min(3600, max(1, math.ceil(3 / (180 * max(cosdec, 1e-8)))))
                        candidates = []
                        for dx in range(-reach, reach + 1):
                            for dy in (-1, 0, 1):
                                for ident, ar, de in epoch_buckets.get(
                                    ((cell[0] + dx) % 7200, cell[1] + dy), ()
                                ):
                                    dra = (ar - ra + math.pi) % (2 * math.pi) - math.pi
                                    distance = math.hypot(dra * cosdec, de - dec) / MAS / 1000
                                    if distance < 3:
                                        candidates.append((distance, ident))
                        # One arcsecond acceptance; reject any second candidate
                        # within three arcseconds rather than guessing a component.
                        if len(candidates) == 1 and candidates[0][0] < 1:
                            distance, h = candidates[0]
                            position_match = True
                            local_matches[str(sid)] = {"hip": h, "distance_arcsec": distance}
                    plx = number(g["parallax"])
                    pmra, pmde, rv = (number(g[x]) for x in ("pmra", "pmdec", "radial_velocity"))
                    flags = GAIA
                    if position_match:
                        flags |= POSITION_MATCH
                    if not math.isfinite(rv):
                        flags |= RV_UNKNOWN
                        rv = 0
                    if (
                        not math.isfinite(plx)
                        or plx <= 0
                        or plx < 5 * number(g["parallax_error"], math.inf)
                    ):
                        flags |= DIST_UNKNOWN
                    if not math.isfinite(pmra) or not math.isfinite(pmde):
                        flags |= PM_UNKNOWN
                        pmra = pmde = 0
                    if number(g["ruwe"], 100) > 1.4:
                        flags |= LOW_QUALITY
                    if h in hip and (flags & (LOW_QUALITY | PM_UNKNOWN)):
                        continue
                    if h in hip:
                        flags |= hip[h][2] & MULTIPLE
                        replaced.add(h)
                    bp_rp = number(g["bp_rp"], 1.0)
                    # A display-only broad-band colour approximation, not calibrated V photometry.
                    bv = max(-0.4, min(2.5, 0.75 * bp_rp - 0.1))
                    row = [
                        sid,
                        h,
                        flags,
                        math.radians(float(g["ra"])),
                        math.radians(float(g["dec"])),
                        pmra * MAS,
                        pmde * MAS,
                        max(number(g["parallax"], 0), 0) / 1000,
                        rv,
                        float(g["ref_epoch"]),
                        float(g["phot_g_mean_mag"]),
                        bv,
                        *[
                            number(g[k], 0)
                            for k in ("ra_error", "dec_error", "pmra_error", "pmdec_error")
                        ],
                    ]
                    out.write(RECORD.pack(*row))
                    counts["gaia"] += 1
            if len(seen_gaia) != 3087828:
                raise RuntimeError(
                    f"Gaia source completeness changed: {len(seen_gaia)}; inspect before publishing"
                )
        for ident, row in hip.items():
            if ident not in replaced:
                out.write(RECORD.pack(*row))
                counts["hipparcos"] += 1
        out.seek(8)
        out.write(struct.pack("<Q", counts["gaia"] + counts["hipparcos"]))
    pathlib.Path(str(target) + ".partial").replace(target)
    manifest = {
        "schema_version": 1,
        "record_bytes": RECORD.size,
        "counts": counts,
        "positional_hip_matches": len(local_matches),
        "gaia_limit": 12 if gaia_path.exists() else None,
        "model": "ICRS optical astrometric parameters, original reference epoch; Gaia TCB",
        "quality_bits": {
            "rv_unknown": 1,
            "distance_unknown": 2,
            "pm_unknown": 4,
            "multiple": 8,
            "gaia": 16,
            "low_quality": 32,
            "position_match": POSITION_MATCH,
        },
        "uncertainty": "Runtime displays diagonal formal-error estimate; raw Gaia correlations retained in source CSV",
        "stars_sha256": sha256(target),
    }
    (DATA / "names.json").write_text(json.dumps(names, ensure_ascii=False) + "\n")
    (DATA / "positional-matches.json").write_text(json.dumps(local_matches, indent=2) + "\n")
    files = [
        "catalog/stars.bin",
        "catalog/names.json",
        "catalog/positional-matches.json",
        "time/cio.bin",
        "time/finals2000A.all",
        "time/naif0012.tls",
        "time/bulletin-c72.txt",
        "kernels/de441_part-1.bsp",
        "kernels/de441_part-2.bsp",
    ]
    manifest["runtime_sha256"] = {name: sha256(ROOT / "data" / name) for name in files}
    manifest["model_version"] = "astra-astrometry-v1"
    identity = json.dumps(
        {"model": manifest["model_version"], "files": manifest["runtime_sha256"]}, sort_keys=True
    )
    manifest["data_id"] = "astra-v1-" + hashlib.sha256(identity.encode()).hexdigest()[:20]
    manifest["source_epoch"] = {"Gaia": "J2016.0 TCB", "Hipparcos": "J1991.25"}
    manifest["snapshot_date"] = "2026-09-04"
    (DATA / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(manifest, indent=2), flush=True)


if __name__ == "__main__":
    build()
