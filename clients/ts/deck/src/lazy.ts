/**
 * The modules loaded the first time they are needed: the hexagon and contour layers, and what
 * draws a basemap. A host's bundler puts each in a chunk of its own, which a page that never draws
 * them does not load.
 */
export function importAggregation(): Promise<typeof import('./aggregation.js')> {
  return import('./aggregation.js');
}

export function importBasemap(): Promise<typeof import('./basemap-draw.js')> {
  return import('./basemap-draw.js');
}
