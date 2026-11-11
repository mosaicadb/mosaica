import {afterEach, describe, expect, it} from 'vitest';
import type {BrowsePage, BrowseRow, FilterDraft, FiltersProjection, Meta, Rgba} from '@mosaicajs/client';
import '../src/field-card.js';
import type {MosaicaFieldCard} from '../src/field-card.js';
import {aggregateEntry, answerAggregate, deep, deepAll, fakeStore, meta, mount, registered, scalar, settle, status, type FakeStore} from './fake-store.js';
import {UNNAMED} from '../src/base.js';

afterEach(() => {
  document.body.innerHTML = '';
});

const topicsLayer = {
  name: 'topics',
  title: 'Topics',
  views: ['s0'],
  membership: 'enumerated',
  hierarchy: {kind: 'nested', pruneChildren: false},
  levels: [],
  computedContent: ['centroid'],
  shape: null,
  suppliedContent: ['name'],
  depsOn: [],
  colourable: true,
  version: 1
} as unknown as Meta['layers'][number];

const META = meta({
  declaredScalars: [
    scalar('archive', 'u16', {category: {vocabulary: 'a', kind: 'declared', visibility: 'public'}, render: true, homes: ['rendered']}),
    scalar('submitted_at', 'timestamp_us', {render: true, homes: ['rendered']}),
    scalar('authors', 'u16', {render: false}),
    scalar('score', 'f64', {render: false}),
    scalar('retracted', 'bool', {render: true, homes: ['rendered']}),
    scalar('title', 'utf8')
  ],
  filterOperands: [
    {column: 'archive', family: 'category', operands: ['eq', 'in']},
    {column: 'submitted_at', family: 'numeric', operands: ['range']},
    {column: 'authors', family: 'numeric', operands: ['range']},
    {column: 'score', family: 'numeric', operands: ['range']},
    {column: 'retracted', family: 'numeric', operands: ['eq', 'in', 'range']},
    {column: 'title', family: 'text', operands: ['match', 'phrase']}
  ],
  layers: [topicsLayer]
});

const filtersOf = (draft: FilterDraft, over: Partial<FiltersProjection> = {}): FiltersProjection => ({draft, expr: null, highlight: null, members: [], regions: [], suggestions: {}, suggestErrors: {}, suggestEpoch: 0, ...over});

async function mountCard(field: string, draft: FilterDraft = {filter: {}, highlight: {}}, over: Partial<FiltersProjection> = {}, attrs = '') {
  const host = await mount(`<mosaica-field-card field="${field}" ${attrs}></mosaica-field-card>`);
  const card = host.querySelector('mosaica-field-card') as MosaicaFieldCard;
  const store = fakeStore({meta: META, status: status({}), filters: filtersOf(draft, over)});
  store.set('view', {...store.get('view'), id: 's0'});
  card.store = store;
  await settle(host);
  return {host, card, store};
}

/** The spec registered under the id beginning `prefix`. */
const spec = (store: FakeStore, prefix: string) => [...registered(store)].find(([id]) => id.startsWith(prefix))?.[1];
const lastFilters = (store: FakeStore) => store.calls.filter((c) => c.name === 'setFilters').at(-1)?.args[0] as FilterDraft | undefined;
const rows = (host: HTMLElement) => deepAll(host, '[part~="row"]') as HTMLElement[];
/** The control holding focus inside the card. */
const focused = (card: MosaicaFieldCard) => card.shadowRoot!.activeElement as HTMLElement | null;

/** Answer the card's counts in view and over everything matching. */
async function answer(host: HTMLElement, store: FakeStore, subject: Parameters<typeof aggregateEntry>[0], match: Parameters<typeof aggregateEntry>[0]) {
  answerAggregate(store, 'field-subject', aggregateEntry(subject));
  answerAggregate(store, 'field-match', aggregateEntry(match));
  await settle(host);
}

describe('<mosaica-field-card> on a category', () => {
  it('registers its counts in view with the highlight and over everything matching, each leaving out its own clause', async () => {
    const {store} = await mountCard('archive');
    expect(spec(store, 'field-subject')).toEqual({groupings: [{count: true, by: {field: 'archive', top: 5}}], subject: 'view', highlighted: true, without: 'archive'});
    expect(spec(store, 'field-match')).toEqual({groupings: [{count: true, by: {field: 'archive', top: 100}}], without: 'archive'});
    expect(spec(store, 'field-summary')).toBeUndefined();
  });

  it('counts the values its clauses name whether or not they are among the commonest', async () => {
    const {store} = await mountCard('archive', {filter: {archive: {family: 'category', keys: ['math']}}, highlight: {archive: {family: 'category', keys: ['cs']}}});
    expect(spec(store, 'field-subject')).toMatchObject({groupings: [{count: true, by: {field: 'archive', top: 5}}, {count: true, by: {field: 'archive', values: ['cs', 'math']}}]});
    expect(spec(store, 'field-match')).toMatchObject({groupings: [{count: true, by: {field: 'archive', top: 100}}, {count: true, by: {field: 'archive', values: ['cs', 'math']}}]});
  });

  it('names no more values than the server counts by name', async () => {
    const host = await mount('<mosaica-field-card field="archive"></mosaica-field-card>');
    const card = host.querySelector('mosaica-field-card') as MosaicaFieldCard;
    const store = fakeStore({meta: {...META, selection: {...META.selection, maxAggregateNamed: 2}}, status: status({}), filters: filtersOf({filter: {archive: {family: 'category', keys: ['a', 'b', 'c']}}, highlight: {}})});
    card.store = store;
    await settle(host);
    expect(spec(store, 'field-subject')).toMatchObject({groupings: [{count: true, by: {field: 'archive', top: 5}}, {count: true, by: {field: 'archive', values: ['a', 'b']}}]});
  });

  it('draws each value with its two counts and bars, each a share of its own set', async () => {
    const {host, store} = await mountCard('archive');
    await answer(
      host,
      store,
      [{rows: [{key: 'hep', title: 'hep-ph', count: 60}, {key: 'astro', count: 40}], total: 200, groups: 7}],
      [{rows: [{key: 'cs', count: 1000}, {key: 'hep', count: 500}, {key: 'astro', count: 250}], total: 5000}]
    );
    expect(rows(host).map((r) => [r.dataset.key, r.querySelector('[part="name"]')!.textContent, r.querySelector('[part="count"]')!.textContent])).toEqual([
      ['hep', 'hep-ph', '60 / 500'],
      ['astro', 'astro', '40 / 250']
    ]);
    const widths = (r: HTMLElement) => [parseFloat((r.querySelector('[part="bar-subject"]') as HTMLElement).style.width), parseFloat((r.querySelector('[part="bar-match"]') as HTMLElement).style.width)];
    // A plain share of each set, never stretched to the largest row.
    expect(widths(rows(host)[0]!)).toEqual([30, 10]);
    expect(deep(host, '[part="more"]')!.textContent).toBe('5 more in view');
  });

  it('asks for the values in view outside the commonest matching by name, once, and keeps the whole match’s request as it was', async () => {
    const {host, store} = await mountCard('archive');
    // As the real store: a registration made again drops the answer held, and each request is
    // answered from what the view holds.
    let inView = [{key: 'cs', count: 3}, {key: 'rare', count: 7}];
    const answerFor = (id: string, spec: {groupings: {by?: {values?: string[]}}[]}) =>
      id.startsWith('field-subject')
        ? aggregateEntry([{rows: inView, total: 10, groups: inView.length}])
        : id.startsWith('field-match')
          ? aggregateEntry([{rows: [{key: 'cs', count: 1000}], total: 5000}])
          : aggregateEntry([{rows: (spec.groupings[0]!.by!.values ?? []).map((key) => ({key, count: 50}))}]);
    store.setAggregate = (id: string, given: unknown) => {
      store.calls.push({name: 'setAggregate', args: [id, given]});
      const held = new Map(store.get('aggregates'));
      held.delete(id);
      store.set('aggregates', held);
      if (given === null) return;
      queueMicrotask(() => store.set('aggregates', new Map(store.get('aggregates')).set(id, answerFor(id, given as never))));
    };
    const asked = (prefix: string) => store.calls.filter((c) => c.name === 'setAggregate' && (c.args[0] as string).startsWith(prefix) && c.args[1] !== null);
    // The registrations the card made on mounting are answered as the store would.
    for (const [id, given] of registered(store)) store.set('aggregates', new Map(store.get('aggregates')).set(id, answerFor(id, given as never)));
    for (let i = 0; i < 4; i++) await settle(host);
    expect(asked('field-match').map((c) => c.args[1])).toEqual([{groupings: [{count: true, by: {field: 'archive', top: 100}}], without: 'archive'}]);
    expect(asked('field-outside').map((c) => c.args[1])).toEqual([{groupings: [{count: true, by: {field: 'archive', values: ['rare']}}], without: 'archive'}]);
    const rare = rows(host).find((r) => r.dataset.key === 'rare')!;
    expect(rare.querySelector('[part="count"]')!.textContent).toBe('7 / 50');
    expect(parseFloat((rare.querySelector('[part="bar-match"]') as HTMLElement).style.width)).toBe(1);
    // A pan whose view holds the same values outside asks nothing more for them.
    inView = [{key: 'cs', count: 2}, {key: 'rare', count: 5}];
    store.set('aggregates', new Map(store.get('aggregates')).set([...registered(store).keys()].find((id) => id.startsWith('field-subject'))!, answerFor('field-subject', {groupings: []})));
    for (let i = 0; i < 4; i++) await settle(host);
    expect(asked('field-outside')).toHaveLength(1);
    expect(asked('field-match')).toHaveLength(1);
  });

  it('puts a value in or out of the filter and the highlight from its row', async () => {
    const {host, store} = await mountCard('archive');
    await answer(host, store, [{rows: [{key: 'cs', count: 10}], total: 10}], [{rows: [{key: 'cs', count: 20}], total: 20}]);
    const changes: unknown[] = [];
    host.addEventListener('mosaica-filterchange', (e) => changes.push((e as CustomEvent).detail));
    const row = rows(host)[0]!;
    (row.querySelector('[part="filter"]') as HTMLButtonElement).click();
    expect(lastFilters(store)).toEqual({filter: {archive: {family: 'category', keys: ['cs']}}, highlight: {}});
    (row.querySelector('[part="highlight"]') as HTMLButtonElement).click();
    expect(lastFilters(store)).toEqual({filter: {}, highlight: {archive: {family: 'category', keys: ['cs']}}});
    expect(changes).toEqual([
      {column: 'archive', verb: 'filter', expr: {archive: {in: ['cs']}}},
      {column: 'archive', verb: 'highlight', expr: {archive: {in: ['cs']}}}
    ]);
    // Pressed again, a value comes out of the clause.
    store.set('filters', filtersOf({filter: {archive: {family: 'category', keys: ['cs']}}, highlight: {}}));
    await settle(host);
    const filter = rows(host)[0]!.querySelector('[part="filter"]') as HTMLButtonElement;
    expect(filter.getAttribute('aria-pressed')).toBe('true');
    filter.click();
    expect(lastFilters(store)).toEqual({filter: {archive: {family: 'category', keys: []}}, highlight: {}});
  });

  it('filters to a value from a click on its row, and leaves the highlight to its button', async () => {
    const {host, store} = await mountCard('archive');
    await answer(host, store, [{rows: [{key: 'cs', count: 10}], total: 10}], [{rows: [{key: 'cs', count: 20}], total: 20}]);
    const row = rows(host)[0]!;
    (row.querySelector('[part="name"]') as HTMLElement).click();
    expect(lastFilters(store)).toEqual({filter: {archive: {family: 'category', keys: ['cs']}}, highlight: {}});
    store.set('filters', filtersOf({filter: {archive: {family: 'category', keys: ['cs']}}, highlight: {}}));
    await settle(host);
    (rows(host)[0]!.querySelector('[part="name"]') as HTMLElement).click();
    expect(lastFilters(store)).toEqual({filter: {archive: {family: 'category', keys: []}}, highlight: {}});
    store.set('filters', filtersOf({filter: {}, highlight: {}}));
    await settle(host);
    (rows(host)[0]!.querySelector('[part="highlight"]') as HTMLButtonElement).click();
    expect(lastFilters(store)).toEqual({filter: {}, highlight: {archive: {family: 'category', keys: ['cs']}}});
  });

  it('greys the values its filter leaves out, showing only how many they would add, and shares the rest within it', async () => {
    const {host, store} = await mountCard('archive', {filter: {archive: {family: 'category', keys: ['cs']}}, highlight: {}});
    await answer(
      host,
      store,
      [{rows: [{key: 'cs', count: 40}, {key: 'math', count: 30}], total: 70}, {rows: [{key: 'cs', count: 40}]}],
      [{rows: [{key: 'math', count: 900}, {key: 'cs', count: 100}], total: 1000}, {rows: [{key: 'cs', count: 100}]}]
    );
    const [cs, math] = rows(host);
    expect(cs!.dataset.state).toBe('in');
    expect(cs!.querySelector('[part="count"]')!.textContent).toBe('40 / 100');
    expect(parseFloat((cs!.querySelector('[part="bar-subject"]') as HTMLElement).style.width)).toBe(100);
    expect(math!.dataset.state).toBe('out');
    expect(math!.querySelector('[part="count"]')!.textContent).toBe('900');
    expect(parseFloat((math!.querySelector('[part="bar-match"]') as HTMLElement).style.width)).toBe(0);
  });

  it('keeps focus on the value whose button was pressed as its row moves to the top', async () => {
    const {host, card, store} = await mountCard('archive');
    const three = [{key: 'cs', count: 30}, {key: 'math', count: 20}, {key: 'stat', count: 10}];
    await answer(host, store, [{rows: three, total: 60}], [{rows: three, total: 60}]);
    const filter = rows(host)[2]!.querySelector('[part="filter"]') as HTMLButtonElement;
    filter.focus();
    filter.click();
    // The store publishes the clause the press set, and the value its clause names is listed first.
    store.set('filters', filtersOf(lastFilters(store)!));
    await settle(host);
    expect(rows(host).map((r) => r.dataset.key)).toEqual(['stat', 'cs', 'math']);
    expect(focused(card)!.getAttribute('part')).toBe('filter');
    expect(focused(card)!.getAttribute('aria-label')).toContain('stat');
    expect(focused(card)!.closest('[part~="row"]')!.getAttribute('data-key')).toBe('stat');
  });

  it('moves focus to the row that takes its place when the row whose button was pressed goes', async () => {
    const {host, card, store} = await mountCard('archive', {filter: {archive: {family: 'category', keys: ['rare']}}, highlight: {}});
    const inView = [{key: 'cs', count: 30}, {key: 'math', count: 20}];
    const tables = [{rows: inView, total: 50}, {rows: [{key: 'rare', count: 0}]}];
    await answer(host, store, tables, tables);
    expect(rows(host).map((r) => r.dataset.key)).toEqual(['rare', 'cs', 'math']);
    const filter = rows(host)[0]!.querySelector('[part="filter"]') as HTMLButtonElement;
    filter.focus();
    filter.click();
    // Out of the clause, a value with nothing in view is no longer listed.
    store.set('filters', filtersOf(lastFilters(store)!));
    await settle(host);
    expect(rows(host).map((r) => r.dataset.key)).toEqual(['cs', 'math']);
    expect(focused(card)!.getAttribute('part')).toBe('filter');
    expect(focused(card)!.closest('[part~="row"]')!.getAttribute('data-key')).toBe('cs');
  });

  it('moves focus to the search box when the only row goes with the button pressed', async () => {
    const {host, card, store} = await mountCard('archive', {filter: {archive: {family: 'category', keys: ['rare']}}, highlight: {}});
    const tables = [{rows: [], total: 0}, {rows: [{key: 'rare', count: 0}]}];
    await answer(host, store, tables, tables);
    expect(rows(host).map((r) => r.dataset.key)).toEqual(['rare']);
    const filter = rows(host)[0]!.querySelector('[part="filter"]') as HTMLButtonElement;
    filter.focus();
    filter.click();
    store.set('filters', filtersOf(lastFilters(store)!));
    await settle(host);
    expect(rows(host)).toHaveLength(0);
    expect(focused(card)!.localName).toBe('mosaica-filter');
    expect(focused(card)!.shadowRoot!.activeElement!.getAttribute('part')).toBe('entry');
  });

  it('gives focus back to a value’s swatch when Tab leaves the colour picker it opened', async () => {
    const {host, card, store} = await mountCard('archive');
    store.set('legend', {...store.get('legend'), colourBy: 'archive', categories: {archive: [{key: 'cs', code: 1, title: null}, {key: 'math', code: 2, title: null}]}, ranks: {archive: {1: 0, 2: 1}}});
    const two = [{key: 'cs', count: 10}, {key: 'math', count: 5}];
    await answer(host, store, [{rows: two, total: 15}], [{rows: two, total: 15}]);
    const swatch = rows(host)[1]!.querySelector('[part="swatch"]') as HTMLButtonElement;
    swatch.focus();
    swatch.click();
    await settle(host);
    const picker = deep(host, '[part="colour-popover"]')!;
    expect(picker.contains(focused(card))).toBe(true);
    // Tab from the picker's last control leaves it.
    const hex = deep(host, '[part="hex"]') as HTMLInputElement;
    hex.focus();
    hex.dispatchEvent(new KeyboardEvent('keydown', {key: 'Tab', bubbles: true, composed: true, cancelable: true}));
    await settle(host);
    expect(deep(host, '[part="colour-popover"]')).toBeNull();
    expect(focused(card)).toBe(swatch);
    expect(focused(card)!.closest('[part~="row"]')!.getAttribute('data-key')).toBe('math');
  });

  it('gives a screen reader each count with what it counts, the subject’s by what the subject is, beside the figures drawn', async () => {
    const {host, store} = await mountCard('archive');
    await answer(host, store, [{rows: [{key: 'cs', count: 60}], total: 200}], [{rows: [{key: 'cs', count: 500}], total: 5000}]);
    const count = () => rows(host)[0]!.querySelector('[part="count"]')!;
    /** What follows each figure in the words beside the count. */
    const said = () => /^60 (.+), 500 (.+)$/.exec(count().nextElementSibling!.textContent!)!.slice(1);
    expect(count().getAttribute('aria-hidden')).toBe('true');
    expect(count().nextElementSibling!.classList.contains('sr-only')).toBe(true);
    const inView = said();
    expect(inView[0]).not.toBe(inView[1]);
    // With a highlight set, the first count is of the highlighted items.
    store.set('filters', filtersOf({filter: {}, highlight: {title: {family: 'text', query: 'lattice', phrase: true}}}));
    await settle(host);
    expect(said()[0]).not.toBe(inView[0]);
    expect(said()[1]).toBe(inView[1]);
  });

  it('shows a value both filtered to and highlighted with both its buttons pressed', async () => {
    const both = {family: 'category' as const, keys: ['cs']};
    const {host, store} = await mountCard('archive', {filter: {archive: both}, highlight: {archive: both}});
    const tables = [{rows: [{key: 'cs', count: 10}, {key: 'math', count: 5}], total: 15}, {rows: [{key: 'cs', count: 10}]}];
    await answer(host, store, tables, tables);
    const pressed = (r: HTMLElement) => ['highlight', 'filter'].map((part) => r.querySelector(`[part="${part}"]`)!.getAttribute('aria-pressed'));
    expect(pressed(rows(host)[0]!)).toEqual(['true', 'true']);
    expect(pressed(rows(host)[1]!)).toEqual(['false', 'false']);
    // At rest each is told by a shape: a tick where the swatch would stand, and a pen before the counts.
    const cues = (r: HTMLElement) => ({tick: r.querySelector('.mark svg') !== null, inCount: [...r.querySelectorAll('[part="count"] .cue')].map((c) => (c.classList.contains('lit') ? 'pen' : 'tick'))});
    expect(cues(rows(host)[0]!)).toEqual({tick: true, inCount: ['pen']});
    expect(cues(rows(host)[1]!)).toEqual({tick: false, inCount: []});
    // The map coloured by the field, the swatch stands where the tick would, and the tick goes before the counts.
    store.set('legend', {...store.get('legend'), colourBy: 'archive', categories: {archive: [{key: 'cs', code: 1, title: null}]}, ranks: {archive: {1: 0}}});
    await settle(host);
    expect(rows(host)[0]!.querySelector('[part="swatch"]')).not.toBeNull();
    expect(cues(rows(host)[0]!)).toEqual({tick: false, inCount: ['tick', 'pen']});
    // The buttons, drawn or not, are in the row for Tab to reach, and a cut name has its whole text as its title.
    expect(rows(host)[0]!.querySelectorAll('[part="verbs"] button')).toHaveLength(2);
    expect(rows(host)[0]!.querySelector('[part="name"]')!.getAttribute('title')).toBe('cs');
  });

  it('is a group named by its heading, which holds the field’s name alone, and names its list and search box by the field', async () => {
    const {host, store} = await mountCard('archive');
    await answer(host, store, [{rows: [{key: 'cs', count: 10}], total: 10}], [{rows: [{key: 'cs', count: 20}], total: 20, groups: 3}]);
    const title = deep(host, '[part="title"]')!;
    expect(title.tagName).toBe('H3');
    expect(title.textContent).toBe('Archive');
    expect(title.children).toHaveLength(0);
    expect(deep(host, '[role="group"][aria-labelledby]')!.getAttribute('aria-labelledby')).toBe(title.id);
    expect(deep(host, '[part="rows"]')!.getAttribute('aria-label')).toContain('Archive');
    const entry = deep(host, 'mosaica-filter')!.shadowRoot!.querySelector('[part="entry"]')!;
    expect(entry.getAttribute('placeholder')).toContain('3');
    expect(entry.getAttribute('aria-label')).toContain('Archive');
    expect(entry.getAttribute('aria-label')).toContain(entry.getAttribute('placeholder'));
  });

  it('colours the map by the field from its paint button, pressed while it does, and by nothing when pressed again', async () => {
    const {host, store} = await mountCard('archive');
    const colours: unknown[] = [];
    host.addEventListener('mosaica-colourchange', (e) => colours.push((e as CustomEvent).detail));
    const paint = () => deep(host, '[part="paint"]') as HTMLButtonElement;
    expect(paint().getAttribute('aria-pressed')).toBe('false');
    paint().click();
    expect(store.calls.filter((c) => c.name === 'setColourBy').at(-1)!.args).toEqual(['archive']);
    store.set('legend', {...store.get('legend'), colourBy: 'archive', categories: {archive: [{key: 'cs', code: 1, title: null}]}, ranks: {archive: {1: 0}}});
    await answer(host, store, [{rows: [{key: 'cs', count: 10}], total: 10}], [{rows: [{key: 'cs', count: 10}], total: 10}]);
    expect(paint().getAttribute('aria-pressed')).toBe('true');
    // The colouring card shows each value's colour, which opens the colour picker.
    const swatch = rows(host)[0]!.querySelector('button[part="swatch"]') as HTMLButtonElement;
    expect(swatch.getAttribute('style')).toContain('--c:');
    // Its solid bar is drawn in the value's own colour.
    const colour = /--c:([^;]+)/.exec(swatch.getAttribute('style')!)![1]!;
    expect((rows(host)[0]!.querySelector('[part="bar-subject"]') as HTMLElement).getAttribute('style')).toContain(`background:${colour}`);
    swatch.click();
    await settle(host);
    expect(deep(host, '[part="colour-popover"]')).not.toBeNull();
    paint().click();
    expect(store.calls.filter((c) => c.name === 'setColourBy').at(-1)!.args).toEqual([null]);
    expect(colours).toEqual([{colourBy: 'archive'}, {colourBy: null}]);
  });

  it('gives a value the colour chosen in its picker, drawn at once and reported as one change', async () => {
    const {host, store} = await mountCard('archive');
    const chosen: unknown[] = [];
    host.addEventListener('mosaica-valuecolour', (e) => chosen.push((e as CustomEvent).detail));
    store.set('legend', {...store.get('legend'), colourBy: 'archive', categories: {archive: [{key: 'cs', code: 1, title: null}]}, ranks: {archive: {1: 0}}});
    await answer(host, store, [{rows: [{key: 'cs', count: 10}], total: 10}], [{rows: [{key: 'cs', count: 10}], total: 10}]);
    (rows(host)[0]!.querySelector('button[part="swatch"]') as HTMLButtonElement).click();
    await settle(host);
    const choices = deepAll(host, '[part="choice"]') as HTMLButtonElement[];
    choices[2]!.click();
    await settle(host);
    const picked = /#[0-9a-f]{6}/.exec(choices[2]!.getAttribute('aria-label')!)![0];
    expect(rows(host)[0]!.querySelector('[part="swatch"]')!.getAttribute('style')).toContain(picked);
    (deep(host, '[part="reset"]') as HTMLButtonElement).click();
    await settle(host);
    expect(rows(host)[0]!.querySelector('[part="swatch"]')!.getAttribute('style')).not.toContain(picked);
    expect(chosen).toEqual([
      {column: 'archive', changes: [{value: 'cs', colour: picked}]},
      {column: 'archive', changes: [{value: 'cs', colour: null}]}
    ]);
  });

  it('shows a custom colour as it is dragged, and puts back the colour from before when the picker closes before the drag ends', async () => {
    const {host, store} = await mountCard('archive');
    const chosen: unknown[] = [];
    host.addEventListener('mosaica-valuecolour', (e) => chosen.push((e as CustomEvent).detail));
    store.set('legend', {...store.get('legend'), colourBy: 'archive', categories: {archive: [{key: 'cs', code: 1, title: null}]}, ranks: {archive: {1: 0}}});
    await answer(host, store, [{rows: [{key: 'cs', count: 10}], total: 10}], [{rows: [{key: 'cs', count: 10}], total: 10}]);
    const swatchStyle = () => rows(host)[0]!.querySelector('[part="swatch"]')!.getAttribute('style');
    const before = swatchStyle();
    (rows(host)[0]!.querySelector('button[part="swatch"]') as HTMLButtonElement).click();
    await settle(host);
    const sv = deep(host, '[part="sv"]') as HTMLElement;
    sv.getBoundingClientRect = () => ({left: 0, top: 0, right: 100, bottom: 100, width: 100, height: 100, x: 0, y: 0, toJSON: () => ({})});
    sv.dispatchEvent(new PointerEvent('pointerdown', {clientX: 90, clientY: 10, bubbles: true}));
    await new Promise((r) => setTimeout(r, 50));
    await settle(host);
    expect(swatchStyle()).not.toBe(before);
    sv.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true, composed: true}));
    await settle(host);
    expect(deep(host, '[part="colour-popover"]')).toBeNull();
    expect(swatchStyle()).toBe(before);
    expect(chosen).toEqual([]);
  });

  it('offers no paint button on a field the map cannot be coloured by', async () => {
    const {host} = await mountCard('authors');
    expect(deep(host, '[part="paint"]')).toBeNull();
  });

  it('folds to its heading with a small chart, asking for its counts in view alone', async () => {
    const {host, card, store} = await mountCard('archive', {filter: {}, highlight: {}}, {}, 'folded');
    expect(spec(store, 'field-subject')).toBeDefined();
    expect(spec(store, 'field-match')).toBeUndefined();
    answerAggregate(store, 'field-subject', aggregateEntry([{rows: [{key: 'cs', count: 30}, {key: 'math', count: 10}], total: 40}]));
    await settle(host);
    expect(rows(host)).toHaveLength(0);
    expect(deepAll(host, '[part="spark"] span').map((b) => (b as HTMLElement).style.height)).toEqual(['14px', '5px']);
    const toggled: unknown[] = [];
    card.addEventListener('mosaica-fold', (e) => toggled.push((e as CustomEvent).detail));
    (deep(host, '[part="fold"]') as HTMLButtonElement).click();
    await settle(host);
    expect(toggled).toEqual([{field: 'archive', folded: false}]);
    expect(spec(store, 'field-match')).toBeDefined();
  });
});

describe('<mosaica-field-card> on a date', () => {
  const YEAR = (y: number) => Date.UTC(y, 0, 1);
  const bins = (counts: number[]) => counts.map((count, i) => ({lower: YEAR(2019 + i), upper: YEAR(2020 + i), count}));

  it('registers a sampled histogram in view and over everything matching, and the field’s figures over what the viewer may see', async () => {
    const {store} = await mountCard('submitted_at');
    const histogram = {count: true, by: {field: 'submitted_at', bins: 20, sample: 100_000}};
    expect(spec(store, 'field-subject')).toEqual({groupings: [histogram], subject: 'view', highlighted: true, without: 'submitted_at'});
    expect(spec(store, 'field-match')).toEqual({groupings: [histogram], without: 'submitted_at'});
    expect(spec(store, 'field-summary')).toEqual({groupings: [{summary: ['submitted_at']}], subject: 'visible'});
  });

  it('turns a range dragged across the bins into the field’s range clause, in the position chosen', async () => {
    const {host, store} = await mountCard('submitted_at');
    await answer(host, store, [{rows: bins([1, 2, 3, 4]), total: 10}], [{rows: bins([10, 20, 30, 40]), total: 100}]);
    const plot = deep(host, '[part="plot"]') as HTMLElement;
    plot.getBoundingClientRect = () => ({left: 0, top: 0, width: 400, height: 56, right: 400, bottom: 56, x: 0, y: 0, toJSON: () => ({})}) as DOMRect;
    const pointer = (type: string, x: number) => plot.dispatchEvent(new PointerEvent(type, {clientX: x, button: 0, pointerId: 1, bubbles: true}));
    pointer('pointerdown', 150);
    pointer('pointermove', 250);
    await settle(host);
    expect(deep(host, '[part~="band"][data-verb="brush"]')).not.toBeNull();
    pointer('pointerup', 250);
    await settle(host);
    const box = deep(host, '[part="brush"]')!;
    expect(box.textContent!.replace(/\s+/g, ' ').trim()).toContain('2020 – 2021 · 50 items');
    (deep(host, '[part="brush-filter"]') as HTMLButtonElement).click();
    // The whole years, in microseconds: from the first instant of 2020 to the last of 2021.
    expect(lastFilters(store)).toEqual({filter: {submitted_at: {family: 'numeric', gte: YEAR(2020) * 1000, lte: YEAR(2022) * 1000 - 1}}, highlight: {}});
    // A range reaching the last bin leaves its upper end open, since the bins span every value.
    store.set('filters', filtersOf({filter: {}, highlight: {}}));
    pointer('pointerdown', 350);
    pointer('pointerup', 399);
    await settle(host);
    expect(deep(host, '[part="brush"]')!.textContent).toContain('2022');
    (deep(host, '[part="brush-filter"]') as HTMLButtonElement).click();
    expect(lastFilters(store)).toEqual({filter: {submitted_at: {family: 'numeric', gte: YEAR(2022) * 1000, lte: null}}, highlight: {}});
  });

  it('chooses a range with the arrow keys and Enter, and highlights it', async () => {
    const {host, store} = await mountCard('submitted_at');
    await answer(host, store, [{rows: bins([1, 2, 3, 4]), total: 10}], [{rows: bins([10, 20, 30, 40]), total: 100}]);
    const plot = deep(host, '[part="plot"]') as HTMLElement;
    const key = (k: string, shiftKey = false) => plot.dispatchEvent(new KeyboardEvent('keydown', {key: k, shiftKey, bubbles: true}));
    key('ArrowRight');
    key('ArrowRight');
    key('ArrowRight', true);
    key('Enter');
    await settle(host);
    (deep(host, '[part="brush-highlight"]') as HTMLButtonElement).click();
    // The first press takes the first bin, the next moves on, and Shift widens: 2020 and 2021.
    expect(lastFilters(store)).toEqual({filter: {}, highlight: {submitted_at: {family: 'numeric', gte: YEAR(2020) * 1000, lte: YEAR(2022) * 1000 - 1}}});
  });

  it('outlines a range set on the field, and marks counts taken from a sample as estimates', async () => {
    const {host, store} = await mountCard('submitted_at', {filter: {}, highlight: {submitted_at: {family: 'numeric', gte: YEAR(2020) * 1000, lte: null}}});
    await answer(host, store, [{rows: bins([1, 2]), total: 3}], [{rows: bins([1000, 2000]), total: 3000, sample: {sampled: true, items: 100}}]);
    const band = deep(host, '[part~="band"][data-verb="highlight"]') as HTMLElement;
    // From the first instant of 2020, half way along 2019 to 2021, to the open end.
    expect(band.style.left).toMatch(/^calc\(49\.9\d% - 1px\)$/);
    expect(band.style.width).toMatch(/^calc\(50\.0\d% \+ 2px\)$/);
    expect((deep(host, '[part~="bin"]') as HTMLElement).title).toContain('≈1,000 matching');
    const plot = deep(host, '[part="plot"]') as HTMLElement;
    plot.dispatchEvent(new KeyboardEvent('keydown', {key: 'ArrowRight', bubbles: true}));
    plot.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', bubbles: true}));
    await settle(host);
    expect(deep(host, '[part="brush"]')!.textContent).toContain('≈1,000 items');
    // A press anywhere else closes the box.
    document.body.dispatchEvent(new PointerEvent('pointerdown', {bubbles: true, composed: true}));
    await settle(host);
    expect(deep(host, '[part="brush"]')).toBeNull();
  });

  it('says the range the arrow keys move and its count, for a reader, while it is chosen', async () => {
    const {host, store} = await mountCard('submitted_at');
    await answer(host, store, [{rows: bins([1, 2, 3, 4]), total: 10}], [{rows: bins([10, 20, 30, 40]), total: 100}]);
    const plot = deep(host, '[part="plot"]') as HTMLElement;
    const live = () => deep(host, '[role="status"][aria-live="polite"]')!.textContent!.trim();
    expect(live()).toBe('');
    plot.dispatchEvent(new KeyboardEvent('keydown', {key: 'ArrowRight', bubbles: true}));
    plot.dispatchEvent(new KeyboardEvent('keydown', {key: 'ArrowRight', shiftKey: true, bubbles: true}));
    await settle(host);
    expect(live()).toBe('2019 – 2020, 30 items');
  });

  /** Type `text` into a field of the range box and leave it. */
  const typeIn = async (host: HTMLElement, input: HTMLInputElement, text: string) => {
    input.value = text;
    input.dispatchEvent(new Event('input'));
    input.dispatchEvent(new Event('change'));
    await settle(host);
  };
  /** What a field's `aria-describedby` points at says. */
  const described = (card: MosaicaFieldCard, input: HTMLInputElement) =>
    (input.getAttribute('aria-describedby') ?? '')
      .split(' ')
      .filter(Boolean)
      .map((id) => card.shadowRoot!.getElementById(id)!);
  const ends = (host: HTMLElement) => [deep(host, '[part="brush-from"]') as HTMLInputElement, deep(host, '[part="brush-to"]') as HTMLInputElement] as const;

  it('opens the range box from Set range with nothing dragged, and applies the dates typed in its two fields', async () => {
    const {host, card, store} = await mountCard('submitted_at');
    await answer(host, store, [{rows: bins([1, 2, 3, 4]), total: 10}], [{rows: bins([10, 20, 30, 40]), total: 100}]);
    const open = deep(host, '[part="range"]') as HTMLButtonElement;
    expect(open.getAttribute('aria-expanded')).toBe('false');
    // A real button, off screen until the keyboard's focus is on it.
    expect(open.tagName).toBe('BUTTON');
    expect(open.classList.contains('sr-only')).toBe(true);
    expect(open.getAttribute('aria-label')).toContain(open.textContent!.trim());
    expect(open.getAttribute('aria-label')).toContain('Submitted at');
    open.click();
    await settle(host);
    expect(open.getAttribute('aria-expanded')).toBe('true');
    const [from, to] = ends(host);
    expect(focused(card)).toBe(from);
    expect([from.value, to.value]).toEqual(['', '']);
    for (const input of [from, to]) expect(input.getAttribute('aria-label')).toContain('Submitted at');
    expect(from.getAttribute('aria-label')).not.toBe(to.getAttribute('aria-label'));
    // A day, and a month read to its last instant: neither is a bin's edge.
    await typeIn(host, from, '15 Mar 2020');
    await typeIn(host, to, 'June 2021');
    expect(deep(host, '[part~="band"][data-verb="brush"]')).not.toBeNull();
    (deep(host, '[part="brush-filter"]') as HTMLButtonElement).click();
    expect(lastFilters(store)).toEqual({filter: {submitted_at: {family: 'numeric', gte: Date.UTC(2020, 2, 15) * 1000, lte: Date.UTC(2021, 6, 1) * 1000 - 1}}, highlight: {}});
    await settle(host);
    expect(deep(host, '[part="brush"]')).toBeNull();
    expect(focused(card)).toBe(open);
  });

  it('shows a dragged range’s ends in the fields, an open end empty, and applies an end that is typed over', async () => {
    const {host, store} = await mountCard('submitted_at');
    await answer(host, store, [{rows: bins([1, 2, 3, 4]), total: 10}], [{rows: bins([10, 20, 30, 40]), total: 100}]);
    const plot = deep(host, '[part="plot"]') as HTMLElement;
    const key = (k: string, shiftKey = false) => plot.dispatchEvent(new KeyboardEvent('keydown', {key: k, shiftKey, bubbles: true}));
    key('ArrowRight');
    key('ArrowRight');
    key('ArrowRight', true);
    key('ArrowRight', true);
    key('Enter');
    await settle(host);
    const [from, to] = ends(host);
    // 2020 to the last bin, whose upper end is left open.
    expect(from.value).toContain('2020');
    expect(to.value).toBe('');
    await typeIn(host, to, '2020');
    (deep(host, '[part="brush-highlight"]') as HTMLButtonElement).click();
    expect(lastFilters(store)).toEqual({filter: {}, highlight: {submitted_at: {family: 'numeric', gte: YEAR(2020) * 1000, lte: YEAR(2021) * 1000 - 1}}});
  });

  it('does not apply a field that holds no date, or a from after the to: it keeps what was typed and says what to type', async () => {
    const {host, card, store} = await mountCard('submitted_at');
    await answer(host, store, [{rows: bins([1, 2, 3, 4]), total: 10}], [{rows: bins([10, 20, 30, 40]), total: 100}]);
    (deep(host, '[part="range"]') as HTMLButtonElement).click();
    await settle(host);
    const [from, to] = ends(host);
    const filter = deep(host, '[part="brush-filter"]') as HTMLButtonElement;
    // The words about a field are in the page, in a status region, before there is anything to say.
    expect(deep(host, '[part="brush"] [role="status"]')!.textContent!.trim()).toBe('');
    await typeIn(host, from, 'soon');
    expect(from.getAttribute('aria-invalid')).toBe('true');
    expect(to.hasAttribute('aria-invalid')).toBe(false);
    expect(described(card, from)).toHaveLength(1);
    expect(described(card, from)[0]!.textContent).toContain('soon');
    expect(described(card, from)[0]!.closest('[role="status"]')).not.toBeNull();
    to.focus();
    filter.click();
    await settle(host);
    expect(lastFilters(store)).toBeUndefined();
    expect(from.value).toBe('soon');
    expect(focused(card)).toBe(from);
    // Two dates the wrong way round are no range.
    await typeIn(host, from, '2022');
    await typeIn(host, to, '2020');
    expect([from, to].map((input) => input.getAttribute('aria-invalid'))).toEqual(['true', 'true']);
    expect(described(card, to).map((d) => d.textContent!.trim()).join('')).not.toBe('');
    filter.click();
    await settle(host);
    expect(lastFilters(store)).toBeUndefined();
    expect([from.value, to.value]).toEqual(['2022', '2020']);
    await typeIn(host, to, '2023');
    expect([from, to].map((input) => input.hasAttribute('aria-invalid'))).toEqual([false, false]);
    filter.click();
    expect(lastFilters(store)).toEqual({filter: {submitted_at: {family: 'numeric', gte: YEAR(2022) * 1000, lte: YEAR(2024) * 1000 - 1}}, highlight: {}});
  });

  it('opens the range box on Enter with no range chosen, on the range the field holds, and gives focus back to the plot on Escape', async () => {
    const {host, card, store} = await mountCard('submitted_at', {filter: {submitted_at: {family: 'numeric', gte: YEAR(2020) * 1000, lte: null}}, highlight: {}});
    await answer(host, store, [{rows: bins([1, 2, 3, 4]), total: 10}], [{rows: bins([10, 20, 30, 40]), total: 100}]);
    const plot = deep(host, '[part="plot"]') as HTMLElement;
    expect(plot.getAttribute('aria-label')).toContain('Submitted at');
    expect(plot.getAttribute('aria-label')).not.toMatch(/drag/i);
    plot.focus();
    plot.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', bubbles: true}));
    await settle(host);
    const [from, to] = ends(host);
    expect(from.value).toContain('2020');
    expect(to.value).toBe('');
    expect(focused(card)).toBe(from);
    from.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));
    await settle(host);
    expect(deep(host, '[part="brush"]')).toBeNull();
    expect(focused(card)).toBe(plot);
  });

  it('closes the range box as focus goes on past it, since it lies over what is under the plot, and not as focus moves inside it', async () => {
    const {host, store} = await mountCard('submitted_at');
    await answer(host, store, [{rows: bins([1, 2, 3, 4]), total: 10}], [{rows: bins([10, 20, 30, 40]), total: 100}]);
    (deep(host, '[part="range"]') as HTMLButtonElement).click();
    await settle(host);
    const [from, to] = ends(host);
    const elsewhere = document.body.appendChild(document.createElement('button'));
    to.focus();
    await settle(host);
    expect(deep(host, '[part="brush"]')).not.toBeNull();
    // Focus that goes to nothing, as when the window loses it, leaves the box open.
    from.dispatchEvent(new FocusEvent('focusout', {relatedTarget: null, bubbles: true, composed: true}));
    await settle(host);
    expect(deep(host, '[part="brush"]')).not.toBeNull();
    elsewhere.focus();
    await settle(host);
    expect(deep(host, '[part="brush"]')).toBeNull();
  });

  it('gives the bins’ counts in a table, the subject’s column headed by what the subject is', async () => {
    const {host, store} = await mountCard('submitted_at');
    await answer(host, store, [{rows: bins([1, 2, 3, 4]), total: 10}], [{rows: bins([10, 20, 30, 40]), total: 100}]);
    const table = deep(host, 'table')!;
    const heads = () => [...table.querySelectorAll('thead th')].map((h) => h.textContent!.trim());
    expect(table.querySelector('caption')!.textContent).toContain('Submitted at');
    expect(heads()).toHaveLength(3);
    const body = [...table.querySelectorAll('tbody tr')].map((r) => [...r.children].map((c) => c.textContent!.trim()));
    expect(body.map((r) => r.slice(1))).toEqual([['1', '10'], ['2', '20'], ['3', '30'], ['4', '40']]);
    expect(body.map((r) => r[0])).toEqual([2019, 2020, 2021, 2022].map((y) => expect.stringContaining(String(y))));
    // The bars themselves are not read twice.
    expect(deepAll(host, '[part~="bin"]').every((b) => b.getAttribute('aria-hidden') === 'true')).toBe(true);
    const inView = heads()[1];
    store.set('filters', filtersOf({filter: {}, highlight: {title: {family: 'text', query: 'lattice', phrase: true}}}));
    await settle(host);
    expect(heads()[1]).not.toBe(inView);
    expect((deep(host, '[part~="bin"]') as HTMLElement).title).not.toContain(inView!.toLowerCase());
  });

  it('gives a screen reader, off screen, the range each position holds, with its verb', async () => {
    const {host, store} = await mountCard('submitted_at', {
      filter: {submitted_at: {family: 'numeric', gte: YEAR(2020) * 1000, lte: YEAR(2022) * 1000 - 1}},
      highlight: {submitted_at: {family: 'numeric', gte: YEAR(2022) * 1000, lte: null}}
    });
    await answer(host, store, [{rows: bins([1, 2, 3, 4]), total: 10}], [{rows: bins([10, 20, 30, 40]), total: 100}]);
    const said = (verb: string) => deep(host, `.applied[data-verb="${verb}"]`)!.textContent!;
    expect(deepAll(host, '.applied').every((words) => words.classList.contains('sr-only'))).toBe(true);
    expect(said('filter')).toMatch(/filter/i);
    expect(said('filter')).toContain('2020 – 2021');
    expect(said('highlight')).toMatch(/highlight/i);
    expect(said('highlight')).toContain('2022');
    store.set('filters', filtersOf({filter: {}, highlight: {}}));
    await settle(host);
    expect(deepAll(host, '.applied')).toHaveLength(0);
  });

  it('gives a screen reader the field’s figures in its heading, and draws none', async () => {
    const {host, store} = await mountCard('submitted_at');
    answerAggregate(store, 'field-summary', aggregateEntry([{rows: []}], 's0', [new Map([['submitted_at', {values: 25n, missing: 5n, min: YEAR(1990), max: YEAR(2020), mean: YEAR(2011)}]])]));
    await settle(host);
    const sub = deep(host, '[part="sub"]')!;
    expect(sub.textContent).toContain('25');
    expect(sub.textContent).toContain('1 Jan 2011');
    expect(Array.from(sub.children).every((el) => el.classList.contains('sr-only'))).toBe(true);
    expect(sub.hasAttribute('title')).toBe(false);
  });
});

describe('<mosaica-field-card> on a number', () => {
  const tens = (counts: number[]) => counts.map((count, i) => ({lower: i * 10, upper: (i + 1) * 10, count}));

  it('keeps everything matching on a scale of its own as the counts in view move, drawing those in view as shares on it', async () => {
    const {host, store} = await mountCard('authors');
    await answer(host, store, [{rows: tens([10, 20, 70]), total: 100}], [{rows: tens([50, 100, 200]), total: 350}]);
    const heights = (part: string) => deepAll(host, `[part~="bin"] [part="${part}"]`).map((el) => parseFloat((el as HTMLElement).style.height));
    const full = heights('bar-match');
    // The tallest share of the match fills the plot, and the others stand in proportion to it.
    expect(full[2]).toBeCloseTo(56);
    expect(full[1]! / full[2]!).toBeCloseTo(0.5);
    // A view holding few items still draws their shape: shares, not counts.
    answerAggregate(store, 'field-subject', aggregateEntry([{rows: tens([1, 1, 2]), total: 4}]));
    await settle(host);
    expect(heights('bar-match')).toEqual(full);
    expect(heights('bar-subject')[2]).toBeGreaterThan(full[2]! * 0.8);
    expect(heights('bar-subject')[0]! / heights('bar-subject')[2]!).toBeCloseTo(0.5);
    // Everything in view in one bin: its bar is cut at the plot's top, and the columns stay put.
    answerAggregate(store, 'field-subject', aggregateEntry([{rows: tens([50, 0, 0]), total: 50}]));
    await settle(host);
    expect(heights('bar-match')).toEqual(full);
    expect(heights('bar-subject')).toEqual([56, 0, 0]);
  });

  it('writes a value of four figures, such as a year, without a thousands mark, and a longer one with them', async () => {
    const {host, store} = await mountCard('authors');
    const edges = [2010, 2012, 2014, 48_212];
    const rows = edges.slice(0, -1).map((lower, i) => ({lower, upper: edges[i + 1]!, count: 1}));
    await answer(host, store, [{rows, total: 3}], [{rows, total: 3}]);
    const axis = deep(host, '[part="axis"]')!.textContent!;
    expect(axis).toContain('2010');
    expect(axis).not.toContain('2,010');
    expect(axis).toContain('48,212');
  });

  it('takes typed numbers for a range, between the bins’ edges too, and no text that is not a number', async () => {
    const {host, card, store} = await mountCard('authors');
    await answer(host, store, [{rows: tens([5, 7, 9]), total: 21}], [{rows: tens([50, 70, 90]), total: 210}]);
    (deep(host, '[part="range"]') as HTMLButtonElement).click();
    await settle(host);
    const from = deep(host, '[part="brush-from"]') as HTMLInputElement;
    const to = deep(host, '[part="brush-to"]') as HTMLInputElement;
    const type = async (input: HTMLInputElement, text: string) => {
      input.value = text;
      input.dispatchEvent(new Event('input'));
      input.dispatchEvent(new Event('change'));
      await settle(host);
    };
    await type(from, 'a few');
    expect(from.getAttribute('aria-invalid')).toBe('true');
    expect(card.shadowRoot!.getElementById(from.getAttribute('aria-describedby')!)!.textContent).toContain('a few');
    await type(from, '3');
    await type(to, '1,250');
    (deep(host, '[part="brush-filter"]') as HTMLButtonElement).click();
    expect(lastFilters(store)).toEqual({filter: {authors: {family: 'numeric', gte: 3, lte: 1250}}, highlight: {}});
  });

  it('writes a number in a field in full, so that what the field shows types back to the same number', async () => {
    for (const [field, held] of [['authors', 1_234_567], ['score', 0.000123]] as const) {
      document.body.innerHTML = '';
      const {host, store} = await mountCard(field, {filter: {[field]: {family: 'numeric', gte: held, lte: null}}, highlight: {}});
      await answer(host, store, [{rows: tens([5, 7, 9]), total: 21}], [{rows: tens([50, 70, 90]), total: 210}]);
      (deep(host, '[part="range"]') as HTMLButtonElement).click();
      await settle(host);
      const from = deep(host, '[part="brush-from"]') as HTMLInputElement;
      expect(Number(from.value)).toBe(held);
      // Typed over with what it showed, the end is the number it was.
      const shown = from.value;
      from.value = '';
      from.dispatchEvent(new Event('input'));
      from.value = shown;
      from.dispatchEvent(new Event('input'));
      from.dispatchEvent(new Event('change'));
      await settle(host);
      expect(from.hasAttribute('aria-invalid')).toBe(false);
      (deep(host, '[part="brush-highlight"]') as HTMLButtonElement).click();
      expect(lastFilters(store)!.highlight[field]).toEqual({family: 'numeric', gte: held, lte: null});
    }
  });

  it('takes a comma or a space in a number only where it groups thousands, and says so of one that does not', async () => {
    const {host, card, store} = await mountCard('score');
    await answer(host, store, [{rows: tens([5, 7, 9]), total: 21}], [{rows: tens([50, 70, 90]), total: 210}]);
    (deep(host, '[part="range"]') as HTMLButtonElement).click();
    await settle(host);
    const from = deep(host, '[part="brush-from"]') as HTMLInputElement;
    const to = deep(host, '[part="brush-to"]') as HTMLInputElement;
    const type = async (input: HTMLInputElement, text: string) => {
      input.value = text;
      input.dispatchEvent(new Event('input'));
      input.dispatchEvent(new Event('change'));
      await settle(host);
    };
    const filter = deep(host, '[part="brush-filter"]') as HTMLButtonElement;
    // Not fifteen, and not one and a half: nothing is applied.
    await type(from, '1,5');
    expect(from.getAttribute('aria-invalid')).toBe('true');
    const said = card.shadowRoot!.getElementById(from.getAttribute('aria-describedby')!)!.textContent!;
    expect(said).toContain('1,5');
    expect(said).toMatch(/comma/i);
    filter.click();
    await settle(host);
    expect(lastFilters(store)).toBeUndefined();
    // Nor is a space between digits that are not in threes, nor two kinds of mark in one number.
    for (const wrong of ['12,34', '1,2345', ',500', '1 00', '1  000', '10 0000', '1,234 567', '1 234,567']) {
      await type(from, wrong);
      expect(from.getAttribute('aria-invalid'), wrong).toBe('true');
    }
    await type(from, '1 00');
    expect(card.shadowRoot!.getElementById(from.getAttribute('aria-describedby')!)!.textContent).toMatch(/space/i);
    await type(from, '1.5');
    await type(to, '1,234,567.25');
    filter.click();
    expect(lastFilters(store)).toEqual({filter: {score: {family: 'numeric', gte: 1.5, lte: 1234567.25}}, highlight: {}});
    // A space, or the narrow space some locales print, groups thousands as a comma does.
    for (const [text, value] of [['1 000', 1000], ['1 234 567.25', 1234567.25], ['1\u202f234\u202f567', 1234567], ['-12 500', -12500]] as const) {
      (deep(host, '[part="range"]') as HTMLButtonElement).click();
      await settle(host);
      const field = deep(host, '[part="brush-from"]') as HTMLInputElement;
      await type(field, text);
      await type(deep(host, '[part="brush-to"]') as HTMLInputElement, '');
      expect(field.hasAttribute('aria-invalid'), text).toBe(false);
      (deep(host, '[part="brush-filter"]') as HTMLButtonElement).click();
      await settle(host);
      expect(lastFilters(store)!.filter['score'], text).toEqual({family: 'numeric', gte: value, lte: null});
    }
  });

  it('counts a range in the box only where its ends are the bins’ edges', async () => {
    const {host, store} = await mountCard('authors');
    await answer(host, store, [{rows: tens([5, 7, 9]), total: 21}], [{rows: tens([50, 70, 90]), total: 210}]);
    const plot = deep(host, '[part="plot"]') as HTMLElement;
    plot.dispatchEvent(new KeyboardEvent('keydown', {key: 'ArrowRight', bubbles: true}));
    plot.dispatchEvent(new KeyboardEvent('keydown', {key: 'ArrowRight', bubbles: true}));
    plot.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', bubbles: true}));
    await settle(host);
    const chosen = () => deep(host, '[part="brush"] .chosen')!.textContent!;
    expect(chosen()).toContain('70');
    const from = deep(host, '[part="brush-from"]') as HTMLInputElement;
    from.value = '12';
    from.dispatchEvent(new Event('input'));
    from.dispatchEvent(new Event('change'));
    await settle(host);
    expect(chosen()).toContain('12');
    expect(chosen()).not.toContain('70');
  });
});

describe('<mosaica-field-card> on a bool', () => {
  /** A bool's two bins, false then true, as the server serves them. */
  const flags = (no: number, yes: number) => [
    {lower: 0n, upper: 1n, count: no},
    {lower: 1n, upper: 2n, count: yes}
  ];
  const counts = (host: HTMLElement) => rows(host).map((r) => [r.dataset.key, r.querySelector('[part="name"]')!.textContent, r.querySelector('[part="count"]')!.textContent, r.dataset.state]);

  it('counts its two values in view and over everything matching, as two bins, each leaving out its own clause', async () => {
    const {store} = await mountCard('retracted');
    const twoBins = {count: true, by: {field: 'retracted', bins: 2, sample: 100_000}};
    expect(spec(store, 'field-subject')).toEqual({groupings: [twoBins], subject: 'view', highlighted: true, without: 'retracted'});
    expect(spec(store, 'field-match')).toEqual({groupings: [twoBins], without: 'retracted'});
  });

  it('lists yes and no with their counts', async () => {
    const {host, store} = await mountCard('retracted');
    await answer(host, store, [{rows: flags(30, 10), total: 45}], [{rows: flags(300, 100), total: 450}]);
    expect(counts(host)).toEqual([
      ['true', 'yes', '10 / 100', ''],
      ['false', 'no', '30 / 300', '']
    ]);
    expect(deep(host, 'mosaica-filter')).toBeNull();
  });

  it('marks counts taken from a sample as estimates', async () => {
    const {host, store} = await mountCard('retracted');
    await answer(host, store, [{rows: flags(30, 10), sample: {sampled: true, items: 40}}], [{rows: flags(300, 100), sample: {sampled: true, items: 400}}]);
    expect(counts(host).map((r) => r[2])).toEqual(['≈10 / ≈100', '≈30 / ≈300']);
  });

  it('filters and highlights by a value from its row, as a range over its 1 or 0', async () => {
    const {host, store} = await mountCard('retracted');
    await answer(host, store, [{rows: flags(30, 10)}], [{rows: flags(300, 100)}]);
    const changes: unknown[] = [];
    host.addEventListener('mosaica-filterchange', (e) => changes.push((e as CustomEvent).detail));
    (rows(host)[0]!.querySelector('[part="name"]') as HTMLElement).click();
    expect(lastFilters(store)).toEqual({filter: {retracted: {family: 'numeric', gte: 1, lte: 1}}, highlight: {}});
    (rows(host)[1]!.querySelector('[part="highlight"]') as HTMLButtonElement).click();
    expect(lastFilters(store)).toEqual({filter: {}, highlight: {retracted: {family: 'numeric', gte: 0, lte: 0}}});
    expect(changes).toEqual([
      {column: 'retracted', verb: 'filter', expr: {retracted: {range: {gte: 1, lte: 1}}}},
      {column: 'retracted', verb: 'highlight', expr: {retracted: {range: {gte: 0, lte: 0}}}}
    ]);
  });

  it('shows the value its filter keeps, adds the other to it, and takes either out', async () => {
    const {host, store} = await mountCard('retracted', {filter: {retracted: {family: 'numeric', gte: 1, lte: 1}}, highlight: {}});
    await answer(host, store, [{rows: flags(30, 10)}], [{rows: flags(300, 100)}]);
    expect(counts(host).map((r) => r[3])).toEqual(['in', 'out']);
    (rows(host)[1]!.querySelector('[part="filter"]') as HTMLButtonElement).click();
    expect(lastFilters(store)).toEqual({filter: {retracted: {family: 'numeric', gte: 0, lte: 1}}, highlight: {}});
    store.set('filters', filtersOf({filter: {retracted: {family: 'numeric', gte: 0, lte: 1}}, highlight: {}}));
    await settle(host);
    expect(counts(host).map((r) => r[3])).toEqual(['in', 'in']);
    (rows(host)[0]!.querySelector('[part="filter"]') as HTMLButtonElement).click();
    expect(lastFilters(store)).toEqual({filter: {retracted: {family: 'numeric', gte: 0, lte: 0}}, highlight: {}});
    store.set('filters', filtersOf({filter: {retracted: {family: 'numeric', gte: 0, lte: 0}}, highlight: {}}));
    await settle(host);
    (rows(host)[1]!.querySelector('[part="filter"]') as HTMLButtonElement).click();
    expect(lastFilters(store)).toEqual({filter: {retracted: {family: 'numeric', gte: null, lte: null}}, highlight: {}});
  });
});

describe('<mosaica-field-card> on a flat layer', () => {
  const flat = {...topicsLayer, hierarchy: {kind: 'flat', pruneChildren: false}} as Meta['layers'][number];
  const FLAT = {...META, layers: [flat]};

  async function mountFlat(over: {colourBy?: string} = {}) {
    const host = await mount('<mosaica-field-card field="cluster:topics"></mosaica-field-card>');
    const card = host.querySelector('mosaica-field-card') as MosaicaFieldCard;
    const store = fakeStore({meta: FLAT, status: status({}), filters: filtersOf({filter: {}, highlight: {}})});
    store.set('view', {...store.get('view'), id: 's0'});
    if (over.colourBy) store.set('legend', {...store.get('legend'), colourBy: over.colourBy});
    return {host, card, store};
  }

  it('ranks the clusters by their counts in view, leaving out its own clauses, and names each as its table does', async () => {
    const {host, card, store} = await mountFlat();
    card.store = store;
    await settle(host);
    // Each asks for the clusters' slots in the palette the map colours from, which the swatches read.
    const ranked = (top: number) => ({count: true, by: {layer: 'topics', top, paletteSize: 'drawn'}});
    expect(spec(store, 'field-subject')).toEqual({groupings: [ranked(5)], subject: 'view', highlighted: true, withoutMembersOf: 'topics'});
    // The whole match counts how many clusters there are, which the search box names.
    expect(spec(store, 'field-match')).toEqual({groupings: [ranked(1)], withoutMembersOf: 'topics'});
    answerAggregate(store, 'field-subject', aggregateEntry([{rows: [{key: 7n, count: 9, title: 'optics'}, {key: 8n, count: 5, title: 'lasers'}], total: 14}]));
    await settle(host);
    await new Promise((r) => setTimeout(r, 0));
    await settle(host);
    // And then the clusters the subject lists, by name, so each row has both counts.
    expect(spec(store, 'field-match')).toEqual({groupings: [{count: true, by: {layer: 'topics', artifacts: [7n, 8n], paletteSize: 'drawn'}}, ranked(1)], withoutMembersOf: 'topics'});
    answerAggregate(store, 'field-match', aggregateEntry([{rows: [{key: 8n, count: 50}, {key: 7n, count: 90}], total: 140}, {rows: [{key: 7n, count: 90}], groups: 12}]));
    await settle(host);
    expect(deep(host, 'mosaica-cluster-filter')!.getAttribute('placeholder')).toBe('Search 12 clusters');
    const entry = deep(host, 'mosaica-cluster-filter')!.shadowRoot!.querySelector('[part="entry"]')!;
    expect(entry.getAttribute('aria-label')).toContain('Topics');
    expect(entry.getAttribute('aria-label')).toContain('Search 12 clusters');
    expect(deep(host, '[part="rows"]')!.getAttribute('aria-label')).toContain('Topics');
    // A share of a few per cent still reads as a bar; plain percentages otherwise.
    const optics = rows(host)[0]!;
    expect((optics.querySelector('[part="bar-match"]') as HTMLElement).getAttribute('style')).toContain('max(3px, 64.3%)');
    expect(rows(host).map((r) => r.querySelector('[part="name"]')!.textContent)).toEqual(['optics', 'lasers']);
    // A flat layer has no parents to name, so no page is asked for.
    expect(store.calls.filter((c) => c.name === 'browse')).toEqual([]);
    const clauses: unknown[] = [];
    host.addEventListener('mosaica-clausechange', (e) => clauses.push((e as CustomEvent).detail));
    (rows(host)[1]!.querySelector('[part="highlight"]') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'setMembers').at(-1)!.args[0]).toEqual([{layer: 'topics', artifact: 8n, outside: false, verb: 'highlight', label: 'lasers'}]);
    expect(clauses).toEqual([{id: '8', layer: 'topics', outside: false, verb: 'highlight', on: true}]);
  });

  it('filters to a cluster from a click on its row, the row taking no focus', async () => {
    const {host, card, store} = await mountFlat();
    card.store = store;
    await settle(host);
    answerAggregate(store, 'field-subject', aggregateEntry([{rows: [{key: 7n, count: 9, title: 'optics'}], total: 9}]));
    await settle(host);
    const clauses: unknown[] = [];
    host.addEventListener('mosaica-clausechange', (e) => clauses.push((e as CustomEvent).detail));
    (rows(host)[0]!.querySelector('[part="name"]') as HTMLElement).click();
    await settle(host);
    expect(store.calls.filter((c) => c.name === 'setMembers').map((c) => c.args[0])).toEqual([[{layer: 'topics', artifact: 7n, outside: false, verb: 'filter', label: 'optics'}]]);
    expect(clauses).toEqual([{id: '7', layer: 'topics', outside: false, verb: 'filter', on: true}]);
    expect(rows(host)[0]!.hasAttribute('tabindex')).toBe(false);
  });

  it('colours each cluster’s swatch from the slot its row carries, a chosen colour over it, in the store’s palette', async () => {
    const {host, card, store} = await mountFlat({colourBy: 'cluster:topics'});
    card.store = store;
    await settle(host);
    const answerBoth = async (palette: 'tableau10' | 'okabe-ito') => {
      const none = undefined;
      answerAggregate(store, 'field-subject', aggregateEntry([{rows: [{key: 7n, count: 9, title: 'optics', slot: 2}, {key: 8n, count: 5, title: 'lasers', slot: null}], total: 14}], 's0', none, palette));
      answerAggregate(store, 'field-match', aggregateEntry([{rows: [{key: 7n, count: 9, slot: 2}, {key: 8n, count: 5, slot: null}]}, {rows: [{key: 7n, count: 9, slot: 2}], groups: 2}], 's0', none, palette));
      for (let i = 0; i < 3; i++) {
        await new Promise((r) => setTimeout(r, 0));
        await settle(host);
      }
    };
    await answerBoth('tableau10');
    const swatches = () => rows(host).map((r) => /--c:([^;]+)/.exec(r.querySelector('[part="swatch"]')!.getAttribute('style')!)![1]);
    // Slot 2 of Tableau 10 is #e15759; a cluster served with no slot is grey.
    expect(swatches()).toEqual(['rgb(225, 87, 89)', 'rgb(118, 126, 140)']);
    // A colour the host chose for a cluster is drawn in place of its slot's.
    store.set('artifacts', {...store.get('artifacts'), overrides: new Map([['topics', new Map([[8n, [1, 2, 3, 255] as const]])]])});
    await settle(host);
    expect(swatches()).toEqual(['rgb(225, 87, 89)', 'rgb(1, 2, 3)']);
    // Under Okabe-Ito the answers held keep Tableau 10's colours until answers for eight colours
    // land; the spec, which names the map's palette, does not change.
    const before = spec(store, 'field-subject');
    store.set('artifacts', {...store.get('artifacts'), palette: 'okabe-ito', overrides: new Map()});
    await settle(host);
    expect(spec(store, 'field-subject')).toEqual(before);
    expect(swatches()[0]).toBe('rgb(225, 87, 89)');
    // Slot 2 of Okabe-Ito is #009e73.
    await answerBoth('okabe-ito');
    expect(swatches()[0]).toBe('rgb(0, 158, 115)');
  });

  it('opens the colour picker from a cluster’s swatch, sets the colour chosen on the store over its slot’s, and reports it', async () => {
    const {host, card, store} = await mountFlat({colourBy: 'cluster:topics'});
    // 9 has a colour on this layer; 7 has one on another layer, which is not this cluster's.
    store.set('artifacts', {...store.get('artifacts'), overrides: new Map([['topics', new Map([[9n, [1, 2, 3, 220] satisfies Rgba]])], ['other', new Map([[7n, [4, 5, 6, 220] satisfies Rgba]])]])});
    card.store = store;
    await settle(host);
    answerAggregate(store, 'field-subject', aggregateEntry([{rows: [{key: 7n, count: 9, title: 'optics', slot: 2}], total: 9}], 's0', undefined, 'tableau10'));
    await settle(host);
    const chosen: unknown[] = [];
    host.addEventListener('mosaica-clustercolour', (e) => chosen.push((e as CustomEvent).detail));
    const swatch = () => rows(host)[0]!.querySelector('[part="swatch"]') as HTMLButtonElement;
    expect(swatch().tagName).toBe('BUTTON');
    swatch().click();
    await settle(host);
    // The picker offers the layer's palette, with the cluster's own colour, slot 2 of Tableau 10, pressed.
    expect(deep(host, '[part="colour-popover"]')!.getAttribute('aria-label')).toBe('Colour of optics');
    expect(deep(host, '[part="choice"][aria-pressed="true"]')!.getAttribute('aria-label')).toContain('#e15759');
    (deep(host, '[part="hex"]') as HTMLInputElement).value = '0A0B0C';
    deep(host, '[part="hex"]')!.dispatchEvent(new Event('change'));
    await settle(host);
    // Set over the slot's colour, beside the colours chosen before.
    const ids = (layer: string) => [...(store.get('artifacts').overrides.get(layer)?.keys() ?? [])].sort();
    expect(ids('topics')).toEqual([7n, 9n]);
    expect(ids('other')).toEqual([7n]);
    expect(swatch().getAttribute('style')).toContain('rgb(10, 11, 12)');
    (deep(host, '[part="reset"]') as HTMLButtonElement).click();
    await settle(host);
    expect(ids('topics')).toEqual([9n]);
    expect(ids('other')).toEqual([7n]);
    expect(swatch().getAttribute('style')).toContain('rgb(225, 87, 89)');
    expect(chosen).toEqual([
      {layer: 'topics', changes: [{mosaicaId: '7', colour: '#0a0b0c'}]},
      {layer: 'topics', changes: [{mosaicaId: '7', colour: null}]}
    ]);
  });

  it('ranks a flat layer’s clusters with no cut', async () => {
    const flatGroups = {...topicsLayer, name: 'groups', hierarchy: {kind: 'flat', pruneChildren: false}} as Meta['layers'][number];
    const host = await mount('<mosaica-field-card field="cluster:groups"></mosaica-field-card>');
    const card = host.querySelector('mosaica-field-card') as MosaicaFieldCard;
    const store = fakeStore({meta: {...META, layers: [flatGroups]}, status: status({}), filters: filtersOf({filter: {}, highlight: {}})});
    store.set('view', {...store.get('view'), id: 's0'});
    card.store = store;
    await settle(host);
    expect(spec(store, 'field-subject')).toMatchObject({groupings: [{count: true, by: {layer: 'groups', top: 5}}]});
  });
});

/** A browse row as the server serves it; `matched` is its count under the filters, where they are set. */
const browseRow = (id: bigint, name: string | null, over: {key?: string; rung?: number; parentIds?: bigint[]; childCount?: number; masked?: bigint; matched?: bigint | null; slot?: number | null} = {}): BrowseRow => ({
  mosaicaId: id,
  key: over.key ?? null,
  name,
  maskedCount: over.masked ?? 10n,
  matchedCount: over.matched ?? null,
  rung: over.rung ?? 0,
  parentIds: over.parentIds ?? [],
  childCount: over.childCount ?? 0,
  slot: over.slot ?? null
});

/** A page of `rows`, the last unless `next` is given. */
const page = (rows: BrowseRow[], next: string | null = null, parents: BrowseRow[] = []): BrowsePage => ({artifacts: rows, parents, next});

/** The browse requests a store has been asked, in order. */
const browses = (store: FakeStore) => store.calls.filter((c) => c.name === 'browse').map((c) => c.args[0] as Record<string, unknown>);

/** Let the browse pages and the renders after them land. */
async function landed(host: HTMLElement) {
  for (let i = 0; i < 3; i++) {
    await new Promise((r) => setTimeout(r, 0));
    await settle(host);
  }
}

/** Put the filter clauses on `ids` of `layer`, as the store does once a row asks for them. */
function filterTo(store: FakeStore, layer: string, ids: [bigint, string][]) {
  store.set('filters', {...store.get('filters'), members: ids.map(([artifact, label]) => ({layer, artifact, outside: false, verb: 'filter' as const, label}))});
}

const rowNames = (host: HTMLElement) => rows(host).map((r) => r.querySelector('[part="name"]')!.textContent);

describe('<mosaica-field-card> on a linked layer', () => {
  const taxonomy = {
    ...topicsLayer,
    name: 'taxonomy',
    title: 'arXiv classification',
    hierarchy: {kind: 'tiered', pruneChildren: false},
    levels: [
      {level: 0, title: 'Archive', zoom: null},
      {level: 1, title: 'Subject class', zoom: null},
      {level: 2, title: 'Topic', zoom: null}
    ]
  } as unknown as Meta['layers'][number];

  async function mountLinked(layer: Meta['layers'][number], script: (store: FakeStore) => void = () => {}) {
    const host = await mount(`<mosaica-field-card field="cluster:${layer.name}"></mosaica-field-card>`);
    const card = host.querySelector('mosaica-field-card') as MosaicaFieldCard;
    const store = fakeStore({meta: meta({layers: [layer]}), status: status({}), filters: filtersOf({filter: {}, highlight: {}})});
    store.set('view', {...store.get('view'), id: 's0'});
    script(store);
    card.store = store;
    await landed(host);
    return {host, card, store};
  }

  const archives = (store: FakeStore) => {
    store.setBrowse('roots', page([browseRow(1n, 'Mathematics', {childCount: 3, masked: 900n}), browseRow(2n, 'Physics', {childCount: 2, masked: 700n}), browseRow(9n, 'q-bio.NC', {rung: 1, masked: 40n})]));
    store.setBrowse(
      'p:1',
      page([
        browseRow(11n, 'math.AG', {rung: 1, parentIds: [1n], masked: 300n, matched: 300n, childCount: 2}),
        browseRow(12n, 'math.NT', {rung: 1, parentIds: [1n], masked: 250n, matched: 250n}),
        browseRow(13n, 'math.LO', {rung: 1, parentIds: [1n], masked: 50n, matched: 0n})
      ])
    );
    store.setBrowse('p:11', page([browseRow(111n, 'Moduli', {rung: 2, parentIds: [11n]}), browseRow(112n, 'Schemes', {rung: 2, parentIds: [11n]})]));
  };

  const crumbNames = (host: HTMLElement) => deepAll(host, '[part="crumb-name"]').map((e) => e.querySelector('.crumb-text')!.textContent);
  const stepButton = (host: HTMLElement, key: string) => deep(host, `[part="row"][data-key="${key}"] [part="step"]`) as HTMLButtonElement | null;
  const setMembersCalls = (store: FakeStore) => store.calls.filter((c) => c.name === 'setMembers');
  /** The text of the element an `aria-labelledby` names, in the card's shadow root. */
  const labelOf = (card: MosaicaFieldCard, el: Element) => card.shadowRoot!.getElementById(el.getAttribute('aria-labelledby')!)!.textContent;

  it('opens on the roots of every level under the store’s filters, grouped under their levels’ headings, and counts nothing else', async () => {
    const {host, card, store} = await mountLinked(taxonomy, archives);
    // No level, so a level-1 class whose archive is withheld is a root too; no `filters`, so the
    // store sends its own.
    expect(browses(store)).toEqual([{layer: 'taxonomy', limit: 20}]);
    const headings = deepAll(host, '[part="level"]');
    expect(headings.map((e) => [e.tagName, e.textContent])).toEqual([
      ['H4', 'Archive'],
      ['H4', 'Subject class']
    ]);
    // Each list is named by the heading over it.
    expect(deepAll(host, '[part="rows"]').map((e) => labelOf(card, e))).toEqual(['Archive', 'Subject class']);
    expect(rowNames(host)).toEqual(['Mathematics', 'Physics', 'q-bio.NC']);
    // With no filter, a row's one count is every item of it the viewer may see.
    expect(rows(host)[0]!.querySelector('[part="count"]')!.textContent).toBe('900');
    expect(registered(store).size).toBe(0);
    // The breadcrumb stands at the top level.
    expect(crumbNames(host)).toEqual(['All']);
    expect(deep(host, '[part="crumb-name"]')!.getAttribute('aria-current')).toBe('location');
    expect(deep(host, '[part="crumbs"]')!.tagName).toBe('NAV');
    expect(deep(host, '[part="chips"]')).toBeNull();
  });

  it('ends a row with children in a button with their number, and a leaf in none', async () => {
    const {host} = await mountLinked(taxonomy, archives);
    expect(stepButton(host, '1')!.textContent).toContain('3');
    expect(stepButton(host, '2')!.textContent).toContain('2');
    expect(stepButton(host, '9')).toBeNull();
    // Each says what it opens, beyond its figure.
    expect(stepButton(host, '1')!.getAttribute('aria-label')).toContain('Mathematics');
  });

  it('steps into a row, listing its children under the filter, and leaves the filter as it is', async () => {
    const {host, card, store} = await mountLinked(taxonomy, archives);
    const clauses: unknown[] = [];
    host.addEventListener('mosaica-clausechange', (e) => clauses.push((e as CustomEvent).detail));
    stepButton(host, '1')!.focus();
    stepButton(host, '1')!.click();
    await landed(host);
    expect(setMembersCalls(store)).toEqual([]);
    expect(clauses).toEqual([]);
    expect(browses(store).at(-1)).toEqual({layer: 'taxonomy', parent: 1n, limit: 20});
    expect(crumbNames(host)).toEqual(['All', 'Mathematics']);
    expect(deepAll(host, '[part="crumb-name"]').map((e) => e.getAttribute('aria-current'))).toEqual([null, 'location']);
    // In the server's order, a class none of whose items match still listed, each matched over masked.
    expect(rowNames(host)).toEqual(['math.AG', 'math.NT', 'math.LO']);
    expect(rows(host).map((r) => r.querySelector('[part="count"]')!.textContent)).toEqual(['300 / 300', '250 / 250', '0 / 50']);
    expect(deepAll(host, '[part="rows"]').map((e) => labelOf(card, e))).toEqual(['Subject class']);
    // Focus moves to the first row that loaded, and the one status region has words for what did.
    expect(focused(card)!.closest('[part="row"]')!.getAttribute('data-key')).toBe('11');
    const regions = deepAll(card.shadowRoot!, '[role="status"]').filter((e) => card.shadowRoot!.contains(e));
    expect(regions).toHaveLength(1);
    expect(regions[0]!.textContent!.trim()).not.toBe('');
    // Filtering to a class adds a clause, which the others combine with by AND.
    (rows(host)[0]!.querySelector('[part="filter"]') as HTMLButtonElement).click();
    expect(setMembersCalls(store).at(-1)!.args[0]).toEqual([{layer: 'taxonomy', artifact: 11n, outside: false, verb: 'filter', label: 'math.AG'}]);
  });

  it('steps back from a crumb, focus on the row stepped out of, and to the top from All', async () => {
    const {host, card, store} = await mountLinked(taxonomy, archives);
    stepButton(host, '1')!.click();
    await landed(host);
    stepButton(host, '11')!.focus();
    stepButton(host, '11')!.click();
    await landed(host);
    expect(crumbNames(host)).toEqual(['All', 'Mathematics', 'math.AG']);
    expect(rowNames(host)).toEqual(['Moduli', 'Schemes']);
    const crumb = (i: number) => deepAll(host, '[part="crumb-name"]')[i] as HTMLButtonElement;
    crumb(1).focus();
    crumb(1).click();
    await landed(host);
    expect(crumbNames(host)).toEqual(['All', 'Mathematics']);
    expect(rowNames(host)).toEqual(['math.AG', 'math.NT', 'math.LO']);
    expect(focused(card)).toBe(stepButton(host, '11'));
    crumb(0).focus();
    crumb(0).click();
    await landed(host);
    expect(crumbNames(host)).toEqual(['All']);
    expect(rowNames(host)).toEqual(['Mathematics', 'Physics', 'q-bio.NC']);
    expect(focused(card)).toBe(stepButton(host, '1'));
    expect(setMembersCalls(store)).toEqual([]);
  });

  it('holds focus on the deepest crumb until a step’s rows land', async () => {
    let release: () => void = () => {};
    const {host, card, store} = await mountLinked(taxonomy, archives);
    const answer = store.browse;
    store.browse = (async (req: {parent?: bigint}) => {
      if (req.parent === 1n) await new Promise<void>((r) => (release = r));
      return answer(req as never);
    }) as typeof store.browse;
    stepButton(host, '1')!.focus();
    stepButton(host, '1')!.click();
    await landed(host);
    expect(rows(host)).toEqual([]);
    expect(focused(card)).toBe(deepAll(host, '[part="crumb-name"]')[1]);
    release();
    await landed(host);
    expect(focused(card)!.closest('[part="row"]')!.getAttribute('data-key')).toBe('11');
  });

  it('marks a crumb its layer’s filter is on, and shows each of the layer’s clauses as a chip with a ×, wherever it was set', async () => {
    const {host, card, store} = await mountLinked(taxonomy, (s) => {
      archives(s);
      s.set('filters', {
        ...s.get('filters'),
        members: [
          {layer: 'taxonomy', artifact: 1n, outside: false, verb: 'filter', label: 'Mathematics'},
          {layer: 'taxonomy', artifact: 20n, outside: false, verb: 'filter', label: 'Condensed matter'},
          {layer: 'taxonomy', artifact: 2n, outside: false, verb: 'highlight', label: 'Physics'}
        ]
      });
    });
    stepButton(host, '1')!.click();
    await landed(host);
    expect(deepAll(host, '[part="crumb"]').map((c) => c.hasAttribute('data-filtered'))).toEqual([false, true]);
    // A clause not on the path is a chip as much as one that is.
    const chips = () => deepAll(host, '[part="chip"]');
    expect(chips().map((c) => [c.getAttribute('data-verb'), c.getAttribute('data-id')])).toEqual([
      ['filter', '1'],
      ['filter', '20'],
      ['highlight', '2']
    ]);
    expect(deep(host, '[part="chips"]')!.closest('[part="crumbs"]')).toBeNull();
    const clauses: unknown[] = [];
    host.addEventListener('mosaica-clausechange', (e) => clauses.push((e as CustomEvent).detail));
    const remove = chips()[1]!.querySelector('[part="chip-remove"]') as HTMLButtonElement;
    remove.focus();
    remove.click();
    expect(setMembersCalls(store).at(-1)!.args[0]).toEqual([
      {layer: 'taxonomy', artifact: 1n, outside: false, verb: 'filter', label: 'Mathematics'},
      {layer: 'taxonomy', artifact: 2n, outside: false, verb: 'highlight', label: 'Physics'}
    ]);
    expect(clauses).toEqual([{id: '20', layer: 'taxonomy', outside: false, verb: 'filter', on: false}]);
    store.set('filters', {...store.get('filters'), members: setMembersCalls(store).at(-1)!.args[0] as never});
    await landed(host);
    // Focus goes to the × of the chip now in its place.
    expect(focused(card)).toBe(chips()[1]!.querySelector('[part="chip-remove"]'));
    // A row the filter or the highlight is on keeps its pressed buttons.
    store.set('filters', {...store.get('filters'), members: [...store.get('filters').members, {layer: 'taxonomy', artifact: 12n, outside: false, verb: 'filter', label: 'math.NT'}]});
    await landed(host);
    expect(rows(host)[1]!.querySelector('[part="filter"]')!.getAttribute('aria-pressed')).toBe('true');
    // Placed where the clauses are shown already, the card leaves its chips out and keeps its ticks.
    card.hideChips = true;
    await landed(host);
    expect(deep(host, '[part="chips"]')).toBeNull();
    expect(deepAll(host, '[part="crumb"]').map((c) => c.hasAttribute('data-filtered'))).toEqual([false, true]);
    expect(rows(host)[1]!.getAttribute('data-state')).toBe('in');
  });

  it('calls a cluster with no name by its key, marked as one, in its row, its crumb, its clause and its chip, and one with neither Unnamed', async () => {
    const mesh = {...topicsLayer, name: 'mesh', title: 'MeSH', hierarchy: {kind: 'dag', pruneChildren: false}} as Meta['layers'][number];
    const {host, store} = await mountLinked(mesh, (s) => {
      s.setBrowse('roots', page([browseRow(1n, null, {key: 'C04', childCount: 1}), browseRow(2n, null)]));
      s.setBrowse('p:1', page([browseRow(5n, null, {key: 'C04.588', parentIds: [1n, 3n]})]));
      s.setBrowse('p:5', page([], null, [browseRow(1n, null, {key: 'C04'}), browseRow(3n, null, {key: 'C10'})]));
    });
    const marks = (e: Element) => [e.hasAttribute('data-key-name'), e.hasAttribute('data-unnamed')];
    expect(rowNames(host)).toEqual(['C04', UNNAMED]);
    expect(rows(host).map((r) => marks(r.querySelector('[part="name"]')!))).toEqual([
      [true, false],
      [false, true]
    ]);
    (rows(host)[1]!.querySelector('[part="filter"]') as HTMLButtonElement).click();
    expect(setMembersCalls(store).at(-1)!.args[0]).toEqual([{layer: 'mesh', artifact: 2n, outside: false, verb: 'filter'}]);
    stepButton(host, '1')!.click();
    await landed(host);
    expect(crumbNames(host)).toEqual(['All', 'C04']);
    expect(deepAll(host, '[part="crumb-name"] .crumb-text').map(marks)).toEqual([
      [false, false],
      [true, false]
    ]);
    expect(rowNames(host)).toEqual(['C04.588']);
    const also = deep(host, '[part="also"]')!;
    expect(also.textContent).toBe('also under C10');
    expect(also.querySelector('[data-key-name]')!.textContent).toBe('C10');
    (rows(host)[0]!.querySelector('[part="filter"]') as HTMLButtonElement).click();
    const clause = {layer: 'mesh', artifact: 5n, outside: false, verb: 'filter', label: 'C04.588'};
    expect(setMembersCalls(store).at(-1)!.args[0]).toEqual([clause]);
    store.set('filters', {...store.get('filters'), members: [clause as never]});
    await landed(host);
    const chip = deep(host, '[part="chip-name"]')!;
    expect(chip.querySelector('[data-key-name]')!.textContent).toBe('C04.588');
  });

  it('counts the position’s items in none of its children once every child has loaded, as the last row of the last list', async () => {
    const {host, store} = await mountLinked(taxonomy, archives);
    expect(spec(store, 'field-remainder')).toBeUndefined();
    stepButton(host, '1')!.click();
    await landed(host);
    const member = (id: string) => ({member_of: {layer: 'taxonomy', artifact: id}});
    expect(spec(store, 'field-remainder')).toEqual({groupings: [{count: true}], filters: {all_of: [member('1'), {none_of: [member('11'), member('12'), member('13')]}]}});
    answerAggregate(store, 'field-remainder', aggregateEntry([{rows: [{count: 42}], total: 42}]));
    await settle(host);
    const rest = deep(host, '[part="remainder"]')!;
    expect(rest.querySelector('[part="count"]')!.textContent).toBe('42');
    expect(rest.parentElement!.getAttribute('role')).toBe('listitem');
    expect(rest.closest('[role="list"]')).toBe(deepAll(host, '[part="rows"]').at(-1));
  });

  it('asks for no remainder while the children run past a page, and says how many are still to come', async () => {
    const {host, card, store} = await mountLinked(taxonomy, (s) => {
      archives(s);
      s.setBrowse('p:1', page([browseRow(11n, 'math.AG', {rung: 1, parentIds: [1n]})], 'c2'));
      s.setBrowse('p:1:c2', page([browseRow(12n, 'math.NT', {rung: 1, parentIds: [1n]})], 'c3'));
    });
    stepButton(host, '1')!.click();
    await landed(host);
    expect(spec(store, 'field-remainder')).toBeUndefined();
    expect(deep(host, '[part="remainder"]')).toBeNull();
    // Mathematics was listed with three children, one of them on the page.
    const more = deep(host, '[part="more"]') as HTMLButtonElement;
    expect(more.textContent).toBe('2 more');
    expect(more.getAttribute('aria-label')).toContain('Mathematics');
    more.focus();
    more.click();
    await landed(host);
    expect(browses(store).at(-1)).toEqual({layer: 'taxonomy', parent: 1n, cursor: 'c2', limit: 20});
    expect(rowNames(host)).toEqual(['math.AG', 'math.NT']);
    expect(deep(host, '[part="more"]')!.textContent).toBe('1 more');
    // Focus moves from More to the first row its page added.
    expect(focused(card)!.closest('[part="row"]')!.getAttribute('data-key')).toBe('12');
  });

  it('offers no More while the rows are fetched again under new filters, and never sends a cursor served under the old ones', async () => {
    let release: () => void = () => {};
    const {host, store} = await mountLinked(taxonomy, (s) => s.setBrowse('roots', page([browseRow(1n, 'Mathematics')], 'c2')));
    expect(deep(host, '[part="more"]')).not.toBeNull();
    const answer = store.browse;
    store.browse = (async (req: {cursor?: string}) => {
      if (req.cursor === undefined) {
        await new Promise<void>((r) => (release = r));
        return page([browseRow(2n, 'Physics')], 'c9');
      }
      return answer(req as never);
    }) as typeof store.browse;
    store.set('filters', {...store.get('filters'), expr: {archive: {in: ['physics']}}});
    await landed(host);
    // The rows under the old filters stay until the new ones land, without their next page.
    expect(rowNames(host)).toEqual(['Mathematics']);
    expect(deep(host, '[part="more"]')).toBeNull();
    release();
    await landed(host);
    expect(rowNames(host)).toEqual(['Physics']);
    (deep(host, '[part="more"]') as HTMLButtonElement).click();
    await landed(host);
    expect(browses(store).filter((r) => r.cursor !== undefined).map((r) => r.cursor)).toEqual(['c9']);
  });

  it('keeps focus on a leaf’s Filter as the filter it adds fetches the rows again', async () => {
    const {host, card, store} = await mountLinked(taxonomy, archives);
    const filter = () => rows(host)[2]!.querySelector('[part="filter"]') as HTMLButtonElement;
    filter().focus();
    filter().click();
    filterTo(store, 'taxonomy', [[9n, 'q-bio.NC']]);
    await landed(host);
    expect(browses(store)).toHaveLength(2);
    expect(rowNames(host)).toEqual(['Mathematics', 'Physics', 'q-bio.NC']);
    expect(focused(card)).toBe(filter());
    expect(filter().getAttribute('aria-pressed')).toBe('true');
  });

  it('keeps the search box, with focus in it, as the rows below it are replaced, and its words are the same at every position', async () => {
    const {host, card, store} = await mountLinked(taxonomy, archives);
    const search = deep(host, 'mosaica-cluster-filter')!;
    const placeholder = search.getAttribute('placeholder');
    stepButton(host, '1')!.click();
    await landed(host);
    expect(deep(host, 'mosaica-cluster-filter')).toBe(search);
    expect(search.getAttribute('placeholder')).toBe(placeholder);
    (search as HTMLElement).focus();
    store.setBrowse('p:1', page([browseRow(14n, 'math.CT', {rung: 1, parentIds: [1n]})]));
    store.set('filters', {...store.get('filters'), expr: {archive: {in: ['math']}}});
    await landed(host);
    expect(rowNames(host)).toEqual(['math.CT']);
    expect(focused(card)).toBe(search);
  });

  it('names a dag row’s other served parents under it', async () => {
    const mesh = {...topicsLayer, name: 'mesh', title: 'MeSH', hierarchy: {kind: 'dag', pruneChildren: false}} as Meta['layers'][number];
    const {host} = await mountLinked(mesh, (s) => {
      s.setBrowse('roots', page([browseRow(1n, 'Neoplasms', {childCount: 2})]));
      s.setBrowse('p:1', page([browseRow(5n, 'Brain tumours', {parentIds: [1n, 3n]}), browseRow(6n, 'Carcinoma', {parentIds: [1n]})]));
      // The children form of the row carries its parents, which name the one the list has not met.
      s.setBrowse('p:5', page([], null, [browseRow(1n, 'Neoplasms'), browseRow(3n, 'Nervous system diseases')]));
    });
    stepButton(host, '1')!.click();
    await landed(host);
    expect(rowNames(host)).toEqual(['Brain tumours', 'Carcinoma']);
    expect(deepAll(host, '[part="also"]').map((e) => e.textContent)).toEqual(['also under Nervous system diseases']);
  });

  it('colours each row from the slot browse serves it in the store’s palette while the map is coloured by the layer', async () => {
    const {host, store} = await mountLinked(taxonomy, (s) => {
      s.set('legend', {...s.get('legend'), colourBy: 'cluster:taxonomy'});
      s.setBrowse('roots', page([browseRow(1n, 'Mathematics', {slot: 2}), browseRow(2n, 'Physics')]));
    });
    expect(browses(store)).toEqual([{layer: 'taxonomy', limit: 20, paletteSize: 10}]);
    const swatches = () => rows(host).map((r) => /--c:([^;]+)/.exec(r.querySelector('[part="swatch"]')!.getAttribute('style')!)![1]);
    expect(swatches()).toEqual(['rgb(225, 87, 89)', 'rgb(118, 126, 140)']);
    store.set('artifacts', {...store.get('artifacts'), overrides: new Map([['taxonomy', new Map([[2n, [1, 2, 3, 255] as const]])]])});
    await settle(host);
    expect(swatches()).toEqual(['rgb(225, 87, 89)', 'rgb(1, 2, 3)']);
  });

  it('draws its counts in the plain colour while there is a highlight, since they leave the highlight out', async () => {
    const {host, card, store} = await mountLinked(taxonomy, archives);
    store.set('filters', {...store.get('filters'), members: [{layer: 'taxonomy', artifact: 2n, outside: false, verb: 'highlight', label: 'Physics'}]});
    await landed(host);
    expect(card.hasAttribute('highlighting')).toBe(false);
    expect(deepAll(host, '[part="chip"]').map((c) => c.getAttribute('data-verb'))).toEqual(['highlight']);
  });

  it('keeps the level choice on a tiered card, which sets the drawn level alone', async () => {
    const {host, store} = await mountLinked(taxonomy, archives);
    const chosen: unknown[] = [];
    host.addEventListener('mosaica-levelchange', (e) => chosen.push(e.detail));
    const select = deep(host, '[part="level-select"]') as HTMLSelectElement;
    select.value = '1';
    select.dispatchEvent(new Event('change'));
    await landed(host);
    expect(chosen).toEqual([{layer: 'taxonomy', level: 1}]);
    expect(rowNames(host)).toEqual(['Mathematics', 'Physics', 'q-bio.NC']);
    expect(browses(store)).toHaveLength(1);
  });
});

describe('<mosaica-field-card> on a stacked layer', () => {
  const bands = {...topicsLayer, name: 'bands', title: 'Bands', hierarchy: {kind: 'stacked', pruneChildren: false}, levels: [{level: 0, title: 'Broad'}, {level: 1, title: 'Fine'}]} as unknown as Meta['layers'][number];

  async function mountStacked() {
    const host = await mount('<mosaica-field-card field="cluster:bands"></mosaica-field-card>');
    const card = host.querySelector('mosaica-field-card') as MosaicaFieldCard;
    const store = fakeStore({meta: meta({layers: [bands]}), status: status({}), filters: filtersOf({filter: {}, highlight: {}})});
    store.set('view', {...store.get('view'), id: 's0'});
    card.store = store;
    await settle(host);
    return {host, card, store};
  }

  const ranked = (level: number) => ({count: true, by: {layer: 'bands', level, top: 5, paletteSize: 'drawn'}});

  it('ranks the clusters at the drawn level under every clause, and steps into one to rank the next level inside it, under a heading', async () => {
    const {host, card, store} = await mountStacked();
    expect(spec(store, 'field-subject')).toEqual({groupings: [ranked(0)], subject: 'view', highlighted: true});
    const placeholder = deep(host, 'mosaica-cluster-filter')!.getAttribute('placeholder');
    answerAggregate(store, 'field-subject', aggregateEntry([{rows: [{key: 4n, count: 9, title: 'north'}], total: 9}]));
    await landed(host);
    const step = rows(host)[0]!.querySelector('[part="step"]') as HTMLButtonElement;
    expect(step.getAttribute('aria-label')).toContain('north');
    step.focus();
    step.click();
    await landed(host);
    // Inside north by a filter of the spec's own; the store's filters are left as they are.
    expect(spec(store, 'field-subject')).toEqual({groupings: [ranked(1)], subject: 'view', highlighted: true, filters: {member_of: {layer: 'bands', artifact: '4'}}});
    expect(store.calls.filter((c) => c.name === 'setMembers')).toEqual([]);
    expect(deepAll(host, '[part="crumb-name"]').map((e) => e.querySelector('.crumb-text')!.textContent)).toEqual(['All', 'north']);
    const heading = deep(host, '[part="level"]')!;
    expect(heading.tagName).toBe('H4');
    expect(heading.textContent).toContain('Fine');
    expect(heading.textContent).toContain('north');
    expect(deep(host, '[part="rows"]')!.getAttribute('aria-labelledby')).toBe(heading.id);
    // The answer held for the level before is not drawn as the rows inside north.
    expect(rows(host)).toEqual([]);
    expect(focused(card)).toBe(deepAll(host, '[part="crumb-name"]')[1]);
    answerAggregate(store, 'field-subject', aggregateEntry([{rows: [{key: 41n, count: 5, title: 'north-east'}], total: 5}]));
    await landed(host);
    expect(rowNames(host)).toEqual(['north-east']);
    // The finest level has none after it to step into.
    expect(rows(host)[0]!.querySelector('[part="step"]')).toBeNull();
    expect(focused(card)!.closest('[part="row"]')!.getAttribute('data-key')).toBe('41');
    expect(deep(host, 'mosaica-cluster-filter')!.getAttribute('placeholder')).toBe(placeholder);
    // The match counts the rows listed inside north too.
    expect(spec(store, 'field-match')).toEqual({groupings: [{count: true, by: {layer: 'bands', level: 1, artifacts: [41n], paletteSize: 'drawn'}}, {...ranked(1), by: {...ranked(1).by, top: 1}}], filters: {member_of: {layer: 'bands', artifact: '4'}}});
  });
});

describe('<mosaica-field-card> across a change of viewer', () => {
  it('drops a browse page still loading for the viewer before, so nothing it met names a cluster to the next', async () => {
    const host = await mount('<mosaica-field-card field="cluster:topics"></mosaica-field-card>');
    const card = host.querySelector('mosaica-field-card') as MosaicaFieldCard;
    const before = fakeStore({meta: META, status: status({}), filters: filtersOf({filter: {}, highlight: {}})});
    before.set('view', {...before.get('view'), id: 's0'});
    // The page of the roots waits until the viewer has changed.
    let release: () => void = () => {};
    const held = new Promise<void>((r) => (release = r));
    before.browse = async () => {
      await held;
      return page([browseRow(1n, 'seen only by the first viewer')]);
    };
    card.store = before;
    await settle(host);

    const after = fakeStore({meta: META, status: status({}), filters: filtersOf({filter: {}, highlight: {}})});
    after.set('view', {...after.get('view'), id: 's0'});
    after.setBrowse('roots', page([browseRow(3n, 'theirs')]));
    card.store = after;
    await settle(host);
    release();
    await landed(host);
    expect(rowNames(host)).toEqual(['theirs']);
    expect(deepAll(host, '[part="name"], [part="also"]').map((e) => e.textContent).join(' ')).not.toContain('first viewer');
  });
});

describe('<mosaica-field-card> on text', () => {
  it('is the field’s search box, and counts nothing', async () => {
    const {host, store} = await mountCard('title');
    expect(deep(host, 'mosaica-filter')).not.toBeNull();
    expect(deep(host, '[part="paint"]')).toBeNull();
    expect(registered(store).size).toBe(0);
  });
});

describe('<mosaica-field-card> on a levelled layer', () => {
  /** The store's level for the layer where none is chosen, as it works it out from what is in view. */
  const followed = (level: number) => new Map([['bands', level]]);

  it('follows the zoom until a level is chosen, and Follow zoom goes back to it', async () => {
    const bands = {...topicsLayer, name: 'bands', title: 'Bands', hierarchy: {kind: 'stacked', pruneChildren: false}, levels: [{level: 0, title: 'Broad', zoom: null}, {level: 1, title: 'Fine', zoom: null}]} as unknown as Meta['layers'][number];
    const host = await mount('<mosaica-field-card field="cluster:bands"></mosaica-field-card>');
    const card = host.querySelector('mosaica-field-card') as MosaicaFieldCard;
    const store = fakeStore({meta: meta({layers: [bands]}), status: status({}), filters: filtersOf({filter: {}, highlight: {}})});
    store.set('view', {...store.get('view'), id: 's0'});
    card.store = store;
    await settle(host);
    // Nothing in view: the coarsest level.
    expect(JSON.stringify(spec(store, 'field-subject'))).toContain('"level":0');
    const select = () => deep(host, '[part="level-select"]') as HTMLSelectElement;
    expect(select().getAttribute('aria-label')).toContain('Bands');
    expect([...select().options].map((o) => [o.textContent, o.value, o.selected])).toEqual([
      ['Follow zoom', '', true],
      ['Broad', '0', false],
      ['Fine', '1', false]
    ]);
    // Zoomed in, the fine level fits in view, and the card counts at it.
    store.set('artifacts', {...store.get('artifacts'), followedLevels: followed(1)});
    await settle(host);
    expect(JSON.stringify(spec(store, 'field-subject'))).toContain('"level":1');
    expect(select().value).toBe('');
    const chosen: unknown[] = [];
    host.addEventListener('mosaica-levelchange', (e) => chosen.push(e.detail));
    select().value = '0';
    select().dispatchEvent(new Event('change'));
    await settle(host);
    expect(JSON.stringify(spec(store, 'field-subject'))).toContain('"level":0');
    select().value = '';
    select().dispatchEvent(new Event('change'));
    await settle(host);
    expect(JSON.stringify(spec(store, 'field-subject'))).toContain('"level":1');
    // The choice names the card's layer, so a listener sets that layer's level alone.
    expect(chosen).toEqual([
      {layer: 'bands', level: 0},
      {layer: 'bands', level: null}
    ]);
  });

  it('counts at the coarsest level in view where the levels declare zoom ranges, and offers no Follow zoom', async () => {
    const bands = {...topicsLayer, name: 'bands', title: 'Bands', hierarchy: {kind: 'stacked', pruneChildren: false}, levels: [{level: 0, title: 'Broad', zoom: [0, 5]}, {level: 1, title: 'Fine', zoom: [4, 16]}]} as unknown as Meta['layers'][number];
    const host = await mount('<mosaica-field-card field="cluster:bands"></mosaica-field-card>');
    const card = host.querySelector('mosaica-field-card') as MosaicaFieldCard;
    const store = fakeStore({meta: meta({layers: [bands]}), status: status({}), filters: filtersOf({filter: {}, highlight: {}})});
    store.set('view', {...store.get('view'), id: 's0'});
    store.set('artifacts', {...store.get('artifacts'), followedLevels: followed(1)});
    card.store = store;
    await settle(host);
    expect(JSON.stringify(spec(store, 'field-subject'))).toContain('"level":1');
    const select = deep(host, '[part="level-select"]') as HTMLSelectElement;
    expect(select.value).toBe('1');
    expect([...select.options].map((o) => o.textContent)).toEqual(['Broad', 'Fine']);
  });
});
