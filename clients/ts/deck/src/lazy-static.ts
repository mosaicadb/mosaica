/**
 * What the components' single-file bundle takes in place of `lazy.ts` (`components/vite.config.ts`).
 * A file with one chunk holds these modules either way, and a dynamic import inlined into it makes
 * the bundler wrap every module in a lazy initialiser.
 */
import * as aggregation from './aggregation.js';
import * as basemap from './basemap-draw.js';

export function importAggregation(): Promise<typeof aggregation> {
  return Promise.resolve(aggregation);
}

export function importBasemap(): Promise<typeof basemap> {
  return Promise.resolve(basemap);
}
