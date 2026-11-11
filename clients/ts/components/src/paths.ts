import {artifactName, type BrowseRow, type Layer, type Store} from '@mosaicajs/client';
import {isLinked} from './aggregate.js';
import {UNNAMED} from './base.js';

/** A cluster's path of parents' names, nearest last, and whether parents above the first step are left out. */
export type Path = {steps: string[]; beyond: boolean};

/**
 * The names and parents of clusters on a layer with parent edges, learnt from browse pages. For
 * each cluster asked about, the children form of the cluster names its served parents, and the
 * walk goes on up from the first of them as each answer lands. A cluster is asked about once until
 * {@link reset}, which also drops an answer still loading.
 */
export class ClusterPaths {
  /** Every cluster a page has served. */
  private readonly met = new Map<bigint, BrowseRow>();
  /** Each cluster's served parents, as the children form gave them. */
  private readonly parentsGiven = new Map<bigint, bigint[]>();
  private readonly asked = new Set<bigint>();
  /** The clusters whose children form has been answered or refused. */
  private readonly done = new Set<bigint>();
  /** How many steps up each cluster asked about is walked. */
  private readonly walks = new Map<bigint, number>();
  private epoch = 0;

  /** `changed` is called as each page lands. */
  constructor(private readonly changed: () => void) {}

  reset(): void {
    this.epoch += 1;
    this.met.clear();
    this.parentsGiven.clear();
    this.asked.clear();
    this.done.clear();
    this.walks.clear();
  }

  /** Note the rows a page served, whose names and parents are then known. */
  meet(rows: Iterable<BrowseRow>): void {
    for (const r of rows) this.met.set(r.mosaicaId, r);
  }

  /** The name a page gave a cluster, or `null` where no page has met it. */
  nameOf(id: bigint): string | null {
    const row = this.met.get(id);
    return row ? (artifactName(row) ?? UNNAMED) : null;
  }

  private parentsOf(id: bigint): readonly bigint[] | undefined {
    return this.parentsGiven.get(id) ?? this.met.get(id)?.parentIds;
  }

  /** Walk `depth` steps up from each of `ids` on a layer with parent edges, asking for each step's parents not yet met. */
  ask(store: Store, layer: Layer, ids: readonly bigint[], depth = 2): void {
    if (!isLinked(layer)) return;
    for (const id of ids) this.walks.set(id, Math.max(depth, this.walks.get(id) ?? 0));
    this.walk(store, layer, ids);
  }

  private walk(store: Store, layer: Layer, ids: Iterable<bigint>): void {
    for (const id of ids) {
      let at: bigint | undefined = id;
      for (let i = 0; i < (this.walks.get(id) ?? 0) && at !== undefined; i++) {
        const parents = this.parentsOf(at);
        if (parents === undefined || parents.some((p) => !this.met.has(p))) {
          this.fetch(store, layer, at);
          break;
        }
        at = parents[0];
      }
    }
  }

  private fetch(store: Store, layer: Layer, id: bigint): void {
    if (this.asked.has(id)) return;
    this.asked.add(id);
    const epoch = this.epoch;
    const landed = (parents: BrowseRow[] | null) => {
      if (epoch !== this.epoch) return;
      this.done.add(id);
      if (parents) {
        this.meet(parents);
        this.parentsGiven.set(id, parents.map((r) => r.mosaicaId));
      }
      this.walk(store, layer, [...this.walks.keys()]);
      this.changed();
    };
    store.browse({layer: layer.name, filters: null, parent: id, limit: 1}).then(
      (p) => landed(p.parents),
      () => landed(null)
    );
  }

  /**
   * A cluster's path of parents up to `depth` steps, nearest last, as far up as the pages have
   * named them; `null` while a step's parents are on their way. A step names every served parent
   * of the step below it, and the step above is taken from the first of them.
   */
  path(id: bigint, depth: number): Path | null {
    const steps: string[] = [];
    let parents = this.parentsOf(id) ?? [];
    let at = id;
    for (let i = 0; i < depth && parents.length > 0; i++) {
      const named = parents.flatMap((p) => this.met.get(p) ?? []);
      if (named.length < parents.length && this.asked.has(at) && !this.done.has(at)) return null;
      if (named.length === 0) break;
      steps.unshift(named.map((r) => artifactName(r) ?? UNNAMED).join(', '));
      at = named[0]!.mosaicaId;
      parents = this.parentsOf(at) ?? [];
    }
    return {steps, beyond: parents.length > 0};
  }

  /** A cluster's path two steps up as one line, as far as it is known. */
  pathOf(id: bigint): string {
    return this.path(id, 2)?.steps.join(' › ') ?? '';
  }
}
