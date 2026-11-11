import type {LitElement} from 'lit';
import {type BrowsePage, type BrowseRow, type Layer, type Refusal, type Store} from '@mosaicajs/client';
import {refusalOf} from '@mosaicajs/client/internal';
import {UNNAMED, countText, focusLost} from './base.js';
import {ClusterPaths} from './paths.js';

/** One node of the walk: a row, its children once fetched, and where its paging got to. */
export type Node = {
  row: BrowseRow;
  /** The path that reached it; a `dag` node under two parents is two nodes with different paths. */
  path: string;
  children: Node[] | null;
  next: string | null;
  loading: boolean;
  refusal: Refusal | null;
};

/** How many requests a refetch of the rows has in flight at once. */
const REFETCH_AT_ONCE = 4;

/**
 * What a request for rows is part of. A `refetch` waits its turn among {@link REFETCH_AT_ONCE};
 * nothing more is asked for once `wanted` is false.
 */
type Fetch = {wanted: () => boolean; refetch: boolean};

/** A row's press, which waits for nothing. */
const PRESSED: Fetch = {wanted: () => true, refetch: false};

/** What a {@link Lineage} reads from the element that holds it. */
export type LineageHost = {
  store(): Store | null;
  /** The layer to browse; `null` asks for nothing. */
  layer(): Layer | null;
  /** Rows per page, which the server's `maxBrowseRows` caps. */
  limit(): number;
  /** Whether the row at `path` is open, so a refetch fetches its children again. */
  isOpen(path: string): boolean;
  /** Called after each change to what the walk holds. */
  changed(): void;
};

/**
 * The served rows of one layer from the browse route, under the store's filters, a page at a time:
 * the children of {@link top}, or the roots where it is `null`, and the children of each node
 * expanded under them. Counts are taken under the filters the store sends, so a change of filters
 * or of {@link top} is a new question, and the rows are fetched again: as many pages of the top
 * rows and of each open row as More had loaded, since a page's cursor is good only for the filters
 * it was served under. Every row met is kept for its name, and a `dag` row's other parents that no
 * page has served are asked for by name.
 */
export class Lineage {
  /** The top rows, once fetched. */
  roots: Node[] | null = null;
  loading = false;
  refusal: Refusal | null = null;
  /** The artifact whose children are the top rows; `null` for the layer's roots. */
  top: bigint | null = null;
  /** The palette size each row's slot is chosen for; `null` asks for no slots. */
  paletteSize: number | null = null;

  /** The names and parents of every row met. */
  private readonly paths: ClusterPaths;
  /** The cursor of the top rows' next page, with the question it was served under. */
  private next: {cursor: string; under: string} | null = null;
  /** The question the top rows were fetched under; see {@link question}. */
  private fetchedUnder = '';
  /** How many pages More has loaded under each path, and under the top's path for the top rows, so a refetch loads as many. */
  private readonly pages = new Map<string, number>();
  /** How many of a refetch's requests are in flight, and those waiting for a place. */
  private asking = 0;
  private waiting: (() => void)[] = [];
  /** The top the rows held were fetched under. */
  private fetchedTop: bigint | null = null;
  /** Moved by {@link reset}, so a page asked for before it is dropped. */
  private epoch = 0;

  constructor(private readonly host: LineageHost) {
    this.paths = new ClusterPaths(() => host.changed());
  }

  /** Drop everything fetched and every name met, as a change of viewer does. */
  reset(): void {
    this.epoch += 1;
    this.roots = null;
    this.next = null;
    this.loading = false;
    this.refusal = null;
    this.pages.clear();
    this.paths.reset();
    this.fetchedUnder = '';
    this.fetchedTop = null;
  }

  /** Drop the rows and their paging, as a change of layer does; the names met are kept. */
  clear(): void {
    this.roots = null;
    this.next = null;
    this.pages.clear();
  }

  /** Whether the top rows have a next page under the question being asked, which {@link loadMore} loads. */
  get hasMore(): boolean {
    return this.next !== null && this.next.under === this.fetchedUnder;
  }

  /** Whether every page of the top rows held has landed. */
  get complete(): boolean {
    return this.roots !== null && this.next === null;
  }

  /** Whether the top rows held were fetched for {@link top} and nothing is on its way. */
  get settled(): boolean {
    return this.roots !== null && this.fetchedTop === this.top && !this.loading;
  }

  /** The path of the top rows' parent: `''` for the roots. */
  private get topPath(): string {
    return this.top === null ? '' : `/${this.top}`;
  }

  /**
   * The question the rows answer, as a string: the layer, the top, the palette size and the
   * store's filters. A change refetches the top rows and the children of the open rows.
   */
  private question(): string {
    // Hashed from `requestFilters()`, what `store.browse` sends, which includes the drawn region.
    // A `member_of` leaf holds its artifact as a decimal string, so it serialises.
    return `${this.host.layer()?.name ?? ''}|${this.top ?? ''}|${this.paletteSize ?? ''}|${JSON.stringify(this.host.store()?.requestFilters() ?? null)}`;
  }

  /**
   * Fetch the top rows where the question they were fetched under is no longer the one being
   * asked. Rows of another top are dropped at once, so they are not shown as its children.
   */
  loadIfStale(): void {
    const q = this.question();
    if (q === this.fetchedUnder || !this.host.store() || !this.host.layer()) return;
    this.fetchedUnder = q;
    if (this.top !== this.fetchedTop) {
      this.fetchedTop = this.top;
      this.roots = null;
      this.next = null;
      this.refusal = null;
    }
    void this.loadRoots();
  }

  /** One page of the layer, under the store's filters. */
  private page(req: {parent?: bigint; q?: string; cursor?: string}): Promise<BrowsePage> | null {
    const s = this.host.store();
    const layer = this.host.layer();
    if (!s || !layer) return null;
    const ceiling = s.get('meta')?.selection.maxBrowseRows ?? this.host.limit();
    return s.browse({...req, layer: layer.name, limit: Math.max(1, Math.min(this.host.limit(), ceiling)), ...(this.paletteSize === null ? {} : {paletteSize: this.paletteSize})});
  }

  /** The rows whose names hold `q`, under the store's filters, as nodes of their own; `null` with no layer to search. */
  search(q: string): Promise<Node[]> | null {
    const request = this.page({q});
    if (!request) return null;
    const epoch = this.epoch;
    return request.then((page) => {
      const nodes = page.artifacts.map((row) => this.node(row, 'q'));
      if (epoch === this.epoch) this.askNames(nodes);
      return nodes;
    });
  }

  /** Run `ask` in one of a refetch's {@link REFETCH_AT_ONCE} places, once one is free. */
  private async placed<T>(ask: () => Promise<T>): Promise<T> {
    if (this.asking < REFETCH_AT_ONCE) this.asking++;
    else await new Promise<void>((free) => this.waiting.push(free));
    try {
      return await ask();
    } finally {
      // A waiting request takes the place over; with none, the place is free.
      const next = this.waiting.shift();
      if (next) next();
      else this.asking--;
    }
  }

  /**
   * The rows under `parent`, or the top rows: the page at `cursor`, or from the first page as many
   * as More had loaded there. The store aborts no request, so a fetch no longer wanted asks for
   * nothing more and returns `null`.
   */
  private async rowsUnder(parent: Node | null, cursor: string | undefined, fetch: Fetch): Promise<{nodes: Node[]; next: string | null} | null> {
    const path = parent?.path ?? this.topPath;
    const id = parent?.row.mosaicaId ?? this.top;
    const pages = cursor === undefined ? (this.pages.get(path) ?? 1) : 1;
    const nodes: Node[] = [];
    let next = cursor ?? null;
    for (let i = 0; i < pages && (i === 0 || next !== null); i++) {
      const ask = async () => (fetch.wanted() ? this.page({...(id === null ? {} : {parent: id}), ...(next === null ? {} : {cursor: next})}) : null);
      const page = await (fetch.refetch ? this.placed(ask) : ask());
      if (!page) return null;
      nodes.push(...page.artifacts.map((row) => this.node(row, path)));
      next = page.next;
    }
    if (cursor !== undefined) this.pages.set(path, (this.pages.get(path) ?? 1) + 1);
    return {nodes, next};
  }

  /** Load the top rows' next page under the question being asked, and return the rows it added. */
  loadMore(): Promise<Node[]> {
    return this.hasMore ? this.loadRoots(this.next!.cursor) : Promise.resolve([]);
  }

  /**
   * The top rows, with the children of every row among them that is open. They replace the rows,
   * or the page at `cursor` joins them, once those have landed, so a walk under one question is
   * drawn whole under the next. Rows asked for under a question since replaced are dropped.
   * Returns the rows it added.
   */
  private async loadRoots(cursor?: string): Promise<Node[]> {
    if (!this.host.store() || !this.host.layer()) return [];
    const epoch = this.epoch;
    const under = this.fetchedUnder;
    const wanted = () => epoch === this.epoch && under === this.fetchedUnder;
    const fetch: Fetch = {wanted, refetch: cursor === undefined};
    this.loading = true;
    this.refusal = null;
    this.host.changed();
    try {
      const got = await this.rowsUnder(null, cursor, fetch);
      if (!got || !wanted()) return [];
      await this.reopen(got.nodes, fetch);
      if (!wanted()) return [];
      this.roots = cursor === undefined ? got.nodes : [...(this.roots ?? []), ...got.nodes];
      this.next = got.next === null ? null : {cursor: got.next, under};
      this.askNames(got.nodes);
      // A row pressed open while these were on their way has no children yet.
      void this.reopen(got.nodes, fetch);
      return got.nodes;
    } catch (error) {
      if (!wanted()) return [];
      this.refusal = refusalOf(error);
      this.roots = this.roots ?? [];
      return [];
    } finally {
      if (wanted()) this.loading = false;
      this.host.changed();
    }
  }

  /** Fetch the children of each open row among `nodes` and under them that has none yet. */
  private async reopen(nodes: readonly Node[], fetch: Fetch): Promise<void> {
    await Promise.all(
      nodes
        .filter((n) => this.host.isOpen(n.path) && n.row.childCount > 0)
        .map((n) => (n.children !== null ? this.reopen(n.children, fetch) : n.loading ? undefined : this.expandUnder(n, false, fetch)))
    );
  }

  /** A node for `row` under the node at `parentPath`, the row kept for its name. */
  private node(row: BrowseRow, parentPath: string): Node {
    this.paths.meet([row]);
    return {row, path: `${parentPath}/${row.mosaicaId}`, children: null, next: null, loading: false, refusal: null};
  }

  /**
   * Ask by name for the other parents of `nodes` that no page has served, through the children
   * form of the first node under each, which names all of that node's parents.
   */
  private askNames(nodes: readonly Node[]): void {
    const s = this.host.store();
    const layer = this.host.layer();
    const through = new Map<bigint, bigint>();
    for (const n of nodes) for (const p of otherParents(n)) if (this.paths.nameOf(p) === null && !through.has(p)) through.set(p, n.row.mosaicaId);
    if (s && layer && through.size > 0) this.paths.ask(s, layer, [...new Set(through.values())], 1);
  }

  /** The name a page gave an artifact, or `null` where none has met it. */
  knownName(id: bigint): string | null {
    return this.paths.nameOf(id);
  }

  /** What to call an artifact: its name where the walk has met one. */
  nameOf(id: bigint): string {
    return this.paths.nameOf(id) ?? UNNAMED;
  }

  /** The names of the served parents of a node's row other than the one it was reached under, which a `dag` row names. */
  alsoUnder(node: Node): string[] {
    return otherParents(node).map((p) => this.nameOf(p));
  }

  /**
   * Expand a node: its children, or with `more` their next page. A node keeps what it fetched
   * while {@link question} is unchanged, so reopening it fetches nothing. Returns the rows it added.
   */
  expand(node: Node, more = false): Promise<Node[]> {
    return this.expandUnder(node, more, PRESSED);
  }

  private async expandUnder(node: Node, more: boolean, fetch: Fetch): Promise<Node[]> {
    if (!this.host.store() || !this.host.layer()) return [];
    const epoch = this.epoch;
    node.loading = true;
    node.refusal = null;
    this.host.changed();
    try {
      const got = await this.rowsUnder(node, more ? (node.next ?? undefined) : undefined, fetch);
      if (!got || epoch !== this.epoch) return [];
      await this.reopen(got.nodes, fetch);
      node.children = more ? [...(node.children ?? []), ...got.nodes] : got.nodes;
      node.next = got.next;
      this.askNames(got.nodes);
      return got.nodes;
    } catch (error) {
      node.refusal = refusalOf(error);
      node.children = node.children ?? [];
      return [];
    } finally {
      node.loading = false;
      this.host.changed();
    }
  }
}

/** The served parents of a node's row other than the one it was reached under. */
function otherParents(node: Node): bigint[] {
  return node.row.parentIds.filter((p) => String(p) !== node.path.split('/').at(-2));
}

/** A browse row's counts as a screen reader is given them: matching over every item where a filter is set, else every item. */
export function spokenCounts(row: BrowseRow): string {
  const masked = countText(Number(row.maskedCount));
  return row.matchedCount === null ? `${masked} items` : `${countText(Number(row.matchedCount))} matching, ${masked} in all`;
}

/**
 * Once More's page has landed, move focus from More to `first`, the first row it added, unless
 * focus has gone elsewhere meanwhile. More may be gone from the page by then.
 */
export async function focusAdded(host: LitElement, more: HTMLElement, first: () => HTMLElement | null | undefined): Promise<void> {
  await host.updateComplete;
  const onMore = host.shadowRoot?.activeElement === more || (!more.isConnected && focusLost());
  if (onMore) first()?.focus();
}
