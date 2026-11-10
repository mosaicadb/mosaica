import '../../src/map.js';
import '../../src/status.js';
import '../../src/selection.js';
import type {RegionProjection} from '@mosaicajs/client';
import {fakeStore, meta, settle, status} from '../fake-store.js';

// Built by `look.browser.ts` with Vite from the packages' sources. The status strip, the selection
// card and a bare map at rest, the card in a box as wide as the explorer's panel (340 px) and as
// its compact one (264 px). `window.look` holds what the test pins: sizes that do not depend on a
// font's width, and the text each element draws.
const view = {
  id: 's0', composition: null, depth: 5,
  visible: {value: 21_406_522, exact: true}, matched: {value: 16_822_190, exact: true}, highlighted: {value: 16_822_190, exact: true}, highlighting: false,
  served: {shown: 5390, total: 16_822_190, exact: true}, provisional: 0,
  inView: {status: 'shown' as const, visible: {value: 21_406_522, exact: true}, matched: {value: 16_822_190, exact: true}, highlighted: {value: 16_822_190, exact: true}, shown: 5390}
};
const region: RegionProjection = {
  shape: {kind: 'box', bbox: [0.4, 0.4, 0.6, 0.6]},
  status: 'shown',
  refusal: null,
  visible: {value: 900, exact: false},
  matched: {value: 800, exact: false},
  served: {shown: 3, total: 800, exact: true},
  verdict: {exact: false, depth: 6},
  held: {ids: BigUint64Array.from([31728047486770n, 32n, 33n]), positions: new Float32Array(6), count: 8}
};

document.body.innerHTML = `
  <div id="wide" style="width:340px"><mosaica-selection></mosaica-selection></div>
  <div id="compact" style="width:264px"><mosaica-selection></mosaica-selection></div>
  <div><mosaica-status id="shown"></mosaica-status></div>
  <div><mosaica-status id="shown-compact" compact></mosaica-status></div>
  <div><mosaica-status id="updating" data-store="updating"></mosaica-status></div>
  <div><mosaica-status id="updating-compact" compact data-store="updating"></mosaica-status></div>
  <mosaica-map density="grid" no-points style="display:block;width:600px;height:400px"></mosaica-map>`;
const rest = fakeStore({meta: meta(), status: status({}), view, region});
const updating = fakeStore({meta: meta(), status: status({status: 'loading', sessionWarm: true}), view});
for (const el of document.querySelectorAll('mosaica-selection, mosaica-status, mosaica-map')) (el as unknown as {store: unknown}).store = el.getAttribute('data-store') === 'updating' ? updating : rest;
await settle(document.body);
await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));

const round = (n: number) => Math.round(n * 100) / 100;
const box = (el: Element | null | undefined) => {
  const r = el?.getBoundingClientRect();
  return r ? {left: round(r.left), top: round(r.top), right: round(r.right), bottom: round(r.bottom), width: round(r.width), height: round(r.height)} : null;
};
/** The text `el` draws: what is not hidden for a screen reader alone, not undrawn and not inside another element's shadow root. */
function drawn(el: Element | null | undefined): string {
  if (!el) return '';
  let out = '';
  const walk = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) out += node.textContent ?? '';
    if (!(node instanceof Element)) return;
    const style = getComputedStyle(node);
    if (node.classList.contains('sr-only') || style.display === 'none' || style.visibility === 'hidden') return;
    for (const child of node.shadowRoot?.childNodes ?? node.childNodes) walk(child);
  };
  walk(el);
  return out.replace(/\s+/g, ' ').trim();
}

function strip(id: string) {
  const root = document.getElementById(id)!.shadowRoot!;
  const cells = [...root.querySelectorAll('.cell')];
  const first = cells[0]!;
  return {
    height: box(root.querySelector('[part="strip"]'))!.height,
    cells: cells.length,
    // Every cell on one line, each starting where the one before it ends.
    oneLine: new Set(cells.map((c) => box(c)!.top)).size === 1,
    abutting: cells.every((c, i) => i === 0 || box(c)!.left === box(cells[i - 1]!)!.right),
    rules: cells.map((c) => getComputedStyle(c).borderLeftWidth),
    paddings: cells.map((c) => getComputedStyle(c).paddingLeft),
    stateCell: {width: box(first)!.width, text: drawn(first), dot: box(first.querySelector('.dot'))},
    text: drawn(root.querySelector('[part="strip"]')),
    faded: cells.map((c) => getComputedStyle(c).opacity)
  };
}

function card(id: string) {
  const root = document.getElementById(id)!.querySelector('mosaica-selection')!.shadowRoot!;
  const part = (name: string) => root.querySelector(`[part="${name}"]`);
  const counts = part('counts')!;
  const rows = [...counts.children].filter((_, i) => i % 2 === 0).map((k) => box(k)!);
  return {
    width: box(root.querySelector('.panel'))!.width,
    titleToCounts: round(box(counts)!.top - box(part('title'))!.bottom),
    stateLine: {height: box(part('state'))?.height ?? 0, text: drawn(part('state')), title: part('state')?.getAttribute('title') ?? null},
    countRows: rows.length,
    countRowPitch: round(rows[1]!.top - rows[0]!.top),
    countsToLabel: round(box(part('label'))!.top - box(counts)!.bottom),
    items: [...root.querySelectorAll('[part="item"]')].map((li) => {
      const words = document.createTreeWalker(li, NodeFilter.SHOW_TEXT, (n) => (n.textContent!.trim() ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP)).nextNode()!;
      const text = document.createRange();
      text.selectNodeContents(words);
      // How far in from the row's edge its text starts.
      return {height: box(li)!.height, text: drawn(li), textLeft: round(text.getBoundingClientRect().left - box(li)!.left)};
    }),
    actions: [...root.querySelectorAll('[part="action"]')].map((b) => box(b)!.height),
    text: drawn(root.querySelector('.panel'))
  };
}

function map() {
  const root = document.querySelector('mosaica-map')!.shadowRoot!;
  const tag = root.querySelector('[part="region-tag"]')!;
  const clear = tag.querySelector('button')!;
  const c = box(clear)!;
  const [cx, cy] = [(c.left + c.right) / 2, (c.top + c.bottom) / 2];
  /** Whether a pointer at this offset from the Clear button's centre is on the button. */
  const hits = (dx: number, dy: number) => root.elementFromPoint(cx + dx, cy + dy)?.closest('button') === clear;
  return {
    tag: {height: box(tag)!.height, clear: {width: c.width, height: c.height}, clearToTagRight: round(box(tag)!.right - c.right), text: drawn(tag)},
    clearTarget: {at11: [hits(-11, 0), hits(11, 0), hits(0, -11), hits(0, 11)], at13: [hits(-13, 0), hits(0, -13), hits(0, 13)]},
    key: {height: box(root.querySelector('[part="density-key"]'))!.height, text: drawn(root.querySelector('[part="density-key"]'))},
    tools: [...root.querySelectorAll('[part="controls"] button')].map((b) => `${box(b)!.width}x${box(b)!.height}`),
    overlayText: drawn(root.querySelector('[part="overlay"]')),
    noteText: drawn(root.querySelector('[part="note"]')),
    // Everything the map draws as text, apart from its slots.
    text: [...root.children].map((child) => drawn(child)).filter(Boolean)
  };
}

(window as unknown as {look: unknown}).look = {
  strips: {shown: strip('shown'), shownCompact: strip('shown-compact'), updating: strip('updating'), updatingCompact: strip('updating-compact')},
  cards: {wide: card('wide'), compact: card('compact')},
  map: map()
};
