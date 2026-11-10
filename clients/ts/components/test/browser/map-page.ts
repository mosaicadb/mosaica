import './capture-errors.js';
import '../../src/map.js';
import '../../src/status.js';
import '../../src/explorer.js';
import type {Band, RegionProjection, StatusProjection} from '@mosaicajs/client';
import {fakeStore, meta, settle, status} from '../fake-store.js';

// Built by `map.browser.ts` with Vite from the packages' sources. A bare map 800 by 500 px with a
// mark near each corner and one at its centre, and a status strip of the same store in a box 150 px
// wide; with `?explorer`, an explorer of that size with only its map and strip, over the same
// store and marks. The tests move the pointer, press keys, select a region with
// `window.selectRegion`, set the store's status with `window.setStatus` and move the camera
// through the map's own methods, and read what the map reported.
const inExplorer = new URLSearchParams(location.search).has('explorer');
document.body.innerHTML = inExplorer
  ? '<mosaica-explorer panels="" style="display:block;width:800px;height:500px"></mosaica-explorer>'
  : '<mosaica-map style="display:block;width:800px;height:500px"></mosaica-map><div id="narrow" style="width:150px;margin-top:20px"><mosaica-status></mosaica-status></div>';
const counts = {status: 'shown' as const, visible: {value: 21_406_522, exact: true}, matched: {value: 16_822_190, exact: true}, highlighted: {value: 16_822_190, exact: true}, shown: 5};
const store = fakeStore({meta: meta(), status: status({})});
store.set('view', {...store.get('view'), id: 's0', visible: counts.visible, matched: counts.matched, highlighted: counts.highlighted, served: {shown: 5, total: 16_822_190, exact: true}, inView: counts});
for (const el of document.querySelectorAll('mosaica-explorer, mosaica-map, mosaica-status')) (el as unknown as {store: unknown}).store = store;
await settle(document.body);
type DrawnMap = HTMLElement & {screenOf(w: [number, number]): [number, number]};
const map = inExplorer ? (document.querySelector('mosaica-explorer') as unknown as {map: DrawnMap}).map : (document.querySelector('mosaica-map') as DrawnMap);

// World units per pixel at the opening camera, to place a mark at a pixel of the map.
const [cx, cy] = map.screenOf([256, 256]);
const perPixel = 1 / (map.screenOf([257, 256])[0] - cx);
const at = map.getBoundingClientRect();
/** The marks' places in the map, in pixels from its top-left corner: its centre, then near each corner. Their ids count from 100. */
const points: [number, number][] = [[at.width / 2, at.height / 2], [at.width - 14, at.height - 12], [14, 12], [at.width - 14, 12], [14, at.height - 12]];
const band = {
  depth: 2, prefix: 0n, x: 0, y: 0,
  ids: BigUint64Array.from(points.map((_, i) => BigInt(100 + i))),
  positions: Float32Array.from(points.flatMap(([x, y]) => [256 + (x - cx) * perPixel, 256 + (y - cy) * perPixel])),
  scalars: {}, served: points.length, capUsed: 500, visible: BigInt(points.length), matched: BigInt(points.length), highlighted: BigInt(points.length),
  highlightBits: null, membership: {}, heldBelow: 0n, identityKey: 'corners', contentKey: 'corners', bytes: 0, touchedAt: 0
} as unknown as Band;
store.set('marks', {bands: [band], standIn: [], count: {shown: points.length, total: points.length, exact: true}});

const w = window as unknown as {
  zooms: number[];
  hovered: string[];
  points: [number, number][];
  selectRegion(bbox: [number, number, number, number]): void;
  setStatus(over: Partial<StatusProjection>): void;
  ready: boolean;
};
w.zooms = [];
map.addEventListener('mosaica-viewchange', (e) => w.zooms.push((e as CustomEvent<{zoom: number}>).detail.zoom));
w.hovered = [];
map.addEventListener('mosaica-hover', (e) => w.hovered.push((e as CustomEvent<{id: string}>).detail.id));
/** The marks' places in the page. */
w.points = points.map(([x, y]) => [at.left + x, at.top + y]);
/** Select a box in the view's coordinates, counted at once, as the store publishes it. */
w.selectRegion = (bbox) => {
  const region: RegionProjection = {
    shape: {kind: 'box', bbox},
    status: 'shown',
    refusal: null,
    visible: {value: 9000, exact: true},
    matched: {value: 4812, exact: true},
    served: {shown: 0, total: 4812, exact: true},
    verdict: {exact: true, depth: null},
    held: {ids: new BigUint64Array(0), positions: new Float32Array(0), count: 0}
  };
  store.set('region', region);
};
w.setStatus = (over) => store.set('status', status(over));
w.ready = true;
