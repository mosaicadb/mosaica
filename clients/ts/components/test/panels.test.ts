import {afterEach, describe, expect, it, vi} from 'vitest';
import {activeCount, type FilterDraft, type FiltersProjection, type Meta} from '@mosaicajs/client';
import '../src/item-card.js';
import '../src/filter.js';
import '../src/filter-panel.js';
import type {MosaicaItemCard} from '../src/item-card.js';
import type {MosaicaFilter} from '../src/filter.js';
import type {MosaicaFilterPanel} from '../src/filter-panel.js';
import type {MosaicaFieldCard} from '../src/field-card.js';
import {aggregateEntry, answerAggregate, deep, deepAll, fakeStore, mount, registered, settle, status, meta, scalar} from './fake-store.js';
import {UNNAMED, dateRangeText, parseDateText} from '../src/base.js';

afterEach(() => {
  document.body.innerHTML = '';
  vi.useRealTimers();
});

const META = meta({
  declaredScalars: [
    scalar('archive', 'u16', {category: {vocabulary: 'a', kind: 'declared', visibility: 'public'}, render: true, homes: ['rendered']}),
    scalar('submitted_at', 'timestamp_us', {render: true, homes: ['rendered']}),
    scalar('title', 'utf8')
  ],
  filterOperands: [
    {column: 'archive', family: 'category', operands: ['eq', 'in']},
    {column: 'title', family: 'text', operands: ['match', 'phrase']},
    {column: 'submitted_at', family: 'numeric', operands: ['range']}
  ]
});

describe('<mosaica-item-card>', () => {
  it('renders fields by name in declaration order, presented by type, with a slot per field', async () => {
    const host = await mount('<mosaica-item-card title-field="title"><a slot="field-title" href="#">my link</a></mosaica-item-card>');
    const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
    el.meta = META;
    // `archive` absent from the record, the extra `note` undeclared: order is declared-then-extra,
    // and the gap is named rather than shifting the fields after it.
    el.item = {id: 12345678901234567890n, detail: {fields: {note: 'x', title: 'A title', submitted_at: 1_700_000_000_000_000}, labels: [], views: [], scoped: {}}};
    await settle(host);
    // The headline is the field the host named; the rest in declared-then-extra order, then the
    // id; the absent `archive` is not rendered.
    expect(deep(host, '[part="headline"]')?.getAttribute('data-name')).toBe('title');
    expect(deepAll(host, '[part="field"]').map((f) => f.getAttribute('data-name'))).toEqual(['submitted_at', 'note', 'mosaica_id']);
    expect(deep(host, '[part="field"][data-name="mosaica_id"] [part="value"]')?.textContent).toBe('12345678901234567890');
    expect(deep(host, '[part="field"][data-name="submitted_at"] [part="value"]')?.textContent).toBe('14 November 2023, 22:13:20 UTC');
    expect(deep(host, 'slot[name="field-title"]')).not.toBeNull();
    expect(host.textContent).toContain('my link');
    expect(deep(host, '[part="field"][data-name="archive"]')).toBeNull();
  });

  it('with no title field, heads the card with the id, once, and shows every other field in the grid', async () => {
    const host = await mount('<mosaica-item-card></mosaica-item-card>');
    const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
    el.meta = META;
    el.item = {id: 42n, detail: {fields: {note: 'x', title: 'A title'}, labels: [], views: [], scoped: {}}};
    await settle(host);
    expect(deep(host, '[part="headline"]')?.textContent).toBe('42');
    expect(deepAll(host, '[part="field"]').map((f) => f.getAttribute('data-name'))).toEqual(['title', 'note']);
  });

  it('fires mosaica-open with the id as a decimal string, bubbling and composed', async () => {
    const host = await mount('<mosaica-item-card></mosaica-item-card>');
    const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
    el.open = () => {};
    el.item = {id: 2n ** 63n + 1n, detail: {fields: {}, labels: [], views: [], scoped: {}}};
    await settle(host);
    let detail: {id?: string} | null = null;
    document.body.addEventListener('mosaica-open', (e) => (detail = (e as CustomEvent).detail));
    (deep(host, '[part="open"]') as HTMLButtonElement).click();
    expect(detail!.id).toBe('9223372036854775809');
    expect(typeof detail!.id).toBe('string');
  });

  describe('image-fields', () => {
    const photo = {id: 9n, detail: {fields: {thumb: 'https://images.example.org/a.jpg', other: 'https://images.example.org/b.jpg', bad: 'javascript:alert(1)', local: '/thumbs/c.png', caption: 'A cat'}, labels: [], views: [], scoped: {}}};
    async function card(markup: string) {
      const host = await mount(markup);
      const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
      el.item = photo;
      await settle(host);
      return {host, image: (name: string) => deep(host, `[part="field"][data-name="${name}"] [part="image"]`) as HTMLImageElement | null};
    }

    it('draws each named field holding an http or https address as its image, without a referrer, and the rest as text', async () => {
      const {host, image} = await card('<mosaica-item-card image-fields="thumb, bad local"></mosaica-item-card>');
      const img = image('thumb')!;
      expect(img.getAttribute('src')).toBe('https://images.example.org/a.jpg');
      expect(img.getAttribute('referrerpolicy')).toBe('no-referrer');
      expect(img.getAttribute('loading')).toBe('lazy');
      expect(image('other')).toBeNull();
      expect(image('bad')).toBeNull();
      expect(image('caption')).toBeNull();
      expect(image('local')!.getAttribute('src')).toBe(new URL('/thumbs/c.png', document.baseURI).href);
      expect(deep(host, '[part="field"][data-name="bad"] [part="value"]')!.textContent).toBe('javascript:alert(1)');
    });

    it('shows an image that does not load as its address', async () => {
      const {host, image} = await card('<mosaica-item-card image-fields="thumb"></mosaica-item-card>');
      image('thumb')!.dispatchEvent(new Event('error'));
      await settle(host);
      expect(image('thumb')).toBeNull();
      expect(deep(host, '[part="field"][data-name="thumb"] [part="value"]')!.textContent).toBe('https://images.example.org/a.jpg');
    });

    it('draws images on the compact card for the fields it shows', async () => {
      const {image} = await card('<mosaica-item-card compact card-fields="thumb" image-fields="thumb"></mosaica-item-card>');
      expect(image('thumb')!.getAttribute('src')).toBe('https://images.example.org/a.jpg');
    });
  });

  describe('Open', () => {
    const geonames = {id: 7n, detail: {fields: {geonameid: 2643743, name: 'St. Mary’s & Co/Ltd?', link: 'javascript:alert(1)'}, labels: [], views: [], scoped: {}}};
    async function card(markup: string, set: (el: MosaicaItemCard) => void = () => {}) {
      const host = await mount(markup);
      const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
      set(el);
      el.item = geonames;
      await settle(host);
      return {host, el, open: () => deep(host, '[part="open"]') as HTMLElement | null};
    }

    it('is hidden where neither open-url nor open is set, on the full and the compact card', async () => {
      expect((await card('<mosaica-item-card></mosaica-item-card>')).open()).toBeNull();
      expect((await card('<mosaica-item-card compact></mosaica-item-card>')).open()).toBeNull();
    });

    it('links to open-url filled from the item, each value URL-encoded, in a new tab', async () => {
      const {open} = await card('<mosaica-item-card compact open-url="https://www.geonames.org/{geonameid}?q={name}&id={mosaica_id}"></mosaica-item-card>');
      const a = open() as HTMLAnchorElement;
      expect(a.getAttribute('href')).toBe('https://www.geonames.org/2643743?q=St.%20Mary%E2%80%99s%20%26%20Co%2FLtd%3F&id=7');
      expect(a.getAttribute('target')).toBe('_blank');
      expect(a.getAttribute('rel')).toContain('noopener');
    });

    it('is hidden for an item with no value, or an empty one, for a field the template names, and for a name only a built-in answers', async () => {
      expect((await card('<mosaica-item-card open-url="https://example.org/{wikidata}"></mosaica-item-card>')).open()).toBeNull();
      expect((await card('<mosaica-item-card open-url="https://example.org/{constructor}/{toString}"></mosaica-item-card>')).open()).toBeNull();
      const blank = await card('<mosaica-item-card open-url="https://example.org/{note}"></mosaica-item-card>');
      blank.el.item = {...geonames, detail: {...geonames.detail, fields: {note: ''}}};
      await settle(blank.host);
      expect(blank.open()).toBeNull();
    });

    it('is hidden where a value cannot be URL-encoded, and the card still renders', async () => {
      const {host, el, open} = await card('<mosaica-item-card open-url="https://example.org/{note}"></mosaica-item-card>');
      el.item = {...geonames, detail: {...geonames.detail, fields: {note: 'a\uD800b'}}};
      await settle(host);
      expect(open()).toBeNull();
      expect(deep(host, '[part="field"][data-name="note"]')).not.toBeNull();
    });

    it('fires mosaica-open on a middle-click of the link', async () => {
      const {host, open} = await card('<mosaica-item-card open-url="https://www.geonames.org/{geonameid}"></mosaica-item-card>');
      let fired = 0;
      host.addEventListener('mosaica-open', () => (fired += 1));
      open()!.dispatchEvent(new MouseEvent('auxclick', {button: 1, bubbles: true}));
      open()!.dispatchEvent(new MouseEvent('auxclick', {button: 2, bubbles: true}));
      expect(fired).toBe(1);
    });

    it('never links to an address that is not http or https, whatever a value holds', async () => {
      // A value cannot supply the scheme: it is encoded, so the address stays on the page's scheme or is refused.
      const fromValue = (await card('<mosaica-item-card open-url="{link}"></mosaica-item-card>')).open();
      expect(new URL(fromValue!.getAttribute('href')!).protocol).toMatch(/^https?:$/);
      expect((await card('<mosaica-item-card open-url="javascript:void({name})"></mosaica-item-card>')).open()).toBeNull();
      expect((await card('<mosaica-item-card open-url="data:text/html,{name}"></mosaica-item-card>')).open()).toBeNull();
    });

    it('calls open with the id and fields, in place of open-url, and fires mosaica-open', async () => {
      const calls: unknown[] = [];
      const {host, open} = await card('<mosaica-item-card open-url="https://www.geonames.org/{geonameid}"></mosaica-item-card>', (el) => (el.open = (item) => calls.push(item)));
      let fired = 0;
      host.addEventListener('mosaica-open', () => (fired += 1));
      const button = open()!;
      expect(button.tagName).toBe('BUTTON');
      button.click();
      expect(calls).toEqual([{id: '7', fields: geonames.detail.fields}]);
      expect(fired).toBe(1);
    });
  });

  it('distinguishes a miss from a broken pick from a refusal', async () => {
    const host = await mount('<mosaica-item-card></mosaica-item-card>');
    const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
    el.pick = {kind: 'miss'};
    await settle(host);
    expect(deep(host, '[part="state"]')?.getAttribute('data-state')).toBe('empty');

    el.pick = {kind: 'broken', index: 7, layer: 'mosaica-marks-p1', hasIds: false, idCount: 0};
    await settle(host);
    expect(deep(host, '[part="state"]')?.getAttribute('data-state')).toBe('refused');
    expect(deep(host, '[part="refusal"]')).not.toBeNull();

    el.pick = null;
    el.refusal = {code: 'not-found', detail: 'nope', status: 404};
    await settle(host);
    expect(deep(host, '[part="state"]')?.getAttribute('data-state')).toBe('refused');
    expect(deep(host, '[part="refusal"]')).not.toBeNull();
  });

  it('reads the store’s selection when nothing is given by property', async () => {
    const host = await mount('<mosaica-item-card></mosaica-item-card>');
    const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
    const store = fakeStore({meta: META, status: status({})});
    el.store = store;
    await settle(host);
    store.set('selection', {item: {id: 5n, detail: {fields: {archive: 'cs'}, labels: [], views: [], scoped: {}}}, itemRefusal: null, artifact: null, artifactRefusal: null});
    await settle(host);
    expect(deep(host, '[part="field"][data-name="archive"] [part="value"]')?.textContent).toBe('cs');
  });

  it('shows a category value by the title the store gives, asking once for each value', async () => {
    const host = await mount('<mosaica-item-card></mosaica-item-card>');
    const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
    const store = fakeStore({meta: META, status: status({})});
    let answer: (title: string | null) => void = () => {};
    store.setTitles(() => new Promise((resolve) => (answer = resolve)));
    el.store = store;
    store.set('selection', {item: {id: 5n, detail: {fields: {archive: 'cs', title: 'A title'}, labels: [], views: [], scoped: {}}}, itemRefusal: null, artifact: null, artifactRefusal: null});
    await settle(host);
    const shown = () => deep(host, '[part="field"][data-name="archive"] [part="value"]')?.textContent;
    expect(shown()).toBe('cs');
    answer('Computer science');
    await settle(host);
    expect(shown()).toBe('Computer science');
    el.requestUpdate();
    await settle(host);
    expect(store.calls.filter((c) => c.name === 'titleOf').map((c) => c.args)).toEqual([['archive', 'cs']]);
  });

  it('drops the titles when the store forgets what the server answered, whether or not the card was attached then', async () => {
    for (const attached of [true, false]) {
      const host = await mount('<mosaica-item-card></mosaica-item-card>');
      const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
      const store = fakeStore({meta: META, status: status({})});
      store.setTitles(() => 'Computer science');
      el.store = store;
      store.set('selection', {item: {id: 5n, detail: {fields: {archive: 'cs'}, labels: [], views: [], scoped: {}}}, itemRefusal: null, artifact: null, artifactRefusal: null});
      await settle(host);
      const shown = () => deep(host, '[part="field"][data-name="archive"] [part="value"]')?.textContent;
      expect(shown()).toBe('Computer science');
      if (!attached) el.remove();
      // The store forgets, as on a renewal under other terms, and answers `meta` again; under it the value has no title.
      store.setTitles(() => null);
      store.set('meta', null);
      store.set('meta', {...META});
      if (!attached) host.append(el);
      await settle(host);
      expect(shown()).toBe('cs');
      expect(store.calls.filter((c) => c.name === 'titleOf').length).toBe(2);
      document.body.innerHTML = '';
    }
  });

  it('asks again for a title the store did not give, as after a refusal, at the next render', async () => {
    const host = await mount('<mosaica-item-card></mosaica-item-card>');
    const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
    const store = fakeStore({meta: META, status: status({})});
    store.setTitles(() => null);
    el.store = store;
    store.set('selection', {item: {id: 5n, detail: {fields: {archive: 'cs'}, labels: [], views: [], scoped: {}}}, itemRefusal: null, artifact: null, artifactRefusal: null});
    await settle(host);
    const shown = () => deep(host, '[part="field"][data-name="archive"] [part="value"]')?.textContent;
    expect(shown()).toBe('cs');
    store.setTitles(() => 'Computer science');
    el.requestUpdate();
    await settle(host);
    await settle(host);
    expect(shown()).toBe('Computer science');
  });

  it('shows no answer asked for under a `meta` the store has since replaced', async () => {
    const host = await mount('<mosaica-item-card></mosaica-item-card>');
    const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
    const store = fakeStore({meta: META, status: status({})});
    const answers: ((title: string | null) => void)[] = [];
    store.setTitles(() => new Promise((resolve) => answers.push(resolve)));
    el.store = store;
    store.set('selection', {item: {id: 5n, detail: {fields: {archive: 'cs'}, labels: [], views: [], scoped: {}}}, itemRefusal: null, artifact: null, artifactRefusal: null});
    await settle(host);
    store.set('meta', {...META});
    await settle(host);
    answers[0]!('Computer science');
    await settle(host);
    expect(deep(host, '[part="field"][data-name="archive"] [part="value"]')?.textContent).toBe('cs');
  });

  it('under `compact`, shows the fields `card-fields` names in its order, skipping a name the record lacks, with the rest under Show all', async () => {
    const host = await mount('<mosaica-item-card compact title-field="title" card-fields="note nope submitted_at"></mosaica-item-card>');
    const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
    el.meta = META;
    el.item = {id: 9n, detail: {fields: {archive: 'cs', submitted_at: 1_700_000_000_000_000, title: 'T', note: 'n', extra: 'e'}, labels: [], views: [], scoped: {}}};
    await settle(host);
    const names = () => deepAll(host, '[part="field"]').map((f) => f.getAttribute('data-name'));
    expect(names()).toEqual(['note', 'submitted_at']);
    expect(deep(host, '[part="show-all"]')?.textContent).toBe('Show all 5 fields');
    (deep(host, '[part="show-all"]') as HTMLButtonElement).click();
    await settle(host);
    expect(names()).toEqual(['note', 'submitted_at', 'archive', 'extra', 'mosaica_id']);
    // Unset, the first three in declaration order.
    el.cardFields = '';
    (deep(host, '[part="show-all"]') as HTMLButtonElement).click();
    await settle(host);
    expect(names()).toEqual(['archive', 'submitted_at', 'note']);
  });
});

/** A filters projection over `draft`, with whatever else `over` names. */
const filtersOf = (draft: FilterDraft, over: Partial<FiltersProjection> = {}): FiltersProjection => ({
  draft,
  expr: null,
  highlight: null,
  members: [],
  regions: [],
  suggestions: {},
  suggestErrors: {},
  suggestEpoch: 0,
  ...over
});

const value = (code: number, key: string, title: string | null, count?: number) => ({code, key, title, match: {field: 'key' as const, start: 0, len: 0}, ...(count === undefined ? {} : {count})});

async function mountFilter(column: string, filters: FiltersProjection, attrs = '') {
  const host = await mount(`<mosaica-filter column="${column}" ${attrs}></mosaica-filter>`);
  const el = host.querySelector('mosaica-filter') as MosaicaFilter;
  const store = fakeStore({meta: META, status: status({}), filters});
  el.store = store;
  await settle(host);
  return {host, el, store};
}

/** Type into the control's box, as a viewer does. */
async function type(host: HTMLElement, text: string) {
  const entry = deep(host, '[part="entry"]') as HTMLInputElement;
  entry.value = text;
  entry.dispatchEvent(new Event('input'));
  await settle(host);
  return entry;
}

const drafts = (store: ReturnType<typeof fakeStore>) => store.calls.filter((c) => c.name === 'setFilters').map((c) => c.args[0] as FilterDraft);

describe('<mosaica-filter> on a category', () => {
  const empty = () => filtersOf({filter: {archive: {family: 'category', keys: []}}, highlight: {}});

  it('asks for nothing and lists nothing until something is typed', async () => {
    const {host, store} = await mountFilter('archive', empty());
    expect(store.calls.filter((c) => c.name === 'suggest')).toHaveLength(0);
    expect(deep(host, '[part="values"]')).toBeNull();
    await type(host, 'ma');
    expect(store.calls.filter((c) => c.name === 'suggest').map((c) => c.args)).toEqual([['archive', 'ma', 'filter']]);
  });

  it('chooses the suggestion the arrow keys reach with Enter, and nothing before they reach one or for text that suggests nothing', async () => {
    const {host, store, el} = await mountFilter('archive', empty());
    const entry = await type(host, 'zz.unlisted');
    const key = async (k: string) => {
      entry.dispatchEvent(new KeyboardEvent('keydown', {key: k}));
      await settle(host);
    };
    await key('Enter');
    expect(drafts(store)).toHaveLength(0);
    await type(host, 'c');
    store.set('filters', {...store.get('filters'), suggestions: {archive: {q: 'c', verb: 'filter', values: [value(1, 'cs', 'CS'), value(2, 'cond', null), value(3, 'chem', null)], more: false, total: null}}});
    await settle(host);
    // A page can land between the last keystroke and Enter, so Enter takes no row the keys did not reach.
    await key('Enter');
    expect(drafts(store)).toHaveLength(0);
    expect(entry.hasAttribute('aria-activedescendant')).toBe(false);
    await key('ArrowDown');
    await key('Enter');
    expect(drafts(store).at(-1)!.filter['archive']).toEqual({family: 'category', keys: ['cs']});
    await key('ArrowDown');
    await key('ArrowDown');
    expect(el.shadowRoot!.querySelector('[part~="tick"][data-active]')!.id).toBe(entry.getAttribute('aria-activedescendant'));
    await key('Enter');
    expect(drafts(store).at(-1)!.filter['archive']).toEqual({family: 'category', keys: ['cs', 'chem']});
    // Past the last it wraps to the first, and Enter on a chosen value takes it out.
    await key('ArrowDown');
    await key('Enter');
    expect(drafts(store).at(-1)!.filter['archive']).toEqual({family: 'category', keys: ['chem']});
  });

  it('renders only the page that answers the box in front of it, and chooses a suggestion', async () => {
    const {host, store} = await mountFilter('archive', empty());
    await type(host, 'mach');
    expect(store.calls.filter((c) => c.name === 'suggest').at(-1)!.args).toEqual(['archive', 'mach', 'filter']);
    // A page for a different `q` (an earlier keystroke, landed late) is not shown for the box.
    store.set('filters', {...store.get('filters'), suggestions: {archive: {q: 'mac', verb: 'filter', values: [value(9, 'stat.ML', 'Machine Learning (Statistics)')], more: false, total: null}}});
    await settle(host);
    expect(deepAll(host, '[part~="tick"]')).toHaveLength(0);
    store.set('filters', {
      ...store.get('filters'),
      suggestions: {archive: {q: 'mach', verb: 'filter', values: [{code: 41207, key: 'cs.LG', title: 'Machine Learning', match: {field: 'title', start: 0, len: 4}}], more: true, total: null}}
    });
    await settle(host);
    const ticks = deepAll(host, '[part~="tick"]');
    expect(ticks).toHaveLength(1);
    expect(deep(host, '[part~="tick"] mark')?.textContent).toBe('Mach');
    expect(deep(host, '[part="more"]')).not.toBeNull();
    (ticks[0] as HTMLElement).click();
    expect((drafts(store)[0]!.filter['archive'] as {keys: string[]}).keys).toEqual(['cs.LG']);
  });

  it('shows each value’s count and its share of the total the server counted over', async () => {
    const {host, store} = await mountFilter('archive', empty());
    // The view's own totals are not what the counts are over.
    store.set('view', {...store.get('view'), visible: {value: 1000, exact: true}, matched: {value: 400, exact: true}});
    await type(host, 'c');
    store.set('filters', {...store.get('filters'), suggestions: {archive: {q: 'c', verb: 'filter', values: [value(1, 'cs', 'CS', 250), value(2, 'cond', null, 0)], more: false, total: 500}}});
    await settle(host);
    expect(deepAll(host, '[part="value-count"]').map((c) => c.textContent)).toEqual(['250', '0']);
    expect(deepAll(host, '[part="bar"]').map((b) => (b as HTMLElement).style.width)).toEqual(['50.0%', '0.0%']);
  });

  it('draws no bar where the page carries no total', async () => {
    const {host, store} = await mountFilter('archive', empty());
    await type(host, 'c');
    store.set('filters', {...store.get('filters'), suggestions: {archive: {q: 'c', verb: 'filter', values: [value(1, 'cs', 'CS', 250)], more: false, total: null}}});
    await settle(host);
    expect(deepAll(host, '[part="value-count"]').map((c) => c.textContent)).toEqual(['250']);
    expect(deepAll(host, '[part="bar"]')).toHaveLength(0);
  });

  it('has the store forget the column when the box is emptied and when the control goes away', async () => {
    const {host, store} = await mountFilter('archive', empty());
    const forgot = () => store.calls.filter((c) => c.name === 'forgetSuggestions').map((c) => c.args);
    await type(host, 'c');
    expect(forgot()).toEqual([]);
    await type(host, '');
    expect(forgot()).toEqual([['archive']]);
    await type(host, 'ma');
    host.querySelector('mosaica-filter')!.remove();
    expect(forgot()).toEqual([['archive'], ['archive']]);
  });

  it('shows a refusal beside the box, with its code, and keeps the box', async () => {
    const {host} = await mountFilter('archive', filtersOf({filter: {}, highlight: {}}, {suggestErrors: {archive: {code: 'vocabulary-withheld', detail: '', status: 403}}}));
    await type(host, 'c');
    expect(deep(host, '[part="entry"]')).not.toBeNull();
    expect(deep(host, '[part="refusal"]')?.getAttribute('data-code')).toBe('vocabulary-withheld');
  });

  it('asks again for what is typed once the store’s suggestEpoch moves', async () => {
    const {host, store} = await mountFilter('archive', empty());
    await type(host, 'ma');
    store.set('filters', {...store.get('filters'), suggestions: {}, suggestErrors: {}, suggestEpoch: 1});
    await settle(host);
    expect(store.calls.filter((c) => c.name === 'suggest').map((c) => c.args)).toEqual([
      ['archive', 'ma', 'filter'],
      ['archive', 'ma', 'filter']
    ]);
  });
});

describe('<mosaica-filter> on a category, for a keyboard and a screen reader', () => {
  const empty = () => filtersOf({filter: {archive: {family: 'category', keys: []}}, highlight: {}});
  const page = (store: ReturnType<typeof fakeStore>, q: string, values: ReturnType<typeof value>[], more = false) =>
    store.set('filters', {...store.get('filters'), suggestions: {archive: {q, verb: 'filter', values, more, total: null}}});
  const status = (host: HTMLElement) => deep(host, '[role="status"]')!;

  it('says in a status line, there before the list, how many values are listed and that none match', async () => {
    const {host, store} = await mountFilter('archive', empty());
    expect(status(host).textContent!.trim()).toBe('');
    await type(host, 'c');
    page(store, 'c', [value(1, 'cs', 'CS'), value(2, 'cond', null)]);
    await settle(host);
    expect(status(host).textContent).toContain('2');
    page(store, 'c', []);
    await settle(host);
    expect(status(host).querySelector('[part="more"]')!.textContent!.trim()).not.toBe('');
    expect(status(host).textContent).not.toContain('2');
  });

  it('says that the values are loading only once they have been awaited a while, and once however much is typed meanwhile', async () => {
    vi.useFakeTimers({toFake: ['setTimeout', 'clearTimeout']});
    const {host, store} = await mountFilter('archive', empty());
    const said = () => status(host).textContent!.trim();
    const pass = async (ms: number) => {
      await settle(host);
      vi.advanceTimersByTime(ms);
      await settle(host);
    };
    await type(host, 'c');
    // At once the list's place is drawn, and nothing is said.
    expect(status(host).querySelector('.skel')).not.toBeNull();
    expect(said()).toBe('');
    // An answer that lands soon is all that is said.
    await pass(100);
    page(store, 'c', [value(1, 'cs', 'CS')]);
    await pass(400);
    expect(said()).toContain('1');
    const listed = said();
    await type(host, 'co');
    expect(said()).toBe('');
    await pass(300);
    const loading = said();
    expect(loading).not.toBe('');
    expect(loading).not.toBe(listed);
    for (const text of ['con', 'cond']) {
      await type(host, text);
      await pass(300);
      expect(said()).toBe(loading);
    }
    page(store, 'cond', [value(2, 'cond', null)]);
    await pass(0);
    expect(said()).toBe(listed);
  });

  it('shows no message that nothing matches for a list that closed as focus left it', async () => {
    const {host, store} = await mountFilter('archive', empty());
    const entry = await type(host, 'c');
    page(store, 'c', [value(1, 'cs', 'CS')], true);
    await settle(host);
    expect(status(host).textContent!.trim()).not.toBe('');
    entry.dispatchEvent(new Event('blur'));
    await settle(host);
    expect(deep(host, '[part="values"]')).toBeNull();
    expect(status(host).textContent!.trim()).toBe('');
    // A search that matched nothing still says so.
    page(store, 'c', []);
    await settle(host);
    expect(status(host).textContent!.trim()).not.toBe('');
  });

  it('moves to the first and the last suggestion with Home and End once one is reached, leaving them to the text before, and scrolls the one reached into view', async () => {
    const {host, store} = await mountFilter('archive', empty());
    const entry = await type(host, 'c');
    page(store, 'c', [value(1, 'cs', 'CS'), value(2, 'cond', null), value(3, 'chem', null)]);
    await settle(host);
    const scrolled: string[] = [];
    vi.spyOn(Element.prototype, 'scrollIntoView').mockImplementation(function (this: Element) {
      scrolled.push(this.id);
    });
    const press = async (key: string) => {
      const event = new KeyboardEvent('keydown', {key, cancelable: true});
      entry.dispatchEvent(event);
      await settle(host);
      return event;
    };
    const active = () => deep(host, '[part~="tick"][data-active]')?.id ?? null;
    expect((await press('End')).defaultPrevented).toBe(false);
    expect(active()).toBeNull();
    await press('ArrowDown');
    expect(active()).toBe('value-1');
    expect((await press('End')).defaultPrevented).toBe(true);
    expect(active()).toBe('value-3');
    await press('Home');
    expect(active()).toBe('value-1');
    expect(scrolled).toEqual(['value-1', 'value-3', 'value-1']);
    vi.restoreAllMocks();
  });

  it('is a list autocomplete whose options toggle, and as a picker one whose options do not', async () => {
    const {host, store, el} = await mountFilter('archive', empty());
    const entry = await type(host, 'c');
    expect(entry.getAttribute('aria-autocomplete')).toBe('list');
    page(store, 'c', [value(1, 'cs', 'CS')]);
    await settle(host);
    expect(deep(host, '[role="listbox"]')!.getAttribute('aria-multiselectable')).toBe('true');
    el.choose = () => {};
    await settle(host);
    expect(deep(host, '[role="listbox"]')!.hasAttribute('aria-multiselectable')).toBe(false);
  });
});

describe('<mosaica-filter> names its box', () => {
  const named = (host: HTMLElement) => deep(host, '[part="entry"]')!.getAttribute('aria-label')!;
  const shown = (host: HTMLElement) => deep(host, '[part="entry"]')!.getAttribute('placeholder')!;

  it('by its column and the placeholder it shows, its own or the one it is given', async () => {
    const category = await mountFilter('archive', filtersOf({filter: {archive: {family: 'category', keys: []}}, highlight: {}}));
    expect(named(category.host)).toContain('Archive');
    expect(named(category.host)).toContain(shown(category.host));
    document.body.innerHTML = '';
    const text = await mountFilter('title', filtersOf({filter: {title: {family: 'text', query: '', phrase: true}}, highlight: {}}));
    expect(named(text.host)).toContain('Title');
    expect(named(text.host)).toContain(shown(text.host));
    document.body.innerHTML = '';
    const given = await mountFilter('archive', filtersOf({filter: {archive: {family: 'category', keys: []}}, highlight: {}}), 'placeholder="Search 1,234 values"');
    expect(named(given.host)).toContain('Archive');
    expect(named(given.host)).toContain('Search 1,234 values');
  });
});

describe('<mosaica-filter> on a category, its suggestions', () => {
  it('show while the box has focus and close as focus leaves it', async () => {
    const {host, store} = await mountFilter('archive', filtersOf({filter: {archive: {family: 'category', keys: []}}, highlight: {}}));
    const entry = await type(host, 'c');
    store.set('filters', {...store.get('filters'), suggestions: {archive: {q: 'c', verb: 'filter', values: [value(1, 'cs', 'CS')], more: false, total: null}}});
    await settle(host);
    expect(deepAll(host, '[part~="tick"]')).toHaveLength(1);
    entry.dispatchEvent(new Event('blur'));
    await settle(host);
    expect(deep(host, '[part="values"]')).toBeNull();
    entry.dispatchEvent(new Event('focus'));
    await settle(host);
    expect(deepAll(host, '[part~="tick"]')).toHaveLength(1);
  });
});

describe('<mosaica-filter> on text', () => {
  it('sends one box’s words, phrases and alternatives after the typing pause', async () => {
    vi.useFakeTimers();
    const {host, store} = await mountFilter('title', filtersOf({filter: {title: {family: 'text', query: '', phrase: true}}, highlight: {}}));
    const changes: unknown[] = [];
    host.addEventListener('mosaica-filterchange', (e) => changes.push((e as CustomEvent).detail));
    await type(host, '"graph neural" OR lattice');
    expect(drafts(store)).toHaveLength(0);
    await vi.runAllTimersAsync();
    expect(drafts(store)[0]!.filter['title']).toEqual({family: 'text', query: '"graph neural" OR lattice', phrase: true});
    expect(changes).toEqual([{column: 'title', verb: 'filter', expr: {any_of: [{title: {phrase: 'graph neural'}}, {title: {match: 'lattice'}}]}}]);
  });

  it('shows a clause set from outside that no query writes read-only', async () => {
    const expr = {title: {match: 'salt OR pepper'}};
    const {host} = await mountFilter('title', filtersOf({filter: {title: {family: 'text', query: '', phrase: true, expr}}, highlight: {}}));
    const entry = deep(host, '[part="entry"]') as HTMLInputElement;
    expect(entry.readOnly).toBe(true);
    expect(entry.value).toBe(JSON.stringify(expr));
  });

  it('shows a spinner in the box that started a query, once it has run a quarter of a second, and in no other box', async () => {
    vi.useFakeTimers({toFake: ['setTimeout', 'clearTimeout']});
    const host = await mount('<mosaica-filter id="asked" column="title"></mosaica-filter><mosaica-filter id="other" column="title"></mosaica-filter>');
    const store = fakeStore({meta: META, status: status({}), filters: filtersOf({filter: {title: {family: 'text', query: '', phrase: true}}, highlight: {}})});
    for (const el of host.querySelectorAll('mosaica-filter')) (el as MosaicaFilter).store = store;
    await settle(host);
    const box = (id: string) => host.querySelector(`#${id}`)!.shadowRoot!;
    const spinning = (id: string) => box(id).querySelector('[part="spinner"]') !== null;
    const pass = async (ms: number) => {
      await settle(host);
      vi.advanceTimersByTime(ms);
      await settle(host);
    };

    // The store says a query is running as soon as the box's change is set.
    store.set('status', status({status: 'loading', querying: true}));
    const entry = box('asked').querySelector('[part="entry"]') as HTMLInputElement;
    entry.value = 'lattice';
    entry.dispatchEvent(new Event('input'));
    await pass(400);
    expect(drafts(store)).toHaveLength(1);
    await pass(200);
    expect(spinning('asked')).toBe(false);

    await pass(100);
    expect(spinning('asked')).toBe(true);
    expect(spinning('other')).toBe(false);
    // The status strip is what says a query is running; the spinner is drawn and not read.
    expect(box('asked').querySelector('[part="spinner"]')!.getAttribute('aria-hidden')).toBe('true');
    expect(box('asked').querySelector('[part="spinner"]')!.hasAttribute('role')).toBe(false);

    store.set('status', status({}));
    await settle(host);
    expect(spinning('asked')).toBe(false);
    // The next query is another control's.
    store.set('status', status({status: 'loading', querying: true}));
    await pass(300);
    expect(spinning('asked')).toBe(false);
  });

  it('drops what was typed and not yet sent when the draft is replaced underneath, as a cancelled query replaces it', async () => {
    vi.useFakeTimers({toFake: ['setTimeout', 'clearTimeout']});
    const loaded = filtersOf({filter: {title: {family: 'text', query: 'lattice', phrase: true}}, highlight: {}});
    const {host, store} = await mountFilter('title', loaded);
    const entry = await type(host, 'lattice gauge');
    vi.advanceTimersByTime(400);
    await settle(host);
    expect(drafts(store)).toHaveLength(1);
    store.set('filters', filtersOf(drafts(store)[0]!));
    await type(host, 'lattice gauge theory');

    // The query is cancelled before the typing pause ends.
    store.set('filters', loaded);
    await settle(host);
    vi.advanceTimersByTime(400);
    await settle(host);
    expect(drafts(store)).toHaveLength(1);
    expect(entry.value).toBe('lattice');
  });

  it('empties when the draft no longer holds the control the box sent', async () => {
    vi.useFakeTimers({toFake: ['setTimeout', 'clearTimeout']});
    const {host, store} = await mountFilter('title', filtersOf({filter: {title: {family: 'text', query: '', phrase: true}}, highlight: {}}));
    const entry = await type(host, 'lattice');
    vi.advanceTimersByTime(400);
    await settle(host);
    store.set('filters', filtersOf(drafts(store)[0]!));
    await settle(host);
    expect(entry.value).toBe('lattice');

    store.set('filters', filtersOf({filter: {}, highlight: {}}));
    await settle(host);
    expect((deep(host, '[part="entry"]') as HTMLInputElement).value).toBe('');
  });
});

describe('<mosaica-filter> on a keyword column', () => {
  const meta: Meta = {...META, filterOperands: [{column: 'author', family: 'keyword', operands: ['eq', 'prefix']}]};

  it('offers only the operators the column publishes, and sends the draft’s operator', async () => {
    vi.useFakeTimers();
    const host = await mount('<mosaica-filter column="author"></mosaica-filter>');
    const el = host.querySelector('mosaica-filter') as MosaicaFilter;
    const store = fakeStore({meta, status: status({})});
    store.set('filters', filtersOf({filter: {author: {family: 'keyword', needle: '', op: 'eq'}}, highlight: {}}));
    el.store = store;
    await settle(host);
    const button = deep(host, '[part="mode"]') as HTMLButtonElement;
    expect(button.textContent?.trim()).toBe('is');
    button.click();
    await settle(host);
    const operators = () => deepAll(host, '[part~="operator"]');
    expect(operators().map((o) => [o.getAttribute('data-op'), o.getAttribute('aria-checked')])).toEqual([
      ['eq', 'true'],
      ['prefix', 'false']
    ]);
    button.click();
    await settle(host);
    expect(operators()).toHaveLength(0);
    await type(host, 'Knuth');
    await vi.runAllTimersAsync();
    const sent = () => store.calls.filter((c) => c.name === 'setFilters').at(-1)!.args[0] as {filter: {author: {op: string; needle: string}}};
    expect(sent().filter.author).toMatchObject({op: 'eq', needle: 'Knuth'});
    // Choosing an operator closes the menu and sends it at once.
    (deep(host, '[part="mode"]') as HTMLButtonElement).click();
    await settle(host);
    (operators()[1] as HTMLButtonElement).click();
    await settle(host);
    expect(operators()).toHaveLength(0);
    expect(sent().filter.author).toMatchObject({op: 'prefix', needle: 'Knuth'});
  });
});

describe('<mosaica-filter-panel>', () => {
  async function mountPanel(draft: FilterDraft, attrs = '', over: Partial<FiltersProjection> = {}) {
    const host = await mount(`<mosaica-filter-panel ${attrs}></mosaica-filter-panel>`);
    const panel = host.querySelector('mosaica-filter-panel') as MosaicaFilterPanel;
    const store = fakeStore({meta: META, status: status({}), filters: filtersOf(draft, over)});
    panel.store = store;
    await settle(host);
    const sent = () => store.calls.filter((c) => c.name === 'setFilters').at(-1)!.args[0] as FilterDraft;
    const cards = () => deepAll(host, 'mosaica-field-card') as MosaicaFieldCard[];
    const fields = () => cards().map((c) => [c.field, !c.folded]);
    return {host, panel, store, sent, cards, fields};
  }
  const none = (): FilterDraft => ({filter: {archive: {family: 'category', keys: []}, title: {family: 'text', query: '', phrase: true}, submitted_at: {family: 'numeric', gte: null, lte: null}}, highlight: {}});

  it('lists no card until one is pinned, holds a clause or is added', async () => {
    const {host, fields} = await mountPanel(none());
    expect(fields()).toEqual([]);
    (deep(host, '[part="add"]') as HTMLButtonElement).click();
    await settle(host);
    expect(deepAll(host, '[part~="add-option"]').map((o) => o.getAttribute('data-column'))).toEqual(['archive', 'title', 'submitted_at']);
    const search = deep(host, '[part="add-search"]') as HTMLInputElement;
    search.value = 'sub';
    search.dispatchEvent(new Event('input'));
    await settle(host);
    (deep(host, '[part~="add-option"]') as HTMLButtonElement).click();
    await settle(host);
    expect(fields()).toEqual([['submitted_at', true]]);
    expect(deep(host, '[part="add-list"]')).toBeNull();
    expect(deep(host, '[part="add-note"]')!.textContent).toBe('2 more fields');
  });

  it('adds a card at the end of the list, so the cards shown do not move, and marks it Shown in Add field', async () => {
    const {host, fields} = await mountPanel(none());
    const add = async (column: string) => {
      (deep(host, '[part="add"]') as HTMLButtonElement).click();
      await settle(host);
      (deep(host, `[part~="add-option"][data-column="${column}"]`) as HTMLButtonElement).click();
      await settle(host);
    };
    await add('submitted_at');
    await add('archive');
    expect(fields().map(([c]) => c)).toEqual(['submitted_at', 'archive']);
    (deep(host, '[part="add"]') as HTMLButtonElement).click();
    await settle(host);
    const said = deepAll(host, '[part~="add-option"]').map((o) => [o.getAttribute('data-column'), o.querySelector('.kind')?.textContent ?? '']);
    expect(said).toEqual([
      ['archive', 'Shown'],
      ['title', ''],
      ['submitted_at', 'Shown']
    ]);
  });

  it('checks the listed fields in Add field, and choosing one takes its card off with its clause in both positions', async () => {
    const draft: FilterDraft = {filter: {...none().filter, archive: {family: 'category', keys: ['cs']}}, highlight: {archive: {family: 'category', keys: ['math']}}};
    const {host, store, sent, fields} = await mountPanel(draft);
    const changes: unknown[] = [];
    host.addEventListener('mosaica-filterchange', (e) => changes.push((e as CustomEvent).detail));
    const open = async () => {
      (deep(host, '[part="add"]') as HTMLButtonElement).click();
      await settle(host);
    };
    const options = () => deepAll(host, '[part~="add-option"]').map((o) => [o.getAttribute('data-column'), o.getAttribute('aria-checked')]);
    await open();
    (deep(host, '[part~="add-option"][data-column="title"]') as HTMLButtonElement).click();
    await settle(host);
    expect(fields()).toEqual([
      ['archive', true],
      ['title', true]
    ]);
    await open();
    expect(options()).toEqual([
      ['archive', 'true'],
      ['title', 'true'],
      ['submitted_at', 'false']
    ]);
    (deep(host, '[part~="add-option"][data-column="archive"]') as HTMLButtonElement).click();
    expect(sent()).toEqual({filter: {...draft.filter, archive: {family: 'category', keys: []}}, highlight: {archive: {family: 'category', keys: []}}});
    expect(changes).toEqual([
      {column: 'archive', verb: 'filter', expr: null},
      {column: 'archive', verb: 'highlight', expr: null}
    ]);
    store.set('filters', filtersOf(sent()));
    await settle(host);
    expect(fields()).toEqual([['title', true]]);
    // A field holding no clause comes off without a write.
    const writes = store.calls.filter((c) => c.name === 'setFilters').length;
    await open();
    (deep(host, '[part~="add-option"][data-column="title"]') as HTMLButtonElement).click();
    await settle(host);
    expect(fields()).toEqual([]);
    expect(store.calls.filter((c) => c.name === 'setFilters')).toHaveLength(writes);
  });

  it('adds the first unlisted match on Enter in the Add field search, and never takes a card off', async () => {
    const draft: FilterDraft = {filter: {...none().filter, archive: {family: 'category', keys: ['cs']}}, highlight: {}};
    const {host, store, fields} = await mountPanel(draft);
    const enter = async (text: string) => {
      (deep(host, '[part="add"]') as HTMLButtonElement).click();
      await settle(host);
      const search = deep(host, '[part="add-search"]') as HTMLInputElement;
      search.value = text;
      search.dispatchEvent(new Event('input'));
      await settle(host);
      search.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', bubbles: true, composed: true}));
      await settle(host);
    };
    // `archive` is listed and matches first; Enter passes over it to `title`.
    await enter('i');
    expect(fields()).toEqual([
      ['archive', true],
      ['title', true]
    ]);
    // Only listed fields match: Enter does nothing, and the clause stays.
    await enter('archive');
    expect(fields()).toEqual([
      ['archive', true],
      ['title', true]
    ]);
    expect(store.calls.filter((c) => c.name === 'setFilters')).toHaveLength(0);
  });

  it('puts the first match in the tab order again after the search changes', async () => {
    const {host, panel} = await mountPanel(none());
    (deep(host, '[part="add"]') as HTMLButtonElement).click();
    await settle(host);
    const search = deep(host, '[part="add-search"]') as HTMLInputElement;
    const press = (key: string) => panel.shadowRoot!.activeElement!.dispatchEvent(new KeyboardEvent('keydown', {key, bubbles: true, composed: true}));
    press('ArrowDown');
    press('End');
    await settle(host);
    const tabbable = () => deepAll(host, '[part~="add-option"]').filter((o) => o.getAttribute('tabindex') === '0').map((o) => o.getAttribute('data-column'));
    expect(tabbable()).toEqual(['submitted_at']);
    search.value = 't';
    search.dispatchEvent(new Event('input'));
    await settle(host);
    expect(tabbable()).toEqual(['title']);
  });

  it('keeps a pinned card listed when it is chosen in Add field', async () => {
    const {host, store, fields} = await mountPanel(none(), 'pinned="title"');
    (deep(host, '[part="add"]') as HTMLButtonElement).click();
    await settle(host);
    const option = deep(host, '[part~="add-option"][data-column="title"]') as HTMLButtonElement;
    expect(option.getAttribute('aria-checked')).toBe('true');
    expect(option.getAttribute('aria-disabled')).toBe('true');
    option.click();
    await settle(host);
    expect(fields()).toEqual([['title', true]]);
    expect(store.calls.filter((c) => c.name === 'setFilters')).toHaveLength(0);
  });

  it('moves through Add field with the arrow keys, Home and End, and closes on Escape', async () => {
    const {host, panel} = await mountPanel(none());
    (deep(host, '[part="add"]') as HTMLButtonElement).click();
    await settle(host);
    const focused = () => (panel.shadowRoot!.activeElement as HTMLElement | null)?.getAttribute('data-column') ?? panel.shadowRoot!.activeElement?.getAttribute('part');
    const press = (key: string) => panel.shadowRoot!.activeElement!.dispatchEvent(new KeyboardEvent('keydown', {key, bubbles: true, composed: true}));
    expect(focused()).toBe('add-search');
    press('ArrowDown');
    expect(focused()).toBe('archive');
    press('ArrowDown');
    expect(focused()).toBe('title');
    press('End');
    expect(focused()).toBe('submitted_at');
    press('Home');
    expect(focused()).toBe('archive');
    press('ArrowUp');
    expect(focused()).toBe('add-search');
    press('Escape');
    await settle(host);
    expect(deep(host, '[part="add-list"]')).toBeNull();
    expect(focused()).toBe('add');
  });

  it('closes Add field on a press outside it and as focus leaves it, and not on a press inside it', async () => {
    const {host, panel} = await mountPanel(none());
    const open = async () => {
      (deep(host, '[part="add"]') as HTMLButtonElement).click();
      await settle(host);
    };
    const list = () => deep(host, '[part="add-list"]');
    await open();
    deep(host, '[part="add-search"]')!.dispatchEvent(new PointerEvent('pointerdown', {bubbles: true, composed: true}));
    await settle(host);
    expect(list()).not.toBeNull();
    document.body.dispatchEvent(new PointerEvent('pointerdown', {bubbles: true, composed: true}));
    await settle(host);
    expect(list()).toBeNull();
    await open();
    const elsewhere = document.body.appendChild(document.createElement('button'));
    const adder = panel.shadowRoot!.querySelector('.adder')!;
    // Focus going to nothing, as on a press on the list's scrollbar, leaves it open.
    adder.dispatchEvent(new FocusEvent('focusout', {relatedTarget: null, bubbles: true, composed: true}));
    await settle(host);
    expect(list()).not.toBeNull();
    adder.dispatchEvent(new FocusEvent('focusout', {relatedTarget: elsewhere, bubbles: true, composed: true}));
    await settle(host);
    expect(list()).toBeNull();
  });

  it('says in a status line how many fields Add field lists as its search narrows them, and that none match', async () => {
    const {host} = await mountPanel(none());
    (deep(host, '[part="add"]') as HTMLButtonElement).click();
    await settle(host);
    const status = () => deep(host, '[part="add-list"] [role="status"]')!;
    expect(status().textContent).toContain('3');
    const search = deep(host, '[part="add-search"]') as HTMLInputElement;
    search.value = 'sub';
    search.dispatchEvent(new Event('input'));
    await settle(host);
    expect(status().textContent).toContain('1');
    search.value = 'nothing by this name';
    search.dispatchEvent(new Event('input'));
    await settle(host);
    expect(status().textContent!.trim()).not.toBe('');
    expect(status().textContent).not.toMatch(/\d/);
    expect(deepAll(host, '[part~="add-option"]')).toHaveLength(0);
  });

  it('keeps a card listed while its clause is emptied under the user', async () => {
    vi.useFakeTimers();
    const draft: FilterDraft = {filter: {...none().filter, title: {family: 'text', query: 'graph', phrase: true}}, highlight: {}};
    const {host, store, fields} = await mountPanel(draft);
    expect(fields()).toEqual([['title', true]]);
    await type(host, '');
    await vi.runAllTimersAsync();
    // The store publishes the emptied clause; the card the user is editing stays.
    store.set('filters', filtersOf(drafts(store).at(-1)!));
    await settle(host);
    expect(fields()).toEqual([['title', true]]);
  });

  it('lists the pinned cards in meta’s order, and folds one to a line from its own button', async () => {
    const {host, fields} = await mountPanel(none(), 'pinned="title archive"');
    expect(fields()).toEqual([
      ['archive', true],
      ['title', true]
    ]);
    (deep(host, 'mosaica-field-card[data-field="archive"]')!.shadowRoot!.querySelector('[part="fold"]') as HTMLButtonElement).click();
    await settle(host);
    expect(fields()).toEqual([
      ['archive', false],
      ['title', true]
    ]);
  });

  it('lists the card of the field the map is coloured by, which says what its colours mean', async () => {
    const {host, store, fields} = await mountPanel(none());
    store.set('legend', {...store.get('legend'), colourBy: 'submitted_at'});
    await settle(host);
    expect(fields()).toEqual([['submitted_at', true]]);
  });

  it('has no Filter / Highlight switch: a card’s row sets either clause', async () => {
    const {host} = await mountPanel(none(), 'pinned="archive"');
    expect(deep(host, '[part="mode"]')).toBeNull();
    expect(deepAll(host, '[role="radiogroup"]')).toHaveLength(0);
  });

  it('folds every card but one in the compact layout, and opening one folds the one open before', async () => {
    const {host, panel, fields} = await mountPanel(none(), 'pinned="title archive submitted_at" compact');
    expect(fields()).toEqual([
      ['archive', false],
      ['title', false],
      ['submitted_at', false]
    ]);
    const fold = (field: string) => deep(host, `mosaica-field-card[data-field="${field}"]`)!.shadowRoot!.querySelector('[part="fold"]') as HTMLButtonElement;
    expect(fold('archive').getAttribute('aria-expanded')).toBe('false');
    fold('title').click();
    await settle(host);
    expect(fields()).toEqual([
      ['archive', false],
      ['title', true],
      ['submitted_at', false]
    ]);
    fold('archive').click();
    await settle(host);
    expect(fields()).toEqual([
      ['archive', true],
      ['title', false],
      ['submitted_at', false]
    ]);
    panel.compact = false;
    await settle(host);
    expect(fields().every(([, open]) => open)).toBe(true);
  });

  describe('the subject row', () => {
    it('reads In view and the matching count in view, beside All matching from its own aggregate', async () => {
      const {host, store} = await mountPanel(none());
      expect(registered(store).get([...registered(store).keys()].find((k) => k.startsWith('fields-match'))!)).toEqual({groupings: [{count: true}]});
      store.set('view', {...store.get('view'), inView: {status: 'shown', visible: {value: 900, exact: true}, matched: {value: 206, exact: true}, highlighted: {value: 206, exact: true}, shown: 206}});
      answerAggregate(store, 'fields-match', aggregateEntry([{rows: [], total: 50_000}]));
      await settle(host);
      expect(deep(host, '[part="subject-name"]')!.textContent).toBe('In view');
      expect(deep(host, '[part="subject-count"]')!.textContent).toBe('206');
      expect(deep(host, '[part="all-count"]')!.textContent).toBe('50,000');
      expect(deep(host, '[part="clear-highlight"]')).toBeNull();
    });

    it('reads Highlighted while a highlight is set, and its × clears every highlight clause', async () => {
      const draft: FilterDraft = {filter: {...none().filter, archive: {family: 'category', keys: ['cs']}}, highlight: {submitted_at: {family: 'numeric', gte: 1, lte: 2}}};
      const {host, store, sent} = await mountPanel(draft, '', {members: [{layer: 'topics', artifact: 3n, outside: false, verb: 'highlight'}]});
      store.set('view', {...store.get('view'), inView: {status: 'shown', visible: {value: 900, exact: true}, matched: {value: 500, exact: true}, highlighted: {value: 43, exact: true}, shown: 43}});
      await settle(host);
      expect(deep(host, '[part="subject-name"]')!.textContent).toBe('Highlighted');
      expect(deep(host, '[part="subject-count"]')!.textContent).toBe('43');
      const changes: unknown[] = [];
      host.addEventListener('mosaica-filterchange', (e) => changes.push((e as CustomEvent).detail));
      (deep(host, '[part="clear-highlight"]') as HTMLButtonElement).click();
      expect(activeCount(sent(), 'highlight')).toBe(0);
      expect(activeCount(sent(), 'filter')).toBe(1);
      expect(store.calls.filter((c) => c.name === 'setMembers').at(-1)!.args[0]).toEqual([]);
      expect(changes).toEqual([{column: null, verb: 'highlight', expr: null}]);
    });
  });

  describe('a column both filtered and highlighted', () => {
    const both: FilterDraft = {
      filter: {archive: {family: 'category', keys: ['cs.LG', 'cs.CV']}, title: {family: 'text', query: '', phrase: true}},
      highlight: {archive: {family: 'category', keys: ['cs.CV', 'stat.ML']}}
    };
    const chip = (host: HTMLElement, verb: string) => deep(host, `[part="chip"][data-column="archive"][data-verb="${verb}"]`)!;

    it('shows a chip per clause in one line, filter first, and × removes one and leaves the other', async () => {
      const {host, sent} = await mountPanel(both);
      expect(deepAll(host, '[part="chip"]').map((c) => c.getAttribute('data-verb'))).toEqual(['filter', 'highlight']);
      (chip(host, 'highlight').querySelector(':scope > button:last-child') as HTMLButtonElement).click();
      expect(sent()).toEqual({filter: both.filter, highlight: {archive: {family: 'category', keys: []}}});
      (chip(host, 'filter').querySelector(':scope > button:last-child') as HTMLButtonElement).click();
      expect(sent()).toEqual({filter: {...both.filter, archive: {family: 'category', keys: []}}, highlight: both.highlight});
    });

    it('opens and focuses a card on show()', async () => {
      const {host, panel, cards} = await mountPanel({...both, highlight: {}}, 'pinned="title"');
      (cards()[0]!.shadowRoot!.querySelector('[part="fold"]') as HTMLButtonElement).click();
      await settle(host);
      panel.show('title');
      await settle(host);
      const card = cards().find((c) => c.field === 'title')!;
      expect(card.folded).toBe(false);
      expect(panel.shadowRoot!.activeElement).toBe(card);
    });

    it('asks for the cards under chips-only, and show() lists the chip’s card', async () => {
      const {host, panel} = await mountPanel(both, 'chips-only');
      expect(deepAll(host, 'mosaica-field-card')).toHaveLength(0);
      const asked: unknown[] = [];
      host.addEventListener('mosaica-chipopen', (e) => asked.push((e as CustomEvent).detail));
      (chip(host, 'highlight').querySelector('[part="edit"]') as HTMLButtonElement).click();
      expect(asked).toEqual([{column: 'archive', verb: 'highlight'}]);
      panel.chipsOnly = false;
      panel.show('archive');
      await settle(host);
      expect(deepAll(host, 'mosaica-field-card').map((c) => (c as MosaicaFieldCard).field)).toContain('archive');
    });

    it('empties both positions on Clear all', async () => {
      const {host, sent} = await mountPanel(both);
      (deep(host, '[part="clear"]') as HTMLButtonElement).click();
      expect(activeCount(sent())).toBe(0);
      expect(sent().highlight).toEqual({});
    });
  });

  describe('removing a chip', () => {
    const three: FilterDraft = {
      filter: {archive: {family: 'category', keys: ['cs']}, title: {family: 'text', query: 'graph', phrase: true}},
      highlight: {archive: {family: 'category', keys: ['math']}}
    };
    const closes = (host: HTMLElement) => deepAll(host, '[part="chip"] > button:last-child') as HTMLButtonElement[];
    /** Press a control from the keyboard, and have the store publish what the press set, as the real one does. */
    const press = async (host: HTMLElement, store: ReturnType<typeof fakeStore>, button: HTMLButtonElement) => {
      const before = store.calls.length;
      button.focus();
      button.click();
      let filters = store.get('filters');
      for (const {name, args} of store.calls.slice(before)) {
        if (name === 'setFilters') filters = {...filters, draft: args[0] as FilterDraft};
        if (name === 'setMembers') filters = {...filters, members: args[0] as FiltersProjection['members']};
        if (name === 'setRegions') filters = {...filters, regions: args[0] as FiltersProjection['regions']};
      }
      store.set('filters', filters);
      await settle(host);
    };
    const focused = (panel: MosaicaFilterPanel) => panel.shadowRoot!.activeElement as HTMLElement | null;
    const chipOf = (el: HTMLElement | null) => {
      const chip = el?.closest('[part="chip"]');
      return chip ? [chip.getAttribute('data-verb'), chip.getAttribute('data-column') ?? chip.getAttribute('data-artifact')] : null;
    };

    it('moves focus to the × of the chip after it, else of the one before, else to Add field', async () => {
      const {host, panel, store} = await mountPanel(three);
      expect(closes(host).map((b) => chipOf(b))).toEqual([['filter', 'archive'], ['filter', 'title'], ['highlight', 'archive']]);
      await press(host, store, closes(host)[1]!);
      expect(closes(host)).toHaveLength(2);
      expect(focused(panel)).toBe(closes(host)[1]);
      expect(chipOf(focused(panel))).toEqual(['highlight', 'archive']);
      await press(host, store, closes(host)[1]!);
      expect(chipOf(focused(panel))).toEqual(['filter', 'archive']);
      await press(host, store, closes(host)[0]!);
      expect(closes(host)).toHaveLength(0);
      expect(focused(panel)!.getAttribute('part')).toBe('add');
    });

    it('moves focus along the chips of clusters as it does along those of columns', async () => {
      const clause = (id: bigint) => ({layer: 'topics', artifact: id, outside: false, verb: 'filter' as const, label: `topic ${id}`});
      const {host, panel, store} = await mountPanel({filter: {}, highlight: {}}, '', {members: [clause(1n), clause(2n)]});
      await press(host, store, closes(host)[0]!);
      expect(chipOf(focused(panel))).toEqual(['filter', '2']);
    });

    it('moves focus to Add field after Clear all', async () => {
      const {host, panel, store} = await mountPanel(three);
      await press(host, store, deep(host, '[part="clear"]') as HTMLButtonElement);
      expect(deep(host, '[part="chips"]')).toBeNull();
      expect(focused(panel)!.getAttribute('part')).toBe('add');
    });

    it('moves focus to Add field after the highlight is cleared from the subject row, and not onto a filter’s ×', async () => {
      const {host, panel, store} = await mountPanel(three);
      await press(host, store, deep(host, '[part="clear-highlight"]') as HTMLButtonElement);
      expect(deep(host, '[part="clear-highlight"]')).toBeNull();
      // The filters' chips stay, and a second press of the key removes none of them.
      expect(closes(host)).toHaveLength(2);
      expect(focused(panel)!.getAttribute('part')).toBe('add');
    });

    it('under chips-only, says that no filter is left once its last chip has gone, in words that take focus and go as focus leaves', async () => {
      const {host, panel, store} = await mountPanel({filter: {archive: {family: 'category', keys: ['cs']}}, highlight: {}}, 'chips-only');
      await press(host, store, closes(host)[0]!);
      expect(deepAll(host, '[part="chip"]')).toHaveLength(0);
      const words = focused(panel)!;
      expect(words.textContent!.trim()).not.toBe('');
      expect(words.closest('[part="chips"]')).not.toBeNull();
      // The panel itself, which draws nothing, is not what holds focus.
      expect(panel.hasAttribute('tabindex')).toBe(false);
      words.blur();
      await settle(host);
      expect(deepAll(host, '[part]')).toHaveLength(0);
    });

    it('under chips-only, drops those words when a filter is set again', async () => {
      const {host, panel, store} = await mountPanel({filter: {archive: {family: 'category', keys: ['cs']}}, highlight: {}}, 'chips-only');
      await press(host, store, closes(host)[0]!);
      expect(focused(panel)).not.toBeNull();
      store.set('filters', filtersOf({filter: {archive: {family: 'category', keys: ['math']}}, highlight: {}}));
      await settle(host);
      expect(deepAll(host, '[part="chip"]')).toHaveLength(1);
      await press(host, store, closes(host)[0]!);
      // Removed with focus elsewhere, the last chip leaves nothing behind.
      const elsewhere = document.body.appendChild(document.createElement('button'));
      store.set('filters', filtersOf({filter: {archive: {family: 'category', keys: ['stat']}}, highlight: {}}));
      await settle(host);
      elsewhere.focus();
      store.set('filters', filtersOf({filter: {}, highlight: {}}));
      await settle(host);
      expect(deepAll(host, '[part]')).toHaveLength(0);
    });

    it('leaves focus where the viewer has moved it', async () => {
      const {host, panel, store} = await mountPanel(three);
      const elsewhere = document.body.appendChild(document.createElement('button'));
      const close = closes(host)[1]!;
      close.focus();
      close.click();
      elsewhere.focus();
      store.set('filters', filtersOf(store.calls.filter((c) => c.name === 'setFilters').at(-1)!.args[0] as FilterDraft));
      await settle(host);
      expect(document.activeElement).toBe(elsewhere);
      expect(focused(panel)).toBeNull();
    });
  });

  it('writes the word on a highlight’s chip and none on a filter’s, and names each × by what it removes and its verb', async () => {
    const draft: FilterDraft = {filter: {archive: {family: 'category', keys: ['cs']}}, highlight: {archive: {family: 'category', keys: ['math']}}};
    const member = (id: bigint, verb: 'filter' | 'highlight') => ({layer: 'topics', artifact: id, outside: false, verb, label: `topic ${id}`});
    const {host} = await mountPanel(draft, '', {members: [member(1n, 'filter'), member(2n, 'highlight')]});
    const chips = deepAll(host, '[part="chip"]');
    expect(chips.map((c) => [c.getAttribute('data-verb'), c.querySelector('.verb')?.textContent ?? null])).toEqual([
      ['filter', null],
      ['highlight', 'highlight'],
      ['filter', null],
      ['highlight', 'highlight']
    ]);
    const names = chips.map((c) => c.querySelector(':scope > button:last-child')!.getAttribute('aria-label')!);
    expect(names.map((n) => /filter/i.test(n))).toEqual([true, false, true, false]);
    expect(names.map((n) => /highlight/i.test(n))).toEqual([false, true, false, true]);
    expect(names[0]).toContain('Archive');
    expect(names[2]).toContain('topic 1');
    expect(names[3]).toContain('topic 2');
  });

  it('names the values a clause on a bool keeps in its chip', async () => {
    const host = await mount('<mosaica-filter-panel></mosaica-filter-panel>');
    const panel = host.querySelector('mosaica-filter-panel') as MosaicaFilterPanel;
    const withBool = {...META, declaredScalars: [...META.declaredScalars, scalar('retracted', 'bool')], filterOperands: [...META.filterOperands, {column: 'retracted', family: 'numeric' as const, operands: ['eq', 'in', 'range']}]};
    const draft: FilterDraft = {filter: {retracted: {family: 'numeric', gte: 1, lte: 1}}, highlight: {retracted: {family: 'numeric', gte: 0, lte: 1}}};
    panel.store = fakeStore({meta: withBool, status: status({}), filters: filtersOf(draft)});
    await settle(host);
    expect(deepAll(host, '[part="chip"]').map((c) => c.querySelector('[part="edit"]')!.textContent!.replace(/\s+/g, ' ').trim())).toEqual(['Retracted: yes', 'Retracted: yes, no']);
  });

  it('leaves the subject row and the chips out under controls-only, and the cards out under chips-only', async () => {
    const draft: FilterDraft = {filter: {...none().filter, archive: {family: 'category', keys: ['cs']}}, highlight: {}};
    const controls = await mountPanel(draft, 'controls-only');
    expect(deepAll(controls.host, '[part="chip"]')).toHaveLength(0);
    expect(deep(controls.host, '[part="subject"]')).toBeNull();
    expect(deepAll(controls.host, 'mosaica-field-card')).toHaveLength(1);
    document.body.innerHTML = '';
    const chips = await mountPanel(draft, 'chips-only');
    expect(deepAll(chips.host, '[part="chip"]')).toHaveLength(1);
    expect(deepAll(chips.host, 'mosaica-field-card')).toHaveLength(0);
  });

  it('does not rebuild a card under the user on a store tick', async () => {
    const {store, cards, host} = await mountPanel({...none(), filter: {...none().filter, archive: {family: 'category', keys: ['cs']}}});
    const before = cards()[0];
    store.set('status', status({status: 'loading'}));
    await settle(host);
    expect(cards()[0]).toBe(before);
  });

  it('marks a member_of chip with no label and no served name as unnamed, never by its key', async () => {
    const {host} = await mountPanel({filter: {}, highlight: {}}, 'chips-only', {members: [{layer: 'clusters', artifact: 4n, outside: false, verb: 'filter'}]});
    const chip = deep(host, '[part="chip"][data-artifact="4"]')!;
    expect(chip.textContent).toContain(UNNAMED);
    expect(chip.textContent).not.toContain('clusters');
  });

  it('draws an artifact filtered and highlighted as two chips, and removes one of them', async () => {
    const clause = {layer: 'mesh/descriptors', artifact: 546_790n, outside: false};
    const {host, store} = await mountPanel({filter: {}, highlight: {}}, 'chips-only', {
      members: [
        {...clause, verb: 'highlight'},
        {...clause, verb: 'filter'}
      ]
    });
    const chips = deepAll(host, '[part="chip"][data-artifact="546790"]');
    expect(chips.map((c) => c.getAttribute('data-verb'))).toEqual(['filter', 'highlight']);
    (chips[0]!.querySelector('button') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'setMembers').at(-1)!.args[0]).toEqual([{...clause, verb: 'highlight'}]);
  });
});

describe('<mosaica-filter-panel> regions held in other views', () => {
  const TWO_VIEWS = {...META, views: [...META.views, {...META.views[0]!, id: 'topics', displayName: 'Topics', quantisation: {xMin: -5, xMax: 5, yMin: -5, yMax: 5}}]};
  const lasso = {kind: 'lasso' as const, points: [[0, 0], [1, 0], [0, 1]] as [number, number][]};

  async function mountHeld() {
    const host = await mount('<mosaica-filter-panel chips-only></mosaica-filter-panel>');
    const panel = host.querySelector('mosaica-filter-panel') as MosaicaFilterPanel;
    const store = fakeStore({meta: TWO_VIEWS, status: status({}), filters: filtersOf({filter: {}, highlight: {}}, {regions: [{view: 'topics', shape: lasso}]})});
    store.set('view', {...store.get('view'), id: 's0'});
    panel.store = store;
    await settle(host);
    const events: {type: string; detail: unknown}[] = [];
    for (const type of ['mosaica-selectchange', 'mosaica-viewswitch']) host.addEventListener(type, (e) => events.push({type, detail: (e as CustomEvent).detail}));
    return {host, store, events, chip: () => deep(host, '[part="chip"][data-view="topics"]')};
  }

  it('names the shape and the view it was drawn in, and × removes it alone', async () => {
    const {store, events, chip} = await mountHeld();
    expect(chip()!.getAttribute('data-verb')).toBe('filter');
    expect(chip()!.textContent).toContain('Lasso in Topics');
    expect(chip()!.querySelector(':scope > button:last-child')!.getAttribute('aria-label')).toContain('Lasso in Topics');
    expect(chip()!.querySelector(':scope > button:last-child')!.getAttribute('aria-label')).toMatch(/filter/i);
    (chip()!.querySelector(':scope > button:last-child') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'setRegions').at(-1)!.args[0]).toEqual([]);
    expect(events).toEqual([{type: 'mosaica-selectchange', detail: {shape: null, view: 'topics', status: 'cleared'}}]);
  });

  it('switches to the view when pressed', async () => {
    const {store, events, chip} = await mountHeld();
    (chip()!.querySelector('[part="edit"]') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'setCurrentView').map((c) => c.args[0])).toEqual(['topics']);
    expect(events).toEqual([{type: 'mosaica-viewswitch', detail: {from: 's0', to: 'topics', sameFrame: false}}]);
  });

  it('is removed by Clear all', async () => {
    const {host, store, events} = await mountHeld();
    (deep(host, '[part="clear"]') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'setRegions').at(-1)!.args[0]).toEqual([]);
    expect(events).toContainEqual({type: 'mosaica-selectchange', detail: {shape: null, view: 'topics', status: 'cleared'}});
  });
});

describe('<mosaica-filter-panel> cluster fields', () => {
  const layer = (name: string, over: Partial<Meta['layers'][number]> = {}) =>
    ({name, title: `${name} title`, views: ['s0'], membership: 'enumerated', hierarchy: {kind: 'nested', pruneChildren: false}, levels: [], computedContent: ['centroid'], shape: null, suppliedContent: ['name'], depsOn: [], version: 1, ...over}) as Meta['layers'][number];
  const LAYERED = {...META, layers: [layer('topics'), layer('labels', {depsOn: ['topics']}), layer('elsewhere', {views: ['s9']})]};
  const topic = (id: bigint, verb: 'filter' | 'highlight', label?: string) => ({layer: 'topics', artifact: id, outside: false, verb, ...(label ? {label} : {})});
  /** What a chip names, apart from the word that says it is a highlight. */
  const named = (chip: Element) => {
    const copy = chip.cloneNode(true) as Element;
    copy.querySelector('.verb')?.remove();
    return copy.textContent!.trim();
  };

  async function mountLayered(members: ReturnType<typeof topic>[] = []) {
    const host = await mount('<mosaica-filter-panel></mosaica-filter-panel>');
    const panel = host.querySelector('mosaica-filter-panel') as MosaicaFilterPanel;
    const store = fakeStore({meta: LAYERED, status: status({}), filters: filtersOf({filter: {}, highlight: {}}, {members})});
    store.set('view', {...store.get('view'), id: 's0'});
    panel.store = store;
    await settle(host);
    return {host, panel, store};
  }

  it('offers each layer of the view that attaches to no other in Add field, after the columns', async () => {
    const {host} = await mountLayered();
    (deep(host, '[part="add"]') as HTMLButtonElement).click();
    await settle(host);
    const options = deepAll(host, '[part~="add-option"]');
    expect(options.map((o) => o.getAttribute('data-column') ?? `layer:${o.getAttribute('data-layer')}`)).toEqual(['archive', 'title', 'submitted_at', 'layer:topics']);
    expect(options.at(-1)!.textContent).toContain('topics title');
    (options.at(-1) as HTMLButtonElement).click();
    await settle(host);
    expect(deep(host, 'mosaica-field-card[data-field="cluster:topics"]')).not.toBeNull();
  });

  it('lists a layer holding a clause, and shows its clauses as chips, the filter’s first', async () => {
    const {host} = await mountLayered([topic(8n, 'highlight', 'Optics'), topic(7n, 'filter', 'Neural networks'), topic(9n, 'filter')]);
    expect(deep(host, 'mosaica-field-card[data-field="cluster:topics"]')).not.toBeNull();
    const chips = deepAll(host, '[part="chip"]').map((c) => [c.getAttribute('data-verb'), named(c)]);
    expect(chips).toEqual([
      ['filter', 'Neural networks'],
      ['filter', UNNAMED],
      ['highlight', 'Optics']
    ]);
  });

  it('shows a clause on a layer with no card, in either position, as a chip that takes it off', async () => {
    const lit = {layer: 'labels', artifact: 3n, outside: false, verb: 'highlight' as const, label: 'diffusion, guidance'};
    const elsewhere = {layer: 'elsewhere', artifact: 4n, outside: true, verb: 'filter' as const, label: 'North'};
    const {host, store} = await mountLayered([topic(7n, 'filter', 'Neural networks'), lit as never, elsewhere as never]);
    const chip = (artifact: string) => deep(host, `[part="chip"][data-artifact="${artifact}"]`)!;
    expect(named(chip('3'))).toBe('diffusion, guidance');
    expect(named(chip('4'))).toBe('Outside North');
    (chip('3').querySelector('button') as HTMLButtonElement).click();
    expect(store.calls.filter((c) => c.name === 'setMembers').at(-1)!.args[0]).toEqual([topic(7n, 'filter', 'Neural networks'), elsewhere]);
  });

  it('takes a layer off with every clause on it, in both positions, when its checked entry is chosen again', async () => {
    const {host, store} = await mountLayered([topic(7n, 'filter'), topic(8n, 'highlight')]);
    (deep(host, '[part="add"]') as HTMLButtonElement).click();
    await settle(host);
    const entry = deep(host, '[part~="add-option"][data-layer="topics"]') as HTMLButtonElement;
    expect(entry.getAttribute('aria-checked')).toBe('true');
    entry.click();
    await settle(host);
    expect(store.calls.filter((c) => c.name === 'setMembers').at(-1)!.args[0]).toEqual([]);
  });
});

describe('<mosaica-filter-panel chips-only>', () => {
  it('renders nothing while no clause is applied, and the chips once one is', async () => {
    const host = await mount('<mosaica-filter-panel chips-only></mosaica-filter-panel>');
    const store = fakeStore({meta: META, status: status({})});
    (host.querySelector('mosaica-filter-panel') as unknown as {store: unknown}).store = store;
    await settle(host);
    expect(deepAll(host, '[part]')).toHaveLength(0);
    store.set('filters', filtersOf({filter: {archive: {family: 'category', keys: ['cs']}}, highlight: {}}));
    await settle(host);
    expect(deepAll(host, '[part="chip"]')).toHaveLength(1);
  });
});

describe('a date range as a chip says it', () => {
  const at = (y: number, m: number, d: number) => Date.UTC(y, m, d) * 1000;
  const end = (y: number, m: number, d: number) => Date.UTC(y, m, d + 1) * 1000 - 1;

  it('writes whole years, days within a year, days across years and open ends', () => {
    expect(dateRangeText(at(2019, 0, 1), end(2024, 11, 31))).toBe('2019 – 2024');
    expect(dateRangeText(at(2024, 0, 1), end(2024, 11, 31))).toBe('2024');
    expect(dateRangeText(at(2024, 2, 3), end(2024, 5, 14))).toBe('3 Mar – 14 Jun 2024');
    expect(dateRangeText(at(2019, 2, 3), end(2024, 5, 14))).toBe('3 Mar 2019 – 14 Jun 2024');
    expect(dateRangeText(at(2019, 2, 3), null)).toBe('from 3 Mar 2019');
    expect(dateRangeText(null, end(2024, 5, 14))).toBe('until 14 Jun 2024');
  });

  it('reads back what it writes', () => {
    expect(parseDateText('3 Mar 2019', false)).toBe(at(2019, 2, 3));
    expect(parseDateText('14 June 2024', true)).toBe(end(2024, 5, 14));
    expect(parseDateText('2024-06-14', false)).toBe(at(2024, 5, 14));
    expect(parseDateText('Jun 2024', true)).toBe(end(2024, 5, 30));
    expect(parseDateText('31 Feb 2024', false)).toBeNull();
    expect(parseDateText('soon', false)).toBeNull();
    // The word typed is the start of a month's name, not a word that starts with one.
    expect(parseDateText('Junk 2019', false)).toBeNull();
    expect(parseDateText('Sept 2019', false)).toBe(at(2019, 8, 1));
    // A year below 100 is that year, not one in the 1900s.
    expect(dateRangeText(parseDateText('0050', false), null)).toBe('from 1 Jan 50');
  });

  it('keeps the day of an instant before 1970', () => {
    expect(dateRangeText(null, end(1969, 5, 14))).toBe('until 14 Jun 1969');
    expect(dateRangeText(at(1950, 0, 1), end(1959, 11, 31))).toBe('1950 – 1959');
  });

  it('calls a range whole years only from the first instant of one to the last instant of another', () => {
    expect(dateRangeText(at(2019, 0, 1), at(2024, 11, 31))).toBe('1 Jan 2019 – 31 Dec 2024');
  });
});
