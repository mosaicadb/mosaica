#!/usr/bin/env python3
"""Encode the world outline that `GET /v1/basemaps/world` serves, from Natural Earth 1:50m.

    python3 -I scripts/build-world-basemap.py LAND COASTLINE BORDERS OUT

LAND, COASTLINE and BORDERS are these three files of nvkelso/natural-earth-vector at tag v5.1.2,
downloaded as they are; each is refused unless its SHA-256 matches the one below.

    https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/geojson/ne_50m_land.geojson
    https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/geojson/ne_50m_coastline.geojson
    https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/geojson/ne_50m_admin_0_boundary_lines_land.geojson

OUT is written in the `MBW1` format, all integers little-endian with no padding:

    b"MBW1"
    land:      u32 polygons; per polygon u16 rings (outer first, then holes);
               per ring u32 points, then the points
    coastline: u32 lines; per line u32 points, then the points
    borders:   the same as coastline
    a point:   i16 x, i16 y; longitude = x * 180 / 32767, latitude = y * 90 / 32767

A multi-part feature becomes one polygon or line per part. After quantising, a point equal to the
one before it is dropped, and then a ring left with fewer than four points or a line with fewer
than two is dropped; a polygon whose outer ring is dropped is dropped whole.

Standard library only. The counts per layer and the size are printed.
"""

from __future__ import annotations

import hashlib
import json
import struct
import sys
from pathlib import Path

TAG = "v5.1.2"
BASE = f"https://raw.githubusercontent.com/nvkelso/natural-earth-vector/{TAG}/geojson/"
INPUTS = {
    "ne_50m_land.geojson": "e874b27a51d146452be360cafb3cc50c86001074a67d534113e6534682f9826b",
    "ne_50m_coastline.geojson": "271f1c4c1908312bac6b29d158ea1356544beafc129f260005300913aa5ea283",
    "ne_50m_admin_0_boundary_lines_land.geojson": (
        "2faac4f6b34386f3d21b6e018cf151f241f00e5c936d44dd17d7d9bfb147fa48"
    ),
}
MAGIC = b"MBW1"
SCALE = 32767


def load(path: Path, name: str) -> list[dict]:
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != INPUTS[name]:
        sys.exit(
            f"{path} has SHA-256 {digest}, not that of {BASE}{name}; download that file again."
        )
    return [f["geometry"] for f in json.loads(data)["features"] if f.get("geometry")]


def quantise(coords: list) -> list[tuple[int, int]]:
    points: list[tuple[int, int]] = []
    for lon, lat, *_ in coords:
        x = max(-SCALE, min(SCALE, round(lon * SCALE / 180)))
        y = max(-SCALE, min(SCALE, round(lat * SCALE / 90)))
        if not points or points[-1] != (x, y):
            points.append((x, y))
    return points


def polygons(geometries: list[dict]) -> list[list[list[tuple[int, int]]]]:
    out = []
    for g in geometries:
        parts = {"Polygon": [g["coordinates"]], "MultiPolygon": g["coordinates"]}[g["type"]]
        for part in parts:
            rings = [quantise(ring) for ring in part]
            if len(rings[0]) < 4:
                continue
            out.append([rings[0]] + [r for r in rings[1:] if len(r) >= 4])
    return out


def lines(geometries: list[dict]) -> list[list[tuple[int, int]]]:
    out = []
    for g in geometries:
        parts = {"LineString": [g["coordinates"]], "MultiLineString": g["coordinates"]}[g["type"]]
        for part in parts:
            points = quantise(part)
            if len(points) >= 2:
                out.append(points)
    return out


def put_points(buf: bytearray, points: list[tuple[int, int]]) -> None:
    buf += struct.pack("<I", len(points))
    for x, y in points:
        buf += struct.pack("<hh", x, y)


def main(argv: list[str]) -> None:
    if len(argv) != 5:
        sys.exit(__doc__)
    land_path, coast_path, border_path, out_path = map(Path, argv[1:])
    land = polygons(load(land_path, "ne_50m_land.geojson"))
    coast = lines(load(coast_path, "ne_50m_coastline.geojson"))
    borders = lines(load(border_path, "ne_50m_admin_0_boundary_lines_land.geojson"))

    buf = bytearray(MAGIC)
    buf += struct.pack("<I", len(land))
    for rings in land:
        buf += struct.pack("<H", len(rings))
        for ring in rings:
            put_points(buf, ring)
    for layer in (coast, borders):
        buf += struct.pack("<I", len(layer))
        for points in layer:
            put_points(buf, points)
    out_path.write_bytes(buf)

    rings = sum(len(p) for p in land)
    land_points = sum(len(r) for p in land for r in p)
    print(f"land: {len(land)} polygons, {rings} rings, {land_points} points")
    print(f"coastline: {len(coast)} lines, {sum(map(len, coast))} points")
    print(f"borders: {len(borders)} lines, {sum(map(len, borders))} points")
    print(f"{out_path}: {len(buf)} bytes")


if __name__ == "__main__":
    main(sys.argv)
