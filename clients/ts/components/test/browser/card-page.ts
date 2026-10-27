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
const overlapping = (a: DOMRect, b: DOMRect) => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
/**
 * Pick points across the map's top-left, the card expanded, and list those whose card is shown
 * over the panel, the status strip, the map's tools or the Layers button. A point under one of
 * these cannot be clicked, so is skipped.
 */
async function covering(): Promise<string[]> {
  const tools = [map.shadowRoot!.querySelector('[part="controls"]'), explorer.shadowRoot!.querySelector('.layers .group')].flatMap((e) => (e ? [e.getBoundingClientRect()] : []));
  const over = [...explorer.shadowRoot!.querySelectorAll('[part="panel"], .in-map-strip')].map((e) => e.getBoundingClientRect()).concat(tools);
  const covered: string[] = [];
  const [ax, ay] = map.screenOf([0, 0]);
  const scale = map.screenOf([1, 1])[0] - ax;
  // Only a point within a card's width of the tools, which sit at the map's top-left, can put a card over them.
  for (let sx = 10; sx < 800; sx += 40)
    for (let sy = 10; sy < 500; sy += 40) {
      const [x, y] = [mapBox.left + sx, mapBox.top + sy];
      if (over.some((o) => o.left <= x && x <= o.right && o.top <= y && y <= o.bottom)) continue;
      map.pickedAt = {kind: 'item', id: 5n, world: [(sx - ax) / scale, (sy - ay) / scale]};
      (explorer as unknown as {requestUpdate(): void}).requestUpdate();
      await settle(document.body);
      await frames(3);
      const card = explorer.shadowRoot!.querySelector('[part~="callout"]') as HTMLElement | null;
      if (!card || card.hidden) continue;
      const r = card.getBoundingClientRect();
      if (over.some((t) => overlapping(r, t))) covered.push(`${sx},${sy}`);
    }
  return covered;
}
const covered = await covering();
// Without the Layers button under them, the map's tools alone.
explorer.setAttribute('panels', 'toolbar filters selection detail');
await settle(document.body);
await frames(3);
const coveredToolsAlone = await covering();
(window as unknown as {result: unknown}).result = {
  covered,
  coveredToolsAlone,
  alone: alone.getBoundingClientRect().height,
  before,
  after: box.height,
  inside: box.top >= mapBox.top && box.bottom <= mapBox.bottom,
  mapHeight: mapBox.height
};
