import type {Composition} from './compose.js';
import {ValueReservoir, countCodesCached, countCodesInPiece, extendRanks, lacksValues, sizesPoints, widenDomain, widenDomainOver, type Domain, type Ranks, type ValueSample} from './encoding.js';
import {refusalOf, type Refusal} from './presented.js';
import type {CategoryValue, DeclaredScalar} from './types.js';

/**
 * The prefix that makes a colour setting name a layer instead of a column, as in
 * `setColourBy('cluster:topics')`, which colours points by the `topics` layer's clusters.
 *
 * @category Layers and views
 */
export const CLUSTER_PREFIX = 'cluster:';

/**
 * The `legend` projection: what the points are coloured and sized by, and what a legend needs to
 * explain it. It builds up from the marks drawn: names and palette ranks for a category column, a
 * range for a number column, and for the size column whether any point has no value. Colouring by
 * a layer (`cluster:<layer>`) adds nothing here. {@link Store.clear} empties it but keeps
 * `colourBy` and `sizeBy`.
 *
 * @category Projections
 */
export type LegendProjection = {
  /**
   * Palette ranks for each category column, by code. Values are ranked by how many items with that
   * value the viewer may see across the whole view, most first, so a value gets the same colour each time
   * the same data is loaded. If the server refuses that count, codes are ranked by how often they
   * appear among the marks drawn. A rank does not change once given. A code is drawn in the
   * unmapped colour until its rank arrives.
   */
  ranks: Record<string, Ranks>;
  /** The range of values drawn for each number column. It widens as marks arrive and does not narrow. */
  domains: Record<string, Domain>;
  /** A sample of the values drawn, for a column sized by rank. */
  samples: Record<string, ValueSample>;
  /** For the column sized by, whether any point drawn had no finite value. Such points are drawn as rings. */
  missing: Record<string, boolean>;
  /** The codes drawn for each category column, with their names. */
  categories: Record<string, CategoryValue[]>;
  /** For each category column whose names could not be fetched, why. The store does not ask again until it is cleared. */
  categoryErrors: Record<string, Refusal>;
  /** What the points are coloured by, as `setColourBy` last set it: a column, `cluster:<layer>`, or `null` for one colour. */
  colourBy: string | null;
  /** The number column the points are sized by, as `setSizeBy` last set it, or `null` for one size. */
  sizeBy: string | null;
};

/** The legend with nothing accumulated and nothing chosen. */
export const EMPTY_LEGEND: LegendProjection = {ranks: {}, domains: {}, samples: {}, missing: {}, categories: {}, categoryErrors: {}, colourBy: null, sizeBy: null};

/**
 * The legend of the colour and size columns, accumulated from the marks drawn: ranks and names for
 * a category, a domain for a number, and a sample of the size column's values. A cluster colour
 * accumulates nothing here, because the vis side resolves it from the session artifact table.
 */
export class Legend {
  private state: LegendProjection = EMPTY_LEGEND;
  /** Whether the size column is sized by rank, and so sampled. */
  private rank = false;
  /** The reservoir behind the size column's sample while it is sized by rank. */
  private reservoir: ValueReservoir | null = null;
  /** The size column's bands already looked at for points with no value. */
  private scanned = new WeakSet<object>();
  /** Moved by {@link clear}, so names and orders asked for before it are not taken. */
  private epoch = 0;
  /** Per category column, its values' order by count as {@link order} answered, by key, or whether it is still asked or was refused. */
  private orders = new Map<string, Map<string, number> | 'asked' | 'refused'>();
  /** Per category column whose order is asked for, the codes the marks brought meanwhile, with their counts. */
  private waiting = new Map<string, Map<number, number>>();
  /** Per category column, the codes whose names were asked for, so none is asked twice. */
  private naming = new Map<string, Set<number>>();
  private disposed = false;

  constructor(
    /** Names for category codes, asked for when codes are first drawn. */
    private readonly resolve: (column: string, codes: number[]) => Promise<CategoryValue[]>,
    private readonly publish: (legend: LegendProjection) => void,
    /**
     * The keys of a category column's commonest values among the items this viewer may see, most
     * first and ties by key. Without it, codes are ranked as the marks bring them.
     */
    private readonly order: ((column: string) => Promise<string[]>) | null = null
  ) {}

  get colourBy(): string | null {
    return this.state.colourBy;
  }

  setColourBy(column: string | null): void {
    this.set({...this.state, colourBy: column});
  }

  get sizeBy(): string | null {
    return this.state.sizeBy;
  }

  /** Size by `column`, keeping a sample of its values where `rank` is set. */
  setSizeBy(column: string | null, rank = false): void {
    if (column !== this.state.sizeBy) this.scanned = new WeakSet();
    if (column !== this.state.sizeBy || rank !== this.rank) this.reservoir = column !== null && rank ? new ValueReservoir() : null;
    this.rank = rank;
    this.set({...this.state, sizeBy: column});
  }

  /** Extend the legend of the colour and size columns over one frame. */
  accumulate(frame: Composition, columns: readonly DeclaredScalar[]): void {
    const colourBy = this.state.colourBy;
    const colour = colourBy && !colourBy.startsWith(CLUSTER_PREFIX) ? columns.find((c) => c.name === colourBy) : undefined;
    if (colour?.category) this.accumulateCodes(frame, colour.name);
    else if (colour) this.widen(frame, colour.name);
    const size = columns.find((c) => c.name === this.state.sizeBy && sizesPoints(c));
    if (size) {
      if (size !== colour) this.widen(frame, size.name);
      this.scanMissing(frame, size.name);
      if (this.reservoir) this.sample(frame, size.name, this.reservoir);
    }
  }

  /** Rank the codes of a category column drawn in the frame, and ask for the names of new ones. */
  private accumulateCodes(frame: Composition, colourBy: string): void {
    const counts = new Map<number, number>();
    for (const band of frame.exact) {
      const values = band.scalars[colourBy];
      if (values) for (const [code, n] of countCodesCached(values)) counts.set(code, (counts.get(code) ?? 0) + n);
    }
    // The stand-in pieces seed the ranks while this depth's own bands stream in.
    if (counts.size === 0) for (const piece of frame.standIn) countCodesInPiece(counts, piece, colourBy);
    if (counts.size === 0) return;
    const order = this.orderOf(colourBy);
    if (order !== 'refused' && !this.state.categoryErrors[colourBy]) {
      if (order === 'asked') {
        const met = this.waiting.get(colourBy) ?? new Map<number, number>();
        for (const [code, n] of counts) met.set(code, (met.get(code) ?? 0) + n);
        this.waiting.set(colourBy, met);
      }
      void this.name(colourBy, counts);
      return;
    }
    this.rankAsMet(colourBy, counts);
  }

  /** Rank the codes in `counts` that have no rank by how often the marks brought them, and ask for their names. */
  private rankAsMet(column: string, counts: Map<number, number>): void {
    const ranks = extendRanks(this.state.ranks[column] ?? {}, counts);
    if (ranks !== this.state.ranks[column]) {
      this.set({...this.state, ranks: {...this.state.ranks, [column]: ranks}});
      void this.name(column, counts);
    }
  }

  /** The column's order by count: asked for on first use, and `refused` where there is no way to ask. */
  private orderOf(column: string): Map<string, number> | 'asked' | 'refused' {
    const held = this.orders.get(column);
    if (held) return held;
    if (!this.order) {
      this.orders.set(column, 'refused');
      return 'refused';
    }
    this.orders.set(column, 'asked');
    const epoch = this.epoch;
    this.order(column).then(
      (keys) => {
        if (this.disposed || epoch !== this.epoch) return;
        this.waiting.delete(column);
        this.orders.set(column, new Map(keys.map((key, i) => [key, i])));
        this.rankByOrder(column);
      },
      () => {
        if (this.disposed || epoch !== this.epoch) return;
        // The codes the marks brought while the order was asked for are ranked at once.
        this.orders.set(column, 'refused');
        const met = this.waiting.get(column);
        this.waiting.delete(column);
        if (met) this.rankAsMet(column, met);
      }
    );
    return 'asked';
  }

  /**
   * Rank each named code of `column` that has no rank by its value's place in the column's order.
   * A value outside the order is ranked after it, by key, which leaves it unmapped in any palette
   * shorter than the order.
   */
  private rankByOrder(column: string): void {
    const order = this.orders.get(column);
    if (!(order instanceof Map)) return;
    const held = this.state.ranks[column] ?? {};
    const unranked = (this.state.categories[column] ?? []).filter((v) => held[v.code] === undefined);
    if (unranked.length === 0) return;
    const next = {...held};
    let tail = Math.max(order.size, ...Object.values(held).map((r) => r + 1));
    for (const v of unranked.sort((a, b) => (a.key < b.key ? -1 : a.key > b.key ? 1 : 0))) next[v.code] = order.get(v.key) ?? tail++;
    this.set({...this.state, ranks: {...this.state.ranks, [column]: next}});
  }

  /** Widen a number column's domain over the frame's exact bands and stand-ins. */
  private widen(frame: Composition, name: string): void {
    let domain: Domain | null = this.state.domains[name] ?? null;
    for (const band of frame.exact) {
      const values = band.scalars[name];
      if (values) domain = widenDomain(domain, values);
    }
    for (const piece of frame.standIn) {
      const values = piece.band.scalars[name];
      if (values) domain = widenDomainOver(domain, values, piece.indices, piece.limit);
    }
    if (domain && domain !== this.state.domains[name]) {
      this.set({...this.state, domains: {...this.state.domains, [name]: domain}});
    }
  }

  /** Note whether a point drawn lacks a value; a band without the column draws every point as one. */
  private scanMissing(frame: Composition, name: string): void {
    if (this.state.missing[name]) return;
    for (const band of frame.exact) {
      const values = band.scalars[name];
      // A band fetched before the column was asked for is looked at once its replacement lands.
      if (!values || this.scanned.has(band)) continue;
      this.scanned.add(band);
      if (band.ids.length > 0 && lacksValues(values)) {
        this.set({...this.state, missing: {...this.state.missing, [name]: true}});
        return;
      }
    }
  }

  /** Offer the frame's exact bands to the column's sample, and publish it when it has grown enough. */
  private sample(frame: Composition, name: string, reservoir: ValueReservoir): void {
    for (const band of frame.exact) {
      const values = band.scalars[name];
      if (values) reservoir.offer(values);
    }
    const taken = reservoir.take();
    if (taken) this.set({...this.state, samples: {...this.state.samples, [name]: taken}});
  }

  /** Forget everything accumulated, keeping the colour and size columns. */
  clear(): void {
    this.epoch += 1;
    this.orders.clear();
    this.waiting.clear();
    this.naming.clear();
    this.reservoir = this.state.sizeBy !== null && this.rank ? new ValueReservoir() : null;
    this.scanned = new WeakSet();
    this.set({...EMPTY_LEGEND, colourBy: this.state.colourBy, sizeBy: this.state.sizeBy});
  }

  dispose(): void {
    this.disposed = true;
  }

  private set(state: LegendProjection): void {
    this.state = state;
    this.publish(state);
  }

  /** Ask for the names of the codes drawn that are not named yet. A refused column is not asked again. */
  private async name(column: string, counts: Map<number, number>): Promise<void> {
    if (this.state.categoryErrors[column]) return;
    const held = new Set((this.state.categories[column] ?? []).map((v) => v.code));
    const asked = this.naming.get(column) ?? new Set<number>();
    this.naming.set(column, asked);
    const wanted = [...counts.keys()].filter((code) => !held.has(code) && !asked.has(code));
    if (wanted.length === 0) return;
    for (const code of wanted) asked.add(code);
    const epoch = this.epoch;
    try {
      const resolved = await this.resolve(column, wanted);
      if (this.disposed || epoch !== this.epoch) return;
      const byCode = new Map((this.state.categories[column] ?? []).map((v) => [v.code, v]));
      for (const v of resolved) byCode.set(v.code, v);
      this.set({...this.state, categories: {...this.state.categories, [column]: [...byCode.values()]}});
      this.rankByOrder(column);
    } catch (error) {
      if (this.disposed || epoch !== this.epoch) return;
      this.set({...this.state, categoryErrors: {...this.state.categoryErrors, [column]: refusalOf(error)}});
    }
  }
}
