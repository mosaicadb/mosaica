import type {Layer} from '@deck.gl/core';
import type {Store} from '@mosaicajs/client';
import {basemapLayers, basemapMisfit, type BasemapValue, type DensityCamera} from '@mosaicajs/deck';
import {basemapValueError, frameKey, tileCover, withoutUndefined} from '@mosaicajs/deck/internal';

/**
 * What `<mosaica-map>`'s `basemap` takes: one {@link BasemapValue} for every view, a mapping from
 * view id to one, or a deck.gl layer the host built, drawn as it is.
 */
export type BasemapChoice = BasemapValue | Readonly<Record<string, BasemapValue>> | Layer;

/** How long the camera must be still before tiles are composed for it, so a wheel gesture composes once. */
const SETTLE_MS = 180;

/** How long after a basemap fails to load before a change asks for it again: doubling from the first to the last. */
const RETRY_FIRST_MS = 1_000;
const RETRY_MAX_MS = 60_000;

/**
 * What the map tells its basemap: the store, the choice, the ground, and the camera where the map
 * has a size.
 */
export type BasemapInput = {store: Store | null; choice: BasemapChoice | null; scheme: 'light' | 'dark'; camera: DensityCamera | undefined};

/** A plain object, as a host or the notebook writes a value or a mapping: not a class instance, an array or a Map. */
function isPlain(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && Object.getPrototypeOf(value) === Object.prototype;
}

/** Whether `value` is written as one tile or image basemap: a plain object with a `tiles` or an `image` key. */
function isSingle(value: Readonly<Record<string, unknown>>): boolean {
  return Object.hasOwn(value, 'tiles') || Object.hasOwn(value, 'image');
}

/**
 * What a choice is: a single value, a mapping, a layer, or none of them
 * and why. A plain object with a `tiles` or `image` key is a single value, checked on what it
 * holds; any other plain object is a mapping, each of its values checked as a single value is.
 */
function read(choice: unknown): {mapping: Readonly<Record<string, BasemapValue>>} | {value: unknown} | {layer: Layer} | {wrong: string} {
  if (choice === null || choice === undefined) return {value: undefined};
  // A layer holds a `clone` method, which no mapping's value can be.
  if (isLayer(choice)) return {layer: choice};
  if (isPlain(choice)) choice = withoutUndefined(choice);
  if (isPlain(choice) && !isSingle(choice)) {
    const bad = mappingError(choice);
    if (bad) return {wrong: `basemap maps view ids to basemaps, and the entry for ${JSON.stringify(bad[0])} is not one: ${bad[1]}`};
    return {mapping: Object.fromEntries(Object.entries(choice).map(([k, v]) => [k, withoutUndefined(v)])) as Readonly<Record<string, BasemapValue>>};
  }
  const error = basemapValueError(choice);
  return error === null ? {value: choice} : {wrong: error};
}

/** A mapping's first entry that is not a basemap value, and why; an entry whose value is undefined is left out. */
function mappingError(mapping: Readonly<Record<string, unknown>>): [string, string] | null {
  for (const [key, value] of Object.entries(mapping)) {
    const error = basemapValueError(value);
    if (error !== null) return [key, error];
  }
  return null;
}

function isLayer(choice: unknown): choice is Layer {
  return typeof choice === 'object' && choice !== null && 'props' in choice && 'id' in choice && typeof (choice as {clone?: unknown}).clone === 'function';
}

/**
 * Whether `basemap` changed: a layer by identity, any other value by what it holds, so a host that
 * writes an equal literal on every render does not start the basemap again.
 */
export function basemapChanged(value: unknown, old: unknown): boolean {
  if (isLayer(value) || isLayer(old)) return value !== old;
  // A Map or an object with no prototype is refused; the equal plain object is read afresh.
  if (isPlain(value) !== isPlain(old)) return true;
  const a = keyOf(value);
  return a === null || a !== keyOf(old);
}

/** A value as JSON with each object's keys sorted, so key order does not tell values apart; `null` where it cannot be written so. */
function keyOf(value: unknown): string | null {
  try {
    return JSON.stringify(value, (_, v: unknown) => (isPlain(v) ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) : v)) ?? 'undefined';
  } catch {
    return null;
  }
}

/**
 * The map's basemap: which value the current view draws, what draws it, and what the map says
 * about it. A value that does not fit the view it was given on, a value a mapping names for a view
 * it does not fit, and a choice that is none of the forms are refusals; a single value that does
 * not fit a view switched to later is a note, and draws nothing there.
 *
 * {@link layers} are never handed to deck.gl: the map draws a clone of each, so a layer deck has
 * finalised, after the map left the page, is never drawn again.
 */
export class MapBasemap {
  /** What to draw under the points, as layers to clone. */
  layers: readonly Layer[] = [];
  /** Why the value given cannot be drawn under the view it was given for. */
  refusal: string | null = null;
  /** Why nothing is drawn under this view, where the value fits another or did not load. */
  note: string | null = null;
  /** The tile server's credit, while tiles are drawn. */
  attribution: string | null = null;

  /** The view a single value was given on, which a misfit there refuses. */
  private givenOn: string | null = null;
  /** The frame, value and ground last decided; a change asks again. */
  private asked = '';
  /** Moved once per decision; an answer asked for under an older one is dropped. */
  private generation = 0;
  private frame = '';
  private tiles = false;
  /** What last failed to load, when a change may ask for it again, and how long the wait after its next failure is. */
  private failed = {key: '', retryAt: 0, note: '', wait: RETRY_FIRST_MS};
  /** Each value's part of the key, written once per value given. */
  private valueKeys = new WeakMap<object, string>();
  private settle: ReturnType<typeof setTimeout> | null = null;
  private input: BasemapInput | null = null;

  constructor(private readonly changed: () => void) {}

  /** Start again for a new value or a new store: draw nothing until it is decided. */
  reset(): void {
    this.givenOn = null;
    this.asked = '';
    this.failed = {key: '', retryAt: 0, note: '', wait: RETRY_FIRST_MS};
    this.valueKeys = new WeakMap();
    this.tiles = false;
    this.generation += 1;
    this.show([], null, null, null);
  }

  /** Stop following the camera and drop any answer in flight, as the map leaves the page. The next {@link sync} asks again. */
  dispose(): void {
    if (this.settle !== null) clearTimeout(this.settle);
    this.settle = null;
    this.asked = '';
    this.generation += 1;
  }

  /** Follow the camera: tiles are composed again once it has rested. */
  moved(input: BasemapInput): void {
    this.input = input;
    if (this.settle !== null) clearTimeout(this.settle);
    this.settle = null;
    if (!this.tiles) return;
    this.settle = setTimeout(() => {
      this.settle = null;
      if (this.input) this.sync(this.input, true);
    }, SETTLE_MS);
  }

  /**
   * Decide what the current view draws, and ask for it where the frame, the value or the ground
   * changed. `recompose` asks again for tiles at the camera as it stands.
   */
  sync(input: BasemapInput, recompose = false): void {
    this.input = input;
    const {store, choice, scheme} = input;
    const meta = store?.get('meta');
    const view = meta?.views?.find((v) => v.id === store!.get('view').id);
    const chosen = read(choice);
    this.tiles = false;
    if ('layer' in chosen) return this.decided('layer', () => this.show([chosen.layer], null, null, null));
    if (!store || !view) return this.decided('', () => this.show([], null, null, null));
    this.givenOn ??= view.id;
    const frame = frameKey(view);
    if (frame !== this.frame) {
      this.frame = frame;
      this.show([], this.refusal, this.note, null);
    }
    if ('wrong' in chosen) return this.decided(`refused|${chosen.wrong}`, () => this.show([], chosen.wrong, null, null));
    const own = 'mapping' in chosen ? (Object.hasOwn(chosen.mapping, view.id) ? chosen.mapping[view.id] : undefined) : chosen.value;
    const value = (own ?? (view.projection === 'none' ? 'none' : 'world')) as BasemapValue;
    const misfit = basemapMisfit(view, value);
    this.tiles = misfit === null && typeof value === 'object' && 'tiles' in value;
    if (misfit !== null) {
      const refused = 'mapping' in chosen || view.id === this.givenOn;
      return this.decided(`misfit|${frame}|${misfit}`, () => this.show([], refused ? misfit : null, refused ? null : misfit, null));
    }
    // Tiles fail by the cover they asked for: a pan to other tiles asks for them at once.
    const cover = this.tiles && view.tile ? tileCover(view.tile.z, input.camera) : null;
    const key = `${frame}|${scheme}|${this.valueKey(value)}`;
    const failedKey = cover ? `${key}|${cover.level}/${cover.x0}/${cover.y0}/${cover.x1}/${cover.y1}` : key;
    if (value === 'none') return this.decided(key, () => this.show([], null, null, null));
    if (key === this.asked && !(recompose && this.tiles)) return;
    if (failedKey === this.failed.key && performance.now() < this.failed.retryAt) return this.show([], null, this.failed.note, null);
    // Tiles that failed are asked for at a new cover only once the camera rests there.
    if (cover && !recompose && failedKey !== this.failed.key && this.failed.key.startsWith(`${key}|`)) return this.show([], null, this.failed.note, null);
    this.asked = key;
    const mine = ++this.generation;
    const attribution = this.tiles ? ((value as {attribution?: string}).attribution ?? null) : null;
    basemapLayers(store, view, value, {scheme, camera: input.camera}).then(
      (layers) => {
        if (mine !== this.generation) return;
        this.failed = {key: '', retryAt: 0, note: '', wait: RETRY_FIRST_MS};
        this.show(layers, null, null, attribution);
      },
      (error: unknown) => {
        if (mine !== this.generation) return;
        // Asked again at a change after a wait that doubles while it goes on failing, and starts
        // again for another value, ground or set of tiles.
        const note = `The basemap did not load: ${error instanceof Error ? error.message : String(error)}`;
        const wait = failedKey === this.failed.key ? this.failed.wait : RETRY_FIRST_MS;
        this.asked = '';
        this.failed = {key: failedKey, retryAt: performance.now() + wait, note, wait: Math.min(2 * wait, RETRY_MAX_MS)};
        this.show([], null, note, null);
      }
    );
  }

  /** Settle on an outcome that needs no request, once per outcome. */
  private decided(key: string, act: () => void): void {
    if (key === this.asked) return;
    this.asked = key;
    this.generation += 1;
    act();
  }

  private valueKey(value: BasemapValue): string {
    if (typeof value === 'string') return value;
    let key = this.valueKeys.get(value);
    if (key === undefined) this.valueKeys.set(value, (key = keyOf(value) ?? ''));
    return key;
  }

  private show(layers: readonly Layer[], refusal: string | null, note: string | null, attribution: string | null): void {
    if (layers.length === this.layers.length && layers.every((l, i) => l === this.layers[i]) && refusal === this.refusal && note === this.note && attribution === this.attribution) return;
    Object.assign(this, {layers, refusal, note, attribution});
    this.changed();
  }
}
