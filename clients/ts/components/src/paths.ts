import {artifactName, type BrowseRow, type Layer, type Store} from '@mosaicajs/client';
import {isLinked} from './aggregate.js';
import {UNNAMED} from './base.js';

/**
 * The names of clusters' parents on a layer with parent edges, learnt from browse pages: for each
 * cluster asked about, a page of its served parents, and then one of its first parent's. Each is
 * asked for once until {@link reset}, which also drops an answer still loading.
 */
export class ClusterPaths {
  /** Every cluster met on a browse page. */
  private readonly met = new Map<bigint, BrowseRow>();
  /** Each cluster's served parents, once asked for. */
  private readonly parentsOf = new Map<bigint, bigint[]>();
  private readonly asked = new Set<bigint>();
  private epoch = 0;

  /** `changed` is called as each page lands. */
  constructor(private readonly changed: () => void) {}

  reset(): void {
    this.epoch += 1;
    this.met.clear();
    this.parentsOf.clear();
    this.asked.clear();
  }

  /** The rung a cluster was met at, where a page has named it. */
  rungOf(id: bigint): number | undefined {
    return this.met.get(id)?.rung;
  }

  /** The name a page gave a cluster, or `null` where no page has met it. */
  nameOf(id: bigint): string | null {
    const row = this.met.get(id);
    return row ? (artifactName(row) ?? UNNAMED) : null;
  }

  /** Ask for the parents of each of `ids` and of its first parent, on a layer with parent edges. */
  ask(store: Store, layer: Layer, ids: readonly bigint[]): void {
    if (!isLinked(layer)) return;
    const epoch = this.epoch;
    const ask = (id: bigint) => {
      if (this.asked.has(id)) return;
      this.asked.add(id);
      void store
        .browse({layer: layer.name, filters: null, parent: id, limit: 1})
        .then((p) => {
          if (epoch !== this.epoch) return;
          for (const r of p.parents) this.met.set(r.mosaicaId, r);
          this.parentsOf.set(id, p.parents.map((r) => r.mosaicaId));
          this.changed();
        })
        .catch(() => undefined);
    };
    for (const id of ids) {
      ask(id);
      const first = this.parentsOf.get(id)?.[0];
      if (first !== undefined) ask(first);
    }
  }

  /**
   * A cluster's path of parents two steps up, nearest last, as far up as the pages have named
   * them. A step names every served parent of the one below it, and the step above is taken from
   * the first of them.
   */
  pathOf(id: bigint): string {
    const steps: string[] = [];
    let parents = this.parentsOf.get(id) ?? this.met.get(id)?.parentIds ?? [];
    for (let i = 0; i < 2 && parents.length > 0; i++) {
      const named = parents.flatMap((p) => this.met.get(p) ?? []);
      if (named.length === 0) break;
      steps.unshift(named.map((r) => artifactName(r) ?? UNNAMED).join(', '));
      parents = this.parentsOf.get(named[0]!.mosaicaId) ?? named[0]!.parentIds;
    }
    return steps.join(' › ');
  }
}
