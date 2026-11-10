import {afterEach, describe, expect, it} from 'vitest';
import type {RegionProjection} from '@mosaicajs/client';
import '../src/map.js';
import {keptInside} from '../src/map.js';
import {deep, fakeStore, meta, mount, settle, status} from './fake-store.js';

/** The drawn region's tag: its count on the region's edge, and a × that clears the selection. */

afterEach(() => {
  document.body.innerHTML = '';
});

const region = (outside: boolean): RegionProjection => ({
  shape: {kind: 'box', bbox: [0.2, 0.2, 0.6, 0.6], outside},
  status: 'shown',
  refusal: null,
  visible: {value: 9000, exact: true},
  matched: {value: 4812, exact: true},
  served: {shown: 10, total: 4812, exact: true},
  verdict: {exact: true, depth: null},
  held: {ids: new BigUint64Array(0), positions: new Float32Array(0), count: 0}
});

async function mapWith(r: RegionProjection | null) {
  const host = await mount('<mosaica-map></mosaica-map>');
  const map = host.querySelector('mosaica-map') as HTMLElement & {store: unknown};
  const store = fakeStore({meta: meta(), status: status({}), region: r});
  map.store = store;
  await settle(host);
  return {host, store};
}

describe('<mosaica-map> region tag', () => {
  it('shows the count matched inside a drawn region, and clears the selection from its ×', async () => {
    const {host, store} = await mapWith(region(false));
    const tag = deep(host, '[part="region-tag"]')!;
    expect(tag).not.toBeNull();
    expect(tag.querySelector('mosaica-count')).not.toBeNull();
    (tag.querySelector('button') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'select').map((c) => c.args[0])).toEqual([null]);
  });

  it('sits above the region’s corner where there is room, and stays inside the map wherever the corner is', async () => {
    const {host} = await mapWith(region(false));
    const map = host.querySelector('mosaica-map') as HTMLElement & {lookAt(x: number, y: number): boolean; screenOf(world: [number, number]): [number, number]};
    const [width, height, w, h] = [1000, 800, 120, 26];
    Object.defineProperty(map, 'clientWidth', {configurable: true, value: width});
    Object.defineProperty(map, 'clientHeight', {configurable: true, value: height});
    const tag = deep(host, '[part="region-tag"]') as HTMLElement;
    Object.defineProperty(tag, 'offsetWidth', {configurable: true, value: w});
    Object.defineProperty(tag, 'offsetHeight', {configurable: true, value: h});
    /** Where the tag is once the camera looks at `x`, `y` in the view's coordinates. */
    const placed = async (x: number, y: number) => {
      expect(map.lookAt(x, y)).toBe(true);
      await settle(host);
      expect(deep(host, '[part="region-tag"]')).toBe(tag);
      const at = /translate\((-?[\d.]+)px, (-?[\d.]+)px\)/.exec(tag.style.transform)!;
      return [Number(at[1]), Number(at[2])] as const;
    };

    // The region's corner is at 0.2, 0.2, a fifth of the way across the 512-unit world.
    const [left, top] = await placed(0.4, 0.4);
    const corner = map.screenOf([0.2 * 512, 0.2 * 512]);
    expect(left).toBeCloseTo(corner[0]);
    expect(top + h).toBeLessThan(corner[1]);
    expect(corner[1] - (top + h)).toBeLessThan(h);

    for (const [x, y] of [[9, 0.4], [-9, 0.4], [0.4, 9], [0.4, -9], [9, 9], [-9, -9]] as const) {
      const [l, t] = await placed(x, y);
      expect(l, `${x},${y}`).toBeGreaterThanOrEqual(0);
      expect(t, `${x},${y}`).toBeGreaterThanOrEqual(0);
      expect(l + w, `${x},${y}`).toBeLessThanOrEqual(width);
      expect(t + h, `${x},${y}`).toBeLessThanOrEqual(height);
    }
  });

  it('keeps a box inside its bounds by moving it the least it can', () => {
    const [min, max] = [[10, 4], [990, 796]] as const;
    expect(keptInside([400, 300], [120, 26], min, max)).toEqual([400, 300]);
    expect(keptInside([950, 300], [120, 26], min, max)).toEqual([870, 300]);
    expect(keptInside([400, 790], [120, 26], min, max)).toEqual([400, 770]);
    expect(keptInside([-50, -50], [120, 26], min, max)).toEqual([10, 4]);
    // A box wider than its bounds starts at their near edge.
    expect(keptInside([400, 300], [2000, 26], min, max)).toEqual([10, 300]);
  });

  it('has no tag with no region, or for a region drawn as an artifact', async () => {
    expect(deep((await mapWith(null)).host, '[part="region-tag"]')).toBeNull();
    document.body.innerHTML = '';
    const artifact: RegionProjection = {...region(false), shape: {kind: 'artifact', id: 3n, outside: false}} as RegionProjection;
    expect(deep((await mapWith(artifact)).host, '[part="region-tag"]')).toBeNull();
  });
});
