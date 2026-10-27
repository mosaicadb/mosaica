import {retryDelayMs, type Clock, type RetryOptions} from './driver.js';
import {PartialAggregate} from './aggregate.js';
import {MosaicaError, jsonBody} from './client.js';
import {refusalOf, type Refusal} from './presented.js';
import {paletteOfSize, type PaletteName} from './palette.js';
import type {AggregateBy, AggregateCut, AggregateRequest, AggregateResult, AggregateTable, FilterExpr, Grouping} from './types.js';

/**
 * One grouping in an {@link AggregateSpec}. The same as a {@link Grouping}, except that a layer
 * grouping's `cut` and `paletteSize` can be `'drawn'` to use the map's current values.
 * `cut: 'drawn'` ranks a `nested` or `dag` layer's artifacts at the cut the map currently draws,
 * under the cluster budget ({@link Store.setClusterBudget}). `paletteSize: 'drawn'` returns each
 * artifact's slot in the palette the map colours clusters from ({@link Store.setPalette}). Both
 * are read when each request is sent.
 *
 * @category Store
 */
export type AggregateSpecGrouping = Omit<Grouping, 'by'> & {
  /** As {@link Grouping.by}, except that a layer's `cut` and `paletteSize` may be `'drawn'`. */
  by?: Exclude<AggregateBy, {layer: string}> | DrawnLayerBy;
};

/**
 * A layer grouping in an {@link AggregateSpecGrouping}. `cut` and `paletteSize` may be `'drawn'`,
 * meaning the cut and palette size the map draws that layer with.
 *
 * @category Store
 */
export type DrawnLayerBy = Extract<AggregateBy, {layer: string}> extends infer B
  ? B extends {layer: string}
    ? Omit<B, 'cut' | 'paletteSize'> & {cut?: AggregateCut | 'drawn'; paletteSize?: number | 'drawn'}
    : never
  : never;

/** Whether `grouping` ranks at the cut the store's map draws. */
export function isDrawn(grouping: AggregateSpecGrouping): boolean {
  return grouping.by !== undefined && 'cut' in grouping.by && grouping.by.cut === 'drawn';
}

/** Whether `grouping` asks for slots in the palette the store's map colours clusters from. */
export function isPaletteDrawn(grouping: AggregateSpecGrouping): boolean {
  return grouping.by !== undefined && 'paletteSize' in grouping.by && grouping.by.paletteSize === 'drawn';
}

/** The palette of the slots `body` asks for, or `null` where no grouping asks for slots. */
function paletteOf(body: AggregateBody): PaletteName | null {
  for (const g of body.groupings) {
    const by = g.by;
    if (by && 'layer' in by && by.paletteSize !== undefined) return paletteOfSize(by.paletteSize);
  }
  return null;
}

/**
 * A live aggregate for {@link Store.setAggregate}: the groupings to count, which items to count,
 * and optionally a set to compare with. The store fills in the request's filters from its own
 * filters and selected region.
 *
 * @category Store
 */
export type AggregateSpec = {
  /** The groupings. The result has one table for each, in the same order. */
  groupings: AggregateSpecGrouping[];
  /**
   * Which items to count. `match`, the default, counts the items that match the store's filters and
   * selected region. `view` counts the same items within the area in view: the selected region if
   * there is one, otherwise the camera's box; it is fetched again once the camera has rested for
   * 250 ms, and `reference` is limited to the same area. `visible` counts every item the viewer may
   * see in the view, wherever the camera is, and sends no filters, so `without`, `withoutMembersOf`
   * and `highlighted` have no effect.
   */
  subject?: 'match' | 'view' | 'visible';
  /**
   * A set to compare each count with: `'visible'` for every item the viewer may see in the view, or
   * a filter expression. With it, each table also has `reference_count` and `lift` columns. Unset
   * means no comparison.
   */
  reference?: FilterExpr | 'visible';
  /**
   * A column whose filter control is left out of the counts, keyed as in the {@link FilterDraft}. A
   * filter control uses this to keep counting the values its own clause excludes. The other
   * controls, the `member_of` clauses and the selected region still apply.
   */
  without?: string;
  /**
   * A layer whose `member_of` clauses are left out of the counts, so a control that lists the
   * layer's artifacts keeps counting the ones its clauses exclude. Other layers' clauses and the
   * selected region still apply.
   */
  withoutMembersOf?: string;
  /**
   * Whether to count only the items that also match the store's highlight, so each count matches
   * the viewport's `highlighted` count. Defaults to `false`. Has no effect when nothing is
   * highlighted.
   */
  highlighted?: boolean;
};

/**
 * The state of one aggregate registered with {@link Store.setAggregate}.
 *
 * @category Projections
 */
export type AggregateEntry = {
  /**
   * `loading` while a request is in flight, or while a `view` aggregate waits for the camera to
   * rest. `retrying` while the server is busy (`429` or `503`) and the store waits to ask again.
   * `shown` once the answer has arrived, and `refused` if the request failed.
   */
  status: 'loading' | 'retrying' | 'shown' | 'refused';
  /**
   * The latest answer, kept while the next request loads. `null` before the first answer, after a
   * refusal, after a view switch, and after {@link Store.clear} or a change of the viewer's
   * identity. For a histogram with a `sample`, the table's `sample` says
   * whether the counts were scaled up from a sample.
   */
  result: AggregateResult | null;
  /** The view `result` was counted in, or `null` if there is no result. */
  view: string | null;
  /**
   * The palette the slots in `result` refer to. `null` if there is no result or no grouping asked
   * for palette slots.
   */
  palette: PaletteName | null;
  /** Why the last request failed, or `null`. */
  refusal: Refusal | null;
  /**
   * One entry per grouping, in order: the figures for each field the grouping lists under
   * `summary`, keyed by field name, or `null` for a grouping without `summary`. Empty if there is
   * no result.
   */
  summaries: readonly (ReadonlyMap<string, FieldSummary> | null)[];
};

/**
 * Summary figures for one number or timestamp field, over every item the viewer may see in the
 * view.
 *
 * @category Projections
 */
export type FieldSummary = {
  /** How many items have a finite value. */
  values: bigint;
  /** How many items have no value. */
  missing: bigint;
  /**
   * The smallest finite value: a `bigint` for an integer field, a number for a float field, and
   * milliseconds since the Unix epoch for a timestamp field. `null` if `values` is 0.
   */
  min: number | bigint | null;
  /** The largest finite value, typed as `min` is. */
  max: number | bigint | null;
  /** The mean of the finite values, in milliseconds since the Unix epoch for a timestamp field. `null` if `values` is 0. */
  mean: number | null;
};

/**
 * The `aggregates` projection: each aggregate registered with {@link Store.setAggregate}, by its
 * id.
 *
 * @category Projections
 */
export type AggregatesProjection = ReadonlyMap<string, AggregateEntry>;

/** An aggregate request less its view, which is the store's at the moment it is sent. */
export type AggregateBody = Omit<AggregateRequest, 'view'>;

/** The figures of each field each grouping of `spec` lists under `summary`, as `result` answers them. */
function summariesOf(spec: AggregateSpec, result: AggregateResult): (ReadonlyMap<string, FieldSummary> | null)[] {
  return spec.groupings.map((grouping, i) => {
    const table = result.tables.find((t) => t.grouping === i);
    if (grouping.summary === undefined || table === undefined) return null;
    const figures = new Map<string, FieldSummary>();
    for (const field of grouping.summary) {
      const summary = summaryOf(table, field);
      if (summary !== null) figures.set(field, summary);
    }
    return figures;
  });
}

function summaryOf(table: AggregateTable, field: string): FieldSummary | null {
  if (table.rows.numRows === 0) return null;
  const at = (figure: string): unknown => table.rows.getChild(`${figure}_${field}`)?.get(0) ?? null;
  const [values, missing] = [at('values'), at('missing')];
  if (typeof values !== 'bigint' || typeof missing !== 'bigint') return null;
  const edge = (v: unknown) => (typeof v === 'bigint' || typeof v === 'number' ? v : null);
  const mean = at('mean');
  return {values, missing, min: edge(at('min')), max: edge(at('max')), mean: typeof mean === 'number' ? mean : null};
}

/**
 * The registered aggregates: one request each in flight at most, a request superseded by a newer
 * one for the same id being aborted, and an answer landing only while it answers the latest ask. A
 * `429` or `503` is sent again after {@link retryDelayMs}; a newer ask cancels the wait.
 *
 * `compose` gives the request a spec sends now, less its view, or `null` where it cannot be sent
 * yet. A request is composed once `ready` has resolved, so it names what the `/v1/meta` read then
 * offers, and `ask` sends it; a retry composes it again. A refresh that asks only where the request
 * changed compares what `compose` gives at the refresh with the request last sent.
 */
export class Aggregates {
  private readonly specs = new Map<string, AggregateSpec>();
  private readonly asking = new Map<string, AbortController>();
  /** Per id: the timer of a retry waiting to be sent. */
  private readonly waiting = new Map<string, unknown>();
  /** Per id: the request last sent, as JSON. */
  private readonly sent = new Map<string, string>();
  private entries: Map<string, AggregateEntry> = new Map();
  private disposed = false;

  constructor(
    private readonly ready: () => Promise<unknown>,
    private readonly ask: (spec: AggregateSpec, body: AggregateBody, signal: AbortSignal) => Promise<{result: AggregateResult; view: string}>,
    private readonly publish: (entries: AggregatesProjection) => void,
    private readonly clock: Clock,
    private readonly retry: RetryOptions,
    private readonly compose: (spec: AggregateSpec) => AggregateBody | null
  ) {}

  /**
   * Register `spec` under `id`, replacing what it held, and ask, or with `now` false hold it as
   * loading until a refresh asks; `null` drops the id.
   */
  set(id: string, spec: AggregateSpec | null, now = true): void {
    if (this.disposed) return;
    if (spec === null) {
      this.stop(id);
      this.specs.delete(id);
      this.sent.delete(id);
      if (this.entries.delete(id)) this.replace(new Map(this.entries));
      return;
    }
    this.specs.set(id, spec);
    this.run([id], true, false, now);
  }

  /**
   * Ask again for the registered aggregates `which` selects, every one by default. With `drop`,
   * the answers held are dropped at once, as after a view switch; without it they stay until the
   * new ones land. With `changed`, an aggregate whose request is the one last sent, and was not
   * refused, is left as it is.
   */
  refresh(drop: boolean, options: {which?: (spec: AggregateSpec) => boolean; changed?: boolean} = {}): void {
    if (this.disposed) return;
    const which = options.which ?? (() => true);
    this.run(
      [...this.specs].filter(([, spec]) => which(spec)).map(([id]) => id),
      drop,
      options.changed ?? false,
      true
    );
  }

  /** Abort every request in flight. Nothing lands afterwards. */
  dispose(): void {
    this.disposed = true;
    for (const id of [...this.asking.keys(), ...this.waiting.keys()]) this.stop(id);
  }

  /** Abort `id`'s request in flight and cancel its waiting retry. */
  private stop(id: string): void {
    this.asking.get(id)?.abort();
    this.asking.delete(id);
    const timer = this.waiting.get(id);
    if (timer !== undefined) this.clock.cancel(timer);
    this.waiting.delete(id);
  }

  private run(ids: string[], drop: boolean, changed: boolean, now: boolean): void {
    const next = new Map(this.entries);
    const asked: string[] = [];
    for (const id of ids) {
      const body = now ? this.compose(this.specs.get(id)!) : null;
      const key = body === null ? null : jsonBody(body);
      const held = next.get(id);
      if (changed && key !== null && key === this.sent.get(id) && held?.status !== 'refused') continue;
      // Still waiting to be sendable, as it was.
      if (changed && key === null && !this.sent.has(id) && held?.status === 'loading') continue;
      this.stop(id);
      const kept = drop ? null : (held?.result ?? null);
      next.set(id, {
        status: 'loading',
        result: kept,
        view: kept === null ? null : (held?.view ?? null),
        palette: kept === null ? null : (held?.palette ?? null),
        refusal: null,
        summaries: kept === null ? [] : (held?.summaries ?? [])
      });
      if (key === null) {
        this.sent.delete(id);
      } else {
        this.sent.set(id, key);
        asked.push(id);
      }
    }
    if (ids.every((id) => next.get(id) === this.entries.get(id))) return;
    this.replace(next);
    for (const id of asked) void this.one(id, this.specs.get(id)!, 0);
  }

  private async one(id: string, spec: AggregateSpec, attempt: number): Promise<void> {
    const controller = new AbortController();
    this.asking.set(id, controller);
    let entry: AggregateEntry;
    let wait: number | null = null;
    try {
      await this.ready();
      if (this.asking.get(id) !== controller || controller.signal.aborted) return;
      const body = this.compose(spec);
      if (body === null) {
        // Not sendable under the meta just read, as a `view` aggregate with no camera; it stays
        // loading until a refresh finds it sendable.
        this.asking.delete(id);
        this.sent.delete(id);
        return;
      }
      this.sent.set(id, jsonBody(body));
      const {result, view} = await this.ask(spec, body, controller.signal);
      entry = {status: 'shown', result, view, palette: paletteOf(body), refusal: null, summaries: summariesOf(spec, result)};
    } catch (error) {
      wait = retryDelayMs(error, attempt, this.retry);
      const held = this.entries.get(id);
      entry =
        wait === null
          ? {status: 'refused', result: null, view: null, palette: null, refusal: refusalOf(error), summaries: []}
          : {status: 'retrying', result: held?.result ?? null, view: held?.view ?? null, palette: held?.palette ?? null, refusal: refusalOf(error), summaries: held?.summaries ?? []};
    }
    // A newer ask for this id, a drop or a dispose has taken its place.
    if (this.asking.get(id) !== controller || controller.signal.aborted) return;
    this.asking.delete(id);
    const next = new Map(this.entries);
    next.set(id, entry);
    this.replace(next);
    if (wait === null) return;
    this.waiting.set(
      id,
      this.clock.after(wait, () => {
        this.waiting.delete(id);
        if (!this.disposed && this.specs.get(id) === spec) void this.one(id, spec, attempt + 1);
      })
    );
  }

  private replace(next: Map<string, AggregateEntry>): void {
    this.entries = next;
    this.publish(next);
  }
}

/** One caller's share of a joined request; `dropped` is set once the request is sent. */
type Part = {
  groupings: Grouping[];
  signal: AbortSignal;
  resolve: (result: AggregateResult) => void;
  reject: (error: unknown) => void;
  dropped: (() => void) | null;
};

/** The parts waiting to be sent as one request. */
type Batch = {token: string; req: Omit<AggregateRequest, 'groupings'>; parts: Part[]};

/**
 * How many microtasks a request waits for others to join it. Elements that answer one store change
 * update one after another within the same task, each in a microtask of its own, and their requests
 * arrive within this many of each other. Counting microtasks rather than waiting for a timer keeps
 * the join independent of the clock.
 */
const JOIN_MICROTASKS = 64;

/**
 * Sends aggregate requests, joining those that differ only in their groupings into one request
 * when they are asked for together. Elements that answer one store change each register their own
 * aggregate, and over the same filters they are one read of the same set. Each caller gets its own
 * tables back, numbered from 0. A caller whose signal aborts is refused at once, and the joined
 * request is aborted once every caller's signal has. Where a part would take a request past
 * `maxGroupings`, that request is closed to further parts and the part starts the next one; a part
 * past it on its own is sent alone. Where the joined request is refused as a contract error or cut
 * short, each part is sent again alone, so one caller's grouping does not refuse the others.
 *
 * @internal
 */
export function joinedAggregate(
  send: (token: string, req: AggregateRequest, signal: AbortSignal) => Promise<AggregateResult>,
  maxGroupings: () => number
): (token: string, req: AggregateRequest, signal: AbortSignal) => Promise<AggregateResult> {
  /** The batch still open to parts, by the request its parts share. */
  const open = new Map<string, Batch>();

  // A send that throws rather than rejecting is that caller's refusal all the same. A read that a
  // later request's error stopped is that error, since a part asked again reads every page.
  const sent = async (token: string, req: AggregateRequest, signal: AbortSignal): Promise<AggregateResult> => {
    try {
      return await send(token, req, signal);
    } catch (error) {
      throw error instanceof PartialAggregate && error.cause !== undefined ? error.cause : error;
    }
  };

  const alone = (token: string, req: Omit<AggregateRequest, 'groupings'>, part: Part): void => {
    if (!part.signal.aborted) sent(token, {...req, groupings: part.groupings}, part.signal).then(part.resolve, part.reject);
  };

  const flush = (key: string, batch: Batch): void => {
    if (open.get(key) === batch) open.delete(key);
    const live = batch.parts.filter((p) => !p.signal.aborted);
    if (live.length === 0) return;
    if (live.length === 1) return alone(batch.token, batch.req, live[0]!);
    const controller = new AbortController();
    let left = live.length;
    for (const part of live) {
      part.dropped = () => {
        if (--left === 0) controller.abort();
      };
    }
    sent(batch.token, {...batch.req, groupings: live.flatMap((p) => p.groupings)}, controller.signal).then(
      (result) => {
        let from = 0;
        for (const part of live) {
          const own = (t: AggregateResult['tables'][number]) => t.grouping >= from && t.grouping < from + part.groupings.length;
          const tables = result.tables.filter(own).map((t) => ({...t, grouping: t.grouping - from}));
          from += part.groupings.length;
          part.resolve({...result, tables});
        }
      },
      (error: unknown) => {
        const separately = error instanceof PartialAggregate || (error instanceof MosaicaError && error.status === 422);
        for (const part of live) {
          if (separately) alone(batch.token, batch.req, part);
          else part.reject(error);
        }
      }
    );
  };

  return (token, req, signal) =>
    new Promise<AggregateResult>((resolve, reject) => {
      if (signal.aborted) return reject(signal.reason);
      const {groupings, ...rest} = req;
      const part: Part = {groupings, signal, resolve, reject, dropped: null};
      signal.addEventListener(
        'abort',
        () => {
          reject(signal.reason);
          part.dropped?.();
        },
        {once: true}
      );
      if (groupings.length > maxGroupings()) return alone(token, rest, part);
      const key = jsonBody([token, rest]);
      const batch = open.get(key);
      if (batch && batch.parts.reduce((n, p) => n + p.groupings.length, 0) + groupings.length <= maxGroupings()) {
        batch.parts.push(part);
        return;
      }
      // A full batch is sent when its own wait ends.
      const started: Batch = {token, req: rest, parts: [part]};
      open.set(key, started);
      const hop = (left: number): void => (left === 0 ? flush(key, started) : queueMicrotask(() => hop(left - 1)));
      hop(JOIN_MICROTASKS);
    });
}
