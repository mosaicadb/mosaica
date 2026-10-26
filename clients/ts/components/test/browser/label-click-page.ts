import './capture-errors.js';
import '../../src/map.js';
import type {Artifact, ArtifactDetail, ArtifactsProjection, Band, Layer} from '@mosaicajs/client';
import {SessionArtifactTable, attachedTextOf, servedLineage} from '@mosaicajs/client/internal';
import {fakeStore, meta, settle, status} from '../fake-store.js';

// Built by `label-click.browser.ts` with Vite from the packages' sources. A bare map with one named
// cluster at the world's centre and a row of marks under its name. The GPU is made slow: work drawn
// finishes 300 ms after it is sent, and reading pixels back waits for it, as a picking pass does on
// a slow or software renderer; `?fast` leaves it as it is. The tests move and click the pointer
// along the name and read `window.opened`, `window.picked` and `window.hovered`.
const gl = WebGL2RenderingContext.prototype as unknown as Record<string, (...args: unknown[]) => unknown>;
let busyUntil = 0;
const slow = !location.search.includes('fast');
for (const name of ['drawArrays', 'drawElements', 'drawArraysInstanced', 'drawElementsInstanced']) {
  const draw = gl[name]!;
  gl[name] = function (this: unknown, ...args: unknown[]) {
    if (slow) busyUntil = performance.now() + 300;
    return draw.apply(this, args);
  };
}
const read = gl['readPixels']!;
gl['readPixels'] = function (this: unknown, ...args: unknown[]) {
  while (performance.now() < busyUntil);
  return read.apply(this, args);
};

const layer: Layer = {name: 'clusters', title: 'Clusters', views: ['s0'], membership: 'enumerated', hierarchy: {kind: 'flat', pruneChildren: false}, levels: [], computedContent: ['centroid'], shape: null, suppliedContent: ['text'], depsOn: [], version: 1};
const META = meta({layers: [layer]});
const cluster: Artifact = {layer: 'clusters', mosaicaId: 7n, key: 'c-7', maskedCount: 40n, centroid: [2 ** 31, 2 ** 31], box: null, content: ['Black Holes and Quantum Gravity'], parentIds: [], rung: 0, matched: null, highlighted: null, target: null, slot: null};
const table = new SessionArtifactTable();
const ordinals = table.take([{mosaicaId: cluster.mosaicaId, layer: cluster.layer, parentIds: cluster.parentIds}]);
const artifacts: ArtifactsProjection = {
  layer: 'clusters',
  layers: ['clusters'],
  served: [cluster],
  colourServed: [],
  attached: attachedTextOf([cluster], META.layers),
  lineage: servedLineage([cluster]),
  status: 'shown',
  refusal: null,
  version: 1,
  held: 1,
  table,
  servedOrdinals: new Set(ordinals),
  shapes: new Map(),
  colours: new Map([...ordinals].map((o) => [o, [200, 80, 40, 255] as const])),
  palette: 'tableau10',
  overrides: new Map(),
  coverage: {current: 0, stale: 0}
};

document.body.innerHTML = '<mosaica-map style="display:block;width:800px;height:500px"></mosaica-map>';
const map = document.querySelector('mosaica-map') as HTMLElement & {store: unknown; screenOf(w: [number, number]): [number, number]};
const store = fakeStore({meta: META, status: status({}), artifacts});
// As the store does, an opened artifact is published as the selection.
store.openArtifact = async (id: bigint) => {
  store.calls.push({name: 'openArtifact', args: [id]});
  store.set('selection', {item: null, itemRefusal: null, artifact: {id, detail: {layer: 'clusters', maskedCount: 40n} as unknown as ArtifactDetail}, artifactRefusal: null});
};
map.store = store;
await settle(document.body);

// A mark every two pixels along the name's line, so a pointer between its letters is over a mark.
const [cx, cy] = map.screenOf([256, 256]);
const perPixel = 1 / (map.screenOf([257, 256])[0] - cx);
const xs = Array.from({length: 241}, (_, i) => 256 + (i - 120) * 2 * perPixel);
const band = {
  depth: 2, prefix: 0n, x: 0, y: 0,
  ids: BigUint64Array.from(xs.map((_, i) => BigInt(100 + i))),
  positions: Float32Array.from(xs.flatMap((x) => [x, 256])),
  scalars: {}, served: xs.length, capUsed: 500, visible: BigInt(xs.length), matched: BigInt(xs.length), highlighted: BigInt(xs.length),
  highlightBits: null, membership: {}, heldBelow: 0n, identityKey: 'row', contentKey: 'row', bytes: 0, touchedAt: 0
} as unknown as Band;
store.set('marks', {bands: [band], standIn: [], count: {shown: xs.length, total: xs.length, exact: true}});

const w = window as unknown as {opened: string[]; picked: string[]; hovered: string[]; centre: [number, number]};
w.opened = [];
w.picked = [];
w.hovered = [];
map.addEventListener('mosaica-hover', (e) => w.hovered.push((e as CustomEvent<{id: string}>).detail.id));
map.addEventListener('mosaica-artifactopen', (e) => w.opened.push((e as CustomEvent<{id: string}>).detail.id));
map.addEventListener('mosaica-pick', (e) => w.picked.push((e as CustomEvent<{id: string}>).detail.id));
const box = map.getBoundingClientRect();
w.centre = [box.left + cx, box.top + cy];
