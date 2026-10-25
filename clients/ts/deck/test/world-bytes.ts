/** The world outline as the server encodes `GET /v1/basemaps/world`, for tests that serve it. */

export type Ring = [number, number][];

/** An `MBW1` body: land polygons (rings of degree pairs), then coastline and border lines. */
export function encodeWorld(land: Ring[][], coastline: Ring[], borders: Ring[], magic = 'MBW1'): ArrayBuffer {
  const bytes: number[] = [...magic].map((c) => c.charCodeAt(0));
  const u32 = (n: number) => bytes.push(n & 255, (n >> 8) & 255, (n >> 16) & 255, (n >>> 24) & 255);
  const u16 = (n: number) => bytes.push(n & 255, (n >> 8) & 255);
  const points = (ring: Ring) => {
    u32(ring.length);
    for (const [lon, lat] of ring) {
      u16(Math.round((lon * 32767) / 180) & 0xffff);
      u16(Math.round((lat * 32767) / 90) & 0xffff);
    }
  };
  u32(land.length);
  for (const polygon of land) {
    u16(polygon.length);
    polygon.forEach(points);
  }
  for (const lines of [coastline, borders]) {
    u32(lines.length);
    lines.forEach(points);
  }
  return new Uint8Array(bytes).buffer;
}
