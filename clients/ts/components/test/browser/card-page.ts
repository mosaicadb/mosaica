import '../../src/index.js';
import type {ItemDetail} from '@mosaicajs/client';
import {fakeStore, meta, settle, status} from '../fake-store.js';

// Built by `card.browser.ts` with Vite from the packages' sources. An item with 40 fields, shown
// in a compact item card on its own and in an explorer's card beside its point, each with Show all
// pressed; `window.result` holds what the test measures.
const fields: Record<string, unknown> = {};
for (let i = 0; i < 40; i++) fields[`field_${String(i).padStart(2, '0')}`] = `value ${i}`;
const item = {id: 5n, detail: {fields, labels: [], views: [], scoped: {}} as ItemDetail};
const showAll = async (card: Element) => {
  (card.shadowRoot!.querySelector('[part="show-all"]') as HTMLButtonElement).click();
  await settle(document.body);
};
const frames = (n: number) => new Promise<void>((resolve) => {
  const step = () => (--n <= 0 ? resolve() : requestAnimationFrame(step));
  requestAnimationFrame(step);
});

document.body.innerHTML = `<div style="width: 300px"><mosaica-item-card compact></mosaica-item-card></div>
  <mosaica-explorer layout="overlay" style="--mosaica-explorer-height: 700px"></mosaica-explorer>`;
const alone = document.querySelector('mosaica-item-card') as HTMLElement & {item: unknown};
alone.item = item;
await settle(document.body);
await showAll(alone);

const explorer = document.querySelector('mosaica-explorer') as HTMLElement & {store: unknown};
const store = fakeStore({meta: meta(), status: status({})});
explorer.store = store;
await settle(document.body);
const map = explorer.shadowRoot!.querySelector('mosaica-map') as HTMLElement & {pickedAt: unknown; screenOf(w: [number, number]): [number, number]};
map.pickedAt = {kind: 'item', id: 5n, world: [0.5, 0.5]};
store.set('selection', {item, itemRefusal: null, artifact: null, artifactRefusal: null});
await settle(document.body);
await frames(4);
const callout = explorer.shadowRoot!.querySelector('[part~="callout"]') as HTMLElement;
const before = callout.getBoundingClientRect().height;
await showAll(callout.querySelector('mosaica-item-card')!);
await frames(4);
const box = callout.getBoundingClientRect();
const mapBox = map.getBoundingClientRect();
(window as unknown as {result: unknown}).result = {
  alone: alone.getBoundingClientRect().height,
  before,
  after: box.height,
  inside: box.top >= mapBox.top && box.bottom <= mapBox.bottom,
  mapHeight: mapBox.height
};
