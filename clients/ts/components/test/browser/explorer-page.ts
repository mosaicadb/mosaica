import '../../src/index.js';
import type {Band, ItemDetail, RegionProjection} from '@mosaicajs/client';
import {fakeStore, meta, scalar, settle, status} from '../fake-store.js';

// Built by `explorer.browser.ts` with Vite from the packages' sources. One explorer filling the
// window over a store with no network, set up as `?case=` names; `window.result` holds what the
// test measures, and `window.focused()` names the control that has focus.
const scene = new URLSearchParams(location.search).get('case') ?? '';
const frames = (n: number) =>
  new Promise<void>((resolve) => {
    const step = () => (--n <= 0 ? resolve() : requestAnimationFrame(step));
    requestAnimationFrame(step);
  });

/** The innermost element drawn at a point, through shadow roots. */
function topAt(x: number, y: number): Element | null {
  let el = document.elementFromPoint(x, y);
  for (;;) {
    const inner = el?.shadowRoot?.elementFromPoint(x, y);
    if (!inner || inner === el) return el;
    el = inner;
  }
}

/** Whether `el` is `container` or inside it, through slots and shadow roots. */
function inside(el: Node | null, container: Node): boolean {
  for (let at = el; at; at = (at instanceof Element ? at.assignedSlot : null) ?? at.parentNode ?? (at instanceof ShadowRoot ? at.host : null)) if (at === container) return true;
  return false;
}

/** Whether `el` is the top element at its own centre. */
function onTop(el: Element): boolean {
  const r = el.getBoundingClientRect();
  return inside(topAt(r.left + r.width / 2, r.top + r.height / 2), el);
}

const box = (el: Element) => {
  const r = el.getBoundingClientRect();
  return {left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width, height: r.height};
};

/** The element with focus, through shadow roots. */
function deepActive(): Element | null {
  let el = document.activeElement;
  while (el?.shadowRoot?.activeElement) el = el.shadowRoot.activeElement;
  return el;
}

const columns = Array.from({length: 40}, (_, i) => scalar(`measure_${String(i).padStart(2, '0')}`, 'u32', {render: true, homes: ['rendered']}));
const META = meta({
  views: [
    {id: 's0', displayName: 'A layout whose name is long enough to need a second line', quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, projection: 'none', worldAspect: null, tileScheme: null, tile: null, roster: null},
    {id: 's1', displayName: 'other', quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, projection: 'none', worldAspect: null, tileScheme: null, tile: null, roster: null}
  ],
  declaredScalars: [scalar('archive', 'u16', {category: {vocabulary: 'a', kind: 'declared', visibility: 'public'}, render: true, homes: ['rendered']}), ...columns],
  filterOperands: [{column: 'archive', family: 'category', operands: ['in']}, ...columns.slice(0, 16).map((c) => ({column: c.name, family: 'numeric' as const, operands: ['range']}))]
});

document.documentElement.style.height = '100%';
document.body.style.height = '100%';
document.body.innerHTML = `<mosaica-explorer layout="overlay" dataset-title="A dataset with a title long enough to need a second line at this width" title-field="title" density="smooth"></mosaica-explorer>`;
const explorer = document.querySelector('mosaica-explorer') as HTMLElement & {store: unknown; updateComplete: Promise<unknown>};
const store = fakeStore({meta: META, status: status({})});
store.set('view', {...store.get('view'), id: 's0'});
explorer.store = store;
await settle(document.body);
await frames(3);
const shadow = explorer.shadowRoot!;
const part = <T extends HTMLElement = HTMLElement>(name: string) => shadow.querySelector<T>(`[part~="${name}"]`);
const press = async (el: HTMLElement | null) => {
  el!.click();
  await settle(document.body);
  await frames(3);
};
const item = {id: 5n, detail: {fields: {title: 'A picked item'}, labels: [], views: [], scoped: {}} as ItemDetail};
const map = shadow.querySelector('mosaica-map') as HTMLElement & {pickedAt: unknown; focus(): void; screenOf(w: [number, number]): [number, number]};

const viewSelect = () => shadow.querySelector('mosaica-view-picker')!.shadowRoot!.querySelector('select')!;

const result: Record<string, unknown> = {};

/** Where focus is: the nearest named container around the control, and the control's own name; null outside the explorer. */
(window as unknown as {focused: () => string | null}).focused = () => {
  const el = deepActive();
  if (!el || !inside(el, explorer)) return null;
  const where = ['callout', 'region-tag', 'selection-card', 'controls', 'layers-popover', 'layers-toggle', 'canvas', 'panel'].find((name) => {
    for (let at: Node | null = el; at; at = (at instanceof Element ? at.assignedSlot : null) ?? at.parentNode ?? (at instanceof ShadowRoot ? at.host : null)) if (at instanceof Element && at.getAttribute('part')?.split(/\s+/).includes(name)) return true;
    return false;
  });
  const strip = shadow.querySelector('.in-map-strip');
  const menu = shadow.querySelector('[role="menu"]');
  const place = menu && inside(el, menu) ? 'menu' : (where ?? (strip && inside(el, strip) ? 'strip' : 'other'));
  return `${place}: ${el.getAttribute('aria-label') ?? (el.textContent?.trim() || el.getAttribute('part') || el.localName)}`;
};
/** What the Layers popover has open. */
(window as unknown as {shown: () => Record<string, boolean>}).shown = () => ({
  popover: part('layers-popover') !== null,
  list: shadow.querySelector('#density-colour-list') !== null,
  menu: shadow.querySelector('[role="menu"]') !== null,
  editor: (shadow.querySelector('mosaica-colour-editor')!.shadowRoot!.querySelector('dialog') as HTMLDialogElement | null)?.open ?? false
});
/** What is open over the map: its tooltip, the card beside a point, a sheet. */
(window as unknown as {over: () => Record<string, boolean>}).over = () => ({
  tooltip: map.shadowRoot!.querySelector('[part="tooltip"]') !== null,
  card: part('callout') !== null,
  sheet: part('sheet') !== null
});
/** The page as it stands once its elements have drawn. */
(window as unknown as {settled: () => Promise<void>}).settled = async () => {
  await settle(document.body);
  await frames(2);
};

if (scene === 'short') {
  // The narrow layout in a short window: the strip and the bar of buttons stay in the explorer's box, on top.
  const buttons = [...shadow.querySelectorAll('[part="tabs"] button')];
  Object.assign(result, {
    explorer: box(explorer),
    strip: box(part('strip-row')!),
    tabs: box(part('tabs')!),
    map: box(map),
    stripOnTop: onTop(part('strip-row')!),
    tabsOnTop: buttons.map(onTop),
    select: {cut: viewSelect().scrollWidth > viewSelect().clientWidth, lines: Math.round(viewSelect().getBoundingClientRect().height / parseFloat(getComputedStyle(viewSelect()).lineHeight))},
    titleCut: part('dataset-title')!.scrollWidth > part('dataset-title')!.clientWidth,
    titleLines: Math.round(part('dataset-title')!.getBoundingClientRect().height / parseFloat(getComputedStyle(part('dataset-title')!).lineHeight))
  });
} else if (scene === 'sheet') {
  // An open sheet ends above the strip, for each of the three sheets.
  const sheets: Record<string, unknown> = {};
  for (const name of ['filters', 'layers', 'detail']) {
    await press(shadow.querySelector(`[part="tabs"] [data-sheet="${name}"]`));
    sheets[name] = {sheet: box(part('sheet')!), stripOnTop: onTop(part('strip-row')!), focused: shadow.activeElement === part('sheet'), inert: map.inert};
  }
  Object.assign(result, {sheets, strip: box(part('strip-row')!), tabs: box(part('tabs')!)});
  // A picked item opens the Item sheet.
  await press(shadow.querySelector('[part="tabs"] [data-sheet="detail"]'));
  store.set('selection', {item, itemRefusal: null, artifact: null, artifactRefusal: null});
  await settle(document.body);
  await frames(3);
  result['picked'] = {card: part('sheet')?.querySelector('mosaica-item-card') !== null, focused: shadow.activeElement === part('sheet')};
} else if (scene === 'cover') {
  // A sheet that reaches over the map's tools: what the map and the sheet are while it is open, and after Escape.
  const tools = map.shadowRoot!.querySelector('[part="controls"]')!;
  await press(shadow.querySelector('[part="tabs"] [data-sheet="filters"]'));
  result['open'] = {inert: map.inert, sheet: box(part('sheet')!), tools: box(tools), focused: shadow.activeElement === part('sheet')};
  part('sheet')!.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true, composed: true, cancelable: true}));
  await settle(document.body);
  await frames(3);
  result['closed'] = {inert: map.inert, sheet: part('sheet') !== null};
  shadow.querySelector<HTMLElement>('[part="tabs"] button')!.focus();
} else if (scene === 'strip') {
  // The status strip with more to say than one line holds at this width.
  store.set('status', status({stale: true}));
  await settle(document.body);
  await frames(3);
  const strip = shadow.querySelector('[part="strip-row"] mosaica-status')!.shadowRoot!.querySelector('[part="strip"]')!;
  const cells = [...strip.children].map(box);
  Object.assign(result, {
    explorer: box(explorer),
    row: box(part('strip-row')!),
    strip: box(strip),
    tabs: box(part('tabs')!),
    cells,
    clipped: strip.scrollHeight > strip.clientHeight + 1 || strip.scrollWidth > strip.clientWidth + 1,
    tabsOnTop: [...shadow.querySelectorAll('[part="tabs"] button')].map(onTop)
  });
} else if (scene === 'menu') {
  // The menus of the Layers sheet or popover fit the window and scroll inside it.
  store.set('legend', {...store.get('legend'), colourBy: 'archive'});
  await settle(document.body);
  const narrow = part('tabs') !== null;
  await press(narrow ? shadow.querySelector('[part="tabs"] [data-sheet="layers"]') : part('layers-toggle'));
  const menus: Record<string, unknown> = {};
  for (const name of ['palette', 'colour'] as const) {
    await press(part(name === 'colour' ? 'colour-by' : 'palette'));
    const menu = part(`${name}-menu`)!;
    const last = [...menu.querySelectorAll('button')].at(-1)!;
    last.focus();
    await frames(2);
    menus[name] = {menu: box(menu), scrolls: menu.scrollHeight > menu.clientHeight, lastOnTop: onTop(last), last: box(last)};
    last.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true, composed: true}));
    await settle(document.body);
  }
  Object.assign(result, {menus, window: {width: innerWidth, height: innerHeight}});
} else if (scene === 'order') {
  // A region and a picked item in the wide layout, for the test to walk with Tab from the map.
  const region: RegionProjection = {shape: {kind: 'box', bbox: [0.4, 0.4, 0.6, 0.6]}, status: 'shown', refusal: null, visible: {value: 3, exact: true}, matched: {value: 3, exact: true}, served: {shown: 1, total: 3, exact: true}, verdict: {exact: true, depth: null}, held: {ids: BigUint64Array.of(5n), positions: new Float32Array(2), count: 1}};
  store.set('region', region);
  // Data updated since the view was drawn, so the strip holds a Refresh button.
  store.set('status', status({stale: true}));
  map.pickedAt = {kind: 'item', id: 5n, world: [0.5, 0.5]};
  store.set('selection', {item, itemRefusal: null, artifact: null, artifactRefusal: null});
  await settle(document.body);
  await frames(4);
  map.focus();
  result['callout'] = part('callout') !== null && !part('callout')!.hidden;
  result['regionTag'] = map.shadowRoot!.querySelector('[part="region-tag"]') !== null;
} else if (scene === 'targets') {
  // The height a pointer has to hit on each control of the Layers popover and of the heading.
  await press(part('layers-toggle'));
  const toggle = part('points-toggle')!;
  const t = toggle.getBoundingClientRect();
  const mid = t.left + t.width / 2;
  const hits = [-11, 0, 11].map((dy) => inside(topAt(mid, t.top + t.height / 2 + dy), toggle));
  const sliders = [...shadow.querySelectorAll<HTMLElement>('[part="display"] input[type="range"]')];
  const select = shadow.querySelector('mosaica-view-picker')!.shadowRoot!.querySelector('select')!;
  // No two sliders' boxes overlap.
  const boxes = sliders.map(box);
  const overlap = boxes.some((a, i) => boxes.some((b, j) => i < j && a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom));
  Object.assign(result, {
    toggle: hits,
    sliders: Object.fromEntries(sliders.map((s) => [s.getAttribute('part'), s.getBoundingClientRect().height])),
    overlap,
    editColours: part('edit-colours')!.getBoundingClientRect().height,
    select: select.getBoundingClientRect().height
  });
} else if (scene === 'footer') {
  // The Fields sheet with more cards than it shows at once, for the test to walk with Tab.
  explorer.setAttribute('pinned-filters', ['archive', ...columns.slice(0, 16).map((c) => c.name)].join(' '));
  await settle(document.body);
  const opens = shadow.querySelector<HTMLElement>('[part="tabs"] [data-sheet="filters"]')!;
  opens.focus();
  /** The control with focus while it is in the sheet's scrolling part, and whether it is the top element at its own centre. */
  (window as unknown as {stop: () => {name: string; onTop: boolean; aboveFooter: boolean} | null}).stop = () => {
    const el = deepActive();
    const body = shadow.querySelector('.sheet-body');
    if (!el || !body || !inside(el, body)) return null;
    const footer = shadow.querySelector('.sheet-footer')!.getBoundingClientRect();
    return {name: `${el.getAttribute('aria-label') ?? el.textContent?.trim() ?? el.localName}`, onTop: onTop(el), aboveFooter: el.getBoundingClientRect().bottom <= footer.top + 0.5};
  };
  (window as unknown as {sheet: () => unknown}).sheet = () => {
    const body = shadow.querySelector('.sheet-body')!;
    return {scrolls: body.scrollHeight > body.clientHeight, footer: box(shadow.querySelector('.sheet-footer')!), sheet: box(part('sheet')!), footerOnTop: [...shadow.querySelectorAll('.sheet-footer button')].map(onTop)};
  };
} else if (scene === 'tooltip') {
  // Two points on the map, large enough to hover, clear of what the test opens. A press on one picks it.
  explorer.setAttribute('radius', '8');
  const mb = map.getBoundingClientRect();
  const narrow = part('tabs') !== null;
  const points: [number, number][] = narrow ? [[mb.left + 250, mb.top + 80], [mb.left + 300, mb.top + 180]] : [[1050, 250], [1100, 600]];
  const [cx, cy] = map.screenOf([256, 256]);
  const perPixel = [1 / (map.screenOf([257, 256])[0] - cx), 1 / (map.screenOf([256, 257])[1] - cy)] as const;
  const band = {
    depth: 2, prefix: 0n, x: 0, y: 0,
    ids: BigUint64Array.from(points.map((_, i) => BigInt(100 + i))),
    positions: Float32Array.from(points.flatMap(([x, y]) => [256 + (x - mb.left - cx) * perPixel[0], 256 + (y - mb.top - cy) * perPixel[1]])),
    scalars: {}, served: points.length, capUsed: 500, visible: BigInt(points.length), matched: BigInt(points.length), highlighted: BigInt(points.length),
    highlightBits: null, membership: {}, heldBelow: 0n, identityKey: 'points', contentKey: 'points', bytes: 0, touchedAt: 0
  } as unknown as Band;
  store.set('marks', {bands: [band], standIn: [], count: {shown: points.length, total: points.length, exact: true}});
  // As the store does, a pick is answered with the item's record, and a cleared selection is published.
  store.pick = async (id: bigint) => store.set('selection', {item: {id, detail: item.detail}, itemRefusal: null, artifact: null, artifactRefusal: null});
  store.clearSelection = () => store.set('selection', {item: null, itemRefusal: null, artifact: null, artifactRefusal: null});
  await settle(document.body);
  await frames(6);
  result['points'] = points;
  (narrow ? shadow.querySelector<HTMLElement>('[part="tabs"] [data-sheet="filters"]') : part('layers-toggle'))!.focus();
} else if (scene === 'layers') {
  // The points coloured by a category, and focus on the Layers button, for the test to drive from the keyboard.
  store.set('legend', {...store.get('legend'), colourBy: 'archive'});
  await settle(document.body);
  part('layers-toggle')!.focus();
}

(window as unknown as {result: unknown}).result = result;
