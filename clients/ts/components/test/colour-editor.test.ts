import {afterEach, describe, expect, it} from 'vitest';
import type {Meta} from '@mosaicajs/client';
import '../src/colour-editor.js';
import type {MosaicaColourEditor} from '../src/colour-editor.js';
import {aggregateEntry, answerAggregate, deep, deepAll, fakeStore, meta, mount, registered, scalar, settle, status} from './fake-store.js';

afterEach(() => {
  document.body.innerHTML = '';
});

const layer = (name: string, kind: string, levels: {level: number; title: string}[] = []) =>
  ({name, title: name === 'topics' ? 'Topics' : name, views: ['s0'], membership: 'enumerated', hierarchy: {kind, pruneChildren: false}, levels, computedContent: ['centroid'], shape: null, suppliedContent: ['name'], depsOn: [], version: 1}) as unknown as Meta['layers'][number];

const META = meta({
  declaredScalars: [scalar('archive', 'u16', {category: {vocabulary: 'a', kind: 'declared', visibility: 'public'}, render: true, homes: ['rendered']})],
  filterOperands: [{column: 'archive', family: 'category', operands: ['in']}],
  layers: [layer('topics', 'nested'), layer('bands', 'stacked', [{level: 0, title: 'Broad'}, {level: 1, title: 'Fine'}])]
});

async function open(field: string, attrs = '') {
  const host = await mount(`<button id="opener">Edit colours</button><mosaica-colour-editor field="${field}" ${attrs}></mosaica-colour-editor>`);
  const editor = host.querySelector('mosaica-colour-editor') as MosaicaColourEditor;
  const store = fakeStore({meta: META, status: status({})});
  store.set('view', {...store.get('view'), id: 's0'});
  editor.store = store;
  await settle(host);
  (host.querySelector('#opener') as HTMLButtonElement).focus();
  await editor.show();
  await settle(host);
  return {host, editor, store};
}

const specOf = (store: ReturnType<typeof fakeStore>, prefix: string) => [...registered(store)].find(([id]) => id.startsWith(prefix))?.[1];
const rowsOf = (host: HTMLElement) => deepAll(host, '[part~="row"]') as HTMLElement[];
const names = (host: HTMLElement) => rowsOf(host).map((r) => [r.dataset.key, r.querySelector('[part="name"]')!.textContent, r.querySelector('[part="count"]')!.textContent]);
const settleTwice = async (host: HTMLElement) => {
  for (let i = 0; i < 3; i++) {
    await new Promise((r) => setTimeout(r, 0));
    await settle(host);
  }
};

/** A tree layer's drawn cut, counted over everything the viewer may see. */
const CUT = [
  {key: 7n, count: 900, title: 'optics', slot: 2},
  {key: 8n, count: 500, title: 'lasers', slot: 0},
  {key: 9n, count: 40, title: 'fibres', slot: 1}
];

describe('<mosaica-colour-editor> on a tree layer', () => {
  it('lists the clusters of the drawn cut by their counts over everything visible, as a modal dialog named by the layer', async () => {
    const {host, store} = await open('cluster:topics');
    expect(specOf(store, 'colours-ranked')).toEqual({groupings: [{count: true, by: {layer: 'topics', top: 1000, cut: 'drawn', paletteSize: 'drawn'}}], subject: 'visible'});
    answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: CUT, groups: 3}], 's0', undefined, 'tableau10'));
    await settle(host);
    const dialog = deep(host, '[part="dialog"]') as HTMLDialogElement;
    expect(dialog.open).toBe(true);
    expect(deep(host, `#${dialog.getAttribute('aria-labelledby')}`)!.textContent).toBe('Topics');
    expect(deep(host, '[part="sub"]')!.textContent).toBe('Clusters drawn · Tableau 10');
    expect(names(host)).toEqual([
      ['7', 'optics', '900'],
      ['8', 'lasers', '500'],
      ['9', 'fibres', '40']
    ]);
    // Each swatch is its slot's colour: slot 2 of Tableau 10.
    expect(rowsOf(host)[0]!.querySelector('[part="swatch"]')!.getAttribute('style')).toContain('#e15759');
    // The search reaches every cluster of the layer, not only those listed.
    expect(deep(host, 'mosaica-cluster-filter')!.getAttribute('placeholder')).toBe('Search clusters');
  });

  it('finds a cluster outside the listed rows with the search box over the whole tree, and lists it first with its count', async () => {
    const {host, store} = await open('cluster:topics');
    answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: CUT, groups: 3}], 's0', undefined, 'tableau10'));
    store.setBrowse('q:holo', {artifacts: [{mosaicaId: 42n, key: null, name: 'holography', maskedCount: 3n, matchedCount: null, rung: 4, parentIds: [], childCount: 0, slot: null}], parents: [], next: null});
    await settle(host);
    const input = deep(host, 'mosaica-cluster-filter')!.shadowRoot!.querySelector('[part="entry"]') as HTMLInputElement;
    input.focus();
    input.value = 'holo';
    input.dispatchEvent(new Event('input'));
    await new Promise((r) => setTimeout(r, 300));
    await settle(host);
    expect(store.calls.some((c) => c.name === 'browse' && (c.args[0] as {q?: string}).q === 'holo')).toBe(true);
    (deep(host, '[part="option"][data-id="42"]') as HTMLButtonElement).click();
    await settle(host);
    // Choosing it puts no clause on.
    expect(store.calls.filter((c) => c.name === 'setMembers')).toEqual([]);
    expect(specOf(store, 'colours-found')).toEqual({groupings: [{count: true, by: {layer: 'topics', artifacts: [42n], paletteSize: 'drawn'}}], subject: 'visible'});
    answerAggregate(store, 'colours-found', aggregateEntry([{rows: [{key: 42n, count: 3, slot: 4}]}], 's0', undefined, 'tableau10'));
    await settleTwice(host);
    expect(names(host)[0]).toEqual(['42', 'holography', '3']);
    expect(rowsOf(host)[0]!.hasAttribute('data-found')).toBe(true);
    expect(rowsOf(host)).toHaveLength(4);
  });

  it('gives every ticked cluster the colour chosen with Set colour, as one change', async () => {
    const {host, store} = await open('cluster:topics');
    answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: CUT, groups: 3}], 's0', undefined, 'tableau10'));
    await settle(host);
    const seen: unknown[] = [];
    host.addEventListener('mosaica-clustercolour', (e) => seen.push((e as CustomEvent).detail));
    for (const i of [0, 2]) {
      const box = rowsOf(host)[i]!.querySelector('[part="check"]') as HTMLInputElement;
      box.checked = true;
      box.dispatchEvent(new Event('change'));
      await settle(host);
    }
    expect(deep(host, '[part="selected"]')!.textContent).toContain('2 selected');
    (deep(host, '[part="set-colour"]') as HTMLButtonElement).click();
    await settle(host);
    (deep(host, '[part="hex"]') as HTMLInputElement).value = '#123456';
    deep(host, '[part="hex"]')!.dispatchEvent(new Event('change'));
    await settle(host);
    expect(seen).toEqual([
      {
        layer: 'topics',
        changes: [
          {mosaicaId: '7', colour: '#123456'},
          {mosaicaId: '9', colour: '#123456'}
        ]
      }
    ]);
    expect([...store.get('artifacts').overrides.get('topics')!.keys()].sort()).toEqual([7n, 9n]);
    expect(rowsOf(host).map((r) => r.querySelector('[part="swatch"]')!.getAttribute('style')!.includes('#123456'))).toEqual([true, false, true]);
    expect(deep(host, '[part="changed"]')!.textContent).toBe('2 colours changed');
    (deep(host, '[part="deselect"]') as HTMLButtonElement).click();
    await settle(host);
    expect(deep(host, '[part="selected"]')).toBeNull();
  });

  it('resets every colour chosen for the layer’s clusters with one change, and none of another layer’s', async () => {
    const {host, store} = await open('cluster:topics');
    const red = [200, 0, 0, 220] as const;
    // 7 is listed; 12 is the layer's but not drawn now; 7 and 99 of another layer are that layer's.
    store.setArtifactColours(new Map([['topics', new Map([[7n, red], [12n, red]])], ['bands', new Map([[7n, red], [99n, red]])]]));
    answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: CUT, groups: 3}], 's0', undefined, 'tableau10'));
    await settle(host);
    expect(deep(host, '[part="changed"]')!.textContent).toBe('2 colours changed');
    const seen: unknown[] = [];
    host.addEventListener('mosaica-clustercolour', (e) => seen.push((e as CustomEvent).detail));
    (deep(host, '[part="reset-all"]') as HTMLButtonElement).click();
    await settle(host);
    expect(seen).toHaveLength(1);
    expect(new Set((seen[0] as {changes: unknown[]}).changes)).toEqual(
      new Set([
        {mosaicaId: '7', colour: null},
        {mosaicaId: '12', colour: null}
      ])
    );
    expect([...store.get('artifacts').overrides.keys()]).toEqual(['bands']);
    expect([...store.get('artifacts').overrides.get('bands')!.keys()]).toEqual([7n, 99n]);
    expect((deep(host, '[part="reset-all"]') as HTMLButtonElement).disabled).toBe(true);
  });

  it('closes only the colour picker on Escape inside it, giving focus back to the swatch it opened from', async () => {
    const {host, store} = await open('cluster:topics');
    answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: CUT, groups: 3}], 's0', undefined, 'tableau10'));
    await settle(host);
    const swatch = rowsOf(host)[1]!.querySelector('[part="swatch"]') as HTMLButtonElement;
    swatch.click();
    await settle(host);
    const choice = deep(host, '[part="choice"]') as HTMLButtonElement;
    choice.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true, composed: true}));
    await settle(host);
    expect(deep(host, '[part="colour-popover"]')).toBeNull();
    expect((deep(host, '[part="dialog"]') as HTMLDialogElement).open).toBe(true);
    const editor = host.querySelector('mosaica-colour-editor')!;
    expect(editor.shadowRoot!.activeElement).toBe(swatch);
  });

  it('follows a change of field while open: drops what was found and ticked, lists nothing until the new answer, and closes on a field it cannot edit', async () => {
    const {host, editor, store} = await open('archive');
    answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: [{key: 'cs', count: 70}, {key: 'math', count: 30}], groups: 2}]));
    await settle(host);
    const box = rowsOf(host)[0]!.querySelector('[part="check"]') as HTMLInputElement;
    box.checked = true;
    box.dispatchEvent(new Event('change'));
    await settle(host);
    expect(deep(host, '[part="selected"]')).not.toBeNull();
    const seen: unknown[] = [];
    host.addEventListener('mosaica-clustercolour', (e) => seen.push((e as CustomEvent).detail));
    editor.field = 'cluster:topics';
    await settle(host);
    // The category's answer is not the layer's, so nothing is listed until the layer's lands.
    expect(rowsOf(host)).toEqual([]);
    expect(deep(host, '[part="selected"]')).toBeNull();
    expect(specOf(store, 'colours-ranked')).toEqual({groupings: [{count: true, by: {layer: 'topics', top: 1000, cut: 'drawn', paletteSize: 'drawn'}}], subject: 'visible'});
    answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: CUT, groups: 3}], 's0', undefined, 'tableau10'));
    await settle(host);
    expect(names(host).map((n) => n[0])).toEqual(['7', '8', '9']);
    (rowsOf(host)[0]!.querySelector('[part="swatch"]') as HTMLButtonElement).click();
    await settle(host);
    (deepAll(host, '[part="choice"]')[1] as HTMLButtonElement).click();
    await settle(host);
    expect(seen).toEqual([{layer: 'topics', changes: [{mosaicaId: '7', colour: '#f28e2b'}]}]);
    // A field that is neither a category nor a layer: the dialog closes, once.
    editor.field = 'year';
    await settle(host);
    expect((deep(host, '[part="dialog"]') as HTMLDialogElement).open).toBe(false);
    expect(document.activeElement).toBe(host.querySelector('#opener'));
  });

  it('outlines a value the map has not drawn yet, which has no palette colour, and still opens its picker', async () => {
    const {host, store} = await open('archive');
    store.set('legend', {...store.get('legend'), categories: {archive: [{key: 'cs', code: 1, title: null}]}, ranks: {archive: {1: 0}}});
    answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: [{key: 'cs', count: 70}, {key: 'math', count: 30}], groups: 2}]));
    await settle(host);
    const swatches = rowsOf(host).map((r) => r.querySelector('[part="swatch"]') as HTMLButtonElement);
    expect(swatches.map((s) => s.hasAttribute('data-unmet'))).toEqual([false, true]);
    swatches[1]!.click();
    await settle(host);
    expect(deep(host, '[part="colour-popover"]')).not.toBeNull();
  });

  it('closes on Escape and on Done, gives focus back to what opened it, and stops asking', async () => {
    const {host, editor, store} = await open('cluster:topics');
    const dialog = deep(host, '[part="dialog"]') as HTMLDialogElement;
    dialog.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true, composed: true}));
    await settle(host);
    expect(dialog.open).toBe(false);
    expect(document.activeElement).toBe(host.querySelector('#opener'));
    expect(specOf(store, 'colours-ranked')).toBeUndefined();
    await editor.show();
    await settle(host);
    expect(dialog.open).toBe(true);
    (deep(host, '[part="done"]') as HTMLButtonElement).click();
    await settle(host);
    expect(dialog.open).toBe(false);
  });
});

describe('<mosaica-colour-editor> on a levelled layer', () => {
  it('lists the clusters of the level coloured, the top level unless one is set', async () => {
    const top = await open('cluster:bands');
    expect(specOf(top.store, 'colours-ranked')).toEqual({groupings: [{count: true, by: {layer: 'bands', level: 0, top: 1000, paletteSize: 'drawn'}}], subject: 'visible'});
    document.body.innerHTML = '';
    const fine = await open('cluster:bands', 'cluster-level="1"');
    expect(specOf(fine.store, 'colours-ranked')).toEqual({groupings: [{count: true, by: {layer: 'bands', level: 1, top: 1000, paletteSize: 'drawn'}}], subject: 'visible'});
    expect(deep(fine.host, '[part="sub"]')!.textContent).toBe('Fine · Tableau 10');
  });
});

describe('<mosaica-colour-editor> on a category', () => {
  it('lists the values by their counts over everything visible, and gives one the colour chosen in its picker', async () => {
    const {host, store} = await open('archive');
    expect(specOf(store, 'colours-ranked')).toEqual({groupings: [{count: true, by: {field: 'archive', top: 1000}}], subject: 'visible'});
    answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: [{key: 'cs', title: 'Computer science', count: 70}, {key: 'math', count: 30}], groups: 2}]));
    await settle(host);
    expect(names(host)).toEqual([
      ['cs', 'Computer science', '70'],
      ['math', 'math', '30']
    ]);
    const seen: unknown[] = [];
    host.addEventListener('mosaica-valuecolour', (e) => seen.push((e as CustomEvent).detail));
    (rowsOf(host)[1]!.querySelector('[part="swatch"]') as HTMLButtonElement).click();
    await settle(host);
    expect(deep(host, '[part="colour-popover"]')!.getAttribute('aria-label')).toBe('Colour of math');
    const choice = deepAll(host, '[part="choice"]')[3] as HTMLButtonElement;
    const hex = /#[0-9a-f]{6}/.exec(choice.getAttribute('aria-label')!)![0];
    choice.click();
    await settle(host);
    expect(seen).toEqual([{column: 'archive', changes: [{value: 'math', colour: hex}]}]);
    expect(rowsOf(host)[1]!.querySelector('[part="swatch"]')!.getAttribute('style')).toContain(hex);
    expect(deep(host, '[part="changed"]')!.textContent).toBe('1 colour changed');
    (deep(host, '[part="reset-all"]') as HTMLButtonElement).click();
    await settle(host);
    expect(seen.at(-1)).toEqual({column: 'archive', changes: [{value: 'math', colour: null}]});
  });
});

describe('<mosaica-colour-editor> under a keyboard and a screen reader', () => {
  async function listed() {
    const opened = await open('cluster:topics');
    answerAggregate(opened.store, 'colours-ranked', aggregateEntry([{rows: CUT, groups: 3}], 's0', undefined, 'tableau10'));
    await settle(opened.host);
    return opened;
  }

  it('moves focus from Deselect, which goes with its bar, to the first row’s checkbox', async () => {
    const {host, editor} = await listed();
    const box = rowsOf(host)[1]!.querySelector('[part="check"]') as HTMLInputElement;
    box.checked = true;
    box.dispatchEvent(new Event('change'));
    await settle(host);
    const deselect = deep(host, '[part="deselect"]') as HTMLButtonElement;
    deselect.focus();
    deselect.click();
    await settle(host);
    expect(deep(host, '[part="selected"]')).toBeNull();
    expect(editor.shadowRoot!.activeElement).toBe(rowsOf(host)[0]!.querySelector('[part="check"]'));
  });

  it('says what each row’s count counts, and whole names with no hover title standing in for them', async () => {
    const {host} = await listed();
    for (const row of rowsOf(host)) {
      const count = row.querySelector('[part="count"]')!;
      expect(count.parentElement!.textContent!.trim().length).toBeGreaterThan(count.textContent!.length);
      expect(row.querySelector('[part="name"]')!.hasAttribute('title')).toBe(false);
    }
  });

  it('says that the rows are loading, or that there are none, in a status region outside the list that is there before it speaks', async () => {
    const {host, store} = await open('cluster:topics');
    const list = deep(host, '[part="rows"]')!;
    const region = deepAll(host, '[role="status"]').find((r) => r.getAttribute('part') !== 'changed')!;
    expect(list.contains(region)).toBe(false);
    expect(list.children).toHaveLength(0);
    const loading = region.textContent;
    expect(loading).not.toBe('');
    answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: [], groups: 0}], 's0', undefined, 'tableau10'));
    await settle(host);
    expect(region.isConnected).toBe(true);
    expect(region.textContent).not.toBe('');
    expect(region.textContent).not.toBe(loading);
    answerAggregate(store, 'colours-ranked', aggregateEntry([{rows: CUT, groups: 3}], 's0', undefined, 'tableau10'));
    await settle(host);
    expect(region.isConnected).toBe(true);
    expect(region.textContent).toBe('');
    expect(rowsOf(host)).toHaveLength(3);
  });
});

describe('the colour picker', () => {
  async function picking() {
    const opened = await open('cluster:topics');
    answerAggregate(opened.store, 'colours-ranked', aggregateEntry([{rows: CUT, groups: 3}], 's0', undefined, 'tableau10'));
    await settle(opened.host);
    const swatch = rowsOf(opened.host)[1]!.querySelector('[part="swatch"]') as HTMLButtonElement;
    swatch.click();
    await settle(opened.host);
    const seen: {changes: {colour: string | null}[]}[] = [];
    opened.host.addEventListener('mosaica-clustercolour', (e) => seen.push((e as CustomEvent).detail));
    const hex = () => deep(opened.host, '[part="hex"]') as HTMLInputElement;
    const type = async (text: string) => {
      hex().value = text;
      hex().dispatchEvent(new Event('change'));
      await settle(opened.host);
    };
    const key = async (on: Element, init: KeyboardEventInit) => {
      const e = new KeyboardEvent('keydown', {bubbles: true, composed: true, cancelable: true, ...init});
      on.dispatchEvent(e);
      await settle(opened.host);
      return e;
    };
    return {...opened, swatch, seen, hex, type, key, popover: () => deep(opened.host, '[part="colour-popover"]')};
  }

  it('keeps text in the hex field that is not a colour, marks the field invalid and says what to write, where it said the format before', async () => {
    const {host, seen, hex, type} = await picking();
    const format = () => deep(host, `#${hex().getAttribute('aria-describedby')}`)!;
    expect(hex().getAttribute('aria-invalid')).toBe('false');
    const before = format().textContent!;
    expect(before).toContain('#');

    await type('#12345');
    expect(hex().value).toBe('#12345');
    expect(hex().getAttribute('aria-invalid')).toBe('true');
    expect(format().getAttribute('role')).toBe('status');
    expect(format().textContent).not.toBe(before);
    expect(format().textContent).toContain('#');
    expect(seen).toEqual([]);

    await type('#123456');
    expect(hex().getAttribute('aria-invalid')).toBe('false');
    expect(format().textContent).toBe(before);
    expect(seen.map((d) => d.changes[0]!.colour)).toEqual(['#123456']);
  });

  it('forgets the invalid text once a colour is chosen another way', async () => {
    const {host, hex, type} = await picking();
    await type('nope');
    expect(hex().getAttribute('aria-invalid')).toBe('true');
    (deepAll(host, '[part="choice"]')[2] as HTMLButtonElement).click();
    await settle(host);
    expect(hex().getAttribute('aria-invalid')).toBe('false');
    expect(hex().value).toMatch(/^#[0-9A-F]{6}$/);
  });

  it('closes on Tab past its last control and on Shift+Tab past its first, giving focus back to its swatch', async () => {
    for (const shiftKey of [false, true]) {
      const {host, editor, swatch, hex, key, popover} = await picking();
      const first = popover()!.querySelector('button')!;
      // Tab between its controls is the browser's.
      expect((await key(shiftKey ? hex() : first, {key: 'Tab', shiftKey})).defaultPrevented).toBe(false);
      expect(popover()).not.toBeNull();
      const e = await key(shiftKey ? first : hex(), {key: 'Tab', shiftKey});
      expect(e.defaultPrevented).toBe(true);
      expect(popover()).toBeNull();
      expect(editor.shadowRoot!.activeElement).toBe(swatch);
      expect((deep(host, '[part="dialog"]') as HTMLDialogElement).open).toBe(true);
      document.body.innerHTML = '';
    }
  });

  it('takes the hex field’s text on Tab out of it, and stays where the text is not a colour', async () => {
    const {seen, hex, key, popover} = await picking();
    hex().value = 'nope';
    await key(hex(), {key: 'Tab'});
    expect(popover()).not.toBeNull();
    expect(hex().getAttribute('aria-invalid')).toBe('true');
    hex().value = '#abcdef';
    await key(hex(), {key: 'Tab'});
    expect(popover()).toBeNull();
    expect(seen.map((d) => d.changes[0]!.colour)).toEqual(['#abcdef']);
  });

  it('gives each slider its value in words, and names the custom controls as a group', async () => {
    const {host} = await picking();
    const sv = deep(host, '[part="sv"]')!;
    const hue = deep(host, '[part="hue"]')!;
    expect(sv.getAttribute('aria-valuetext')).toMatch(/\d+%.*\d+%/);
    expect(hue.getAttribute('aria-valuetext')).toContain(hue.getAttribute('aria-valuenow')!);
    hue.dispatchEvent(new KeyboardEvent('keydown', {key: 'End', bubbles: true}));
    await settle(host);
    expect(hue.getAttribute('aria-valuetext')).toContain('359');
    const group = sv.closest('[role="group"]')!;
    expect(group.contains(hue)).toBe(true);
    expect(group.contains(deep(host, '[part="hex"]'))).toBe(true);
    expect(deep(host, `#${group.getAttribute('aria-labelledby')}`)!.textContent?.trim()).not.toBe('');
  });

  it('marks the colour in use with a tick as well as its pressed state', async () => {
    const {host} = await picking();
    (deepAll(host, '[part="choice"]')[3] as HTMLButtonElement).click();
    await settle(host);
    const choices = deepAll(host, '[part="choice"]');
    expect(choices.map((c) => c.getAttribute('aria-pressed') === 'true')).toEqual(choices.map((_, i) => i === 3));
    expect(choices.map((c) => c.querySelector('svg') !== null)).toEqual(choices.map((_, i) => i === 3));
  });
});
