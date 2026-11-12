import {afterEach, describe, expect, it} from 'vitest';
import type {RegionProjection} from '@mosaicajs/client';
import '../src/selection.js';
import {deepAll, fakeStore, mount, settle, status} from './fake-store.js';

afterEach(() => {
  document.body.innerHTML = '';
});

const shape = {kind: 'box' as const, bbox: [0, 0, 1, 1] as [number, number, number, number]};

/** A region holding `ids`, counted. */
const region = (ids: bigint[]): RegionProjection => ({
  shape,
  status: 'shown' as const,
  refusal: null,
  visible: {value: ids.length, exact: true},
  matched: {value: ids.length, exact: true},
  served: {shown: ids.length, total: ids.length, exact: true},
  verdict: {exact: true, depth: null},
  held: {ids: BigUint64Array.from(ids), positions: new Float32Array(2 * ids.length), count: ids.length}
});

/** A store whose item reads answer from `names`, by `mosaica_id`, and note what each asked for. */
function storeWithNames(names: Record<string, string | null>) {
  const store = fakeStore({status: status({status: 'shown'})});
  const reads: {fields: string[]; ids: string[]}[] = [];
  store.items = (async (req: {fields: string[]; filters: {mosaica_id: {in: string[]}}}) => {
    const ids = req.filters.mosaica_id.in;
    reads.push({fields: req.fields, ids});
    const field = req.fields[0]!;
    const page = {
      numRows: ids.length,
      getChild: (name: string) => ({get: (i: number) => (name === 'mosaica_id' ? BigInt(ids[i]!) : name === field ? (names[ids[i]!] ?? null) : undefined)})
    };
    return (async function* () {
      yield page;
    })();
  }) as never;
  return {store, reads};
}

const listed = (host: HTMLElement) => deepAll(host, '[part="item"] button') as HTMLButtonElement[];

describe('<mosaica-selection> names the marks it lists', () => {
  it('by the title field, read in one request for the marks listed, and by the id where an item has no value', async () => {
    const host = await mount('<mosaica-selection title-field="name"></mosaica-selection>');
    const {store, reads} = storeWithNames({'7': 'Lisbon', '9': null});
    (host.querySelector('mosaica-selection') as unknown as {store: unknown}).store = store;
    store.set('region', region([7n, 9n]));
    await settle(host);
    await settle(host);
    expect(reads).toEqual([{fields: ['name'], ids: ['7', '9']}]);
    expect(listed(host).map((b) => b.textContent)).toEqual(['Lisbon', '9']);
    expect(listed(host).map((b) => b.hasAttribute('data-id'))).toEqual([false, true]);
    listed(host)[0]!.click();
    expect(store.calls.filter((c) => c.name === 'pick').map((c) => c.args[0])).toEqual([7n]);

    // A mark that joins the list is asked about alone.
    store.set('region', region([7n, 9n, 11n]));
    await settle(host);
    await settle(host);
    expect(reads.at(-1)).toEqual({fields: ['name'], ids: ['11']});
  });

  it('by the id alone without a title field, and reads nothing', async () => {
    const host = await mount('<mosaica-selection></mosaica-selection>');
    const {store, reads} = storeWithNames({'7': 'Lisbon'});
    (host.querySelector('mosaica-selection') as unknown as {store: unknown}).store = store;
    store.set('region', region([7n]));
    await settle(host);
    expect(reads).toEqual([]);
    expect(listed(host).map((b) => b.textContent)).toEqual(['7']);
  });
});
