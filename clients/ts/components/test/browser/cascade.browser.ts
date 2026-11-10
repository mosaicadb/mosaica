import {mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium, type Browser, type Page} from 'playwright';
import {build} from 'vite';
import {afterAll, afterEach, beforeAll, describe, expect, it} from 'vitest';

/**
 * The theme tokens, the forwarded parts and the filter panel's look, in Chromium's own cascade,
 * against the single-file bundle a page with no build step loads. Nothing here pins a length that
 * follows from how wide a font's glyphs are: what depends on text is measured in the page, and
 * every check is made in four fonts.
 */

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
let out = '';
let browser: Browser;

/**
 * The fonts each check is drawn in, by the rules that ask for each: the theme's, a serif, a
 * monospace, and the theme's 15% larger in the filter panel and its cards.
 */
const LOOKS = {
  'the theme’s font': '',
  'a serif': 'body { --mosaica-font: serif; }',
  'a monospace': 'body { --mosaica-font: monospace; }',
  'a larger font': 'mosaica-filter-panel, mosaica-filter-panel::part(card) { font-size: 14.95px; }'
};
/** The rules of the look the checks now running are drawn in. */
let lookCss = '';
/** The pages the running check opened. */
const opened: Page[] = [];

afterEach(async () => {
  await Promise.all(opened.splice(0).map((p) => p.close().catch(() => {})));
});

beforeAll(async () => {
  out = mkdtempSync(join(tmpdir(), 'mosaica-components-'));
  await build({configFile: join(root, 'vite.config.ts'), root, logLevel: 'silent', build: {outDir: out, emptyOutDir: true}});
  browser = await chromium.launch({args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader']});
});

afterAll(async () => {
  await browser?.close();
  if (out) rmSync(out, {recursive: true, force: true});
});

/** A page holding `body` and `css`, drawn in the running look, with every element defined and rendered, under the system scheme `system`. */
async function page(body: string, css = '', system: 'light' | 'dark' = 'light'): Promise<Page> {
  const p = await browser.newPage({colorScheme: system});
  opened.push(p);
  await p.setContent(`<!doctype html><html><head><style>${lookCss}${css}</style></head><body>${body}</body></html>`);
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

/** Every check below, in the font `look` names. */
for (const [look, css] of Object.entries(LOOKS)) {
  describe(`drawn in ${look}`, () => {
    beforeAll(() => {
      lookCss = css;
    });

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
      const LONG = 'a value with a name far too long to fit on one line of a card, however wide the card is drawn';
      /** A column whose caption is longer than a card's line. */
      const LONG_FIELD = 'a_field_with_a_caption_much_longer_than_the_line_of_any_card_it_is_drawn_in';
      /** What has a host's long names wrap, before and after the theme defines the token's private name. */
      const WRAPS = 'div { --mosaica-long-text: normal; --_mosaica-long-text: normal; }';

      /**
       * A panel over a category the map is coloured by, a date, a number with figures and a field with
       * a long name; the category filtered to one value and highlighted by another, the date
       * highlighted by a range; on a store that publishes what is set on it.
       */
      async function panel(attributes = 'pinned="archive submitted_at population"', width = 340, css = ''): Promise<Page> {
        const p = await page(`<div style="width: ${width}px"><mosaica-filter-panel ${attributes}></mosaica-filter-panel></div>`, css);
        // Tall enough to hold the panel whole in any font, since a press is found only inside the window.
        await p.setViewportSize({width: 800, height: 1600});
        await p.evaluate(
          async ([long, longField]) => {
            const values = [
              {key: 'cs', count: 30},
              {key: 'math', count: 20},
              {key: long, count: 10},
              {key: 'stat', count: 8},
              {key: 'physics', count: 6}
            ];
            const bins = [10, 20, 30, 40].map((count, i) => ({lower: Date.UTC(2019 + i, 0, 1), upper: Date.UTC(2020 + i, 0, 1), count}));
            const table = (n: number, columns: Record<string, (i: number) => unknown>) => ({numRows: n, getChild: (name: string) => (columns[name] ? {get: columns[name]} : null)});
            const category = table(values.length, {group: () => 'listed', key: (i) => values[i]!.key, title: () => null, count: (i) => BigInt(values[i]!.count)});
            const histogram = table(bins.length, {group: () => 'listed', lower: (i) => bins[i]!.lower, upper: (i) => bins[i]!.upper, count: (i) => BigInt(bins[i]!.count)});
            // The category holds more values than the five listed, so its card offers the rest.
            const answer = (rows: {numRows: number}, total: number) => ({status: 'shown', view: 's0', palette: null, refusal: null, summaries: [null], result: {tables: [{grouping: 0, total, referenceTotal: null, groups: rows.numRows + 6, sample: null, rows}], region: null, recomposed: false, identityKey: 'ik', next: null}});
            const listeners = new Set<() => void>();
            const publish = () => listeners.forEach((fn) => fn());
            let aggregates = new Map<string, unknown>();
            const projections: Record<string, unknown> = {
              meta: {
                declaredScalars: [
                  {name: 'archive', arrowType: 'u16', category: {}, render: true},
                  {name: 'submitted_at', arrowType: 'timestamp_us', category: null, render: true},
                  {name: 'population', arrowType: 'u32', category: null, render: true}
                ],
                layers: [],
                views: [],
                filterOperands: [
                  {column: 'archive', family: 'category', operands: ['in']},
                  {column: 'submitted_at', family: 'numeric', operands: ['range']},
                  {column: 'population', family: 'numeric', operands: ['range']},
                  {column: longField, family: 'keyword', operands: ['eq']}
                ],
                selection: {maxAggregateTop: 1000, maxAggregateNamed: 1000}
              },
              status: {status: 'shown', sessionWarm: true, refusal: null, stale: false, expired: false, retrying: false, unreachable: false},
              view: {id: 's0', inView: null},
              filters: {
                draft: {filter: {archive: {family: 'category', keys: ['math']}}, highlight: {archive: {family: 'category', keys: ['cs']}, submitted_at: {family: 'numeric', gte: Date.UTC(2020, 0, 1) * 1000, lte: null}}},
                expr: null,
                highlight: null,
                members: [],
                regions: [],
                suggestions: {},
                suggestErrors: {},
                suggestEpoch: 0
              },
              legend: {colourBy: 'archive', categories: {archive: values.map((v, i) => ({key: v.key, code: i + 1, title: null}))}, ranks: {archive: {1: 0, 2: 1, 3: 2, 4: 3, 5: 4}}, domains: {}},
              artifacts: {served: [], colours: new Map(), palette: 'tableau10', overrides: new Map()}
            };
            const store = {
              get: (name: string) => (name === 'aggregates' ? aggregates : projections[name]),
              subscribe: (fn: () => void) => {
                listeners.add(fn);
                return () => listeners.delete(fn);
              },
              setAggregate: (id: string, spec: {groupings: {by?: {bins?: number}; summary?: string[]}[]} | null) => {
                aggregates = new Map(aggregates);
                if (spec === null) aggregates.delete(id);
                // The population's figures, which its card's heading writes beside its name.
                else if (spec.groupings[0]?.summary?.includes('population')) aggregates.set(id, {...answer(category, 60), summaries: [new Map([['population', {values: 25n, missing: 0n, min: 1000, max: 9_000_000, mean: 1_234_567.5}]])]});
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
          },
          [LONG, LONG_FIELD] as const
        );
        return p;
      }

      /** The control holding focus, through every shadow root: its tag, its part, its name, its row and its chip. */
      const focused = (p: Page) =>
        p.evaluate(() => {
          let at: Element | null = document.activeElement;
          while (at?.shadowRoot?.activeElement) at = at.shadowRoot.activeElement;
          const chip = at?.closest('[part="chip"]');
          return {
            tag: at?.localName ?? null,
            part: at?.getAttribute('part') ?? null,
            name: at?.getAttribute('aria-label') ?? null,
            row: at?.closest('[part~="row"]')?.getAttribute('data-key') ?? null,
            chip: chip ? `${chip.getAttribute('data-verb')} ${chip.getAttribute('data-column')}` : null
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
      /** Open a folded card from its chevron. */
      const open = async (p: Page, field: string) => {
        await p.locator(`mosaica-field-card[data-field="${field}"] [part="fold"]`).click();
        await settled(p);
      };

      /**
       * The heights the panel's parts are drawn at, beside what each is on main: a number read off
       * main's rules, or one made of them and of a line's height as the page computes it. A line's
       * height is its font size times 1.45 whatever the font, so none depends on how wide a glyph is.
       */
      const heights = (p: Page) =>
        p.evaluate(() => {
          const root = document.querySelector('mosaica-filter-panel')!.shadowRoot!;
          const compact = document.querySelector('mosaica-filter-panel')!.hasAttribute('compact');
          const high = (e: Element | null) => (e ? e.getBoundingClientRect().height : NaN);
          const line = (e: Element) => parseFloat(getComputedStyle(e).lineHeight);
          const off: string[] = [];
          const pin = (what: string, drawn: number, main: number) => {
            if (!(Math.abs(drawn - main) < 0.5)) off.push(`${what}: ${drawn.toFixed(2)} px, on main ${main.toFixed(2)} px`);
          };
          const subject = root.querySelector('[part="subject"]')!;
          // Its two figures stand on one line unless the font is too wide for the panel; on one line, the row is its padding, its rules and the × beside Highlighted.
          const keys = [...subject.querySelectorAll('.key')].map((k) => k.getBoundingClientRect().top);
          if (keys.every((top) => Math.abs(top - keys[0]!) < 1)) pin('the subject row', high(subject), (compact ? 12 : 16) + 2 + 18);
          for (const chip of root.querySelectorAll('[part="chip"]')) {
            const text = chip.querySelector('[part="edit"]')!;
            // One line of text, the chip's padding and its border, where the chip's text is on one line.
            if (high(text) < line(text) * 1.5) pin(`the chip of ${chip.getAttribute('data-column')}`, high(chip), line(text) + 4 + 2);
          }
          const add = root.querySelector('[part="add"]')!;
          pin('the Add field row', high(root.querySelector('.adder')), (compact ? 16 : 22) + line(add) + (compact ? 8 : 10) + 2);
          for (const card of root.querySelectorAll('mosaica-field-card')) {
            const s = card.shadowRoot!;
            const field = card.getAttribute('data-field');
            pin(`the heading of ${field}`, high(s.querySelector('[part="head"]')), 24);
            if (card.hasAttribute('folded')) {
              pin(`the folded card of ${field}`, high(card), 6 + 24 + 6 + 1);
              continue;
            }
            const rows = [...s.querySelectorAll('[part~="row"]')];
            for (const row of rows) pin(`the row of ${row.getAttribute('data-key')!.slice(0, 12)}`, high(row), 4 + line(row.querySelector('.names')!) + 3 + 5 + 4);
            const more = s.querySelector('[part="more"]');
            if (more) pin('the more button', high(more), line(more));
            // The card's padding and rule, its heading, its search box, its rows and the button under them.
            if (rows.length > 0) pin(`the open card of ${field}`, high(card), 10 + 24 + 6 + 28 + 4 + 2 + rows.reduce((sum, row) => sum + high(row), 0) + (more ? 2 + 4 + high(more) : 0) + 12 + 1);
            // The card's padding and rule, its heading, its plot and the axis under it.
            const axis = s.querySelector('[part="axis"]');
            if (axis) pin(`the open card of ${field}`, high(card), 10 + 24 + 8 + 56 + 4 + line(axis) + 12 + 1);
          }
          return off;
        });

      /**
       * The text the panel draws, as the parts that hold it: every text node that is not clipped away,
       * transparent or hidden, by the nearest part or class above it. What a screen reader alone is
       * given is not among them.
       */
      const drawnText = (p: Page) =>
        p.evaluate(() => {
          const holders = new Set<string>();
          const walk = (root: ShadowRoot, prefix: string) => {
            const nodes = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
            for (let node = nodes.nextNode(); node; node = nodes.nextNode()) {
              const el = node.parentElement;
              if (!el || el.closest('style') || (node.textContent ?? '').trim() === '') continue;
              const hidden = el.closest('.sr-only');
              if (hidden && hidden.getBoundingClientRect().width <= 1.5 && !hidden.matches(':focus, :focus-within')) continue;
              if (!el.checkVisibility({opacityProperty: true, visibilityProperty: true})) continue;
              const holder = el.closest('[part], .none, .emptied, option');
              holders.add(`${prefix}${holder ? (holder.getAttribute('part') ?? holder.localName + (holder.className ? `.${holder.className}` : '')) : `<${el.localName}>`}`);
            }
            for (const el of root.querySelectorAll('*')) if (el.shadowRoot) walk(el.shadowRoot, `${el.localName.replace('mosaica-', '')} `);
          };
          walk(document.querySelector('mosaica-filter-panel')!.shadowRoot!, '');
          return [...holders].sort();
        });

      /**
       * The pointer targets a press does not reach from 12 px either side of their middle, in each
       * direction they are not wider or taller than that already. A press belongs to the control the
       * page finds under it, so a target that another lies over is found too.
       */
      const unreached = (p: Page, selectors: string) =>
        p.evaluate((selectors) => {
          const all = (r: ParentNode): Element[] => [...r.querySelectorAll('*')].flatMap((e) => [e, ...(e.shadowRoot ? all(e.shadowRoot) : [])]);
          const under = (x: number, y: number) => {
            let at = document.elementFromPoint(x, y);
            while (at?.shadowRoot) {
              const inner = at.shadowRoot.elementFromPoint(x, y);
              if (!inner || inner === at) break;
              at = inner;
            }
            return at;
          };
          const missed: string[] = [];
          const targets = all(document).filter((e) => e.matches(selectors));
          for (const target of targets) {
            const box = target.getBoundingClientRect();
            const [x, y] = [box.left + box.width / 2, box.top + box.height / 2];
            const reach = (size: number) => Math.max(11.5, size / 2 - 0.5);
            // A line of text is as wide as its words; its height is what a press must find.
            const wide = target.matches('[part="edit"], [part="clear"], [part="more"]') ? box.width / 2 - 0.5 : reach(box.width);
            for (const [dx, dy] of [[-wide, 0], [wide, 0], [0, -reach(box.height)], [0, reach(box.height)]] as const) {
              const hit = under(x + dx, y + dy);
              if (!(hit && (hit === target || target.contains(hit)))) missed.push(`${target.getAttribute('part') ?? target.getAttribute('aria-label')} at ${dx},${dy}`);
            }
          }
          return {count: targets.length, missed};
        }, selectors);

      it('draws the panel, its chips, its cards and their rows at the heights they have on main, in a wide and a compact panel', async () => {
        const wide = await panel();
        expect(await heights(wide)).toEqual([]);
        const compact = await panel('compact pinned="archive submitted_at population"', 264);
        expect(await heights(compact)).toEqual([]);
        await open(compact, 'archive');
        expect(await heights(compact)).toEqual([]);
        await open(compact, 'population');
        expect(await heights(compact)).toEqual([]);
      });

      it('draws no text at rest but in the parts that held text on main', async () => {
        const p = await panel();
        // A card's name, figures, rows, counts, "N more" and axis; the subject row, the chips, Clear all and Add field.
        expect(await drawnText(p)).toEqual(['add', 'add-note', 'all', 'all-count', 'clear', 'edit', 'field-card axis', 'field-card count', 'field-card more', 'field-card name', 'field-card sub', 'field-card title', 'subject-name']);
        // What the earlier rounds gave a screen reader is all still there, off screen.
        const unseen = await p.evaluate(() => {
          const root = document.querySelector('mosaica-filter-panel')!.shadowRoot!;
          const cards = [...root.querySelectorAll('mosaica-field-card')].map((c) => c.shadowRoot!);
          const off = (e: Element | null) => !!e && e.getBoundingClientRect().width <= 1.5 && (e.textContent ?? '').trim() !== '';
          return {
            counts: [...cards[0]!.querySelectorAll('[part~="row"] > .sr-only')].every(off),
            verb: [...root.querySelectorAll('[part="chip"][data-verb="highlight"] > .sr-only')].every(off) && root.querySelectorAll('[part="chip"][data-verb="highlight"] > .sr-only').length === 2,
            range: off(cards[1]!.querySelector('[part="range"]')),
            applied: off(cards[1]!.querySelector('.applied[data-verb="highlight"]')),
            table: off(cards[1]!.querySelector('table')!.parentElement)
          };
        });
        expect(unseen).toEqual({counts: true, verb: true, range: true, applied: true, table: true});
      });

      it('takes a press 12 px either side of the middle of every small control, drawn no larger for it', async () => {
        const p = await panel();
        const drawn = await p.evaluate(() => {
          const root = document.querySelector('mosaica-filter-panel')!.shadowRoot!;
          const card = root.querySelector('mosaica-field-card')!.shadowRoot!;
          const size = (e: Element) => [e.getBoundingClientRect().width, e.getBoundingClientRect().height];
          // The sizes main draws them at; a swatch's square is the 10 px its button holds.
          return {paint: size(card.querySelector('[part="paint"]')!), fold: size(card.querySelector('[part="fold"]')!), clearHighlight: size(root.querySelector('[part="clear-highlight"]')!), swatch: getComputedStyle(card.querySelector('[part="swatch"]')!, '::before').width};
        });
        expect(drawn).toEqual({paint: [22, 22], fold: [22, 22], clearHighlight: [18, 18], swatch: '10px'});
        const rest = await unreached(p, '[part="chip"] button, [part="clear"], [part="clear-highlight"], [part="add"], [part="swatch"], [part="paint"], [part="fold"], [part="more"]');
        expect(rest.count).toBeGreaterThan(20);
        expect(rest.missed).toEqual([]);
        // A row's two buttons, while the pointer is over the row and they are drawn.
        await p.locator('mosaica-field-card[data-field="archive"] [part~="row"][data-key="stat"] [part="name"]').hover();
        const verbs = await unreached(p, '[part~="row"][data-key="stat"] [part="verbs"] button');
        expect(verbs).toEqual({count: 2, missed: []});
        const folded = await panel('compact pinned="archive submitted_at"', 264);
        expect((await unreached(folded, '[part="fold"]')).missed).toEqual([]);
      });

      it('shows a row’s buttons in its counts’ place under the pointer and under the keyboard’s focus, and its counts again once both have left', async () => {
        const p = await panel();
        const row = 'mosaica-field-card[data-field="archive"] [part~="row"][data-key="stat"]';
        const shown = () =>
          p.locator(row).evaluate((r) => ({
            verbs: getComputedStyle(r.querySelector('[part="verbs"]')!).opacity === '1',
            count: getComputedStyle(r.querySelector('[part="count"]')!).visibility === 'visible',
            // The two share one cell, so the row is no wider or taller for the buttons.
            cell: getComputedStyle(r.querySelector('[part="verbs"]')!).gridColumnStart === getComputedStyle(r.querySelector('[part="count"]')!).gridColumnStart
          }));
        expect(await shown()).toEqual({verbs: false, count: true, cell: true});
        await p.locator(`${row} [part="name"]`).hover();
        expect(await shown()).toEqual({verbs: true, count: false, cell: true});
        // A press on Highlight leaves the focus on the button; the counts are back as the pointer leaves.
        await p.locator(`${row} [part="highlight"]`).click();
        await settled(p);
        await p.mouse.move(2, 2);
        expect(await shown()).toEqual({verbs: false, count: true, cell: true});
        // Hidden, the buttons are still Tab's next stops, and are drawn while it is on them.
        await p.locator('mosaica-field-card[data-field="archive"] mosaica-filter [part="entry"]').focus();
        for (let i = 0; i < 40 && (await focused(p)).row !== 'stat'; i++) await p.keyboard.press('Tab');
        expect(await focused(p)).toMatchObject({row: 'stat'});
        expect(await shown()).toEqual({verbs: true, count: false, cell: true});
      });

      it('marks a value the filter keeps with a tick and a highlighted one with a pen, before its counts, in a row no taller for it', async () => {
        const p = await panel();
        const seen = await p.evaluate(() => {
          const card = document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector('mosaica-field-card')!.shadowRoot!;
          const row = (key: string) => [...card.querySelectorAll<HTMLElement>('[part~="row"]')].find((r) => r.dataset['key'] === key)!;
          const cues = (key: string) => [...row(key).querySelectorAll('[part="count"] .cue')].map((c) => (c.classList.contains('lit') ? 'pen' : 'tick'));
          const high = (key: string) => row(key).getBoundingClientRect().height;
          return {math: cues('math'), cs: cues('cs'), stat: cues('stat'), level: Math.abs(high('math') - high('stat')) < 0.1 && Math.abs(high('cs') - high('stat')) < 0.1};
        });
        expect(seen).toEqual({math: ['tick'], cs: ['pen'], stat: [], level: true});
      });

      it('cuts a long name to one line with its whole text as its title, and wraps it where a host has long names wrap', async () => {
        for (const [css, wraps] of [['', false], [WRAPS, true]] as const) {
          const p = await panel(`compact pinned="archive population ${LONG_FIELD}"`, 264, css);
          await open(p, 'archive');
          const seen = await p.evaluate(
            ([long, field]) => {
              const root = document.querySelector('mosaica-filter-panel')!.shadowRoot!;
              const card = (name: string) => root.querySelector(`mosaica-field-card[data-field="${name}"]`)!.shadowRoot!;
              const names = [...card('archive').querySelectorAll<HTMLElement>('[part~="row"]')].find((r) => r.dataset['key'] === long)!.querySelector('.names')!;
              const title = card(field).querySelector('[part="title"]')!;
              const lines = (e: Element) => Math.round(e.getBoundingClientRect().height / parseFloat(getComputedStyle(e).lineHeight));
              const cut = (e: Element) => e.scrollWidth > e.clientWidth + 1;
              const fold = card(field).querySelector('[part="fold"]')!.getBoundingClientRect();
              return {
                name: {lines: lines(names) > 1, cut: cut(names), title: names.querySelector('[part="name"]')!.getAttribute('title') === long},
                title: {lines: lines(title) > 1, cut: cut(title), whole: title.textContent!.length > 60, clear: title.getBoundingClientRect().right <= fold.left + 0.5}
              };
            },
            [LONG, LONG_FIELD] as const
          );
          expect(seen, wraps ? 'wrapping' : 'cut').toEqual({name: {lines: wraps, cut: !wraps, title: true}, title: {lines: wraps, cut: !wraps, whole: true, clear: true}});
          await p.close();
        }
      });

      it('keeps a card’s name, the mark after it and its figures on one line, the figures cut where they do not fit', async () => {
        for (const width of [264, 340]) {
          const p = await panel('compact pinned="archive population"', width);
          const one = (field: string) =>
            p.evaluate((field) => {
              const s = document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector(`mosaica-field-card[data-field="${field}"]`)!.shadowRoot!;
              const box = (selector: string) => s.querySelector(selector)?.getBoundingClientRect() ?? null;
              const [head, title, mark, sub] = ['[part="head"]', '[part="title"]', '.painted', '[part="sub"]'].map(box);
              const within = (b: DOMRect | null) => !b || b.width === 0 || (b.top >= head!.top - 0.5 && b.bottom <= head!.bottom + 0.5);
              return {
                head: Math.abs(head!.height - 24) < 0.5,
                oneLine: Math.abs(title!.height - parseFloat(getComputedStyle(s.querySelector('[part="title"]')!).lineHeight)) < 1,
                // The mark follows the name, and what the name notes follows both, all inside the heading's one line.
                markAfter: !mark || (mark.left >= title!.right - 2.5 && within(mark)),
                subAfter: !sub || sub.width === 0 || (sub.left >= title!.right && within(sub))
              };
            }, field);
          // Folded, the card that colours the map has the mark; open, the number's has its figures.
          expect(await one('archive'), `archive at ${width} px`).toEqual({head: true, oneLine: true, markAfter: true, subAfter: true});
          expect(await p.evaluate(() => document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector('mosaica-field-card')!.shadowRoot!.querySelector('.painted') !== null)).toBe(true);
          await open(p, 'population');
          expect(await one('population'), `population at ${width} px`).toEqual({head: true, oneLine: true, markAfter: true, subAfter: true});
          await p.close();
        }
      });

      it('opens the range box over what is under the plot, its two fields on one line, from a press on a bin or from a button drawn only for the keyboard', async () => {
        const p = await panel();
        const card = 'mosaica-field-card[data-field="submitted_at"]';
        const at = () =>
          p.locator(card).evaluate((c) => {
            const s = c.shadowRoot!;
            const box = s.querySelector('[part="brush"]');
            const [from, to] = ['[part="brush-from"]', '[part="brush-to"]'].map((selector) => s.querySelector(selector)?.getBoundingClientRect());
            const button = s.querySelector('[part="range"]')!.getBoundingClientRect();
            const plot = s.querySelector('[part="plot"]')!.getBoundingClientRect();
            return {
              card: Math.round(c.getBoundingClientRect().height * 10) / 10,
              box: box ? getComputedStyle(box).position : null,
              fields: from && to ? Math.abs(from.top - to.top) < 0.5 && from.right <= to.left : null,
              // The box is kept inside the plot while it is no wider than the 210 px main's rule leaves it.
              inside: box ? box.getBoundingClientRect().width > 210 || (box.getBoundingClientRect().left >= plot.left - 0.5 && box.getBoundingClientRect().right <= plot.right + 0.5) : null,
              button: button.width > 1.5
            };
          });
        const rest = await at();
        expect(rest).toMatchObject({box: null, button: false});
        // A press on one bin, with no drag.
        await p.locator(`${card} [part~="bin"]`).nth(2).click();
        await settled(p);
        expect(await at()).toEqual({card: rest.card, box: 'absolute', fields: true, inside: true, button: false});
        await p.keyboard.press('Escape');
        await settled(p);
        expect(await at()).toEqual(rest);
        // The plot, then the button, which the card is no taller for while it is drawn.
        await p.keyboard.press('Tab');
        expect(await focused(p)).toMatchObject({part: 'range'});
        expect(await at()).toEqual({...rest, button: true});
        await p.keyboard.press('Enter');
        await settled(p);
        expect(await focused(p)).toMatchObject({part: 'brush-from'});
        expect(await at()).toEqual({card: rest.card, box: 'absolute', fields: true, inside: true, button: false});
        // What cannot be applied is said inside the box, which is no wider for the words.
        const before = await p.locator(`${card} [part="brush"]`).evaluate((b) => b.getBoundingClientRect().width);
        await p.keyboard.type('soon');
        await p.keyboard.press('Tab');
        await settled(p);
        const said = await p.locator(card).evaluate((c) => {
          const box = c.shadowRoot!.querySelector('[part="brush"]')!;
          const words = box.querySelector('#from-error')!;
          return {width: box.getBoundingClientRect().width, words: words.textContent!.includes('soon'), inside: words.getBoundingClientRect().right <= box.getBoundingClientRect().right};
        });
        expect(said).toEqual({width: before, words: true, inside: true});
      });

      it('hatches a highlighted range, so it is told from a filtered one by more than its colour', async () => {
        const p = await panel();
        const band = p.locator('mosaica-field-card[data-field="submitted_at"] [part~="band"][data-verb="highlight"]');
        expect(await band.evaluate((b) => getComputedStyle(b).backgroundImage.includes('gradient'))).toBe(true);
        // A pressed Filter stays filled, its icon solid, under the pointer, beside a Highlight that is not.
        const verbs = p.locator('mosaica-field-card [part~="row"][data-key="math"] [part="verbs"]');
        await p.locator('mosaica-field-card [part~="row"][data-key="math"] [part="name"]').hover();
        await verbs.locator('[part="filter"]').hover();
        const drawn = (part: string) => verbs.locator(`[part="${part}"]`).evaluate((b) => [getComputedStyle(b).backgroundColor, getComputedStyle(b.querySelector('svg')!).fill]);
        expect(await drawn('filter')).toEqual(['rgb(27, 29, 33)', 'rgb(255, 255, 255)']);
        expect((await drawn('highlight'))[1]).toBe('none');
      });

      it('under chips-only, puts focus on words it draws once the last chip is removed from the keyboard', async () => {
        const p = await panel('chips-only');
        await p.evaluate(() => document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector<HTMLElement>('[part="chip"] > button:last-child')!.focus());
        for (let i = 0; i < 3; i++) {
          await p.keyboard.press('Enter');
          await settled(p);
        }
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
        const order = () => p.evaluate(() => [...document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector('mosaica-field-card')!.shadowRoot!.querySelectorAll<HTMLElement>('[part~="row"]')].map((r) => r.dataset['key']));
        expect(await order()).toEqual(['cs', 'math', LONG, 'stat', 'physics']);
        await p.evaluate(() => {
          const card = document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector('mosaica-field-card')!.shadowRoot!;
          card.querySelector<HTMLElement>('[part~="row"][data-key="physics"] [part="filter"]')!.focus();
        });
        await p.keyboard.press('Enter');
        await settled(p);
        // The values the clauses name are listed first, so the last row is the third.
        expect(await order()).toEqual(['cs', 'math', 'physics', LONG, 'stat']);
        expect(await focused(p)).toMatchObject({part: 'filter', row: 'physics'});
        expect((await focused(p)).name).toContain('physics');

        await p.evaluate(() => document.querySelector('mosaica-filter-panel')!.shadowRoot!.querySelector<HTMLElement>('[part="chip"][data-column="archive"][data-verb="filter"] > button:last-child')!.focus());
        await p.keyboard.press('Enter');
        await settled(p);
        expect(await focused(p)).toMatchObject({tag: 'button', chip: 'highlight archive'});
        await p.keyboard.press('Enter');
        await settled(p);
        expect(await focused(p)).toMatchObject({tag: 'button', chip: 'highlight submitted_at'});
        await p.keyboard.press('Enter');
        await settled(p);
        expect(await focused(p)).toMatchObject({tag: 'button', part: 'add'});
      });
    });
  });
}
