import {afterEach, beforeAll, describe, expect, it, vi} from 'vitest';
import type {Meta, ViewInfo} from '@mosaicajs/client';
import '../src/map.js';
import '../src/explorer.js';
import type {MosaicaMap} from '../src/map.js';
import {basemapChanged} from '../src/basemap.js';
import {encodeWorld} from '../../deck/test/world-bytes.js';
import {deep, fakeStore, meta, mount, settle, status, type FakeStore} from './fake-store.js';

/**
 * What `<mosaica-map>` draws under the points for each `basemap` value, what it refuses on the view
 * it opens on, and what it says on a view switched to later.
 */

afterEach(() => {
  document.body.innerHTML = '';
});

// The map loads the drawing code on its first basemap; loaded once here, so a slow first import
// under load does not outlast the waits below.
beforeAll(async () => {
  await import('../../deck/src/basemap-draw.js');
});

const WORLD = encodeWorld([[[[-10, -10], [10, -10], [10, 10], [-10, 10]]]], [[[0, 0], [90, 0]]], []);

function view(id: string, projection: ViewInfo['projection'], tiled = false): ViewInfo {
  return {id, displayName: id, quantisation: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}, projection, worldAspect: projection === 'none' ? null : 1, tileScheme: tiled ? 'xyz' : null, tile: tiled ? {z: 0, x: 0, y: 0} : null, roster: null};
}

const DEPLOYMENT: Meta = meta({views: [view('geo', 'web_mercator', true), view('flat', 'none'), view('round', 'equirectangular'), view('round-2025', 'equirectangular')]});

/** A host's own deck.gl layer, as the map draws it. */
const HOST_LAYER = {id: 'host-basemap', props: {}, clone: () => HOST_LAYER} as never;
const WORLD_LAYERS = ['basemap-land', 'basemap-coastline', 'basemap-borders'];

/** A map over the fake store, open on `opening`, with `attrs` on its tag and `basemap` set as a property. */
async function map(opening: string, attrs = '', basemap?: unknown): Promise<{host: HTMLElement; el: MosaicaMap; store: FakeStore}> {
  const store = fakeStore({meta: DEPLOYMENT, status: status({})});
  store.set('view', {...store.get('view'), id: opening});
  store.setWorld(WORLD);
  const host = await mount(`<mosaica-map ${attrs}></mosaica-map>`);
  const el = host.querySelector('mosaica-map') as MosaicaMap;
  if (basemap !== undefined) (el as unknown as {basemap: unknown}).basemap = basemap;
  el.store = store;
  await drawn(host);
  return {host, el, store};
}

/** Settle, and give the basemap's code and answer time to land. */
async function drawn(host: HTMLElement): Promise<void> {
  for (let i = 0; i < 5; i++) {
    await settle(host);
    await new Promise((r) => setTimeout(r, 20));
  }
}

async function switchTo(host: HTMLElement, store: FakeStore, id: string): Promise<void> {
  store.set('view', {...store.get('view'), id});
  await drawn(host);
}

const refusal = (el: Element) => deep(el.shadowRoot!, '[part="overlay"] [part="refusal"][data-code="basemap"]');
const note = (el: Element) => deep(el.shadowRoot!, '[part="note"]');
const attribution = (el: Element) => deep(el.shadowRoot!, '[part="attribution"]');
const asked = (store: FakeStore) => store.calls.filter((c) => c.name === 'basemapWorld').length;

describe('<mosaica-map> basemap', () => {
  it('draws the world outline unset on a geographic view, and asks for it once', async () => {
    const {host, el, store} = await map('geo');
    expect(asked(store)).toBe(1);
    expect(refusal(el)).toBeNull();
    expect(note(el)).toBeNull();
    // Another view with a projection uses the outline already read.
    await switchTo(host, store, 'round');
    expect(note(el)).toBeNull();
  });

  it('draws nothing unset on a view with no projection, and asks for nothing', async () => {
    const {el, store} = await map('flat');
    expect(asked(store)).toBe(0);
    expect(refusal(el)).toBeNull();
    expect(note(el)).toBeNull();
  });

  it('refuses world on the view it opens on when that view has no projection', async () => {
    const {el, store} = await map('flat', 'basemap="world"');
    expect(refusal(el)?.textContent?.trim()).toBeTruthy();
    expect(asked(store)).toBe(0);
  });

  it('refuses tiles on the view it opens on when the view has no tile scheme', async () => {
    const {el} = await map('round', '', {tiles: 'https://tiles.example/{z}/{x}/{y}.png', attribution: 'Example tiles'});
    expect(refusal(el)).not.toBeNull();
    expect(attribution(el)).toBeNull();
  });

  it('refuses an attribute it does not know', async () => {
    const {el} = await map('geo', 'basemap="osm"');
    expect(refusal(el)).not.toBeNull();
  });

  it('draws nothing on a view switched to that one value does not fit, and says so there alone', async () => {
    const {host, el, store} = await map('geo', 'basemap="world"');
    expect(note(el)).toBeNull();
    await switchTo(host, store, 'flat');
    expect(refusal(el)).toBeNull();
    expect(note(el)?.textContent?.trim()).toBeTruthy();
    await switchTo(host, store, 'round');
    expect(note(el)).toBeNull();
  });

  it('refuses what a mapping names for a view that it does not fit, and follows the mapping per view', async () => {
    const {host, el, store} = await map('geo', '', {geo: 'none', flat: 'world'});
    expect(asked(store)).toBe(0);
    expect(refusal(el)).toBeNull();
    await switchTo(host, store, 'flat');
    expect(refusal(el)).not.toBeNull();
    // A view the mapping leaves out takes the default.
    await switchTo(host, store, 'round');
    expect(refusal(el)).toBeNull();
    expect(asked(store)).toBe(1);
  });

  it('credits the tile server in its corner while tiles are drawn', async () => {
    const {host, el, store} = await map('geo', '', {tiles: 'https://tiles.example/{z}/{x}/{y}.png', attribution: 'Example tiles'});
    expect(refusal(el)).toBeNull();
    expect(attribution(el)?.textContent).toBe('Example tiles');
    await switchTo(host, store, 'round');
    expect(attribution(el)).toBeNull();
    expect(note(el)).not.toBeNull();
  });

  it('draws an image on any view', async () => {
    const {el} = await map('flat', '', {image: 'https://example.org/plan.png', bounds: [0, 0, 1, 1]});
    expect(refusal(el)).toBeNull();
    expect(note(el)).toBeNull();
  });

  it('says so where the outline is refused or unreadable, and leaves the view as it was', async () => {
    const store = fakeStore({meta: DEPLOYMENT, status: status({})});
    store.set('view', {...store.get('view'), id: 'geo'});
    store.setWorld(Object.assign(new Error('the session has ended'), {code: 'expired-token'}));
    const host = await mount('<mosaica-map></mosaica-map>');
    const el = host.querySelector('mosaica-map') as MosaicaMap;
    el.store = store;
    await drawn(host);
    expect(note(el)).not.toBeNull();
    expect(refusal(el)).toBeNull();
    expect(store.get('status').status).toBe('shown');

    store.setWorld(encodeWorld([], [], [], 'MBW9'));
    el.basemap = 'world';
    await drawn(host);
    expect(note(el)).not.toBeNull();
  });

  it('is passed through by the explorer', async () => {
    const store = fakeStore({meta: DEPLOYMENT, status: status({})});
    store.set('view', {...store.get('view'), id: 'flat'});
    const host = await mount('<mosaica-explorer basemap="world"></mosaica-explorer>');
    const explorer = host.querySelector('mosaica-explorer') as unknown as {store: unknown; map: MosaicaMap | null};
    explorer.store = store;
    await drawn(host);
    expect(refusal(explorer.map!)).not.toBeNull();
  });

  it('keeps a layer the host sets while the outline is still being fetched', async () => {
    const store = fakeStore({meta: DEPLOYMENT, status: status({})});
    store.set('view', {...store.get('view'), id: 'geo'});
    let answer!: (bytes: ArrayBuffer) => void;
    store.setWorld(new Promise((resolve) => (answer = resolve)));
    const host = await mount('<mosaica-map></mosaica-map>');
    const el = host.querySelector('mosaica-map') as MosaicaMap;
    el.store = store;
    await drawn(host);
    el.basemap = HOST_LAYER;
    await drawn(host);
    answer(WORLD);
    await drawn(host);
    expect(el.probe.basemap).toEqual(['host-basemap']);
  });

  it('draws the outline again when the value goes back to world from a host layer', async () => {
    const {host, el} = await map('geo', 'basemap="world"');
    expect(el.probe.basemap).toEqual(WORLD_LAYERS);
    el.basemap = HOST_LAYER;
    await drawn(host);
    expect(el.probe.basemap).toEqual(['host-basemap']);
    el.basemap = 'world';
    await drawn(host);
    expect(el.probe.basemap).toEqual(WORLD_LAYERS);
  });

  it('drops what the store it had before drew, and its answer in flight', async () => {
    const {host, el} = await map('geo');
    expect(el.probe.basemap).toEqual(WORLD_LAYERS);
    let answer!: (bytes: ArrayBuffer) => void;
    const old = el.store as FakeStore;
    old.setWorld(new Promise((resolve) => (answer = resolve)));
    old.set('view', {...old.get('view'), id: 'round'});
    await drawn(host);
    // The next store's meta has not arrived.
    const next = fakeStore({status: status({status: 'loading'})});
    el.store = next;
    await drawn(host);
    expect(el.probe.basemap).toEqual([]);
    answer(WORLD);
    await drawn(host);
    expect(el.probe.basemap).toEqual([]);
    next.set('meta', DEPLOYMENT);
    next.set('view', {...next.get('view'), id: 'flat'});
    await drawn(host);
    expect(el.probe.basemap).toEqual([]);
    expect(note(el)).toBeNull();
  });

  it('draws nothing and says nothing once the store is taken away', async () => {
    const {host, el} = await map('flat', 'basemap="world"');
    expect(refusal(el)).not.toBeNull();
    el.store = null;
    await drawn(host);
    expect(refusal(el)).toBeNull();
    expect(el.probe.basemap).toEqual([]);
  });

  it('draws the outline again after leaving the page and coming back', async () => {
    const {host, el} = await map('geo');
    const errors: unknown[] = [];
    const onError = (e: ErrorEvent) => errors.push(e.error);
    window.addEventListener('error', onError);
    el.remove();
    await new Promise((r) => setTimeout(r, 400));
    host.append(el);
    await drawn(host);
    window.removeEventListener('error', onError);
    expect(el.probe.basemap).toEqual(WORLD_LAYERS);
    expect(note(el)).toBeNull();
    expect(errors).toEqual([]);
  });

  it('asks again for an outline that failed, at a later change, and the note goes', async () => {
    const store = fakeStore({meta: DEPLOYMENT, status: status({})});
    store.set('view', {...store.get('view'), id: 'geo'});
    store.setWorld(Object.assign(new Error('503 starting: the server is starting'), {status: 503}));
    const host = await mount('<mosaica-map></mosaica-map>');
    const el = host.querySelector('mosaica-map') as MosaicaMap;
    el.store = store;
    await drawn(host);
    expect(note(el)).not.toBeNull();
    store.setWorld(WORLD);
    await new Promise((r) => setTimeout(r, 1_100));
    store.set('status', status({}));
    await drawn(host);
    expect(note(el)).toBeNull();
    expect(el.probe.basemap).toEqual(WORLD_LAYERS);
    expect(asked(store)).toBe(2);
  });

  it('reads an object with a tiles or image key as one value, and any other as a mapping', async () => {
    const odd = meta({views: [view('tiles', 'none'), view('toString', 'equirectangular')]});
    for (const [opening, choice, outcome] of [
      // An image at the relative URL `world`.
      ['toString', {image: 'world'}, ['basemap-image']],
      // Not a template: it lacks {z}, {x} and {y}, or one of them.
      ['toString', {tiles: 'world'}, 'refused'],
      ['toString', {tiles: 'https://x/{z}/{x}.png'}, 'refused'],
      // An empty mapping draws each view's default; a key it does not hold is not read off the prototype.
      ['toString', {}, WORLD_LAYERS],
      ['tiles', {}, []]
    ] as const) {
      const store = fakeStore({meta: odd, status: status({})});
      store.set('view', {...store.get('view'), id: opening});
      store.setWorld(WORLD);
      const host = await mount('<mosaica-map></mosaica-map>');
      const el = host.querySelector('mosaica-map') as MosaicaMap;
      el.basemap = choice as never;
      el.store = store;
      await drawn(host);
      const what = JSON.stringify(choice);
      if (outcome === 'refused') {
        expect(refusal(el), what).not.toBeNull();
      } else {
        expect(refusal(el), what).toBeNull();
        expect(el.probe.basemap, what).toEqual(outcome);
      }
      host.remove();
    }
  });

  it('tells the host when its refusal or note changes, and only then', async () => {
    const store = fakeStore({meta: DEPLOYMENT, status: status({})});
    store.set('view', {...store.get('view'), id: 'flat'});
    store.setWorld(WORLD);
    const host = await mount('<mosaica-map basemap="world"></mosaica-map>');
    const el = host.querySelector('mosaica-map') as MosaicaMap;
    const said: unknown[] = [];
    host.addEventListener('mosaica-basemapchange', (e) => said.push((e as CustomEvent).detail));
    el.store = store;
    await drawn(host);
    store.set('status', status({}));
    await drawn(host);
    expect(said).toEqual([{refusal: expect.any(String), note: null}]);
    // Given again on a view it fits, then a view switched to that it does not.
    store.set('view', {...store.get('view'), id: 'geo'});
    el.basemap = 'none';
    await drawn(host);
    el.basemap = 'world';
    await drawn(host);
    store.set('view', {...store.get('view'), id: 'flat'});
    await drawn(host);
    expect(said.slice(1)).toEqual([{refusal: null, note: null}, {refusal: null, note: expect.any(String)}]);
  });

  it('composes its first tiles for the map’s camera, not the whole frame, where it has a size', async () => {
    const fetched: string[] = [];
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({drawImage: () => {}} as never);
    vi.stubGlobal('fetch', async (url: string) => {
      fetched.push(url);
      return new Response(new Blob([new Uint8Array(1)]));
    });
    vi.stubGlobal('createImageBitmap', async () => ({}));
    try {
      const store = fakeStore({meta: DEPLOYMENT, status: status({})});
      store.set('view', {...store.get('view'), id: 'geo'});
      const host = await mount('<mosaica-map></mosaica-map>');
      const el = host.querySelector('mosaica-map') as MosaicaMap;
      Object.defineProperty(el, 'clientWidth', {value: 400});
      Object.defineProperty(el, 'clientHeight', {value: 400});
      el.basemap = {tiles: 'https://open.example/{z}/{x}/{y}.png', attribution: 'T'};
      el.store = store;
      await drawn(host);
      // 400 px at the map's zoom (about 0 before the fit, -0.48 after) is level 1: its four tiles,
      // not the 64 of level 3 a map with no camera composes.
      expect(fetched.map((u) => u.split('/')[3])).toEqual(['1', '1', '1', '1']);
    } finally {
      vi.restoreAllMocks();
      vi.unstubAllGlobals();
    }
  });

  it('refuses a mapping holding something that is not a basemap', async () => {
    const {el, store} = await map('geo', '', {tile: 'https://tiles.example/{z}/{x}/{y}.png'});
    expect(refusal(el)).not.toBeNull();
    expect(asked(store)).toBe(0);
  });

  it('keeps the outline drawn across a switch between views that share a frame', async () => {
    const {host, el, store} = await map('round');
    expect(el.probe.basemap).toEqual(WORLD_LAYERS);
    // Every set of basemap layers the map draws from here on.
    const drawnSince: string[][] = [];
    let held = el.probe.basemap;
    Object.defineProperty(el.probe, 'basemap', {get: () => held, set: (ids: string[]) => drawnSince.push((held = ids))});
    await switchTo(host, store, 'round-2025');
    expect(drawnSince).toEqual([]);
    expect(el.probe.basemap).toEqual(WORLD_LAYERS);
    expect(asked(store)).toBe(1);
  });

  it('composes tiles again once the camera has rested', async () => {
    const fetched: string[] = [];
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({drawImage: () => {}} as never);
    vi.stubGlobal('fetch', async (url: string) => {
      fetched.push(url);
      return new Response(new Blob([new Uint8Array(1)]));
    });
    vi.stubGlobal('createImageBitmap', async () => ({}));
    try {
      const {host, el} = await map('geo', '', {tiles: 'https://rest.example/{z}/{x}/{y}.png', attribution: 'T'});
      // happy-dom lays nothing out; the map is given a size, which tiles are composed for.
      Object.defineProperty(el, 'clientWidth', {value: 600});
      Object.defineProperty(el, 'clientHeight', {value: 400});
      expect(el.probe.basemap).toEqual(['basemap-tiles']);
      const opening = new Set(fetched.map((u) => u.split('/')[3]));
      fetched.length = 0;
      el.fitBbox([0.45, 0.45, 0.55, 0.55]);
      await drawn(host);
      await new Promise((r) => setTimeout(r, 250));
      await drawn(host);
      expect(fetched.length).toBeGreaterThan(0);
      expect(new Set(fetched.map((u) => u.split('/')[3]))).not.toEqual(opening);
    } finally {
      vi.restoreAllMocks();
      vi.unstubAllGlobals();
    }
  });

  it.each([
    ['an array of values', ['world']],
    ['an empty array', []],
    ['a Map', new Map([['geo', 'world']])],
    ['an object with no prototype', Object.assign(Object.create(null), {geo: 'world'})],
    ['tiles with a key they do not take', {tiles: 'https://t.example/{z}/{x}/{y}.png', attribution: 'T', maxzoom: 12}],
    ['an image with a key it does not take', {image: 'a.png', opacity: 0.5}],
    ['an instance of a class', new (class Basemap {
      world = 'world';
    })()]
  ])('refuses %s, and goes on drawing', async (_, choice) => {
    const {host, el} = await map('geo', '', choice);
    expect(refusal(el)).not.toBeNull();
    expect(el.probe.basemap).toEqual([]);
    el.basemap = 'world';
    await drawn(host);
    expect(refusal(el)).toBeNull();
    expect(el.probe.basemap).toEqual(WORLD_LAYERS);
  });

  it('is not started again by an equal value the host writes afresh', async () => {
    const tiles = () => ({tiles: 'https://equal.example/{z}/{x}/{y}.png', attribution: 'T'});
    const {host, el, store} = await map('geo', '', tiles());
    await switchTo(host, store, 'round');
    expect(note(el)).not.toBeNull();
    const said: unknown[] = [];
    host.addEventListener('mosaica-basemapchange', (e) => said.push((e as CustomEvent).detail));
    el.basemap = tiles();
    await drawn(host);
    expect(refusal(el)).toBeNull();
    expect(note(el)).not.toBeNull();
    expect(said).toEqual([]);
  });

  it('says again why a view that just failed draws nothing, when it is shown again', async () => {
    const store = fakeStore({meta: DEPLOYMENT, status: status({})});
    store.set('view', {...store.get('view'), id: 'geo'});
    store.setWorld(new Error('the server is starting'));
    const host = await mount('<mosaica-map></mosaica-map>');
    const el = host.querySelector('mosaica-map') as MosaicaMap;
    el.store = store;
    await drawn(host);
    expect(note(el)).not.toBeNull();
    await switchTo(host, store, 'flat');
    expect(note(el)).toBeNull();
    await switchTo(host, store, 'geo');
    expect(note(el)).not.toBeNull();
    expect(asked(store)).toBe(1);
  });

  it('says so where no tile loads', async () => {
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({drawImage: () => {}} as never);
    vi.stubGlobal('fetch', async () => new Response('', {status: 404}));
    try {
      const said: unknown[] = [];
      const store = fakeStore({meta: DEPLOYMENT, status: status({})});
      store.set('view', {...store.get('view'), id: 'geo'});
      const host = await mount('<mosaica-map></mosaica-map>');
      const el = host.querySelector('mosaica-map') as MosaicaMap;
      host.addEventListener('mosaica-basemapchange', (e) => said.push((e as CustomEvent).detail));
      el.basemap = {tiles: 'https://down.example/{z}/{x}/{y}.png', attribution: 'T'};
      el.store = store;
      await drawn(host);
      expect(note(el)).not.toBeNull();
      expect(el.probe.basemap).toEqual([]);
      expect(said).toEqual([{refusal: null, note: expect.any(String)}]);
    } finally {
      vi.restoreAllMocks();
      vi.unstubAllGlobals();
    }
  });

  it('takes a value left undefined as left out', async () => {
    const {el} = await map('geo', '', {geo: undefined, flat: 'none'});
    expect(refusal(el)).toBeNull();
    expect(el.probe.basemap).toEqual(WORLD_LAYERS);
    const tiled = await map('geo', '', {tiles: 'https://undefined.example/{z}/{x}/{y}.png', attribution: 'T', maxzoom: undefined});
    expect(refusal(tiled.el)).toBeNull();
    expect(attribution(tiled.el)?.textContent).toBe('T');
    const imaged = await map('geo', '', {tiles: undefined, image: 'https://undefined.example/plan.png'});
    expect(refusal(imaged.el)).toBeNull();
    expect(imaged.el.probe.basemap).toEqual(['basemap-image']);
    const empty = await map('geo', '', {image: undefined});
    expect(refusal(empty.el)).toBeNull();
    expect(empty.el.probe.basemap).toEqual(WORLD_LAYERS);
  });

  it('reads afresh a refused object corrected to the equal plain one', async () => {
    const {host, el} = await map('geo', '', Object.assign(Object.create(null), {geo: 'world'}));
    expect(refusal(el)).not.toBeNull();
    el.basemap = {geo: 'world'};
    await drawn(host);
    expect(refusal(el)).toBeNull();
    expect(el.probe.basemap).toEqual(WORLD_LAYERS);
  });

  it('tells values apart by what they hold, whatever order their keys are in', () => {
    const t = 'https://order.example/{z}/{x}/{y}.png';
    expect(basemapChanged({attribution: 'T', tiles: t}, {tiles: t, attribution: 'T'})).toBe(false);
    expect(basemapChanged({geo: {attribution: 'T', tiles: t}}, {geo: {tiles: t, attribution: 'T'}})).toBe(false);
    expect(basemapChanged({tiles: t, attribution: 'T'}, {tiles: t, attribution: 'U'})).toBe(true);
    expect(basemapChanged(new Map(), {})).toBe(true);
  });

  it('waits longer each time the outline fails, and starts again for another ground', async () => {
    let now = 0;
    vi.spyOn(performance, 'now').mockImplementation(() => now);
    try {
      const store = fakeStore({meta: DEPLOYMENT, status: status({})});
      store.set('view', {...store.get('view'), id: 'geo'});
      store.setWorld(new Error('the server is starting'));
      const host = await mount('<mosaica-map></mosaica-map>');
      const el = host.querySelector('mosaica-map') as MosaicaMap;
      el.store = store;
      await drawn(host);
      const at = async (t: number) => {
        now = t;
        store.set('status', status({}));
        await drawn(host);
        return asked(store);
      };
      expect(asked(store)).toBe(1);
      // Waits of 1 s, then 2 s, then 4 s.
      expect(await at(900)).toBe(1);
      expect(await at(1_100)).toBe(2);
      expect(await at(3_000)).toBe(2);
      expect(await at(3_200)).toBe(3);
      expect(await at(7_100)).toBe(3);
      expect(await at(7_300)).toBe(4);
      // Another ground asks at once, and its wait starts again at 1 s.
      el.ground = 'dark';
      await drawn(host);
      expect(asked(store)).toBe(5);
      expect(await at(8_500)).toBe(6);
      expect(note(el)).not.toBeNull();
    } finally {
      vi.restoreAllMocks();
    }
  });

  it('composes tiles that failed again for a new camera once it rests there', async () => {
    const fetched: string[] = [];
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({drawImage: () => {}} as never);
    vi.stubGlobal('fetch', async (url: string) => {
      fetched.push(url);
      return new Response('', {status: 503});
    });
    try {
      const store = fakeStore({meta: DEPLOYMENT, status: status({})});
      store.set('view', {...store.get('view'), id: 'geo'});
      const host = await mount('<mosaica-map></mosaica-map>');
      const el = host.querySelector('mosaica-map') as MosaicaMap;
      Object.defineProperty(el, 'clientWidth', {value: 400});
      Object.defineProperty(el, 'clientHeight', {value: 400});
      el.basemap = {tiles: 'https://pan.example/{z}/{x}/{y}.png', attribution: 'T'};
      el.store = store;
      await drawn(host);
      expect(note(el)).not.toBeNull();
      const first = fetched.length;
      // A store change on the same tiles, inside the wait, asks for nothing.
      store.set('status', status({}));
      await drawn(host);
      expect(fetched.length).toBe(first);
      el.fitBbox([0.1, 0.1, 0.2, 0.2]);
      await new Promise((r) => setTimeout(r, 250));
      await drawn(host);
      expect(fetched.length).toBeGreaterThan(first);
    } finally {
      vi.restoreAllMocks();
      vi.unstubAllGlobals();
    }
  });
});
