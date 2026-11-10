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

/** A row's element, which carries its node's path. */
type RowElement = HTMLElement & {path: string};

/** The control in a row that focus is on: the row's path and place among the rows, and the control's part and verb. */
type Held = {path: string; at: number; part: string; verb: string | null};

/**
 * Use the hierarchy when viewers should be able to reach a layer's artifacts at any level, wherever
 * the camera is. It opens on the top-level artifacts and does not change as the map moves.
 * Expanding a row loads its children a page at a time.
 *
 * The hierarchy offers each layer that depends on no other layer and whose artifacts form a
 * hierarchy, filter layers among them. When there are several, a select switches between them.
 *
 * Each row shows how many of the artifact's items the viewer may see. While filters are set, it
 * also shows how many of those items match. A parent the viewer may not see is left out, so its
 * children appear at the top level. When the filters change, the rows are fetched again with the
 * children of every row that is open, and focus stays on the control it was on.
 *
 * A row's buttons highlight or filter the map by its artifact. Its Fit button only fires
 * `mosaica-artifactfit`, so you handle the event by passing its id to the map's `fitTo`. The
 * explorer does this for a hierarchy you put in one of its slots. `fitTo` finds only the artifacts
 * the map currently draws, so Fit does nothing for a row whose artifact is not drawn at this zoom.
 *
 * The panel makes no requests while it is hidden, so it costs nothing in a closed drawer.
 *
 * @summary A tree for browsing a layer's artifacts and filtering by them.
 * @tagname mosaica-hierarchy
 * @category Elements
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-clausechange']>} mosaica-clausechange - A row's
 *   name, highlight or filter button added or removed a `member_of` clause.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-artifactfit']>} mosaica-artifactfit - A row's Fit
 *   button was pressed.
 * @csspart title - The heading.
 * @csspart state - The state line, with `data-state`. It is a `role="status"` region, which also
 *   says which rows are loading children and which were refused them.
 * @csspart refusal - The words "View refused" or "Layer unavailable", with `data-code`.
 * @csspart reason - The server's reason for a refusal, under "View refused".
 * @csspart layer - The layer select, shown if there are several layers.
 * @csspart search - The search box.
 * @csspart tree - The tree of rows, named by the layer.
 * @csspart row - One row, with `data-id`, and `data-clause` (`filter`, `highlight` or both,
 *   separated by a space) while its artifact is filtered or highlighted.
 * @csspart expander - A row's expand button, with `aria-expanded` while its children are shown. On
 *   a row with no children it has `data-leaf` and is hidden, keeping the names aligned.
 * @csspart name - A row's name, with `data-unnamed` if it has none. Pressing it highlights the
 *   artifact, and it has `aria-pressed` while the artifact is highlighted.
 * @csspart counts - A row's counts.
 * @csspart count-matched - The `<mosaica-count>` of a row's items that match the filters, while a
 *   filter is set.
 * @csspart count-masked - The `<mosaica-count>` of a row's items the viewer may see.
 * @csspart actions - A row's buttons, shown while the pointer or focus is in the row.
 * @csspart highlight - A row's highlight button, with `aria-pressed`.
 * @csspart filter - A row's filter button, with `aria-pressed`.
 * @csspart fit - A row's Fit button, on a layer that draws.
 * @csspart dismiss - A button that removes a row's filter or highlight, one for each, with
 *   `data-verb`: the clause's icon and a ×.
 * @csspart also - The row's other parents, or "Children unavailable" with `data-code` if its
 *   children were refused.
 * @csspart children - An expanded row's children.
 * @csspart more - The More button, which loads the next page and moves focus to its first row.
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
        padding: 1px 0;
        max-height: 320px;
        overflow-y: auto;
      }
      [part='row'] {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        min-height: 28px;
        padding: 0 4px 0 calc(4px + var(--depth, 0) * 14px);
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
        display: inline-grid;
        place-items: center;
        width: 24px;
        height: 24px;
        flex: none;
        border-radius: 4px;
        color: var(--_mosaica-ink-3);
      }
      [part='expander'][data-leaf] {
        visibility: hidden;
      }
      /* A name too long to stand beside the counts and buttons has them under it, and wraps. */
      .lead {
        display: flex;
        align-items: center;
        flex: 1 1 auto;
        min-width: 0;
      }
      [part='name'] {
        flex: 1 1 auto;
        min-width: 0;
        min-height: 24px;
        padding: 2px 4px;
        border-radius: 4px;
        text-align: left;
        overflow-wrap: anywhere;
      }
      .tail {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        justify-content: flex-end;
        gap: 0 4px;
        max-width: 100%;
        margin-left: auto;
      }
      [part='counts'] {
        display: inline-flex;
        gap: 6px;
        align-items: baseline;
        font-size: 12px;
        color: var(--_mosaica-ink-2);
      }
      /* Keeps its place in the row while not drawn, so showing it moves nothing; still reachable by Tab. */
      [part='actions'] {
        display: inline-flex;
        opacity: 0;
        pointer-events: none;
      }
      [part='row']:hover [part='actions'],
      [part='row']:focus-within [part='actions'] {
        opacity: 1;
        pointer-events: auto;
      }
      @media (hover: none) {
        [part='actions'] {
          opacity: 1;
          pointer-events: auto;
        }
      }
      [part='actions'] button {
        display: inline-grid;
        place-items: center;
        width: 24px;
        height: 24px;
        color: var(--_mosaica-ink-2);
        border-radius: 4px;
      }
      [part='actions'] button:hover {
        color: var(--_mosaica-ink);
        background: var(--_mosaica-surface-3);
      }
      [part='actions'] button[aria-pressed='true'] {
        color: var(--_mosaica-ink);
        background: var(--_mosaica-surface-3);
        box-shadow: inset 0 -2px 0 currentColor;
      }
      /* The clause's icon and a ×, in a target 24 px high. */
      [part='dismiss'] {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: 1px;
        min-width: 24px;
        height: 24px;
        padding: 0 3px;
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
        padding-left: calc(32px + var(--depth, 0) * 14px);
        font-size: 11px;
        color: var(--_mosaica-ink-3);
      }
      [part='more'] {
        min-height: 24px;
        margin: 2px 0 2px calc(32px + var(--depth, 0) * 14px);
      }
      [part='children'] {
        list-style: none;
        margin: 0;
        padding: 0;
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
   * refetches the roots and the children of the open rows, whose counts answer the old question.
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

  /** The control focus was on as the last render began. */
  private held: Held | null = null;

  /** Re-asked before every render, so putting the panel away and taking it out again both count. */
  protected override willUpdate(changed: PropertyValues<this>): void {
    super.willUpdate(changed);
    if (!this.shown && this.displayed()) this.shown = true;
    this.held = this.heldControl();
  }

  protected override updated(): void {
    if (this.held) this.keepFocus(this.held);
    this.loadRootsIfStale();
  }

  /** One page of the current layer, under the map's filter, which the store supplies. */
  private page(req: {parent?: bigint; q?: string; cursor?: string}): Promise<BrowsePage> | null {
    const s = this.resolvedStore;
    const layer = this.current();
    if (!s || !layer) return null;
    const ceiling = s.get('meta')?.selection.maxBrowseRows ?? this.limit;
    return s.browse({...req, layer: layer.name, limit: Math.max(1, Math.min(this.limit, ceiling))});
  }

  /**
   * A page of the roots, with the children of every row on it that is open. The page replaces or
   * joins the rows once those have landed, so a walk under one question is drawn whole under the
   * next. A page asked under a question since replaced is dropped. Returns the rows it added.
   */
  private async loadRoots(cursor?: string): Promise<Node[]> {
    const request = this.page(cursor === undefined ? {} : {cursor});
    if (!request) return [];
    const epoch = this.epoch;
    const under = this.fetchedUnder;
    const wanted = () => epoch === this.epoch && under === this.fetchedUnder;
    this.loading = true;
    this.refusal = null;
    try {
      const page = await request;
      if (!wanted()) return [];
      const nodes = page.artifacts.map((row) => this.node(row, ''));
      await this.reopen(nodes);
      if (!wanted()) return [];
      this.roots = cursor === undefined ? nodes : [...(this.roots ?? []), ...nodes];
      this.rootsNext = page.next;
      return nodes;
    } catch (error) {
      if (!wanted()) return [];
      this.refusal = refusalOf(error);
      this.roots = this.roots ?? [];
      return [];
    } finally {
      if (wanted()) this.loading = false;
    }
  }

  /** Fetch the children of each of `nodes` that is open, and of each of those. */
  private async reopen(nodes: readonly Node[]): Promise<void> {
    await Promise.all(
      nodes
        .filter((n) => this.open.has(n.path) && n.row.childCount > 0 && n.children === null)
        .map((n) => this.expand(n))
    );
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
  private async expand(node: Node, more = false): Promise<Node[]> {
    const request = this.page({parent: node.row.mosaicaId, ...(more && node.next ? {cursor: node.next} : {})});
    if (!request) return [];
    const epoch = this.epoch;
    node.loading = true;
    node.refusal = null;
    this.revision++;
    try {
      const page = await request;
      if (epoch !== this.epoch) return [];
      const nodes = page.artifacts.map((row) => this.node(row, node.path));
      await this.reopen(nodes);
      node.children = more ? [...(node.children ?? []), ...nodes] : nodes;
      node.next = page.next;
      return nodes;
    } catch (error) {
      node.refusal = refusalOf(error);
      node.children = node.children ?? [];
      return [];
    } finally {
      node.loading = false;
      this.revision++;
    }
  }

  /** Open a row, fetching its children where it has none yet, or close one that is open or opening. */
  private toggle(node: Node): void {
    const open = new Set(this.open);
    if (open.has(node.path) && (node.children !== null || node.loading)) open.delete(node.path);
    else {
      open.add(node.path);
      if (node.children === null && !node.loading) void this.expand(node);
    }
    this.open = open;
  }

  /**
   * Load the next page of the roots, or of `parent`'s children, and move focus from the More
   * button, which the page replaces or removes, to the first row it added.
   */
  private async more(button: HTMLElement, parent: Node | null): Promise<void> {
    const added = await (parent ? this.expand(parent, true) : this.loadRoots(this.rootsNext ?? undefined));
    await this.updateComplete;
    const active = this.shadowRoot?.activeElement ?? null;
    const first = added[0];
    if (!first || (active !== null && active !== button)) return;
    this.rowElements()
      .find((r) => r.path === first.path)
      ?.querySelector<HTMLElement>('[part="name"]')
      ?.focus();
  }

  private rowElements(): RowElement[] {
    return Array.from(this.renderRoot.querySelectorAll<RowElement>('[part="row"]'));
  }

  /** The control in a row that focus is on, read before a render so {@link keepFocus} can put it back. */
  private heldControl(): Held | null {
    const active = this.shadowRoot?.activeElement;
    const row = active?.closest<RowElement>('[part="row"]');
    const part = active?.getAttribute('part');
    if (!active || !row || !part) return null;
    return {path: row.path, at: this.rowElements().indexOf(row), part, verb: active.getAttribute('data-verb')};
  }

  /**
   * After a render, put focus back on the control it was on, in the row it was on. Where a ×
   * removed itself, focus goes to the row's other ×, else to the button that makes the clause.
   * Where the row is gone, it goes to the row now in its place, else the last, else the search box.
   */
  private keepFocus(held: Held): void {
    const rows = this.rowElements();
    const active = this.shadowRoot?.activeElement ?? null;
    const same = rows.find((r) => r.path === held.path);
    const row = same ?? (active ? null : rows[Math.min(held.at, rows.length - 1)]);
    if (!row) {
      if (!active) this.renderRoot.querySelector<HTMLElement>('[part="search"] input')?.focus();
      return;
    }
    const exact = row.querySelector<HTMLElement>(`[part="${held.part}"]${held.verb ? `[data-verb="${held.verb}"]` : ''}`);
    const other = held.part === 'dismiss' ? (row.querySelector<HTMLElement>('[part="dismiss"]') ?? row.querySelector<HTMLElement>(`[part="actions"] [data-verb="${held.verb}"]`)) : null;
    const to = exact ?? other ?? row.querySelector<HTMLElement>('[part="name"]');
    if (to !== active) to?.focus();
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

  /** What the rows' own fetches say, for the status region: the rows loading children, and those refused them. */
  private rowNotes(nodes: readonly Node[]): string[] {
    return nodes.flatMap((n) => {
      const name = artifactName(n.row) ?? UNNAMED;
      return [...(n.loading ? [`Loading children of ${name}`] : []), ...(n.refusal ? [`Children of ${name} unavailable`] : []), ...this.rowNotes(n.children ?? [])];
    });
  }

  override render(): TemplateResult | typeof nothing {
    const s = this.resolvedStore;
    const heading = html`<h2 part="title">Hierarchy</h2>`;
    const meta = s?.get('meta') ?? null;
    if (!s || !meta) return html`<div class="panel">${heading}${renderState(stateOf(s?.get('status')), s?.get('status'))}</div>`;
    const offered = this.layers();
    if (offered.length === 0) return html`<div class="panel">${heading}<span part="state" data-state="empty">No hierarchical layers</span></div>`;
    const layer = this.current()!;
    const title = layer.title || layer.name;
    const filtered = s.get('filters').expr !== null || s.get('filters').members.length > 0;
    const rows = this.searching ?? this.roots;
    const state = this.refusal ? 'refused' : rows === null ? (this.loading ? 'loading' : 'shown') : rows.length === 0 ? 'empty' : 'shown';
    const said = this.refusal
      ? html`<span part="refusal" data-code=${this.refusal.code}><span class="dot refuse"></span>Layer unavailable</span>`
      : state === 'loading'
        ? html`<span class="dot quiet"></span>Loading`
        : state === 'empty'
          ? this.searching
            ? 'No match'
            : 'Nothing here'
          : nothing;
    const notes = this.rowNotes(rows ?? []);
    return html`<div class="panel">${heading}
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
        aria-label=${`Search ${title}`}
        .value=${this.query}
        placeholder="Search names"
        autocomplete="off"
        @input=${(e: Event) => this.search((e.target as HTMLInputElement).value)}
      /></div>
      <span part="state" role="status" data-state=${state}>${said}${notes.length > 0 ? html`<span class="sr-only">${notes.join('. ')}</span>` : nothing}</span>
      ${rows === null || rows.length === 0 ? nothing : html`<ul part="tree" aria-label=${title}>${rows.map((n) => this.renderNode(n, 0, layer, filtered))}</ul>`}
      ${this.searching === null && this.rootsNext
        ? html`<button part="more" class="more-link" type="button" @click=${(e: Event) => void this.more(e.currentTarget as HTMLElement, null)}>More</button>`
        : nothing}
    </div>`;
  }

  private renderNode(node: Node, depth: number, layer: Layer, filtered: boolean): unknown {
    // A row is expanded while its children are drawn, not while they are on their way.
    const open = this.open.has(node.path) && node.children !== null;
    const id = node.row.mosaicaId;
    const clauses = this.clausesOn(id);
    const name = artifactName(node.row);
    const called = name ?? UNNAMED;
    // A `dag` node is drawn under each served parent; the row names the others.
    const also = node.row.parentIds.filter((p) => String(p) !== node.path.split('/').at(-2));
    const drawn = !isFilterLayer(layer);
    const pressed = (verb: ClauseVerb) => (clauses.includes(verb) ? 'true' : 'false');
    return html`<li>
      <div part="row" style=${`--depth:${depth}`} .path=${node.path} data-id=${idString(id)} data-clause=${clauses.length > 0 ? clauses.sort().join(' ') : nothing}>
        <span class="lead"><button part="expander" type="button" data-leaf=${node.row.childCount === 0 ? '' : nothing}
          aria-expanded=${open ? 'true' : 'false'}
          aria-label=${open ? `Collapse ${name ?? 'row'}` : `Expand ${name ?? 'row'}`}
          @click=${() => this.toggle(node)}>${node.loading ? html`<span class="spinner"></span>` : icon(open ? 'chev' : 'chevr', 12)}</button>
        <button part="name" type="button" data-unnamed=${name === null ? '' : nothing}
          aria-label=${`Highlight ${called}`} aria-pressed=${pressed('highlight')} title="Highlight"
          @click=${() => this.apply(id, 'highlight')}>${called}</button></span>
        <span class="tail">
          <span part="actions">
            <button part="highlight" type="button" data-verb="highlight" aria-label=${`Highlight ${called}`} aria-pressed=${pressed('highlight')}
              title="Highlight this" @click=${() => this.apply(id, 'highlight')}>${icon('highlight', 13)}</button>
            <button part="filter" type="button" data-verb="filter" aria-label=${`Filter to ${called}`} aria-pressed=${pressed('filter')}
              title="Filter to this" @click=${() => this.apply(id, 'filter')}>${icon('filter', 13)}</button>
            ${
              // A filter layer draws nothing, so there is nothing to fit to.
              drawn
                ? html`<button part="fit" type="button" aria-label=${`Fit the map to ${called}`} title="Fit the map to it"
                    @click=${() => emit(this, 'mosaica-artifactfit', {id: idString(id)})}>${icon('fit', 13)}</button>`
                : nothing
            }
          </span>
          <span part="counts">
            ${filtered && node.row.matchedCount !== null
              ? html`<mosaica-count part="count-matched" .masked=${masked(node.row.matchedCount)}></mosaica-count>/`
              : nothing}<mosaica-count part="count-masked" .masked=${masked(node.row.maskedCount)}></mosaica-count>
          </span>
          ${clauses.map(
            (verb) => html`<button part="dismiss" type="button" data-verb=${verb} aria-label=${verb === 'highlight' ? `Stop highlighting ${called}` : `Stop filtering to ${called}`}
              title=${verb === 'highlight' ? 'Stop highlighting' : 'Stop filtering'} @click=${() => this.apply(id, verb)}>${icon(verb, 12)}${icon('close', 10)}</button>`
          )}
        </span>
      </div>
      ${also.length > 0 ? html`<div part="also" style=${`--depth:${depth}`}>also under ${also.map((p) => this.nameOf(p)).join(', ')}</div>` : nothing}
      ${node.refusal ? html`<div part="also" style=${`--depth:${depth}`} data-code=${node.refusal.code}>Children unavailable</div>` : nothing}
      ${open
        ? html`<ul part="children" aria-label=${`Children of ${called}`}>
            ${node.children!.map((c) => this.renderNode(c, depth + 1, layer, filtered))}
            ${node.next
              ? html`<li><button part="more" class="more-link" style=${`--depth:${depth + 1}`} type="button" @click=${(e: Event) => void this.more(e.currentTarget as HTMLElement, node)}>More</button></li>`
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
