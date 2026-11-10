import {mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium, type Browser, type Page} from 'playwright';
import {build} from 'vite';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';

/**
 * The theme tokens and the forwarded parts, in Chromium's own cascade, against the single-file
 * bundle a page with no build step loads.
 */

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
let out = '';
let browser: Browser;

beforeAll(async () => {
  out = mkdtempSync(join(tmpdir(), 'mosaica-components-'));
  await build({configFile: join(root, 'vite.config.ts'), root, logLevel: 'silent', build: {outDir: out, emptyOutDir: true}});
  browser = await chromium.launch({args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader']});
});

afterAll(async () => {
  await browser?.close();
  if (out) rmSync(out, {recursive: true, force: true});
});

/** A page holding `body` and `css`, with every element defined and rendered, under the system scheme `system`. */
async function page(body: string, css = '', system: 'light' | 'dark' = 'light'): Promise<Page> {
  const p = await browser.newPage({colorScheme: system});
  await p.setContent(`<!doctype html><html><head><style>${css}</style></head><body>${body}</body></html>`);
  await p.addScriptTag({path: join(out, 'mosaica-components.js'), type: 'module'});
  await p.evaluate(async () => {
    await customElements.whenDefined('mosaica-explorer');
    const all = (r: ParentNode): Element[] => [...r.querySelectorAll('*')].flatMap((e) => [e, ...(e.shadowRoot ? all(e.shadowRoot) : [])]);
    for (let i = 0; i < 3; i++) for (const e of all(document)) await (e as unknown as {updateComplete?: Promise<unknown>}).updateComplete;
  });
  return p;
}

/**
 * One computed property of the element reached by `path`: each entry a selector, each step after
 * the first taken inside the previous match's shadow root.
 */
function computed(p: Page, path: string[], property: string): Promise<string> {
  return p.evaluate(
    ([path, property]) => {
      let at: Element | null = document.querySelector(path[0]!);
      for (const selector of path.slice(1)) at = at?.shadowRoot?.querySelector(selector) ?? null;
      if (!at) throw new Error(`nothing at ${path.join(' >> ')}`);
      return getComputedStyle(at).getPropertyValue(property);
    },
    [path, property] as const
  );
}

const PAN = ['mosaica-map', '[part="controls"] button[aria-pressed="true"]'];
const INNER_PAN = ['mosaica-explorer', 'mosaica-map', '[part="controls"] button[aria-pressed="true"]'];

describe('theme tokens', () => {
  it('fall back to the light defaults, and to the dark ones under a dark colour scheme', async () => {
    const light = await page('<mosaica-map></mosaica-map>');
    expect(await computed(light, ['mosaica-map'], 'background-color')).toBe('rgb(246, 246, 244)');
    expect(await computed(light, PAN, 'background-color')).toBe('rgb(27, 29, 33)');
    const dark = await page('<mosaica-map></mosaica-map>', ':root { color-scheme: dark; }');
    expect(await computed(dark, ['mosaica-map'], 'background-color')).toBe('rgb(17, 19, 23)');
    expect(await computed(dark, PAN, 'background-color')).toBe('rgb(236, 238, 241)');
  });

  it('give the map the ground its tokens render in, whatever the system prefers', async () => {
    const ground = (p: Page) => p.evaluate(() => (document.querySelector('mosaica-map') as unknown as {drawnGround: string}).drawnGround);
    // A page naming no scheme renders light under a dark system preference, and so does the map.
    const unnamed = await page('<mosaica-map></mosaica-map>', '', 'dark');
    expect(await computed(unnamed, ['mosaica-map'], 'background-color')).toBe('rgb(246, 246, 244)');
    expect(await ground(unnamed)).toBe('light');
    // A page allowing both follows the system preference.
    const both = await page('<mosaica-map></mosaica-map>', ':root { color-scheme: light dark; }', 'dark');
    expect(await computed(both, ['mosaica-map'], 'background-color')).toBe('rgb(17, 19, 23)');
    expect(await ground(both)).toBe('dark');
  });

  it('take a value set on any ancestor', async () => {
    const p = await page('<div><mosaica-map></mosaica-map></div>', 'body { --mosaica-map-bg: rgb(255, 0, 0); --mosaica-accent: rgb(0, 128, 0); }');
    expect(await computed(p, ['mosaica-map'], 'background-color')).toBe('rgb(255, 0, 0)');
    expect(await computed(p, PAN, 'background-color')).toBe('rgb(0, 128, 0)');
  });

  it('take a value set on the element itself over one on an ancestor', async () => {
    const p = await page('<mosaica-map style="--mosaica-map-bg: rgb(0, 0, 255)"></mosaica-map>', 'body { --mosaica-map-bg: rgb(255, 0, 0); }');
    expect(await computed(p, ['mosaica-map'], 'background-color')).toBe('rgb(0, 0, 255)');
  });

  it('reach the elements inside an explorer from the explorer and from above it', async () => {
    const p = await page('<mosaica-explorer style="--mosaica-accent: rgb(1, 2, 3)"></mosaica-explorer>', 'body { --mosaica-map-bg: rgb(9, 8, 7); }');
    expect(await computed(p, INNER_PAN, 'background-color')).toBe('rgb(1, 2, 3)');
    expect(await computed(p, ['mosaica-explorer', 'mosaica-map'], 'background-color')).toBe('rgb(9, 8, 7)');
  });
});

describe('forwarded parts', () => {
  it('style the explorer’s inner map and status strip from the page', async () => {
    const p = await page(
      '<mosaica-explorer></mosaica-explorer>',
      'mosaica-explorer::part(map-controls) { border-top-color: rgb(4, 5, 6); } mosaica-explorer::part(status-strip) { color: rgb(7, 8, 9); }'
    );
    expect(await computed(p, ['mosaica-explorer', 'mosaica-map', '[part="controls"]'], 'border-top-color')).toBe('rgb(4, 5, 6)');
    expect(await computed(p, ['mosaica-explorer', 'mosaica-status', '[part="strip"]'], 'color')).toBe('rgb(7, 8, 9)');
  });
});

describe('the explorer by the width of its container', () => {
  /** What the explorer drew: which arrangement, where the tools sit, and whether the strip is short. */
  const layout = (p: Page) =>
    p.evaluate(() => {
      const root = document.querySelector('mosaica-explorer')!.shadowRoot!;
      return {
        sidebar: root.querySelector('[part="sidebar"]') !== null,
        panel: root.querySelector('[part="panel"]') !== null,
        corner: root.querySelector('mosaica-map')!.getAttribute('controls-corner'),
        compact: root.querySelector('mosaica-map mosaica-status')!.hasAttribute('compact')
      };
    });
  const settled = (p: Page) =>
    p.waitForFunction(() => {
      const root = document.querySelector('mosaica-explorer')?.shadowRoot;
      return root?.querySelector('mosaica-map mosaica-status') != null;
    });

  it('keeps the docked sidebar where there is room, and folds it into the card in a compact container', async () => {
    const wide = await page('<div style="width: 1300px; height: 700px"><mosaica-explorer layout="docked"></mosaica-explorer></div>');
    await settled(wide);
    expect(await layout(wide)).toEqual({sidebar: true, panel: false, corner: 'top-left', compact: false});
    const compact = await page('<div style="width: 900px; height: 560px"><mosaica-explorer layout="docked"></mosaica-explorer></div>');
    await compact.waitForFunction(() => document.querySelector('mosaica-explorer')?.shadowRoot?.querySelector('[part="panel"]') != null);
    expect(await layout(compact)).toEqual({sidebar: false, panel: true, corner: 'bottom-left', compact: true});
  });
});

describe('a list that opens over what sits below it', () => {
  it('shows whole past the bottom of a card that scrolls inside itself', async () => {
    const p = await page('<div id="card" style="width: 340px; height: 160px; overflow-y: auto"><mosaica-filter-panel></mosaica-filter-panel></div>');
    const seen = await p.evaluate(async () => {
      const columns = ['archive', 'title', 'submitted_at', 'author', 'venue', 'year'];
      const projections: Record<string, unknown> = {
        meta: {declaredScalars: [], layers: [], views: [], filterOperands: columns.map((column) => ({column, family: 'keyword', operands: ['eq']})), selection: {}},
        status: {status: 'shown', sessionWarm: true, refusal: null, stale: false, expired: false, retrying: false, unreachable: false},
        view: {id: 's0'},
        filters: {draft: {filter: {}, highlight: {}}, expr: null, highlight: null, members: [], regions: [], suggestions: {}, suggestErrors: {}, suggestEpoch: 0},
        legend: {colourBy: null},
        artifacts: {served: [], colours: new Map()},
        aggregates: new Map()
      };
      const store = {get: (name: string) => projections[name], subscribe: () => () => {}, setAggregate: () => {}};
      const panel = document.querySelector('mosaica-filter-panel') as HTMLElement & {store: unknown; updateComplete: Promise<unknown>};
      panel.store = store;
      await panel.updateComplete;
      panel.shadowRoot!.querySelector<HTMLElement>('[part="add"]')!.click();
      await panel.updateComplete;
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      const list = panel.shadowRoot!.querySelector<HTMLElement>('[part="add-list"]')!;
      const last = [...list.querySelectorAll<HTMLElement>('[part~="add-option"]')].at(-1)!.getBoundingClientRect();
      const card = document.getElementById('card')!.getBoundingClientRect();
      const hit = panel.shadowRoot!.elementFromPoint(last.left + 10, last.top + last.height / 2);
      return {open: list.matches(':popover-open'), below: last.bottom > card.bottom, onTop: hit?.closest('[part~="add-option"]') != null};
    });
    expect(seen).toEqual({open: true, below: true, onTop: true});
  });
});

describe('the map’s toolbar in a bottom corner', () => {
  it('sits against the bottom edge of the map, on its side', async () => {
    const p = await page('<mosaica-map controls-corner="bottom-left" style="--mosaica-map-height: 400px; width: 600px"></mosaica-map><mosaica-map controls-corner="bottom-right" style="--mosaica-map-height: 400px; width: 600px"></mosaica-map>');
    const at = await p.evaluate(() =>
      [...document.querySelectorAll('mosaica-map')].map((m) => {
        const map = m.getBoundingClientRect();
        const bar = m.shadowRoot!.querySelector('[part="controls"]')!.getBoundingClientRect();
        return {bottom: map.bottom - bar.bottom < 40, left: bar.left - map.left < 40, right: map.right - bar.right < 40};
      })
    );
    expect(at).toEqual([
      {bottom: true, left: true, right: false},
      {bottom: true, left: false, right: true}
    ]);
  });
});

describe('the filter panel', () => {
  it('renders a card’s rows from the counts registered, and Tab from its search box reaches a row’s Highlight and shows it', async () => {
    const p = await page('<div style="width: 340px"><mosaica-filter-panel pinned="archive"></mosaica-filter-panel></div>');
    const seen = await p.evaluate(async () => {
      const rows = [
        {key: 'cs', count: 30},
        {key: 'math', count: 20}
      ];
      const columns: Record<string, {get(i: number): unknown}> = {
        group: {get: () => 'listed'},
        key: {get: (i: number) => rows[i]!.key},
        title: {get: () => null},
        count: {get: (i: number) => BigInt(rows[i]!.count)}
      };
      const table = {numRows: rows.length, getChild: (name: string) => columns[name] ?? null};
      const answer = {status: 'shown', view: 's0', refusal: null, summaries: [null], result: {tables: [{grouping: 0, total: 50, referenceTotal: null, groups: 2, sample: null, rows: table}], region: null, recomposed: false, identityKey: 'ik', next: null}};
      const listeners = new Set<() => void>();
      let aggregates = new Map<string, unknown>();
      const projections: Record<string, unknown> = {
        meta: {declaredScalars: [{name: 'archive', arrowType: 'u16', category: {}, render: false}], layers: [], views: [], filterOperands: [{column: 'archive', family: 'category', operands: ['in']}], selection: {maxAggregateTop: 1000}},
        status: {status: 'shown', sessionWarm: true, refusal: null, stale: false, expired: false, retrying: false, unreachable: false},
        view: {id: 's0', inView: null},
        filters: {draft: {filter: {}, highlight: {}}, expr: null, highlight: null, members: [], regions: [], suggestions: {}, suggestErrors: {}, suggestEpoch: 0},
        legend: {colourBy: null, categories: {}, ranks: {}},
        artifacts: {served: [], colours: new Map()}
      };
      const store = {
        get: (name: string) => (name === 'aggregates' ? aggregates : projections[name]),
        subscribe: (fn: () => void) => {
          listeners.add(fn);
          return () => listeners.delete(fn);
        },
        setAggregate: (id: string, spec: unknown) => {
          aggregates = new Map(aggregates);
          if (spec === null) aggregates.delete(id);
          else aggregates.set(id, answer);
          queueMicrotask(() => listeners.forEach((fn) => fn()));
        },
        suggest: () => {},
        forgetSuggestions: () => {}
      };
      const panel = document.querySelector('mosaica-filter-panel') as HTMLElement & {store: unknown; updateComplete: Promise<unknown>};
      panel.store = store;
      for (let i = 0; i < 6; i++) {
        await new Promise((r) => requestAnimationFrame(r));
        await panel.updateComplete;
      }
      const card = panel.shadowRoot!.querySelector('mosaica-field-card')!;
      await (card as unknown as {updateComplete: Promise<unknown>}).updateComplete;
      const names = [...card.shadowRoot!.querySelectorAll('[part="name"]')].map((n) => n.textContent);
      card.shadowRoot!.querySelector('mosaica-filter')!.shadowRoot!.querySelector<HTMLInputElement>('[part="entry"]')!.focus();
      return {names};
    });
    expect(seen.names).toEqual(['cs', 'math']);
    await p.keyboard.press('Tab');
    const focused = await p.evaluate(() => {
      let at: Element | null = document.activeElement;
      while (at?.shadowRoot?.activeElement) at = at.shadowRoot.activeElement;
      const verbs = at?.closest('[part="verbs"]');
      return {part: at?.getAttribute('part') ?? null, row: at?.closest('[part~="row"]')?.getAttribute('data-key') ?? null, shown: verbs ? getComputedStyle(verbs).opacity : null};
    });
    expect(focused).toEqual({part: 'highlight', row: 'cs', shown: '1'});
  });
});

describe('the filter panel under a pointer and a keyboard', () => {
  const LONG = 'a value with a name far too long to fit on one line of a card';

  /**
   * A panel 340 px wide over a category the map is coloured by and a date, the category filtered to
   * one value and the date highlighted by a range, on a store that publishes what is set on it.
   */
  async function panel(attributes = 'pinned="archive submitted_at"'): Promise<Page> {
    const p = await page(`<div style="width: 340px"><mosaica-filter-panel ${attributes}></mosaica-filter-panel></div>`);
    await p.evaluate(async (long) => {
      const values = [
        {key: 'cs', count: 30},
        {key: 'math', count: 20},
        {key: long, count: 10}
      ];
      const bins = [10, 20, 30, 40].map((count, i) => ({lower: Date.UTC(2019 + i, 0, 1), upper: Date.UTC(2020 + i, 0, 1), count}));
      const table = (n: number, columns: Record<string, (i: number) => unknown>) => ({numRows: n, getChild: (name: string) => (columns[name] ? {get: columns[name]} : null)});
      const category = table(values.length, {group: () => 'listed', key: (i) => values[i]!.key, title: () => null, count: (i) => BigInt(values[i]!.count)});
      const histogram = table(bins.length, {group: () => 'listed', lower: (i) => bins[i]!.lower, upper: (i) => bins[i]!.upper, count: (i) => BigInt(bins[i]!.count)});
      const answer = (rows: {numRows: number}, total: number) => ({status: 'shown', view: 's0', palette: null, refusal: null, summaries: [null], result: {tables: [{grouping: 0, total, referenceTotal: null, groups: rows.numRows, sample: null, rows}], region: null, recomposed: false, identityKey: 'ik', next: null}});
      const listeners = new Set<() => void>();
      const publish = () => listeners.forEach((fn) => fn());
      let aggregates = new Map<string, unknown>();
      const projections: Record<string, unknown> = {
        meta: {
          declaredScalars: [
            {name: 'archive', arrowType: 'u16', category: {}, render: true},
            {name: 'submitted_at', arrowType: 'timestamp_us', category: null, render: true}
          ],
          layers: [],
          views: [],
          filterOperands: [
            {column: 'archive', family: 'category', operands: ['in']},
            {column: 'submitted_at', family: 'numeric', operands: ['range']}
          ],
          selection: {maxAggregateTop: 1000, maxAggregateNamed: 1000}
        },
        status: {status: 'shown', sessionWarm: true, refusal: null, stale: false, expired: false, retrying: false, unreachable: false},
        view: {id: 's0', inView: null},
        filters: {
          draft: {filter: {archive: {family: 'category', keys: ['math']}}, highlight: {submitted_at: {family: 'numeric', gte: Date.UTC(2020, 0, 1) * 1000, lte: null}}},
          expr: null,
          highlight: null,
          members: [],
          regions: [],
          suggestions: {},
          suggestErrors: {},
          suggestEpoch: 0
        },
        legend: {colourBy: 'archive', categories: {archive: values.map((v, i) => ({key: v.key, code: i + 1, title: null}))}, ranks: {archive: {1: 0, 2: 1, 3: 2}}, domains: {}},
        artifacts: {served: [], colours: new Map(), palette: 'tableau10', overrides: new Map()}
      };
      const store = {
        get: (name: string) => (name === 'aggregates' ? aggregates : projections[name]),
        subscribe: (fn: () => void) => {
          listeners.add(fn);
          return () => listeners.delete(fn);
        },
        setAggregate: (id: string, spec: {groupings: {by?: {bins?: number}}[]} | null) => {
          aggregates = new Map(aggregates);
          if (spec === null) aggregates.delete(id);
          else aggregates.set(id, spec.groupings[0]?.by?.bins ? answer(histogram, 100) : answer(category, 60));
          queueMicrotask(publish);
        },
        setFilters: (draft: unknown) => {
          projections['filters'] = {...(projections['filters'] as object), draft};
          publish();
        },
        setMembers: () => {},
        suggest: () => {},
        forgetSuggestions: () => {}
      };
      const el = document.querySelector('mosaica-filter-panel') as HTMLElement & {store: unknown; updateComplete: Promise<unknown>};
      el.store = store;
      // For a test to publish the suggestions a typed value is answered with.
      (window as unknown as {suggested: (page: unknown) => void}).suggested = (page) => {
        projections['filters'] = {...(projections['filters'] as object), suggestions: {archive: page}};
        publish();
      };
      for (let i = 0; i < 8; i++) {
        await new Promise((r) => requestAnimationFrame(r));
        await el.updateComplete;
        for (const card of el.shadowRoot!.querySelectorAll('mosaica-field-card')) await (card as unknown as {updateComplete: Promise<unknown>}).updateComplete;
      }
    }, LONG);
    return p;
  }

  /** The control holding focus, through every shadow root: its part, its name, its row and its chip's column. */
  const focused = (p: Page) =>
    p.evaluate(() => {
      let at: Element | null = document.activeElement;
      while (at?.shadowRoot?.activeElement) at = at.shadowRoot.activeElement;
      return {
        tag: at?.localName ?? null,
        part: at?.getAttribute('part') ?? null,
        name: at?.getAttribute('aria-label') ?? null,
        row: at?.closest('[part~="row"]')?.getAttribute('data-key') ?? null,
        chip: at?.closest('[part="chip"]')?.getAttribute('data-column') ?? null
      };
    });
  const settled = (p: Page) =>
    p.evaluate(async () => {
      const el = document.querySelector('mosaica-filter-panel') as HTMLElement & {updateComplete: Promise<unknown>};
      for (let i = 0; i < 4; i++) {
        await new Promise((r) => requestAnimationFrame(r));
        await el.updateComplete;
        for (const card of el.shadowRoot!.querySelectorAll('mosaica-field-card')) await (card as unknown as {updateComplete: Promise<unknown>}).updateComplete;
      }
    });

  it('gives every pointer target 24 px each way, none over another, and a row its counts beside its buttons', async () => {
    const p = await panel();
    const seen = await p.evaluate(() => {
      const root = document.querySelector('mosaica-filter-panel')!.shadowRoot!;
      const cards = [...root.querySelectorAll('mosaica-field-card')].map((c) => c.shadowRoot!);
      const targets = [
        ...root.querySelectorAll('[part="chip"] button, [part="clear-highlight"], [part="clear"], [part="add"]'),
        ...cards.flatMap((c) => [...c.querySelectorAll('[part="swatch"], [part="verbs"] button, [part="paint"], [part="fold"], [part="range"]')])
      ];
      const box = (e: Element) => e.getBoundingClientRect();
      const label = (e: Element) => `${e.getAttribute('part') ?? e.getAttribute('aria-label')} ${Math.round(box(e).width)}x${Math.round(box(e).height)}`;
      // The chip's own text and Clear all are lines of text, which a target's least size leaves out.
      const sized = targets.filter((e) => !['edit', 'clear'].includes(e.getAttribute('part') ?? ''));
      const small = sized.filter((e) => box(e).width < 23.99 || box(e).height < 23.99).map(label);
      const over: string[] = [];
      for (let i = 0; i < targets.length; i++)
        for (let j = i + 1; j < targets.length; j++) {
          const [a, b] = [box(targets[i]!), box(targets[j]!)];
          if (Math.min(a.right, b.right) - Math.max(a.left, b.left) > 0.5 && Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 0.5) over.push(`${label(targets[i]!)} / ${label(targets[j]!)}`);
        }
      const row = cards[0]!.querySelector('[part~="row"]')!;
      row.querySelector<HTMLElement>('[part="filter"]')!.focus();
      const [count, verbs] = [row.querySelector('[part="count"]')!, row.querySelector('[part="verbs"]')!];
      return {
        kinds: [...new Set(sized.map((e) => e.getAttribute('part') ?? 'chip-close'))].sort(),
        small,
        over,
        count: {visibility: getComputedStyle(count).visibility, drawn: box(count).width > 0, beside: box(count).right <= box(verbs).left + 0.5},
        plot: getComputedStyle(cards[1]!.querySelector('[part="plot"]')!).touchAction
      };
    });
    expect(seen.kinds).toEqual(['add', 'chip-close', 'clear-highlight', 'filter', 'fold', 'highlight', 'paint', 'range', 'swatch']);
    expect(seen.small).toEqual([]);
    expect(seen.over).toEqual([]);
    expect(seen.count).toEqual({visibility: 'visible', drawn: true, beside: true});
    expect(seen.plot).toBe('pan-y');
  });

  it('wraps a long value’s name inside its row, and tells a highlighted range from a filtered one by more than its colour', async () => {
    const p = await panel();
    const seen = await p.evaluate((long) => {
      const cards = [...document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelectorAll('mosaica-field-card')].map((c) => c.shadowRoot!);
      const row = [...cards[0]!.querySelectorAll<HTMLElement>('[part~="row"]')].find((r) => r.dataset['key'] === long)!;
      const name = row.querySelector('[part="name"]')!;
      const short = cards[0]!.querySelector('[part~="row"] [part="name"]')!;
      const band = cards[1]!.querySelector('[part~="band"][data-verb="highlight"]')!;
      return {
        whole: name.scrollWidth <= name.clientWidth + 1 && name.getBoundingClientRect().right <= row.getBoundingClientRect().right,
        lines: name.getBoundingClientRect().height > short.getBoundingClientRect().height * 1.5,
        hatched: getComputedStyle(band).backgroundImage.includes('gradient'),
        said: cards[1]!.querySelector('.applied[data-verb="highlight"]')!.textContent
      };
    }, LONG);
    expect(seen.whole).toBe(true);
    expect(seen.lines).toBe(true);
    expect(seen.hatched).toBe(true);
    expect(seen.said).toContain('2020');
    // A pressed Filter stays filled, its icon solid, under the pointer, beside a Highlight that is not.
    const verbs = p.locator('mosaica-field-card [part~="row"][data-key="math"] [part="verbs"]');
    await verbs.locator('[part="filter"]').hover();
    const drawn = (part: string) => verbs.locator(`[part="${part}"]`).evaluate((b) => [getComputedStyle(b).backgroundColor, getComputedStyle(b.querySelector('svg')!).fill]);
    expect(await drawn('filter')).toEqual(['rgb(27, 29, 33)', 'rgb(255, 255, 255)']);
    expect((await drawn('highlight'))[1]).toBe('none');
  });

  it('under chips-only, puts focus on words it draws once the last chip is removed from the keyboard', async () => {
    const p = await panel('chips-only');
    await p.evaluate(() => document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector<HTMLElement>('[part="chip"] > button:last-child')!.focus());
    await p.keyboard.press('Enter');
    await settled(p);
    await p.keyboard.press('Enter');
    await settled(p);
    const held = await p.evaluate(() => {
      const at = document.querySelector('mosaica-filter-panel')!.shadowRoot!.activeElement as HTMLElement | null;
      const box = at?.getBoundingClientRect();
      return {chips: document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelectorAll('[part="chip"]').length, words: at?.textContent?.trim() !== '', drawn: !!box && box.width > 0 && box.height > 0, ringed: at?.matches(':focus-visible') ?? false};
    });
    expect(held).toEqual({chips: 0, words: true, drawn: true, ringed: true});
    // Tab leaves them, and nothing is left in the tab order.
    await p.keyboard.press('Tab');
    await settled(p);
    expect(await p.evaluate(() => document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelectorAll('*:not(style)').length)).toBe(0);
  });

  it('draws a chosen suggestion’s bar in the text colour of the row forced colours highlight', async () => {
    const p = await panel();
    await p.locator('mosaica-field-card[data-field="archive"] mosaica-filter [part="entry"]').click();
    await p.keyboard.type('m');
    await p.evaluate(() =>
      (window as unknown as {suggested: (page: unknown) => void}).suggested({
        q: 'm',
        verb: 'filter',
        values: [
          {code: 2, key: 'math', title: null, match: {field: 'key', start: 0, len: 1}, count: 20},
          {code: 4, key: 'med', title: null, match: {field: 'key', start: 0, len: 1}, count: 10}
        ],
        more: false,
        total: 60
      })
    );
    await settled(p);
    await p.emulateMedia({forcedColors: 'active'});
    const seen = await p.evaluate(() => {
      const probe = document.body.appendChild(document.createElement('span'));
      const system = (colour: string) => {
        probe.style.cssText = `forced-color-adjust: none; color: ${colour}`;
        return getComputedStyle(probe).color;
      };
      const list = document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector('mosaica-field-card')!.shadowRoot!.querySelector('mosaica-filter')!.shadowRoot!;
      const bar = (selected: string) => getComputedStyle(list.querySelector(`[part~="tick"][aria-selected="${selected}"] [part="bar"]`)!).backgroundColor;
      const row = getComputedStyle(list.querySelector('[part~="tick"][aria-selected="true"]')!).backgroundColor;
      return {chosen: bar('true') === system('HighlightText'), other: bar('false') === system('CanvasText'), row: row === system('Highlight'), apart: system('HighlightText') !== system('Highlight')};
    });
    expect(seen).toEqual({chosen: true, other: true, row: true, apart: true});
  });

  it('keeps focus on a value as its row moves, and moves it along the chips to Add field as they are removed', async () => {
    const p = await panel();
    await p.evaluate((long) => {
      const card = document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector('mosaica-field-card')!.shadowRoot!;
      const row = [...card.querySelectorAll<HTMLElement>('[part~="row"]')].find((r) => r.dataset['key'] === long)!;
      row.querySelector<HTMLElement>('[part="filter"]')!.focus();
    }, LONG);
    await p.keyboard.press('Enter');
    await settled(p);
    const order = await p.evaluate(() => [...document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector('mosaica-field-card')!.shadowRoot!.querySelectorAll<HTMLElement>('[part~="row"]')].map((r) => r.dataset['key']));
    // In the filter, the value is listed with the other the filter names, above the rest.
    expect(order).toEqual(['math', LONG, 'cs']);
    expect(await focused(p)).toMatchObject({part: 'filter', row: LONG});
    expect((await focused(p)).name).toContain(LONG);

    await p.evaluate(() => document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector<HTMLElement>('[part="chip"][data-column="archive"] > button:last-child')!.focus());
    await p.keyboard.press('Enter');
    await settled(p);
    expect(await focused(p)).toMatchObject({tag: 'button', chip: 'submitted_at'});
    await p.keyboard.press('Enter');
    await settled(p);
    expect(await focused(p)).toMatchObject({tag: 'button', part: 'add'});
  });
});
