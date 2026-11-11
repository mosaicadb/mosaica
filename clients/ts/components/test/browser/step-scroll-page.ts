import '../../src/index.js';
import type {BrowsePage, BrowseRow, Meta} from '@mosaicajs/client';
import {fakeStore, meta, settle, status} from '../fake-store.js';

// Built by `step-scroll.browser.ts` with Vite from the packages' sources. A layer's card low in a
// column that scrolls, whose children arrive a while after a step asks for them; `window.result`
// holds where the column was scrolled to before the step, while the children were on their way,
// and once they had landed.
const frames = (n: number) =>
  new Promise<void>((resolve) => {
    const step = () => (--n <= 0 ? resolve() : requestAnimationFrame(step));
    requestAnimationFrame(step);
  });
const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

const layer = {name: 'topics', title: 'Topics', views: ['s0'], membership: 'enumerated', hierarchy: {kind: 'nested', pruneChildren: false}, levels: [], computedContent: [], shape: 'derived', suppliedContent: ['name'], depsOn: [], version: 1} as unknown as Meta['layers'][number];
const row = (id: bigint, name: string, childCount: number): BrowseRow => ({mosaicaId: id, key: `k${id}`, name, maskedCount: 1000n - id, matchedCount: null, rung: 0, parentIds: [], childCount, slot: null});

document.body.innerHTML = `<div id="column" style="height: 400px; width: 340px; overflow-y: auto">
    <div style="height: 600px"></div>
    <mosaica-field-card field="cluster:topics"></mosaica-field-card>
  </div>`;
const store = fakeStore({meta: meta({layers: [layer]}), status: status({})});
store.set('view', {...store.get('view'), id: 's0'});
const roots: BrowsePage = {artifacts: Array.from({length: 20}, (_, i) => row(BigInt(i + 1), `Topic ${i + 1}`, 3)), parents: [], next: null};
const children: BrowsePage = {artifacts: [row(101n, 'First child', 0), row(102n, 'Second child', 0), row(103n, 'Third child', 0)], parents: [], next: null};
store.browse = (async (req: {parent?: bigint}) => {
  if (req.parent === undefined) return roots;
  await wait(400);
  return children;
}) as never;
const card = document.querySelector('mosaica-field-card') as HTMLElement & {store: unknown};
card.store = store;
await settle(document.body);
await frames(4);

const column = document.querySelector<HTMLElement>('#column')!;
column.scrollTop = column.scrollHeight;
await frames(2);
const before = column.scrollTop;
const rows = [...card.shadowRoot!.querySelectorAll<HTMLElement>('[part="row"]')];
rows.at(-1)!.querySelector<HTMLElement>('[part="step"]')!.click();
await frames(4);
const waiting = column.scrollTop;
await wait(600);
await settle(document.body);
await frames(4);
const landed = card.shadowRoot!.querySelector('[part="row"][data-key="101"]') !== null;
(window as unknown as {result: unknown}).result = {before, waiting, after: column.scrollTop, landed};
