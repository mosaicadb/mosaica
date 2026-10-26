/**
 * The session artifact table: ordinal to artifact, one table per session.
 *
 * An artifact's id is stable across responses, so one table names every artifact the session has
 * held. The ordinal is a `u32` with `0` reserved for none, and it is what each point's membership
 * carries. Naming happens only here: the decode worker cannot share the table, so it emits a
 * response-local index that the main thread remaps.
 *
 * A layer may hold millions of artifacts, so ordinals are reference-counted and reused. Each
 * holder takes a reference per distinct ordinal when built and releases it when dropped; an ordinal
 * at zero returns to a free list. Live ordinals are bounded by what is held, and a lookup texture
 * sized to them stays small. The holders are the artifact channel's store and every band.
 */

/**
 * One artifact in an {@link ArtifactTable}.
 *
 * @category Projections
 */
export type ArtifactEntry = {
  /** The artifact's `mosaica_id`. */
  mosaicaId: bigint;
  /** The artifact's layer. */
  layer: string;
  /**
   * The ordinals of the artifact's parents, in ascending `mosaica_id` order. Only parents that
   * arrived in the same response as the artifact are listed, so the list may be empty even if the
   * artifact has parents. A tree has at most one parent; a `dag` layer may have several.
   */
  parentOrdinals: readonly number[];
  /**
   * The artifact's level: the declared level on a layer with levels, the depth of its parent chain
   * on a tree, and `0` on a flat layer.
   */
  rung: number;
  /** The artifact's centroid in 32-bit grid units ({@link GRID32}), or `null` if not known. */
  centroid: readonly [number, number] | null;
  /**
   * The artifact's slot in the palette, or `null` if not known. Its colour depends only on this and
   * `paletteSize`, so it can be coloured even when the current view has not served it.
   */
  slot: number | null;
  /** The palette size `slot` refers to, or `null` if `slot` is not known. */
  paletteSize: number | null;
};

/** An artifact as a holder hands it to {@link SessionArtifactTable.take}. */
export type ArtifactRef = {
  /** The artifact's `mosaica_id`. */
  mosaicaId: bigint;
  /** The artifact's layer. */
  layer: string;
  /** The `mosaica_id`s of its parents that the same response served, ascending. */
  parentIds: readonly bigint[];
  /** Its centroid in 32-bit grid units, where the response carried one. */
  centroid?: readonly [number, number] | null;
  /** Its `rung` from the response. Defaults to `0`, the value for a flat layer's artifacts and a tree's roots. */
  rung?: number;
  /**
   * Its slot from a response that asked with `palette_size`, and that size. Left out where the
   * response carried none, as a point's membership does.
   */
  slot?: {slot: number | null; paletteSize: number};
};

/**
 * The ordinal `0`, which means no artifact.
 *
 * @category Projections
 */
export const NO_ORDINAL = 0;

/**
 * A change to one ordinal, as {@link ArtifactTable.changesSince} reports it.
 *
 * - `named`: the ordinal was assigned to an artifact. Only that ordinal's own derived values change.
 * - `placed`: the artifact's centroid or palette slot arrived or changed, so its colour and those of
 *   every ordinal that resolves through it may change.
 * - `linked`: a parent was set on an existing artifact, so its descendants may resolve
 *   differently.
 * - `freed`: the ordinal was released and may be reused.
 *
 * @category Projections
 */
export type ArtifactTableChange = {
  /** The ordinal that changed. */
  ordinal: number;
  /** What changed. */
  kind: 'named' | 'placed' | 'linked' | 'freed';
};

const KINDS = ['named', 'placed', 'linked', 'freed'] as const;

/**
 * How many changes the journal keeps. A reader further behind is told so and rebuilds whole, so
 * the limit bounds memory and not correctness.
 */
const JOURNAL_LIMIT = 1 << 16;

function keyOf(layer: string, mosaicaId: bigint): string {
  return `${layer}\u0000${mosaicaId}`;
}

/**
 * A table of the artifacts the store holds, each numbered with a small integer called an ordinal.
 * Points refer to artifacts by ordinal ({@link BandMembership}), and a renderer looks ordinals up
 * here to find the artifact and its colour. Read it from {@link ArtifactsProjection.table}; only
 * the store writes to it.
 *
 * An ordinal is an unsigned 32-bit integer, with {@link NO_ORDINAL} for none. Once the store no
 * longer holds an artifact, its ordinal is freed and may be given to another artifact.
 *
 * @category Projections
 */
export interface ArtifactTable {
  /**
   * A number that changes whenever the table does. A value derived from the table is up to date
   * while this is unchanged. Compare it for equality only.
   */
  readonly version: number;
  /**
   * Every change since `version`, oldest first, so you can update a colour map or lookup texture
   * instead of rebuilding it.
   *
   * @returns The changes, `[]` if `version` is current, or `null` if the table no longer has them
   *   (for example if `version` is from before the table was last cleared). On `null`, rebuild from `liveEntries()`.
   */
  changesSince(version: number): ArtifactTableChange[] | null;
  /** How many ordinals are in use. */
  readonly live: number;
  /** One more than the highest ordinal assigned: the size a lookup texture indexed by ordinal needs. */
  readonly range: number;
  /** The ordinal of an artifact, or {@link NO_ORDINAL} if the table does not hold it. */
  ordinalOf(layer: string, mosaicaId: bigint): number;
  /** The artifact with this ordinal, or `null` if the ordinal is `0`, freed or never assigned. */
  entry(ordinal: number): ArtifactEntry | null;
  /** Every ordinal in use, with its artifact. */
  liveEntries(): {ordinal: number; entry: ArtifactEntry}[];
  /**
   * Find the ordinal itself or its nearest ancestor that is in `served`, by following parent
   * links. Use it to colour a point by the artifact drawn at the current cut. On a `dag` layer it
   * follows the first parent at each step.
   *
   * @param ordinal - The ordinal to start from, such as a point's entry in
   *   {@link BandMembership.ordinals}.
   * @param served - Anything with `has`, such as a set of ordinals or a colour map.
   * @param served.has - Whether an ordinal is drawn at the current cut.
   * @param maxLevel - The level to colour at: the walk skips ancestors in `served` whose `rung` is
   *   deeper than this. By default it stops at the first one in `served`.
   * @returns The ordinal found, or {@link NO_ORDINAL} if the walk runs out of known parents.
   */
  resolve(ordinal: number, served: {has(ordinal: number): boolean}, maxLevel?: number): number;
}

/**
 * The store's {@link ArtifactTable}, with the writes: each holder takes one reference per distinct
 * ordinal it carries and releases it when dropped.
 */
export class SessionArtifactTable implements ArtifactTable {
  /** Indexed by ordinal; slot 0 is reserved. A freed slot holds null until reused. */
  private entries: (ArtifactEntry | null)[] = [null];
  private refs: number[] = [0];
  private ordinals = new Map<string, number>();
  private free: number[] = [];
  /**
   * The ordinals named by a ref that carried no `rung`, as a point's membership names an artifact
   * whose response sends its artifacts after the points. The first ref carrying one sets it.
   */
  private unranked = new Set<number>();
  /**
   * Incremented whenever an ordinal is named or freed, a centroid arrives, or a link is set on an
   * entry that already existed. Callers derive their colours under it.
   */
  private stamp = 0;
  /**
   * One packed change per version, oldest first. The version of `journal[i]` is
   * `journalFrom + i + 1`, since each change increments the stamp by one, so
   * {@link changesSince} answers by index.
   */
  private journal: number[] = [];
  /** The version `journal[0]` follows: the oldest version {@link changesSince} can answer from. */
  private journalFrom = 0;
  private clears = 0;

  get version(): number {
    return this.stamp;
  }

  /**
   * Moved by {@link clear}. An ordinal named under one generation means nothing under the next, so
   * a holder checks it before it retains or releases one.
   */
  get generation(): number {
    return this.clears;
  }

  /** Records one change against a new version. */
  private record(ordinal: number, kind: ArtifactTableChange['kind']): void {
    this.stamp += 1;
    this.journal.push(ordinal * KINDS.length + KINDS.indexOf(kind));
    if (this.journal.length > JOURNAL_LIMIT) {
      const dropped = this.journal.length - JOURNAL_LIMIT / 2;
      this.journal.splice(0, dropped);
      this.journalFrom += dropped;
    }
  }

  changesSince(version: number): ArtifactTableChange[] | null {
    if (version === this.stamp) return [];
    if (version < this.journalFrom || version > this.stamp) return null;
    const out: ArtifactTableChange[] = [];
    for (let i = version - this.journalFrom; i < this.journal.length; i++) {
      const packed = this.journal[i]!;
      out.push({ordinal: Math.floor(packed / KINDS.length), kind: KINDS[packed % KINDS.length]!});
    }
    return out;
  }

  get live(): number {
    return this.ordinals.size;
  }

  get range(): number {
    return this.entries.length;
  }

  ordinalOf(layer: string, mosaicaId: bigint): number {
    return this.ordinals.get(keyOf(layer, mosaicaId)) ?? NO_ORDINAL;
  }

  entry(ordinal: number): ArtifactEntry | null {
    return this.entries[ordinal] ?? null;
  }

  liveEntries(): {ordinal: number; entry: ArtifactEntry}[] {
    const out: {ordinal: number; entry: ArtifactEntry}[] = [];
    for (const ordinal of this.ordinals.values()) {
      const entry = this.entries[ordinal];
      if (entry) out.push({ordinal, entry});
    }
    return out;
  }

  /**
   * Takes one reference on each of `refs`, assigning an ordinal to any artifact the table does not
   * hold, and returns their ordinals in order. Each ref's parent links are set to the parents the
   * table holds, in the ref's order. Links already held survive a batch that carries the child
   * alone or resolves only some of its parents. A centroid for an entry that had none is recorded,
   * and so is a `rung` for an entry named without one, and a slot that differs from the one held.
   */
  take(refs: readonly ArtifactRef[]): Uint32Array {
    const ordinals = new Uint32Array(refs.length);
    // A link set on an ordinal this batch named is part of naming it, and is not reported as `linked`.
    const namedHere = new Set<number>();
    for (let i = 0; i < refs.length; i++) {
      const ref = refs[i]!;
      const key = keyOf(ref.layer, ref.mosaicaId);
      let ordinal = this.ordinals.get(key);
      if (ordinal === undefined) {
        ordinal = this.free.pop() ?? this.entries.length;
        this.entries[ordinal] = {
          mosaicaId: ref.mosaicaId,
          layer: ref.layer,
          parentOrdinals: [],
          rung: ref.rung ?? 0,
          centroid: ref.centroid ?? null,
          slot: ref.slot?.slot ?? null,
          paletteSize: ref.slot?.paletteSize ?? null
        };
        this.refs[ordinal] = 0;
        this.ordinals.set(key, ordinal);
        if (ref.rung === undefined) this.unranked.add(ordinal);
        namedHere.add(ordinal);
        this.record(ordinal, 'named');
      } else {
        if (ref.centroid && !this.entries[ordinal]!.centroid) {
          this.entries[ordinal] = {...this.entries[ordinal]!, centroid: ref.centroid};
          this.record(ordinal, 'placed');
        }
        const held = this.entries[ordinal]!;
        if (ref.slot && (ref.slot.slot !== held.slot || ref.slot.paletteSize !== held.paletteSize)) {
          // A slot arriving late, or under another palette size, is a colour arriving late.
          this.entries[ordinal] = {...held, slot: ref.slot.slot, paletteSize: ref.slot.paletteSize};
          this.record(ordinal, 'placed');
        }
        if (ref.rung !== undefined && this.unranked.delete(ordinal) && ref.rung !== this.entries[ordinal]!.rung) {
          // A level arriving late moves where a walk with a level to colour at stops, here and
          // below.
          this.entries[ordinal] = {...this.entries[ordinal]!, rung: ref.rung};
          this.record(ordinal, 'linked');
        }
      }
      this.refs[ordinal]! += 1;
      ordinals[i] = ordinal;
    }
    // Links second, once every end of this batch has an ordinal.
    for (let i = 0; i < refs.length; i++) {
      const ref = refs[i]!;
      if (ref.parentIds.length === 0) continue;
      const parents: number[] = [];
      for (const parentId of ref.parentIds) {
        const parent = this.ordinals.get(keyOf(ref.layer, parentId));
        if (parent !== undefined) parents.push(parent);
      }
      if (parents.length === 0) continue;
      const entry = this.entries[ordinals[i]!]!;
      // A partial resolution does not shrink a list already held. The point path's batch comes from
      // the membership column and lands before the channel's, so it often lacks some parents.
      if (parents.length < ref.parentIds.length && entry.parentOrdinals.length > 0) continue;
      if (parents.length === entry.parentOrdinals.length && parents.every((p, j) => p === entry.parentOrdinals[j])) continue;
      const moved = parents[0] !== entry.parentOrdinals[0];
      this.entries[ordinals[i]!] = {...entry, parentOrdinals: parents};
      // A link on an entry that was already here changes what its descendants resolve to. It is
      // reported only when the first parent changed, since {@link resolve} walks the first alone; a
      // second parent arriving behind the first changes no colour.
      if (moved && !namedHere.has(ordinals[i]!)) this.record(ordinals[i]!, 'linked');
    }
    return ordinals;
  }

  /**
   * Takes one more reference on each of `ordinals`, as a band does for the distinct ordinals it
   * carries. `0` and a freed ordinal are skipped.
   */
  retain(ordinals: ArrayLike<number>): void {
    for (let i = 0; i < ordinals.length; i++) {
      const ordinal = ordinals[i]!;
      if (ordinal === NO_ORDINAL || !this.entries[ordinal]) continue;
      this.refs[ordinal]! += 1;
    }
  }

  /**
   * Releases one reference on each of `ordinals`. An ordinal left with none is freed and may be
   * given to another artifact. `0` and a freed ordinal are skipped.
   */
  release(ordinals: ArrayLike<number>): void {
    for (let i = 0; i < ordinals.length; i++) {
      const ordinal = ordinals[i]!;
      if (ordinal === NO_ORDINAL) continue;
      const entry = this.entries[ordinal];
      if (!entry) continue;
      const left = (this.refs[ordinal] ?? 0) - 1;
      if (left > 0) {
        this.refs[ordinal] = left;
        continue;
      }
      this.refs[ordinal] = 0;
      this.entries[ordinal] = null;
      this.unranked.delete(ordinal);
      this.ordinals.delete(keyOf(entry.layer, entry.mosaicaId));
      this.free.push(ordinal);
      this.record(ordinal, 'freed');
    }
  }

  resolve(ordinal: number, served: {has(ordinal: number): boolean}, maxLevel?: number): number {
    let at = ordinal;
    // Bounded by the table's size, so a link cycle cannot hang the caller.
    for (let steps = 0; at !== NO_ORDINAL && steps <= this.entries.length; steps++) {
      const here = this.entries[at];
      if (served.has(at) && (maxLevel === undefined || (here?.rung ?? 0) <= maxLevel)) return at;
      at = this.entries[at]?.parentOrdinals[0] ?? NO_ORDINAL;
    }
    return NO_ORDINAL;
  }

  /**
   * Empties the table and frees every ordinal, as the store does when it forgets a principal.
   * `changesSince` answers `null` for any version from before.
   */
  clear(): void {
    this.clears += 1;
    this.entries = [null];
    this.refs = [0];
    this.ordinals.clear();
    this.free = [];
    this.unranked.clear();
    this.stamp += 1;
    // The journal restarts at the new version, so no reader patches across a clear.
    this.journal = [];
    this.journalFrom = this.stamp;
  }
}
