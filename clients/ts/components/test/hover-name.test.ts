import {afterEach, describe, expect, it, vi} from 'vitest';
import type {Meta} from '@mosaicajs/client';
import {DENSITY_SETTLE_MS} from '@mosaicajs/deck';
import '../src/map.js';
import {tooltipCorner} from '../src/map.js';
import {aggregateEntry, answerAggregate, fakeStore, meta as deployment, mount, settle, status} from './fake-store.js';

/**
 * What a hover says a point is. The host names the field that titles a point (`title-field`);
 * unset, the title is the point's id and nothing is fetched. A field the marks do not carry is
 * asked of the record through the store's `describe`, which opens no card, once the pointer rests.
 */

afterEach(() => {
  document.body.innerHTML = '';
  vi.useRealTimers();
});

const meta = (scalars: {name: string; arrowType: string}[]): Meta =>
  ({
    declaredScalars: scalars.map((s) => ({...s, category: null, render: false, index: true}))
  }) as unknown as Meta;

/**
 * A hover reaches the map through deck's `onHover` callback. There is no deck in this environment,
 * and so no pointer to move over a mark: the tests call the callback as deck would, and read what
 * the map then draws. `map.browser.ts` moves a real pointer.
 */
type HoverMap = {
  store: unknown;
  worldAt: unknown;
  slab: {markAt: unknown};
  onHover(info: unknown): void;
};

/** The tooltip the map has drawn: its title and the lines under it, or null where it draws none. */
async function shown(el: HoverMap): Promise<{title: string; lines: string[]} | null> {
  const host = el as unknown as HTMLElement & {updateComplete: Promise<unknown>};
  await host.updateComplete;
  const tip = host.shadowRoot!.querySelector('[part="tooltip"]');
  if (!tip) return null;
  return {title: tip.querySelector('.t')!.textContent!, lines: tip.querySelector('.s')?.textContent?.split(' · ') ?? []};
}

async function map(scalars: {name: string; arrowType: string}[], attributes = '') {
  const host = await mount(`<mosaica-map ${attributes}></mosaica-map>`);
  const el = host.querySelector('mosaica-map') as unknown as HoverMap;
  const store = fakeStore({status: status({}), meta: meta(scalars)});
  el.store = store;
  // No deck here, so the map cannot unproject: the hovered artifact is resolved from a world
  // point and these tests are about the tooltip, not about the contours.
  el.worldAt = () => null;
  await settle(host);
  return {el, store};
}

/** Deck's answer for a mark: the slot layer, the row, and the ids that layer was given. */
const markAt = (id: bigint) => ({index: 0, x: 10, y: 20, sourceLayer: {id: 'marks-p0', props: {mosaicaIds: new BigUint64Array([id])}}});

/** The marks under the pointer carrying `scalars`, one row each. */
const carrying = (scalars: Record<string, {arrowType: string; value: unknown}>) => () => ({
  band: {scalars: Object.fromEntries(Object.entries(scalars).map(([k, v]) => [k, {arrowType: v.arrowType, values: [v.value]}]))},
  i: 0
});

describe('a hover over a mark', () => {
  it('shows the id at once and the title field from the record once the pointer has rested', async () => {
    vi.useFakeTimers();
    const {el, store} = await map([{name: 'name', arrowType: 'text'}], 'title-field="name"');
    store.describe = async () => ({name: 'Sheena McCurrach Art', country: 'GB'});

    el.onHover(markAt(31728047486770n));
    expect((await shown(el))?.title).toBe('#31728047486770');

    await vi.advanceTimersByTimeAsync(300);
    expect((await shown(el))?.title).toBe('Sheena McCurrach Art');
  });

  it('with no title field, shows the id and asks for nothing, whatever the bundle declares', async () => {
    vi.useFakeTimers();
    const {el, store} = await map([{name: 'name', arrowType: 'text'}]);
    let asked = 0;
    store.describe = async () => {
      asked += 1;
      return {name: 'unreachable'};
    };

    el.onHover(markAt(7n));
    await vi.advanceTimersByTimeAsync(300);
    expect(asked).toBe(0);
    expect((await shown(el))?.title).toBe('#7');
  });

  it('asks once for the mark the pointer settled on, not for the ones it crossed', async () => {
    vi.useFakeTimers();
    const {el, store} = await map([{name: 'name', arrowType: 'text'}], 'title-field="name"');
    const asked: string[] = [];
    store.describe = async (id: bigint) => {
      asked.push(id.toString());
      return {name: `n-${id}`};
    };

    for (const id of [1n, 2n, 3n, 4n]) {
      el.onHover(markAt(id));
      await vi.advanceTimersByTimeAsync(20);
    }
    await vi.advanceTimersByTimeAsync(300);
    expect(asked).toEqual(['4']);
  });

  it('reads a title field the marks carry off the mark, and asks for nothing', async () => {
    vi.useFakeTimers();
    const {el, store} = await map([{name: 'code', arrowType: 'utf8'}], 'title-field="code"');
    el.slab.markAt = carrying({code: {arrowType: 'utf8', value: 'GB-LND'}});
    let asked = 0;
    store.describe = async () => {
      asked += 1;
      return null;
    };

    el.onHover(markAt(5n));
    await vi.advanceTimersByTimeAsync(300);
    expect((await shown(el))?.title).toBe('GB-LND');
    expect(asked).toBe(0);
  });

  it('never takes the title from tooltip-fields: they are lines beneath the id', async () => {
    const {el} = await map([{name: 'code', arrowType: 'utf8'}, {name: 'name', arrowType: 'utf8'}], 'tooltip-fields="code,name"');
    el.slab.markAt = carrying({code: {arrowType: 'utf8', value: 'GB-LND'}, name: {arrowType: 'utf8', value: 'London'}});

    el.onHover(markAt(5n));
    expect(await shown(el)).toEqual({title: '#5', lines: ['GB-LND', 'London']});
  });

  it('shows a timestamp in full to the second, in UTC, without its fraction', async () => {
    const at = 1_700_000_000_123_000;
    const {el} = await map([{name: 'published', arrowType: 'timestamp_us'}], 'tooltip-fields="published"');
    el.slab.markAt = carrying({published: {arrowType: 'timestamp_us', value: at}});

    el.onHover(markAt(5n));
    expect((await shown(el))?.lines).toEqual(['14 November 2023, 22:13:20 UTC']);
  });
});

describe('a hover across a change of viewer', () => {
  it('is dropped when the store’s meta goes null', async () => {
    vi.useFakeTimers();
    const {el, store} = await map([{name: 'name', arrowType: 'text'}], 'title-field="name"');
    store.describe = async () => ({name: 'Sheena McCurrach Art'});
    el.onHover(markAt(31728047486770n));
    await vi.advanceTimersByTimeAsync(300);
    expect((await shown(el))?.title).toBe('Sheena McCurrach Art');

    store.set('meta', null);
    expect(await shown(el)).toBeNull();
  });
});

describe('a hover over the selected item', () => {
  const selected = (id: bigint) => ({item: {id, detail: {fields: {}, labels: [], views: [], scoped: {}}}, itemRefusal: null, artifact: null, artifactRefusal: null});

  it('shows no tooltip over the point whose card is open, and one over any other', async () => {
    const {el, store} = await map([]);
    store.set('selection', selected(7n));
    el.onHover(markAt(7n));
    expect(await shown(el)).toBeNull();
    el.onHover(markAt(8n));
    expect((await shown(el))?.title).toBe('#8');
  });

  it('drops the tooltip when the record of the point under the pointer arrives', async () => {
    const {el, store} = await map([]);
    el.onHover(markAt(7n));
    expect((await shown(el))?.title).toBe('#7');
    store.set('selection', selected(7n));
    expect(await shown(el)).toBeNull();
  });
});

describe('a hover under a highlight', () => {
  it('reads a lit mark, and a dulled one, from the pass that drew it', async () => {
    const {el} = await map([{name: 'name', arrowType: 'utf8'}], 'title-field="name"');
    let asked: number | null = null;
    el.slab.markAt = (slot: number) => {
      asked = slot;
      return carrying({name: {arrowType: 'utf8', value: 'London'}})();
    };
    for (const pass of ['lit', 'dull']) {
      asked = null;
      el.onHover({index: 0, x: 10, y: 20, sourceLayer: {id: `marks-p3-${pass}`, props: {mosaicaIds: new BigUint64Array([5n])}}});
      expect(asked).toBe(3);
      expect((await shown(el))?.title).toBe('London');
    }
  });
});

describe('the columns a hover reads', () => {
  /** The columns the map has asked of `store`, as its last `setPointColumns` left them. */
  const askedOf = (store: ReturnType<typeof fakeStore>): unknown[] =>
    (store.calls.filter((c) => c.name === 'setPointColumns').at(-1)?.args[1] as unknown[] | undefined) ?? [];

  it('are asked of the store, again when they change, and withdrawn from a store the map leaves', async () => {
    const {el, store} = await map([{name: 'name', arrowType: 'text'}], 'title-field="name" tooltip-fields="score, year"');
    expect(askedOf(store)).toEqual(['score', 'year', 'name']);

    const host = (el as unknown as HTMLElement).parentElement!;
    (el as unknown as {tooltipFields: string}).tooltipFields = 'year';
    await settle(host);
    expect(askedOf(store)).toEqual(['year', 'name']);

    const next = fakeStore({status: status({}), meta: meta([])});
    el.store = next;
    await settle(host);
    expect(askedOf(store)).toEqual([]);
    expect(askedOf(next)).toEqual(['year', 'name']);
  });
});

describe('the map in the tab order', () => {
  it('is one stop, after its tools and before the other corners, and focus() lands on it', async () => {
    const {el} = await map([]);
    const root = (el as unknown as HTMLElement).shadowRoot!;
    const stops = [...root.querySelectorAll<HTMLElement>('button, [tabindex]')].filter((e) => e.tabIndex >= 0).map((e) => e.getAttribute('part') ?? e.getAttribute('aria-label'));
    expect(stops).toEqual(['Pan', 'Box select', 'Lasso select', 'Fit to matches', 'canvas']);
    expect((el as unknown as HTMLElement).tabIndex).toBe(-1);
    (el as unknown as HTMLElement).focus();
    expect(root.activeElement?.getAttribute('part')).toBe('canvas');
  });

  it('moves a tabindex the host sets to the canvas, and names the canvas by the host’s aria-label', async () => {
    const {el} = await map([]);
    const host = el as unknown as HTMLElement;
    host.setAttribute('tabindex', '-1');
    host.setAttribute('aria-label', 'Papers');
    await (el as unknown as {updateComplete: Promise<unknown>}).updateComplete;
    const canvas = host.shadowRoot!.querySelector<HTMLElement>('[part="canvas"]')!;
    expect(canvas.tabIndex).toBe(-1);
    expect(canvas.getAttribute('aria-label')).toBe('Papers');
    host.setAttribute('tabindex', '2');
    await (el as unknown as {updateComplete: Promise<unknown>}).updateComplete;
    expect(canvas.tabIndex).toBe(2);
    expect(host.getAttribute('tabindex')).toBe('-1');
  });

  it('has a short name of its own without one from the host, and describes its keys either way', async () => {
    const {el} = await map([]);
    const host = el as unknown as HTMLElement;
    const canvas = host.shadowRoot!.querySelector<HTMLElement>('[part="canvas"]')!;
    const keys = () => host.shadowRoot!.getElementById(canvas.getAttribute('aria-describedby') ?? '')?.textContent ?? '';
    const own = canvas.getAttribute('aria-label')!;
    expect(own.trim()).not.toBe('');
    expect(own).not.toMatch(/arrow/i);
    for (const key of [/arrow/i, /zoom/i, /escape/i]) expect(keys()).toMatch(key);
    const unnamed = keys();
    host.setAttribute('aria-label', 'Papers');
    await (el as unknown as {updateComplete: Promise<unknown>}).updateComplete;
    expect(canvas.getAttribute('aria-label')).toBe('Papers');
    expect(keys()).toBe(unnamed);
  });
});

describe('the map’s keys', () => {
  /** A map 1000 × 800 px over a store whose `meta` has arrived, and each camera move it reports. */
  async function keyed() {
    const host = await mount('<mosaica-map></mosaica-map>');
    const el = host.querySelector('mosaica-map') as HTMLElement & {store: unknown; focusTarget: HTMLElement};
    Object.defineProperty(el, 'clientWidth', {configurable: true, value: 1000});
    Object.defineProperty(el, 'clientHeight', {configurable: true, value: 800});
    const store = fakeStore({meta: deployment(), status: status({})});
    store.set('view', {...store.get('view'), id: 's0'});
    el.store = store;
    await settle(host);
    const moves: {zoom: number; bbox: number[]}[] = [];
    el.addEventListener('mosaica-viewchange', (e) => moves.push((e as CustomEvent<{zoom: number; bbox: number[]}>).detail));
    const press = (key: string) => {
      const event = new KeyboardEvent('keydown', {key, bubbles: true, cancelable: true});
      el.focusTarget.dispatchEvent(event);
      return event.defaultPrevented;
    };
    return {host, el, moves, press};
  }

  // One zoom level for each press of + or - is pinned in `map.browser.ts`: the second level came
  // from deck's own keyboard handling, and there is no deck here.
  it('pan one step for each arrow, and leave any other key to the page', async () => {
    const {moves, press} = await keyed();
    // Zoomed in, so the box the camera shows lies inside the view's extent and moves with it.
    for (let i = 0; i < 3; i++) press('+');
    moves.length = 0;
    expect(press('ArrowRight')).toBe(true);
    expect(press('ArrowRight')).toBe(true);
    expect(press('ArrowDown')).toBe(true);
    expect(moves).toHaveLength(3);
    expect(new Set(moves.map((m) => m.zoom)).size).toBe(1);
    expect(moves[1]!.bbox[0]).toBeGreaterThan(moves[0]!.bbox[0]!);
    expect(moves[1]!.bbox[1]).toBe(moves[0]!.bbox[1]);
    expect(moves[2]!.bbox[0]).toBe(moves[1]!.bbox[0]);
    expect(moves[2]!.bbox[1]).toBeGreaterThan(moves[1]!.bbox[1]!);
    expect(press('a')).toBe(false);
    expect(press('Escape')).toBe(false);
    expect(moves).toHaveLength(3);
  });
});

describe('the density key', () => {
  it('describes what its largest figure counts to a reader of its name, in the words of the figure’s tooltip', async () => {
    vi.useFakeTimers({toFake: ['setTimeout', 'clearTimeout']});
    const host = await mount('<mosaica-map density="grid" no-points></mosaica-map>');
    const el = host.querySelector('mosaica-map') as HTMLElement & {store: unknown; fit(): Promise<void>};
    Object.defineProperty(el, 'clientWidth', {configurable: true, value: 1000});
    Object.defineProperty(el, 'clientHeight', {configurable: true, value: 800});
    const store = fakeStore({meta: deployment(), status: status({})});
    el.store = store;
    await settle(host);
    const key = () => el.shadowRoot!.querySelector('[part="density-key"]')!;
    // No counts yet, so no figure to qualify.
    expect(key().hasAttribute('aria-describedby')).toBe(false);

    const fitted = el.fit();
    answerAggregate(store, 'frame#', aggregateEntry([{rows: []}], store.get('view').id));
    await fitted;
    vi.advanceTimersByTime(DENSITY_SETTLE_MS);
    answerAggregate(store, 'density#', aggregateEntry([{rows: [1, 99].map((count, i) => ({cell: BigInt(i), count}))}], store.get('view').id));
    await settle(host);
    const described = el.shadowRoot!.getElementById(key().getAttribute('aria-describedby') ?? '');
    expect(described?.textContent?.trim()).not.toBe('');
    expect(described?.textContent?.trim()).toBe(key().querySelector('.ends > :last-child')!.getAttribute('title'));
  });
});

describe('the tooltip without a pointer move', () => {
  const tooltip = (el: HoverMap) => (el as unknown as HTMLElement).shadowRoot!.querySelector('[part="tooltip"]');
  const escape = (target: EventTarget) => {
    const key = new KeyboardEvent('keydown', {key: 'Escape', bubbles: true, composed: true, cancelable: true});
    target.dispatchEvent(key);
    return key.defaultPrevented;
  };

  it('is a tooltip to a screen reader', async () => {
    const {el} = await map([]);
    el.onHover(markAt(5n));
    await settle(document.body);
    expect(tooltip(el)?.getAttribute('role')).toBe('tooltip');
  });

  it('goes on Escape with the focus elsewhere on the page, leaves that press to the page, and stays away until the pointer is on another mark', async () => {
    const {el} = await map([]);
    const elsewhere = document.createElement('button');
    document.body.append(elsewhere);
    elsewhere.focus();
    // What the page's own Escape handler is given.
    const prevented: boolean[] = [];
    elsewhere.addEventListener('keydown', (e) => prevented.push(e.defaultPrevented));
    el.onHover(markAt(5n));
    await settle(document.body);
    expect(tooltip(el)).not.toBeNull();

    expect(escape(elsewhere)).toBe(false);
    expect(prevented).toEqual([false]);
    await settle(document.body);
    expect(tooltip(el)).toBeNull();
    // The same with nothing focused.
    el.onHover(markAt(9n));
    await settle(document.body);
    elsewhere.blur();
    expect(escape(document.body)).toBe(false);
    await settle(document.body);
    expect(tooltip(el)).toBeNull();
    el.onHover(markAt(5n));
    await settle(document.body);
    expect(escape(elsewhere)).toBe(false);
    await settle(document.body);
    // The pointer moves on the same mark.
    el.onHover({...markAt(5n), x: 11});
    await settle(document.body);
    expect(tooltip(el)).toBeNull();

    el.onHover(markAt(6n));
    await settle(document.body);
    expect(tooltip(el)).not.toBeNull();
    // And back on the first, it shows again.
    el.onHover(markAt(5n));
    expect((await shown(el))?.title).toBe('#5');
  });

  it('takes the Escape that dismisses it where the focus is in the map: that press clears no selection, and the next one does', async () => {
    const {el, store} = await map([]);
    const canvas = (el as unknown as HTMLElement).shadowRoot!.querySelector<HTMLElement>('[part="canvas"]')!;
    store.set('region', {shape: {kind: 'box', bbox: [0, 0, 1, 1]}, status: 'loading', refusal: null, visible: null, matched: {value: 0, exact: false}, served: {shown: 0, total: 0, exact: false}, verdict: null, held: {ids: new BigUint64Array(0), positions: new Float32Array(0), count: 0}} as never);
    canvas.focus();
    el.onHover(markAt(5n));
    await settle(document.body);
    const cleared = () => store.calls.filter((c) => c.name === 'select').length;

    expect(escape(canvas)).toBe(true);
    await settle(document.body);
    expect(tooltip(el)).toBeNull();
    expect(cleared()).toBe(0);

    expect(escape(canvas)).toBe(true);
    expect(cleared()).toBe(1);

    // With no tooltip, a press elsewhere on the page is not the map's.
    expect(escape(document.body)).toBe(false);
  });

  it('takes the Escape that dismisses it where the focus is in an element that renders the map in its shadow root, and not beside that element', async () => {
    class Holder extends HTMLElement {
      constructor() {
        super();
        this.attachShadow({mode: 'open'}).innerHTML = '<button>Inside</button><mosaica-map></mosaica-map>';
      }
    }
    if (!customElements.get('test-map-holder')) customElements.define('test-map-holder', Holder);
    const host = await mount('<test-map-holder></test-map-holder><button id="beside">Beside</button>');
    const holder = host.querySelector('test-map-holder')!.shadowRoot!;
    const el = holder.querySelector('mosaica-map') as unknown as HoverMap;
    el.store = fakeStore({status: status({}), meta: meta([])});
    el.worldAt = () => null;
    await settle(holder);

    const inside = holder.querySelector('button')!;
    inside.focus();
    el.onHover(markAt(5n));
    expect(await shown(el)).not.toBeNull();
    expect(escape(inside)).toBe(true);
    expect(await shown(el)).toBeNull();

    const beside = host.querySelector<HTMLElement>('#beside')!;
    beside.focus();
    el.onHover(markAt(6n));
    expect(await shown(el)).not.toBeNull();
    expect(escape(beside)).toBe(false);
    expect(await shown(el)).toBeNull();
  });

  it('stops listening to the page once it has gone, and when the map leaves the page', async () => {
    const {el} = await map([]);
    const host = el as unknown as HTMLElement;
    el.onHover(markAt(5n));
    await settle(document.body);
    host.remove();
    expect(escape(document.body)).toBe(false);
    expect((await shown(el))?.title).toBe('#5');
  });
});

describe('where the tooltip is put', () => {
  const bounds: [number, number] = [800, 500];
  const size: [number, number] = [200, 60];
  const inside = ([left, top]: [number, number]) => left >= 0 && top >= 0 && left + size[0] <= bounds[0] && top + size[1] <= bounds[1];

  it('is below and right of the pointer where there is room', () => {
    const [left, top] = tooltipCorner([100, 100], size, bounds);
    expect(left).toBeGreaterThan(100);
    expect(top).toBeGreaterThan(100);
  });

  it('is on the pointer’s other side along an axis where it would cross the map’s far edge', () => {
    const [left, top] = tooltipCorner([790, 100], size, bounds);
    expect(left + size[0]).toBeLessThan(790);
    expect(top).toBeGreaterThan(100);
    const [left2, top2] = tooltipCorner([100, 495], size, bounds);
    expect(left2).toBeGreaterThan(100);
    expect(top2 + size[1]).toBeLessThan(495);
  });

  it('stays inside the map for a pointer anywhere on it', () => {
    for (let x = 0; x <= bounds[0]; x += 25) for (let y = 0; y <= bounds[1]; y += 25) expect(inside(tooltipCorner([x, y], size, bounds)), `${x},${y}`).toBe(true);
  });

  it('starts at the map’s corner where the map is smaller than the tooltip', () => {
    expect(tooltipCorner([50, 20], [300, 100], [120, 60])).toEqual([0, 0]);
  });
});

describe('the tooltip through a press', () => {
  it('goes at a press, stays away while the pointer is held, and comes back on the next hover after it', async () => {
    const {el} = await map([]);
    el.onHover(markAt(5n));
    expect((await shown(el))?.title).toBe('#5');
    const canvas = (el as unknown as HTMLElement).shadowRoot!.querySelector('[part="canvas"]')!;
    canvas.dispatchEvent(new PointerEvent('pointerdown', {button: 0, bubbles: true, composed: true}));
    expect(await shown(el)).toBeNull();
    el.onHover(markAt(5n));
    expect(await shown(el)).toBeNull();
    window.dispatchEvent(new PointerEvent('pointerup'));
    el.onHover(markAt(5n));
    expect((await shown(el))?.title).toBe('#5');
  });
});
