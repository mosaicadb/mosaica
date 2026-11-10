import '../../src/index.js';
import type {Layer} from '@mosaicajs/client';
import {fakeStore, meta, scalar, settle, status} from '../fake-store.js';

// Built by `explorer-look.browser.ts` with Vite from the packages' sources. One explorer at rest
// with a pointer, as a wide or a compact window draws it, over a store with no network: a title
// too long for its line, two views, a layer that brings another, points coloured by a category and
// no number column to size by. `window.result` holds the sizes of what it draws and the text that
// shows: at rest, with the Layers popover open, and with each of its menus open. `?layout=` is the
// explorer's layout, and `?density=` the density it draws, which adds that section's controls.
const frames = (n: number) =>
  new Promise<void>((resolve) => {
    const step = () => (--n <= 0 ? resolve() : requestAnimationFrame(step));
    requestAnimationFrame(step);
  });

const layer = (name: string, title: string, depsOn: string[] = []): Layer => ({name, title, views: ['s0'], membership: 'enumerated', hierarchy: {kind: 'flat', pruneChildren: false}, levels: [], computedContent: ['centroid'], shape: null, suppliedContent: ['name'], depsOn, version: 1});
const view = (id: string, displayName: string) => ({id, displayName, quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, projection: 'none' as const, worldAspect: null, tileScheme: null, tile: null, roster: null});
const LONG = 'a name long enough that no line of the panel or the popover could hold all of it without cutting it short';
const META = meta({
  views: [view('s0', `A layout with ${LONG}`), view('s1', 'Other')],
  declaredScalars: [scalar('archive', 'u16', {category: {vocabulary: 'a', kind: 'declared', visibility: 'public'}, render: true, homes: ['rendered']})],
  filterOperands: [{column: 'archive', family: 'category', operands: ['in']}],
  layers: [layer('clusters', 'Clusters'), layer('labels', 'Cluster names', ['clusters']), layer('districts', `Districts with ${LONG}`)]
});

document.documentElement.style.height = '100%';
document.body.style.height = '100%';
// The page's address chooses among the values each attribute takes, and is never written into the page.
const asked = new URLSearchParams(location.search);
const oneOf = (name: string, values: readonly string[]) => values.find((v) => v === asked.get(name)) ?? values[0]!;
const explorer = document.createElement('mosaica-explorer') as HTMLElement & {store: unknown};
explorer.setAttribute('layout', oneOf('layout', ['overlay', 'docked']));
explorer.setAttribute('density', oneOf('density', ['none', 'smooth', 'hex', 'grid', 'contours']));
explorer.setAttribute('dataset-title', `A dataset with ${LONG}`);
document.body.append(explorer);
const store = fakeStore({meta: META, status: status({})});
store.set('view', {...store.get('view'), id: 's0'});
store.set('legend', {...store.get('legend'), colourBy: 'archive'});
explorer.store = store;
await settle(document.body);
await frames(3);
const shadow = explorer.shadowRoot!;
const map = shadow.querySelector('mosaica-map')!;
const part = (name: string, root: ParentNode = shadow) => root.querySelector<HTMLElement>(`[part~="${name}"]`);
const round = (n: number) => Math.round(n * 100) / 100;
const size = (el: Element | null) => {
  const r = el?.getBoundingClientRect();
  return r ? {width: round(r.width), height: round(r.height)} : null;
};
const height = (el: Element | null) => size(el)?.height ?? null;
const press = async (el: HTMLElement | null) => {
  el!.click();
  await settle(document.body);
  await frames(3);
};

/** Whether text in `el` is drawn: not clipped away to nothing, as text for a screen reader alone is. */
function drawn(el: Element): boolean {
  for (let at: Element | null = el; at; at = at.parentElement ?? ((at.getRootNode() as ShadowRoot).host ?? null)) {
    const style = getComputedStyle(at);
    if (style.display === 'none' || style.visibility === 'hidden' || style.clipPath !== 'none') return false;
  }
  return true;
}

/** Each run of text drawn under `root`, through shadow roots, in order. */
function texts(root: Node): string[] {
  const out: string[] = [];
  const walk = (node: Node) => {
    if (node instanceof Element && node.shadowRoot) walk(node.shadowRoot);
    for (const child of Array.from(node.childNodes)) {
      if (child.nodeType === Node.TEXT_NODE) {
        const text = child.textContent!.replace(/\s+/g, ' ').trim();
        if (text && child.parentElement && !['STYLE', 'OPTION'].includes(child.parentElement.tagName) && drawn(child.parentElement)) out.push(text);
      } else walk(child);
    }
  };
  walk(root);
  return out;
}

/** How many lines of its own line height an element's box holds. */
const lines = (el: Element | null) => (el ? round(el.getBoundingClientRect().height / parseFloat(getComputedStyle(el).lineHeight)) : null);
const cut = (el: Element | null) => (el ? el.scrollWidth > el.clientWidth : null);

const column = part('panel') ?? part('sidebar')!;
const head = column.querySelector('.head')!;
const picker = shadow.querySelector('mosaica-view-picker')!;
const select = picker.shadowRoot!.querySelector('select')!;
const rest = {
  column: size(column)!.width,
  head: height(head),
  title: height(part('dataset-title')),
  titleLines: lines(part('dataset-title')),
  titleCut: cut(part('dataset-title')),
  titleHolds: part('dataset-title')!.textContent,
  titleTitle: part('dataset-title')!.getAttribute('title'),
  picker: height(picker),
  selectFits: select.getBoundingClientRect().right <= head.getBoundingClientRect().right,
  tools: size(map.shadowRoot!.querySelector('[part="controls"]')),
  layers: size(shadow.querySelector('.layers .group')),
  text: texts(head)
};

await press(part('layers-toggle'));
const popover = part('layers-popover')!;
const sections = Array.from(popover.querySelectorAll('.sec'));
const layerPicker = popover.querySelector('mosaica-layer-picker')!.shadowRoot!;
const entries = Array.from(layerPicker.querySelectorAll('[part="entry"]'));
const sliders = Array.from(popover.querySelectorAll<HTMLElement>('input[type="range"]'));
/** The room a control takes in its row: its box and its margins. */
const room = (el: HTMLElement) => {
  const style = getComputedStyle(el);
  return round(el.getBoundingClientRect().height + parseFloat(style.marginTop) + parseFloat(style.marginBottom));
};
const open = {
  width: size(popover)!.width,
  // What the popover holds, whatever height the window leaves it.
  content: round(sections.reduce((sum, sec) => sum + sec.getBoundingClientRect().height, 0) + popover.querySelector('mosaica-layer-picker')!.getBoundingClientRect().height),
  sections: sections.map(height),
  layerPicker: height(popover.querySelector('mosaica-layer-picker')),
  entries: entries.map(height),
  names: entries.map((e) => ({lines: lines(e.querySelector('[part="name"]')), cut: cut(e.querySelector('[part="name"]')), title: e.getAttribute('title')})),
  toggle: size(part('points-toggle')),
  sizeBy: {height: height(part('size-by')), title: part('size-by')!.getAttribute('title')},
  colourBy: height(part('colour-by')),
  palette: height(part('palette')),
  editColours: height(part('edit-colours')),
  modes: height(part('density-mode')),
  densityScale: height(part('density-scale')),
  densityColours: height(part('density-colours')),
  sliders: Object.fromEntries(sliders.map((s) => [s.getAttribute('part'), room(s)])),
  text: texts(popover)
};

/** A menu's width, the height of each entry, and where the tick on the chosen entry is against the entry's own content. */
async function menu(button: string, name: string) {
  await press(part(button));
  const el = part(`${name}-menu`)!;
  const options = Array.from(el.querySelectorAll<HTMLElement>('[role="menuitemradio"]'));
  const chosen = options.find((o) => o.getAttribute('aria-checked') === 'true')!;
  const tick = chosen.querySelector('.tick')?.getBoundingClientRect() ?? null;
  const over = (a: DOMRect, b: DOMRect) => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
  // The text and the swatches of the chosen entry, each as the box of what is drawn.
  const content = Array.from(chosen.querySelectorAll('.swatches, .bar, .t, .line, .lead > span:not(.tick), .kind')).map((c) => {
    const range = document.createRange();
    range.selectNodeContents(c);
    return c.matches('.swatches, .bar') ? c.getBoundingClientRect() : range.getBoundingClientRect();
  });
  const out = {
    width: size(el)!.width,
    entries: options.map(height),
    lefts: options.map((o) => round((o.querySelector('.swatches, .lead')!.getBoundingClientRect().left - o.getBoundingClientRect().left))),
    tick: tick ? {inside: tick.left >= chosen.getBoundingClientRect().left && tick.right <= chosen.getBoundingClientRect().right, covers: content.some((c) => over(tick, c))} : null,
    text: texts(el)
  };
  chosen.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true, composed: true, cancelable: true}));
  await settle(document.body);
  await frames(2);
  return out;
}
const palette = await menu('palette', 'palette');
const colour = await menu('colour-by', 'colour');

(window as unknown as {result: unknown}).result = {rest, open, palette, colour};
