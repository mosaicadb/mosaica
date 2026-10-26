import {css, html, nothing, type PropertyValues, type TemplateResult} from 'lit';
import {property, state} from 'lit/decorators.js';
import {artifactName, browsableLayers, isFilterLayer, withMember, withoutMember, type BrowsePage, type BrowseRow, type ClauseVerb, type Layer, type Masked, type Refusal} from '@mosaicajs/client';
import {refusalOf} from '@mosaicajs/client/internal';
import {MosaicaElement, UNNAMED, emit, idString} from './base.js';
import {attachContextRoot, defineOnce} from './define.js';
import {icon} from './icons.js';
import {renderState, stateOf} from './states.js';
import {chrome, tokens} from './tokens.js';
import './count.js';


/** One node of the walk: a row, its children once fetched, and where its paging got to. */
type Node = {
  row: BrowseRow;
  /** The path that reached it; a `dag` node under two parents is two nodes with different paths. */
  path: string;
  children: Node[] | null;
  next: string | null;
  loading: boolean;
  refusal: Refusal | null;
};

const masked = (n: bigint): Masked => ({value: Number(n), exact: true});

/**
 * The hierarchy lets viewers browse a layer's artifacts as a tree and filter the map by them. It
 * opens on the top-level artifacts and stays put while the map moves, so viewers can reach an
 * artifact at any level wherever the camera is. It offers every layer whose artifacts form a
 * hierarchy, including filter layers, which the map does not draw, and a select switches between
 * them when there are several.
 *
 * Each row shows how many of the artifact's items the viewer may see, and how many of them match
 * while filters are set. Expanding a row loads its children a page at a time. A parent the viewer
 * may not see is left out, so its children appear at the top level.
 *
 * A row's buttons highlight or filter the map by its artifact. Its Fit button fires
 * `mosaica-artifactfit`, which `<mosaica-explorer>` handles by fitting its map to the artifact.
 * Elsewhere, pass the event's id to the map's `fitTo`. The panel makes no requests while it is
 * hidden, so it costs nothing in a closed drawer.
 *
 * @summary A tree for browsing a layer's artifacts and filtering by them.
 * @tagname mosaica-hierarchy
 * @category Elements
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-clausechange']>} mosaica-clausechange - A row's
 *   name, highlight or filter button added or removed a `member_of` clause.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-artifactfit']>} mosaica-artifactfit - A row's Fit
 *   button was pressed.
 * @csspart title - The heading.
 * @csspart state - The state line, with `data-state`.
 * @csspart refusal - The words "View refused" or "Layer unavailable", with `data-code`.
 * @csspart reason - The server's reason for a refusal, under "View refused".
 * @csspart layer - The layer select, shown if there are several layers.
 * @csspart search - The search box.
 * @csspart tree - The tree of rows.
 * @csspart row - One row, with `data-id`, and `data-clause` (`filter`, `highlight` or both,
 *   separated by a space) while its artifact is filtered or highlighted.
 * @csspart expander - A row's expand button. On a row with no children it has `data-leaf` and is
 *   hidden, keeping the names aligned.
 * @csspart name - A row's name, with `data-unnamed` if it has none. Pressing it highlights the
 *   artifact.
 * @csspart counts - A row's counts.
 * @csspart count-matched - The `<mosaica-count>` of a row's items that match the filters, while a
 *   filter is set.
 * @csspart count-masked - The `<mosaica-count>` of a row's items the viewer may see.
 * @csspart actions - A row's buttons.
 * @csspart highlight - A row's highlight button, with `aria-pressed`.
 * @csspart filter - A row's filter button, with `aria-pressed`.
 * @csspart fit - A row's Fit button, on a layer that draws.
 * @csspart dismiss - A × that removes a row's filter or highlight, one for each, with `data-verb`.
 * @csspart also - The row's other parents, or "Children unavailable" with `data-code` if its
 *   children were refused.
 * @csspart children - An expanded row's children.
 * @csspart more - The More button, which loads the next page.
 */
export class MosaicaHierarchy extends MosaicaElement {
  static override styles = [
    tokens,
    chrome,
    css`
      :host {
        display: block;
      }
      [part='layer'] {
        width: 100%;
        margin-bottom: 8px;
      }
      [part='search'] {
        margin-bottom: 8px;
      }
      [part='tree'] {
        list-style: none;
        margin: 0;
        padding: 0;
        max-height: 320px;
        overflow-y: auto;
      }
      [part='row'] {
        position: relative;
        display: flex;
        align-items: center;
        gap: 6px;
        min-height: 28px;
        padding: 0 4px 0 calc(2px + var(--depth, 0) * 14px);
        border-radius: var(--_mosaica-radius-control);
      }
      [part='row']:hover {
        background: var(--_mosaica-surface-2);
      }
      [part='row'][data-clause~='filter'] {
        background: var(--_mosaica-surface-2);
        font-weight: 500;
      }
      [part='row'][data-clause~='highlight'] {
        background: var(--_mosaica-highlight-soft);
        color: var(--_mosaica-highlight);
      }
      [part='expander'] {
        display: inline-flex;
        width: 14px;
        color: var(--_mosaica-ink-3);
      }
      [part='expander'][data-leaf] {
        visibility: hidden;
      }
      [part='name'] {
        flex: 1;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        text-align: left;
      }
      [part='counts'] {
        display: inline-flex;
        gap: 6px;
        align-items: baseline;
        font-size: 12px;
        color: var(--_mosaica-ink-2);
      }
      /* Over the counts, so hidden buttons take no width from the name; still reachable by Tab. */
      [part='actions'] {
        position: absolute;
        right: 2px;
        top: 50%;
        transform: translateY(-50%);
        display: inline-flex;
        gap: 2px;
        padding-left: 6px;
        background: var(--_mosaica-surface-2);
        border-radius: var(--_mosaica-radius-control);
        opacity: 0;
        pointer-events: none;
      }
      [part='row']:hover [part='actions'],
      [part='row']:focus-within [part='actions'] {
        opacity: 1;
        pointer-events: auto;
      }
      [part='actions'] button {
        display: inline-flex;
        padding: 3px;
        color: var(--_mosaica-ink-2);
        border-radius: 4px;
      }
      [part='actions'] button:hover {
        color: var(--_mosaica-ink);
        background: var(--_mosaica-surface-3);
      }
      [part='dismiss'] {
        display: inline-grid;
        place-items: center;
        width: 20px;
        height: 20px;
        flex: none;
        border-radius: 4px;
        color: var(--_mosaica-ink-2);
      }
      [part='dismiss'][data-verb='highlight'] {
        color: var(--_mosaica-highlight);
      }
      [part='dismiss']:hover {
        background: var(--_mosaica-surface-3);
      }
      [part='also'] {
        padding-left: calc(18px + var(--depth, 0) * 14px);
        font-size: 11px;
        color: var(--_mosaica-ink-3);
      }
      [part='more'] {
        margin: 4px 0 4px calc(18px + var(--depth, 0) * 14px);
      }
      [part='refusal'] {
        display: flex;
        align-items: center;
        gap: 8px;
        font-size: 12px;
      }
    `
  ];

  /** The layer to browse. Unset or unknown, the first layer the bundle offers. */
  @property() accessor layer = '';
  /** Rows per page, from 1 up to the server's `maxBrowseRows`. */
  @property({type: Number}) accessor limit = 50;

  @state() private accessor roots: Node[] | null = null;
  @state() private accessor rootsNext: string | null = null;
  @state() private accessor query = '';
  @state() private accessor searching: Node[] | null = null;
  @state() private accessor loading = false;
  @state() private accessor refusal: Refusal | null = null;
  /** Which paths are open, so a re-render of the tree keeps the walk. */
  @state() private accessor open = new Set<string>();
  /** Bumped to re-render after a node's children land, since the tree in `roots` is mutated. */
  @state() private accessor revision = 0;

  /** The question the roots were fetched under; see {@link question}. */
  private fetchedUnder = '';
  /**
   * Every name the walk has seen, by identifier. A filter layer's artifacts are never served in a
   * viewport, so this is the client's only source of their names.
   */
  private names = new Map<bigint, string>();
  private searchTimer: ReturnType<typeof setTimeout> | null = null;
  /** Moved by {@link resetServerData}, so a page asked for before it is dropped. */
  private epoch = 0;

  protected override resetServerData(): void {
    this.epoch += 1;
    this.roots = null;
    this.rootsNext = null;
    this.searching = null;
    this.loading = false;
    this.refusal = null;
    this.open = new Set();
    this.names.clear();
    this.fetchedUnder = '';
    if (this.searchTimer) clearTimeout(this.searchTimer);
    this.searchTimer = null;
    this.query = '';
  }

  private layers(): Layer[] {
    return browsableLayers(this.resolvedStore?.get('meta')?.layers ?? []);
  }

  private current(): Layer | null {
    const offered = this.layers();
    return offered.find((l) => l.name === this.layer) ?? offered[0] ?? null;
  }

  /**
   * The question the panel is asking, as a string: the layer and the map's filter. A change
   * refetches the roots and drops the walk, whose counts answer the old question.
   */
  private question(): string {
    // Hashed from `requestFilters()`, what `store.browse` sends, which includes the drawn region.
    // A `member_of` leaf holds its artifact as a decimal string, so it serialises.
    return `${this.current()?.name ?? ''}|${JSON.stringify(this.resolvedStore?.requestFilters() ?? null)}`;
  }

  /**
   * Whether the panel is shown. A hidden panel sends no request, so a panel in a closed drawer
   * does not load the roots alongside the first viewport.
   *
   * `checkVisibility` is asked on every render; a reveal that re-renders nothing (a drawer opening
   * above it) is caught by the intersection observer. A runtime with neither counts as shown.
   */
  @state() private accessor shown = false;
  private observer: IntersectionObserver | null = null;

  override connectedCallback(): void {
    super.connectedCallback();
    if (typeof IntersectionObserver === 'undefined') return;
    this.observer ??= new IntersectionObserver((entries) => {
      if (!entries.some((e) => e.isIntersecting) || this.shown) return;
      this.shown = true;
      this.loadRootsIfStale();
    });
    this.observer.observe(this);
  }

  override disconnectedCallback(): void {
    this.observer?.disconnect();
    this.observer = null;
    super.disconnectedCallback();
  }

  /** Whether this element is displayed; see {@link shown}. */
  private displayed(): boolean {
    return typeof this.checkVisibility === 'function' ? this.checkVisibility() : true;
  }

  /** The roots, where the question they were fetched under is no longer the one being asked. */
  private loadRootsIfStale(): void {
    const q = this.question();
    if (this.shown && q !== this.fetchedUnder && this.current()) {
      this.fetchedUnder = q;
      void this.loadRoots();
    }
  }

  protected override onStoreChange(): void {
    this.loadRootsIfStale();
    super.onStoreChange();
  }

  /** Re-asked before every render, so putting the panel away and taking it out again both count. */
  protected override willUpdate(changed: PropertyValues<this>): void {
    super.willUpdate(changed);
    if (!this.shown && this.displayed()) this.shown = true;
  }

  protected override updated(): void {
    if (this.shown && this.roots === null && this.current() && this.resolvedStore && !this.loading && this.fetchedUnder === '') {
      this.fetchedUnder = this.question();
      void this.loadRoots();
    }
  }

  /** One page of the current layer, under the map's filter, which the store supplies. */
  private page(req: {parent?: bigint; q?: string; cursor?: string}): Promise<BrowsePage> | null {
    const s = this.resolvedStore;
    const layer = this.current();
    if (!s || !layer) return null;
    const ceiling = s.get('meta')?.selection.maxBrowseRows ?? this.limit;
    return s.browse({...req, layer: layer.name, limit: Math.max(1, Math.min(this.limit, ceiling))});
  }

  private async loadRoots(cursor?: string): Promise<void> {
    const request = this.page(cursor === undefined ? {} : {cursor});
    if (!request) return;
    const epoch = this.epoch;
    this.loading = true;
    this.refusal = null;
    try {
      const page = await request;
      if (epoch !== this.epoch) return;
      const nodes = page.artifacts.map((row) => this.node(row, ''));
      this.roots = cursor === undefined ? nodes : [...(this.roots ?? []), ...nodes];
      this.rootsNext = page.next;
    } catch (error) {
      if (epoch !== this.epoch) return;
      this.refusal = refusalOf(error);
      this.roots = this.roots ?? [];
    } finally {
      if (epoch === this.epoch) this.loading = false;
    }
  }

  private node(row: BrowseRow, parentPath: string): Node {
    const name = artifactName(row);
    if (name !== null) this.names.set(row.mosaicaId, name);
    return {row, path: `${parentPath}/${row.mosaicaId}`, children: null, next: null, loading: false, refusal: null};
  }

  /** What to call an artifact: its name where the walk has met one. */
  private nameOf(id: bigint): string {
    return this.names.get(id) ?? UNNAMED;
  }

  /**
   * Expand a node: its children, paged. A node keeps what it fetched while {@link question} is
   * unchanged, so reopening it fetches nothing.
   */
  private async expand(node: Node, more = false): Promise<void> {
    const request = this.page({parent: node.row.mosaicaId, ...(more && node.next ? {cursor: node.next} : {})});
    if (!request) return;
    const epoch = this.epoch;
    node.loading = true;
    node.refusal = null;
    this.revision++;
    try {
      const page = await request;
      if (epoch !== this.epoch) return;
      const nodes = page.artifacts.map((row) => this.node(row, node.path));
      node.children = more ? [...(node.children ?? []), ...nodes] : nodes;
      node.next = page.next;
    } catch (error) {
      node.refusal = refusalOf(error);
      node.children = node.children ?? [];
    } finally {
      node.loading = false;
      this.revision++;
    }
  }

  private toggle(node: Node): void {
    const open = new Set(this.open);
    if (open.has(node.path)) open.delete(node.path);
    else {
      open.add(node.path);
      if (node.children === null && !node.loading) void this.expand(node);
    }
    this.open = open;
  }

  private search(text: string): void {
    this.query = text;
    if (this.searchTimer) clearTimeout(this.searchTimer);
    // Debounced, like every typed control.
    this.searchTimer = setTimeout(() => {
      this.searchTimer = null;
      const q = text.trim();
      if (q.length === 0) {
        this.searching = null;
        return;
      }
      const request = this.page({q});
      if (!request) return;
      const epoch = this.epoch;
      void request
        .then((page) => {
          if (epoch === this.epoch) this.searching = page.artifacts.map((row) => this.node(row, 'q'));
        })
        .catch((error: unknown) => {
          if (epoch !== this.epoch) return;
          this.refusal = refusalOf(error);
          this.searching = [];
        });
    }, 250);
  }

  /** The positions of the clauses on this row's artifact, which colour the row and set the buttons. */
  private clausesOn(id: bigint): ClauseVerb[] {
    const layer = this.current()?.name;
    const held = this.resolvedStore?.get('filters').members ?? [];
    return held.filter((c) => c.layer === layer && c.artifact === id && !c.outside).map((c) => c.verb);
  }

  private apply(id: bigint, verb: ClauseVerb): void {
    const s = this.resolvedStore;
    const layer = this.current();
    if (!s || !layer) return;
    const held = s.get('filters').members;
    const on = this.clausesOn(id).includes(verb);
    const label = this.names.get(id);
    s.setMembers(
      on
        ? withoutMember(held, layer.name, id, verb)
        : withMember(held, {layer: layer.name, artifact: id, outside: false, verb, ...(label === undefined ? {} : {label})})
    );
    emit(this, 'mosaica-clausechange', {id: idString(id), layer: layer.name, outside: false, verb, on: !on});
  }

  override render(): TemplateResult | typeof nothing {
    const s = this.resolvedStore;
    const heading = html`<h2 part="title">Hierarchy</h2>`;
    const meta = s?.get('meta') ?? null;
    if (!s || !meta) return html`<div class="panel">${heading}${renderState(stateOf(s?.get('status')), s?.get('status'))}</div>`;
    const offered = this.layers();
    if (offered.length === 0) return html`<div class="panel">${heading}<span part="state" data-state="empty">No hierarchical layers</span></div>`;
    const layer = this.current()!;
    const filtered = s.get('filters').expr !== null || s.get('filters').members.length > 0;
    const rows = this.searching ?? this.roots;
    return html`<div class="panel">${heading}
      <span part="state" data-state=${this.refusal ? 'refused' : this.loading && rows === null ? 'loading' : 'shown'}></span>
      ${offered.length > 1
        ? html`<select part="layer" aria-label="Hierarchy layer" .value=${layer.name} @change=${(e: Event) => {
            this.layer = (e.target as HTMLSelectElement).value;
            this.roots = null;
            this.searching = null;
            this.open = new Set();
          }}>
            ${offered.map((l) => html`<option value=${l.name} ?selected=${l.name === layer.name}>${l.title || l.name}</option>`)}
          </select>`
        : nothing}
      <div part="search" class="input">${icon('search', 14)}<input
        type="search"
        aria-label=${`Search ${layer.title || layer.name}`}
        .value=${this.query}
        placeholder="Search names"
        autocomplete="off"
        @input=${(e: Event) => this.search((e.target as HTMLInputElement).value)}
      /></div>
      ${this.refusal ? html`<span part="refusal" data-code=${this.refusal.code}><span class="dot refuse"></span>Layer unavailable</span>` : nothing}
      ${rows === null || (this.refusal && rows.length === 0)
        ? nothing
        : rows.length === 0
          ? html`<span part="state" data-state="empty">${this.searching ? 'No match' : 'Nothing here'}</span>`
          : html`<ul part="tree">${rows.map((n) => this.renderNode(n, 0, layer, filtered))}</ul>`}
      ${this.searching === null && this.rootsNext
        ? html`<button part="more" class="more-link" type="button" @click=${() => void this.loadRoots(this.rootsNext ?? undefined)}>More</button>`
        : nothing}
    </div>`;
  }

  private renderNode(node: Node, depth: number, layer: Layer, filtered: boolean): unknown {
    const open = this.open.has(node.path);
    const clauses = this.clausesOn(node.row.mosaicaId);
    const name = artifactName(node.row);
    // A `dag` node is drawn under each served parent; the row names the others.
    const also = node.row.parentIds.filter((p) => String(p) !== node.path.split('/').at(-2));
    const drawn = !isFilterLayer(layer);
    return html`<li>
      <div part="row" style=${`--depth:${depth}`} data-id=${idString(node.row.mosaicaId)} data-clause=${clauses.length > 0 ? clauses.sort().join(' ') : nothing}>
        <button part="expander" type="button" data-leaf=${node.row.childCount === 0 ? '' : nothing}
          aria-expanded=${open ? 'true' : 'false'}
          aria-label=${open ? `Collapse ${name ?? 'row'}` : `Expand ${name ?? 'row'}`}
          @click=${() => this.toggle(node)}>${icon(open ? 'chev' : 'chevr', 12)}</button>
        <button part="name" type="button" data-unnamed=${name === null ? '' : nothing}
          title="Highlight"
          @click=${() => this.apply(node.row.mosaicaId, 'highlight')}>${name ?? UNNAMED}</button>
        <span part="counts">
          ${filtered && node.row.matchedCount !== null
            ? html`<mosaica-count part="count-matched" .masked=${masked(node.row.matchedCount)}></mosaica-count>/`
            : nothing}<mosaica-count part="count-masked" .masked=${masked(node.row.maskedCount)}></mosaica-count>
        </span>
        ${clauses.map(
          (verb) => html`<button part="dismiss" type="button" data-verb=${verb} aria-label=${verb === 'highlight' ? `Stop highlighting ${name ?? UNNAMED}` : `Stop filtering to ${name ?? UNNAMED}`}
            title=${verb === 'highlight' ? 'Stop highlighting' : 'Stop filtering'} @click=${() => this.apply(node.row.mosaicaId, verb)}>${icon('close', 12)}</button>`
        )}
        <span part="actions">
          <button part="highlight" type="button" data-verb="highlight" aria-pressed=${clauses.includes('highlight') ? 'true' : 'false'}
            title="Highlight this" @click=${() => this.apply(node.row.mosaicaId, 'highlight')}>${icon('highlight', 13)}</button>
          <button part="filter" type="button" data-verb="filter" aria-pressed=${clauses.includes('filter') ? 'true' : 'false'}
            title="Filter to this" @click=${() => this.apply(node.row.mosaicaId, 'filter')}>${icon('filter', 13)}</button>
          ${
            // A filter layer draws nothing, so there is nothing to fit to.
            drawn
              ? html`<button part="fit" type="button" title="Fit the map to it"
                  @click=${() => emit(this, 'mosaica-artifactfit', {id: idString(node.row.mosaicaId)})}>${icon('fit', 13)}</button>`
              : nothing
          }
        </span>
      </div>
      ${also.length > 0 ? html`<div part="also" style=${`--depth:${depth}`}>also under ${also.map((p) => this.nameOf(p)).join(', ')}</div>` : nothing}
      ${node.refusal ? html`<div part="also" style=${`--depth:${depth}`} data-code=${node.refusal.code}>Children unavailable</div>` : nothing}
      ${open && node.children
        ? html`<ul part="children" class="list" style="list-style:none;margin:0;padding:0">
            ${node.children.map((c) => this.renderNode(c, depth + 1, layer, filtered))}
            ${node.next
              ? html`<li><button part="more" class="more-link" style=${`--depth:${depth + 1}`} type="button" @click=${() => void this.expand(node, true)}>More</button></li>`
              : nothing}
          </ul>`
        : nothing}
    </li>`;
  }
}

attachContextRoot();
defineOnce('mosaica-hierarchy', MosaicaHierarchy);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-hierarchy': MosaicaHierarchy;
  }
}
