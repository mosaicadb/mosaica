import {describe, expect, it} from 'vitest';
import {chrome, tokens} from '../src/tokens.js';

/**
 * The theme's default colours against each other, in both schemes, by the contrast ratio of their
 * relative luminances: text at 4.5:1 on every surface it is written on, and what is drawn without
 * words (a border, a ring, a track, a knob, a dot, a bar) at 3:1 on what it is drawn on. Each
 * colour is read from the theme's own CSS: a token's default from `tokens`, and the token a shared
 * rule draws with from `chrome`.
 */

const SCHEMES = ['light', 'dark'] as const;
type Scheme = (typeof SCHEMES)[number];
const SURFACES = ['surface', 'surface-2', 'surface-3'];

/** Each colour token's default pair. */
const DEFAULTS = new Map(
  [...tokens.cssText.matchAll(/--_mosaica-([a-z0-9-]+):\s*var\(--mosaica-\1,\s*light-dark\((#[0-9a-f]{6}),\s*(#[0-9a-f]{6})\)\)/g)].map((m) => [m[1]!, {light: m[2]!, dark: m[3]!}])
);

function hex(token: string, scheme: Scheme): string {
  const pair = DEFAULTS.get(token);
  if (!pair) throw new Error(`no light and dark default for ${token}`);
  return pair[scheme];
}

/** The token a shared rule gives `property`, from the first rule whose selectors include this one. */
function drawn(selector: string, property: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const body = new RegExp(`(?:^|[},/])\\s*${escaped}\\s*(?:,[^{]*)?\\{([^}]*)\\}`).exec(chrome.cssText)?.[1];
  const token = body && new RegExp(`(?:^|;)\\s*${property}:[^;]*var\\(--_mosaica-([a-z0-9-]+)\\)`).exec(body)?.[1];
  if (!token) throw new Error(`${selector} has no ${property} from a token`);
  return token;
}

function luminance(colour: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(colour.slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
}

function contrast(a: string, b: string, scheme: Scheme): number {
  const [low, high] = [luminance(hex(a, scheme)), luminance(hex(b, scheme))].sort((x, y) => x - y);
  return (high! + 0.05) / (low! + 0.05);
}

describe.each(SCHEMES)('the default colours in the %s scheme', (scheme) => {
  it.each(['ink', 'ink-2', 'ink-3'])('write %s as text on every surface', (ink) => {
    for (const surface of SURFACES) expect(contrast(ink, surface, scheme), `${ink} on ${surface}`).toBeGreaterThanOrEqual(4.5);
  });

  it("draw an input's border against the surface", () => {
    const border = drawn(":where(.input, input:not([type='checkbox']):not([type='radio']), select)", 'border');
    expect(contrast(border, 'surface', scheme)).toBeGreaterThanOrEqual(3);
  });

  it.each([':focus-visible', '.input:focus-within'])('draw the focus ring of %s against every surface and the map', (selector) => {
    const ring = drawn(selector, 'outline');
    for (const ground of [...SURFACES, 'map-bg']) expect(contrast(ring, ground, scheme), `on ${ground}`).toBeGreaterThanOrEqual(3);
  });

  it.each(['.switch', ".switch[aria-checked='true']"])('draw the track of %s against the surface, and its knob against the track', (selector) => {
    const track = drawn(selector, 'background');
    expect(contrast(track, 'surface', scheme), 'track').toBeGreaterThanOrEqual(3);
    expect(contrast(drawn('.switch .knob', 'background'), track, scheme), 'knob').toBeGreaterThanOrEqual(3);
  });

  it.each([
    ['.dot', 'background'],
    ['.dot.quiet', 'border'],
    ['.dot.warn', 'background'],
    ['.dot.refuse', 'background']
  ])('draw %s against every surface', (selector, property) => {
    const dot = drawn(selector, property);
    for (const surface of SURFACES) expect(contrast(dot, surface, scheme), `on ${surface}`).toBeGreaterThanOrEqual(3);
  });

  it.each([".seg button[aria-pressed='true']", ".list > .item[aria-selected='true']", '.list > .item.on'])('mark %s with a bar that stands against its background', (selector) => {
    const bar = drawn(selector, 'box-shadow');
    expect(contrast(bar, drawn(selector, 'background'), scheme)).toBeGreaterThanOrEqual(3);
  });
});
