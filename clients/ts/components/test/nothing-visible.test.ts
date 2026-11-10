import {afterEach, describe, expect, it} from 'vitest';
import '../src/map.js';
import '../src/status.js';
import {aggregateEntry, answerAggregate, deep, fakeStore, mount, registered, settle, status} from './fake-store.js';

afterEach(() => {
  document.body.innerHTML = '';
});

/**
 * A viewer who can see no item in the view at all is told so, in words on the map and on the
 * strip, and is not told Nothing in view, which reads as "pan elsewhere".
 */
describe('a viewer who can see nothing', () => {
  async function page() {
    const host = await mount('<mosaica-map></mosaica-map><mosaica-status></mosaica-status>');
    const store = fakeStore({status: status({status: 'empty', sessionWarm: true})});
    for (const el of host.querySelectorAll('mosaica-map, mosaica-status')) (el as unknown as {store: unknown}).store = store;
    await settle(host);
    return {host, store, map: host.querySelector('mosaica-map')!, strip: host.querySelector('mosaica-status')!};
  }
  const text = (el: Element) => deep(el.shadowRoot!, '[part="state"]')?.textContent?.trim() ?? '';
  const live = (el: Element) => deep(el.shadowRoot!, '[part="state"] [role="status"]')!;

  it('is counted over the whole visible set once while the view is empty, and named in words that are drawn and announced, by the map and by the strip', async () => {
    const {host, store, map, strip} = await page();
    const asked = [...registered(store)].filter(([id]) => id.startsWith('visible-total'));
    expect(asked.map(([, spec]) => spec)).toEqual([{groupings: [{count: true}], subject: 'visible'}]);
    const camera = text(map);
    expect(camera).toBe(text(strip));
    const regions = [live(map), live(strip)];
    answerAggregate(store, 'visible-total', aggregateEntry([{rows: [], total: 0}]));
    await settle(host);
    expect(deep(map.shadowRoot!, '[part="state"]')?.getAttribute('data-visible')).toBe('none');
    expect(text(map)).not.toBe(camera);
    expect(text(strip)).toBe(text(map));
    for (const [i, el] of [map, strip].entries()) {
      expect(live(el)).toBe(regions[i]);
      expect(live(el).textContent?.trim()).toBe(text(map));
      expect(live(el).closest('.sr-only')).toBeNull();
    }
  });

  it('is not claimed where the viewer can see items outside the camera', async () => {
    const {host, store, map, strip} = await page();
    const camera = text(map);
    answerAggregate(store, 'visible-total', aggregateEntry([{rows: [], total: 12}]));
    await settle(host);
    expect(deep(map.shadowRoot!, '[part="state"]')?.getAttribute('data-visible')).toBeNull();
    expect(text(map)).toBe(camera);
    expect(text(strip)).toBe(camera);
  });

  it('is asked once however often the view goes empty, and the answer kept', async () => {
    const {host, store} = await page();
    answerAggregate(store, 'visible-total', aggregateEntry([{rows: [], total: 12}]));
    for (let i = 0; i < 5; i++) {
      store.set('status', status({status: 'loading', sessionWarm: true}));
      await settle(host);
      store.set('status', status({status: 'empty', sessionWarm: true}));
      await settle(host);
    }
    expect(store.calls.filter((c) => c.name === 'setAggregate' && String(c.args[0]).startsWith('visible-total'))).toHaveLength(1);
  });
});
