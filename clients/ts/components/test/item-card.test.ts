import {afterEach, describe, expect, it, vi} from 'vitest';
import '../src/item-card.js';
import type {MosaicaItemCard} from '../src/item-card.js';
import {deep, deepAll, fakeStore, meta, mount, scalar, settle, status} from './fake-store.js';

/**
 * `<mosaica-item-card>` as a keyboard and a screen reader meet it: its headings and its fields'
 * terms, the words that say what a control did, and text a clamp would otherwise hide.
 */

afterEach(() => {
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const META = meta({declaredScalars: [scalar('title', 'utf8'), scalar('abstract', 'utf8'), scalar('thumb', 'utf8')]});
const LONG = 'A sentence long enough to be prose, which the card keeps to a few lines until it is asked for whole.';
const paper = {id: 7n, detail: {fields: {title: 'A paper on maps', abstract: LONG, thumb: 'https://images.example.org/a.jpg'}, labels: [], views: [], scoped: {}}};
const selection = {item: null, itemRefusal: null, artifact: null, artifactRefusal: null};

async function card(attrs = '', item: typeof paper | null = paper) {
  const host = await mount(`<mosaica-item-card ${attrs}></mosaica-item-card>`);
  const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
  el.meta = META;
  el.item = item;
  await settle(host);
  return {host, el};
}

/** Give every element more text than its box holds, as a clamp does; happy-dom lays nothing out. */
function cutShort() {
  vi.spyOn(HTMLElement.prototype, 'scrollHeight', 'get').mockReturnValue(200);
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(100);
}

describe('<mosaica-item-card> structure', () => {
  it('is headed by a title, with the item’s headline a heading under it and no control inside either', async () => {
    const {host} = await card('title-field="title"');
    const title = deep(host, 'h2')!;
    expect(title.textContent?.trim()).not.toBe('');
    const headline = deep(host, '[part="headline"]')!;
    expect(headline.tagName).toBe('H3');
    expect(headline.textContent).toBe('A paper on maps');
    for (const heading of [title, headline]) expect(heading.querySelector('button')).toBeNull();
    expect(title.compareDocumentPosition(headline) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('gives each field as a term and its value, on the full card and the compact one', async () => {
    for (const attrs of ['title-field="title"', 'compact title-field="title"']) {
      const {host} = await card(attrs);
      const fields = deepAll(host, '[part="field"]');
      expect(fields.length).toBeGreaterThan(0);
      for (const field of fields) {
        expect(field.parentElement!.tagName, attrs).toBe('DL');
        expect(field.querySelector('[part="label"]')!.tagName, attrs).toBe('DT');
        expect(field.querySelector('[part="value"]')!.tagName, attrs).toBe('DD');
      }
      document.body.innerHTML = '';
    }
  });

  it('writes a whole number of four figures, such as a year, without a thousands mark, and a longer one with them', async () => {
    const host = await mount('<mosaica-item-card></mosaica-item-card>');
    const el = host.querySelector('mosaica-item-card') as MosaicaItemCard;
    el.meta = meta({declaredScalars: [scalar('year', 'i32'), scalar('population', 'i64')]});
    el.item = {id: 7n, detail: {fields: {year: 2012, population: 48_213}, labels: [], views: [], scoped: {}}};
    await settle(host);
    const value = (name: string) => deep(host, `[part="field"][data-name="${name}"] [part="value"]`)!.textContent!.trim();
    expect(value('year')).toBe('2012');
    expect(value('population')).toBe('48,213');
  });

  it('heads the group-scoped values of each key under the headline, as terms and values', async () => {
    const {host} = await card('title-field="title"', {...paper, detail: {...paper.detail, scoped: {mood: {'2026-Q2': 'calm'}}}} as typeof paper);
    const key = deep(host, '[part="key"]')!;
    expect(key.tagName).toBe('H4');
    expect(key.textContent).toBe('2026-Q2');
    const field = deep(host, '[part="scoped"] [part="field"]')!;
    expect(field.parentElement!.tagName).toBe('DL');
    expect([field.querySelector('dt')!.textContent, field.querySelector('dd')!.textContent]).toEqual(['Mood', 'calm']);
  });

  it('names an image by the item’s headline, and by its field where the item has no headline', async () => {
    const titled = await card('title-field="title" image-fields="thumb"');
    expect(deep(titled.host, '[part="image"]')!.getAttribute('alt')).toBe('A paper on maps');
    const untitled = await card('image-fields="thumb"');
    expect(deep(untitled.host, '[part="image"]')!.getAttribute('alt')).toBe('Thumb');
  });
});

describe('<mosaica-item-card> says what happened', () => {
  it('keeps one status region in the header through no item, a miss, an item and a refusal', async () => {
    const {host, el} = await card('', null);
    const region = deep(host, '[role="status"]')!;
    const state = () => region.querySelector('[part="state"]')!.getAttribute('data-state');
    const said = () => region.textContent?.trim();
    expect([state(), said() !== '']).toEqual(['detached', false]);

    el.pick = {kind: 'miss'};
    await settle(host);
    expect(deep(host, '[role="status"]')).toBe(region);
    expect([state(), said() !== '']).toEqual(['empty', true]);

    el.item = paper;
    await settle(host);
    expect(deep(host, '[role="status"]')).toBe(region);
    expect([state(), said() !== '']).toEqual(['shown', false]);

    el.item = null;
    el.pick = null;
    el.refusal = {code: 'not-found', detail: 'nope', status: 404};
    await settle(host);
    expect(deep(host, '[role="status"]')).toBe(region);
    expect(state()).toBe('refused');
    expect(region.querySelector('[part="refusal"]')!.getAttribute('data-code')).toBe('not-found');
  });

  it('says so for a store with nothing selected', async () => {
    const host = await mount('<mosaica-item-card></mosaica-item-card>');
    (host.querySelector('mosaica-item-card') as MosaicaItemCard).store = fakeStore({meta: META, status: status({}), selection});
    await settle(host);
    const region = deep(host, '[role="status"]')!;
    expect(region.querySelector('[part="state"]')!.getAttribute('data-state')).toBe('empty');
    expect(region.textContent?.trim()).not.toBe('');
  });

  it('says in a status region beside Copy id that the id was copied, or that it could not be', async () => {
    const {host} = await card('title-field="title"');
    const copy = deep(host, '[part="copy"]') as HTMLButtonElement;
    const region = copy.parentElement!.querySelector('[role="status"]')!;
    expect(region.textContent).toBe('');
    const written: string[] = [];
    vi.stubGlobal('navigator', {clipboard: {writeText: async (text: string) => void written.push(text)}});
    copy.click();
    await settle(host);
    await settle(host);
    expect(written).toEqual(['7']);
    const copied = region.textContent;
    expect(copied).not.toBe('');

    vi.stubGlobal('navigator', {clipboard: {writeText: () => Promise.reject(new Error('denied'))}});
    copy.click();
    await settle(host);
    await settle(host);
    expect(region.textContent).not.toBe('');
    expect(region.textContent).not.toBe(copied);

    // A page that is not secure has no clipboard at all.
    vi.stubGlobal('navigator', {});
    copy.click();
    await settle(host);
    await settle(host);
    expect(region.textContent).not.toBe(copied);
    expect(region.textContent).not.toBe('');
  });

  it('adds to the name of the Open link, which opens in a new tab, a note that is read and not drawn, with the icon on the full card alone', async () => {
    // A function says nothing of where it opens, so its button's name is its visible text.
    const {host, el} = await card('compact');
    el.open = () => {};
    await settle(host);
    const button = deep(host, '[part="open"]')!;
    expect(button.querySelector('.sr-only')).toBeNull();
    const visible = button.textContent!.trim();
    document.body.innerHTML = '';

    for (const attrs of ['', 'compact']) {
      const {host} = await card(`${attrs} open-url="https://example.org/{mosaica_id}"`);
      const open = deep(host, '[part="open"]')!;
      expect(open.getAttribute('target'), attrs).toBe('_blank');
      const note = open.querySelector('.sr-only')!;
      expect(note.textContent!.trim(), attrs).not.toBe('');
      // The name holds the visible text and the note after it.
      expect(open.textContent!.trim().startsWith(visible), attrs).toBe(true);
      expect(open.textContent!.trim().length, attrs).toBeGreaterThan(visible.length);
      expect(open.querySelector('svg') !== null, attrs).toBe(attrs === '');
      document.body.innerHTML = '';
    }
  });
});

describe('<mosaica-item-card> text a clamp cuts short', () => {
  it('gives a pointer a long value’s whole text as its title, and nothing else a title', async () => {
    const {host} = await card('title-field="title"');
    const titled = deepAll(host, '[title]');
    expect(titled).toEqual([deep(host, '[part="field"][data-name="abstract"] [part="value"]')]);
    expect(titled[0]!.getAttribute('title')).toBe(LONG);
  });

  it('offers a long value whole with its own button on the full card, named by its field', async () => {
    cutShort();
    const {host, el} = await card('title-field="title"');
    await settle(host);
    const more = () => deep(host, '[part="field"][data-name="abstract"] [part="more"]') as HTMLButtonElement | null;
    expect(more()!.getAttribute('aria-expanded')).toBe('false');
    expect(more()!.getAttribute('aria-label')).toContain('Abstract');
    expect(more()!.getAttribute('aria-label')).toContain(more()!.textContent!);
    const button = more()!;
    button.focus();
    button.click();
    await settle(host);
    expect(more()).toBe(button);
    expect(button.getAttribute('aria-expanded')).toBe('true');
    expect(deep(host, '[part="field"][data-name="abstract"] [part="value"]')!.textContent).toBe(LONG);
    // Folding it again keeps the button, and focus on it.
    button.click();
    await settle(host);
    expect(more()).toBe(button);
    expect(button.getAttribute('aria-expanded')).toBe('false');
    expect(el.shadowRoot!.activeElement).toBe(button);
    // A short value has none.
    expect(deep(host, '[part="field"][data-name="thumb"] [part="more"]')).toBeNull();
  });

  it('offers no button for a long value that fits', async () => {
    const {host} = await card('title-field="title"');
    expect(deep(host, '[part="more"]')).toBeNull();
  });

  it('offers Show all on a compact card whose text is cut short though every field is shown, and keeps it through a press', async () => {
    cutShort();
    const one = {id: 9n, detail: {fields: {abstract: LONG}, labels: [], views: [], scoped: {}}} as unknown as typeof paper;
    const {host, el} = await card('compact', one);
    await settle(host);
    const showAll = deep(host, '[part="show-all"]') as HTMLButtonElement;
    expect(showAll).not.toBeNull();
    expect(deepAll(host, '[part="field"]')).toHaveLength(1);
    expect(showAll.getAttribute('aria-expanded')).toBe('false');
    showAll.focus();
    showAll.click();
    await settle(host);
    expect(deep(host, '[part="show-all"]')).toBe(showAll);
    expect(showAll.getAttribute('aria-expanded')).toBe('true');
    showAll.click();
    await settle(host);
    expect(deep(host, '[part="show-all"]')).toBe(showAll);
    expect(el.shadowRoot!.activeElement).toBe(showAll);
  });

  it('offers no Show all on a compact card that shows every field whole', async () => {
    const one = {id: 9n, detail: {fields: {abstract: LONG}, labels: [], views: [], scoped: {}}} as unknown as typeof paper;
    const {host} = await card('compact', one);
    expect(deep(host, '[part="show-all"]')).toBeNull();
  });
});
