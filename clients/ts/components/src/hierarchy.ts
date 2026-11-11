import {css, html, nothing, type PropertyValues, type TemplateResult} from 'lit';
import {property, state} from 'lit/decorators.js';
import {browsableLayers, isFilterLayer, withMember, withoutMember, type ClauseVerb, type Layer, type Masked, type Refusal} from '@mosaicajs/client';
import {refusalOf} from '@mosaicajs/client/internal';
import {MosaicaElement, called, calledList, emit, focusLost, idString, labelOf} from './base.js';
import {attachContextRoot, defineOnce} from './define.js';
import {icon} from './icons.js';
import {Lineage, focusAdded, spokenCounts, type Node} from './lineage.js';
import {renderState, stateOf} from './states.js';
import {chrome, tokens} from './tokens.js';
import './count.js';


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
 * also shows how many of those items match, and a screen reader is given each count with what it
 * counts. An artifact the viewer may not see is left out, and its children appear under its
 * nearest ancestors the viewer may see, or at the top level where it has none. A row under several
 * served parents names the others, and a parent no page has served yet is asked for by name. When
 * the filters change, the rows are fetched again, a few requests at a time: as many pages of the
 * roots and of each open row as More had loaded, since a page's cursor is good only for the
 * filters it was served under. Focus stays on the control it was on.
 *
 * A row's name is one line, cut with an ellipsis where it is too long, unless
 * `--mosaica-long-text` is `normal`. A row's buttons highlight or filter the map by its artifact.
 * They are shown in place of its counts while the pointer or focus is in the row, and Escape puts
 * them away again; where there is no pointer to hover with, they stand beside the counts. Its Fit
 * button only fires
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
 * @csspart state - The state line, with `data-state`, inside a `role="status"` region, which also
 *   says which rows are loading children and which were refused them.
 * @csspart refusal - The words "View refused" or "Layer unavailable", with `data-code`.
 * @csspart reason - The server's reason for a refusal, under "View refused".
 * @csspart layer - The layer select, shown if there are several layers.
 * @csspart search - The search box.
 * @csspart tree - The tree of rows, named by the layer.
 * @csspart row - One row, with `data-id`, `data-clause` (`filter`, `highlight` or both, separated
 *   by a space) while its artifact is filtered or highlighted, and `data-buttons-away` once Escape has put
 *   its buttons away.
 * @csspart expander - A row's expand button, with `aria-expanded` while its children are shown. On
 *   a row with no children it has `data-leaf` and is hidden, keeping the names aligned.
 * @csspart name - A row's name. Where the artifact has none it is the artifact's key, with
 *   `data-key-name`, and where it has no key either it is the word Unnamed, with `data-unnamed`.
 *   Pressing it highlights the artifact, and it has `aria-pressed` while the artifact is
 *   highlighted.
 * @csspart counts - A row's counts as they are drawn, hidden from a screen reader, which is given
 *   them beside it with what each counts. They are not drawn while the row's buttons are shown
 *   over them.
 * @csspart count-matched - The `<mosaica-count>` of a row's items that match the filters, while a
 *   filter is set.
 * @csspart count-masked - The `<mosaica-count>` of a row's items the viewer may see.
 * @csspart actions - A row's buttons, shown over its counts while the pointer or focus is in the
 *   row.
 * @csspart highlight - A row's highlight button, with `aria-pressed`.
 * @csspart filter - A row's filter button, with `aria-pressed`.
 * @csspart fit - A row's Fit button, on a layer that draws.
 * @csspart dismiss - A × that removes a row's filter or highlight, one for each, with `data-verb`.
 *   The highlight's has a bar under it.
 * @csspart also - The row's other parents, each with `data-key-name` where it is called by its key,
 *   or "Children unavailable" with `data-code` if its children were refused.
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
      /* The box reaches 3 px left of the rows, so the expander's target at a row's edge is whole. */
      [part='tree'] {
        list-style: none;
        margin: 0 0 0 -3px;
        padding: 0 0 0 3px;
        max-height: 320px;
        overflow-y: auto;
      }
      [part='row'] {
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
      /* Drawn 14 px wide. The pointer's target is 24 px square around it, in the row's own room. */
      [part='expander'] {
        position: relative;
        display: inline-flex;
        align-items: center;
        width: 14px;
        height: 24px;
        flex: none;
        color: var(--_mosaica-ink-3);
      }
      [part='expander']::before {
        content: '';
        position: absolute;
        inset: 0 -5px;
      }
      [part='expander']:focus-visible {
        outline-offset: 0;
      }
      [part='expander'][data-leaf] {
        visibility: hidden;
      }
      /* One line, cut with an ellipsis, unless the host asks for long names to wrap. */
      [part='name'] {
        flex: 1;
        min-width: 0;
        min-height: 24px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: var(--_mosaica-long-text);
        overflow-wrap: anywhere;
        text-align: left;
      }
      [part='counts'] {
        display: inline-flex;
        gap: 6px;
        align-items: baseline;
        font-size: 12px;
        color: var(--_mosaica-ink-2);
      }
      /* The counts and the row's buttons. Where there is no pointer to hover with, side by side. */
      .figures {
        position: relative;
        display: inline-flex;
        align-items: center;
        gap: 4px;
        flex: none;
      }
      [part='actions'] {
        display: inline-flex;
      }
      /* With a pointer, the buttons take no room: they are shown over the counts, which are not
         drawn meanwhile, and over the end of a name beside short counts, while the pointer or
         focus is in the row and Escape has not put them away. They are above the counts, which
         come after them and, not drawn, would still lie over them. */
      @media (hover: hover) {
        [part='actions'] {
          position: absolute;
          right: 0;
          z-index: 1;
          padding-left: 6px;
          background: var(--_mosaica-surface-2);
          border-radius: var(--_mosaica-radius-control);
          opacity: 0;
          pointer-events: none;
        }
        [part='row']:not([data-buttons-away]):is(:hover, :focus-within) [part='actions'] {
          opacity: 1;
          pointer-events: auto;
        }
        [part='row']:not([data-buttons-away]):is(:hover, :focus-within) [part='counts'] {
          opacity: 0;
          pointer-events: none;
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
      /* Drawn 20 px square, with a 24 px target around it. */
      [part='dismiss'] {
        position: relative;
        display: inline-grid;
        place-items: center;
        width: 20px;
        height: 20px;
        flex: none;
        border-radius: 4px;
        color: var(--_mosaica-ink-2);
      }
      [part='dismiss']::before {
        content: '';
        position: absolute;
        inset: -2px;
      }
      /* The × that removes a highlight has a bar under it, so it differs from the filter's by more than its colour. */
      [part='dismiss'][data-verb='highlight'] {
        color: var(--_mosaica-highlight);
        box-shadow: inset 0 -2px 0 currentColor;
      }
      [part='dismiss']:hover {
        background: var(--_mosaica-surface-3);
      }
      [part='also'] {
        padding-left: calc(18px + var(--depth, 0) * 14px);
        font-size: 11px;
        color: var(--_mosaica-ink-3);
      }
      [part='also'] [data-key-name] {
        color: inherit;
      }
      /* Drawn as a line of text, with a target 24 px high and at least as wide around it. */
      [part='more'] {
        position: relative;
        margin: 4px 0 4px calc(18px + var(--depth, 0) * 14px);
      }
      [part='more']::before {
        content: '';
        position: absolute;
        inset: -4px 0;
        min-width: 24px;
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

  @state() private accessor query = '';
  @state() private accessor searching: Node[] | null = null;
  /** Why the last search was refused. */
  @state() private accessor searchRefusal: Refusal | null = null;
  /** Which paths are open, so a re-render of the tree keeps the walk. */
  @state() private accessor open = new Set<string>();
  /** The paths of the rows whose buttons Escape has put away, each until the pointer and focus leave it. */
  @state() private accessor buttonsAway: ReadonlySet<string> = new Set();

  /** The browse pages of the current layer: its roots and the children of the rows opened. */
  private readonly lineage = new Lineage({
    store: () => this.resolvedStore,
    layer: () => this.current(),
    limit: () => this.limit,
    isOpen: (path) => this.open.has(path),
    changed: () => this.requestUpdate()
  });
  private searchTimer: ReturnType<typeof setTimeout> | null = null;
  /** Moved by {@link resetServerData}, so a search asked for before it is dropped. */
  private epoch = 0;

  protected override resetServerData(): void {
    this.epoch += 1;
    this.lineage.reset();
    this.searching = null;
    this.searchRefusal = null;
    this.open = new Set();
    this.buttonsAway = new Set();
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
    document.addEventListener('keydown', this.onEscape);
    if (typeof IntersectionObserver === 'undefined') return;
    this.observer ??= new IntersectionObserver((entries) => {
      if (!entries.some((e) => e.isIntersecting) || this.shown) return;
      this.shown = true;
      this.loadRootsIfStale();
    });
    this.observer.observe(this);
  }

  override disconnectedCallback(): void {
    document.removeEventListener('keydown', this.onEscape);
    this.observer?.disconnect();
    this.observer = null;
    super.disconnectedCallback();
  }

  /**
   * Escape puts away the buttons shown over a row's counts, wherever focus is, since the pointer
   * alone shows them. Focus on one of them goes to the row's name.
   */
  private onEscape = (e: KeyboardEvent): void => {
    if (e.key !== 'Escape') return;
    const active = this.shadowRoot?.activeElement ?? null;
    const focused = active?.closest<RowElement>('[part="row"]') ?? null;
    const shown = [focused, this.renderRoot.querySelector<RowElement>('[part="row"]:hover')].filter((row) => row !== null && !this.buttonsAway.has(row.path));
    if (shown.length === 0) return;
    this.buttonsAway = new Set([...this.buttonsAway, ...shown.map((row) => row!.path)]);
    if (active?.closest('[part="actions"]')) focused?.querySelector<HTMLElement>('[part="name"]')?.focus();
  };

  /** The buttons Escape put away come back once the pointer and focus have both left the row. */
  private leftRow(e: Event, path: string): void {
    if (!this.buttonsAway.has(path)) return;
    const row = e.currentTarget as HTMLElement;
    const to = e.type === 'focusout' ? ((e as FocusEvent).relatedTarget as Element | null) : (this.shadowRoot?.activeElement ?? null);
    if (row.contains(to) || (e.type === 'focusout' && row.matches(':hover'))) return;
    this.shownAgain(path);
  }

  /** Let a row show its buttons again. */
  private shownAgain(path: string): void {
    if (this.buttonsAway.has(path)) this.buttonsAway = new Set([...this.buttonsAway].filter((p) => p !== path));
  }

  /** Whether this element is displayed; see {@link shown}. */
  private displayed(): boolean {
    return typeof this.checkVisibility === 'function' ? this.checkVisibility() : true;
  }

  /** The roots, where the question they were fetched under is no longer the one being asked. */
  private loadRootsIfStale(): void {
    if (this.shown) this.lineage.loadIfStale();
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

  /** Open a row, fetching its children where it has none yet, or close one that is open or opening. */
  private toggle(node: Node): void {
    const open = new Set(this.open);
    if (open.has(node.path) && (node.children !== null || node.loading)) open.delete(node.path);
    else {
      open.add(node.path);
      if (node.children === null && !node.loading) void this.lineage.expand(node);
    }
    this.open = open;
  }

  /**
   * Load the next page of the roots, or of `parent`'s children, and move focus from the More
   * button, which the page replaces or removes, to the first row it added. Focus that has gone
   * anywhere else meanwhile stays there.
   */
  private async more(button: HTMLElement, parent: Node | null): Promise<void> {
    const added = await (parent ? this.lineage.expand(parent, true) : this.lineage.loadMore());
    const first = added[0];
    if (first) await focusAdded(this, button, () => this.rowElements().find((r) => r.path === first.path)?.querySelector<HTMLElement>('[part="name"]'));
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
    // Focus is still in the tree, or the render removed the control that had it.
    if (!active && !focusLost()) return;
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
        this.searchRefusal = null;
        return;
      }
      const request = this.lineage.search(q);
      if (!request) return;
      const epoch = this.epoch;
      void request
        .then((nodes) => {
          if (epoch !== this.epoch) return;
          this.searching = nodes;
          this.searchRefusal = null;
        })
        .catch((error: unknown) => {
          if (epoch !== this.epoch) return;
          this.searchRefusal = refusalOf(error);
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
    s.setMembers(on ? withoutMember(held, layer.name, id, verb) : withMember(held, {layer: layer.name, artifact: id, outside: false, verb, ...labelOf(this.lineage.nameOf(id))}));
    emit(this, 'mosaica-clausechange', {id: idString(id), layer: layer.name, outside: false, verb, on: !on});
  }

  /** What the rows' own fetches say, for the status region: the rows loading children, and those refused them. */
  private rowNotes(nodes: readonly Node[]): string[] {
    return nodes.flatMap((n) => {
      const name = called(n.row.name, n.row.key).text;
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
    const {roots, loading} = this.lineage;
    const refusal = this.searchRefusal ?? this.lineage.refusal;
    const rows = this.searching ?? roots;
    const state = refusal ? 'refused' : rows === null ? (loading ? 'loading' : 'shown') : rows.length === 0 ? 'empty' : 'shown';
    const said = refusal
      ? html`<span part="refusal" data-code=${refusal.code}><span class="dot refuse"></span>Layer unavailable</span>`
      : state === 'loading'
        ? html`<span class="sr-only">Loading</span>`
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
            this.lineage.clear();
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
      <div role="status"><span part="state" data-state=${state}>${said}</span>${notes.length > 0 ? html`<span class="sr-only">${notes.join('. ')}</span>` : nothing}</div>
      ${rows === null || rows.length === 0 ? nothing : html`<ul part="tree" aria-label=${title}>${rows.map((n) => this.renderNode(n, 0, layer))}</ul>`}
      ${this.searching === null && this.lineage.hasMore
        ? html`<button part="more" class="more-link" type="button" @click=${(e: Event) => void this.more(e.currentTarget as HTMLElement, null)}>More</button>`
        : nothing}
    </div>`;
  }

  private renderNode(node: Node, depth: number, layer: Layer): unknown {
    // A row is expanded while its children are drawn, not while they are on their way.
    const open = this.open.has(node.path) && node.children !== null;
    const id = node.row.mosaicaId;
    const clauses = this.clausesOn(id);
    const name = called(node.row.name, node.row.key);
    const text = name.text;
    // A `dag` node is drawn under each served parent; the row names the others.
    const also = this.lineage.alsoUnder(node);
    const drawn = !isFilterLayer(layer);
    const pressed = (verb: ClauseVerb) => (clauses.includes(verb) ? 'true' : 'false');
    return html`<li>
      <div part="row" style=${`--depth:${depth}`} .path=${node.path} data-id=${idString(id)} data-clause=${clauses.length > 0 ? clauses.sort().join(' ') : nothing}
        data-buttons-away=${this.buttonsAway.has(node.path) ? '' : nothing} @pointerleave=${(e: Event) => this.leftRow(e, node.path)} @focusout=${(e: Event) => this.leftRow(e, node.path)}>
        <button part="expander" type="button" data-leaf=${node.row.childCount === 0 ? '' : nothing}
          aria-expanded=${open ? 'true' : 'false'}
          aria-label=${open ? `Collapse ${name.from === 'none' ? 'row' : text}` : `Expand ${name.from === 'none' ? 'row' : text}`}
          @click=${() => this.toggle(node)}>${node.loading ? html`<span class="spinner"></span>` : icon(open ? 'chev' : 'chevr', 12)}</button>
        <button part="name" type="button" ?data-key-name=${name.from === 'key'} ?data-unnamed=${name.from === 'none'}
          aria-label=${`Highlight ${text}`} aria-pressed=${pressed('highlight')} title="Highlight"
          @click=${() => this.apply(id, 'highlight')}>${text}</button>
        <span class="figures">
          <span part="actions" @focusin=${() => this.shownAgain(node.path)}>
            <button part="highlight" type="button" data-verb="highlight" aria-label=${`Highlight ${text}`} aria-pressed=${pressed('highlight')}
              title="Highlight this" @click=${() => this.apply(id, 'highlight')}>${icon('highlight', 13)}</button>
            <button part="filter" type="button" data-verb="filter" aria-label=${`Filter to ${text}`} aria-pressed=${pressed('filter')}
              title="Filter to this" @click=${() => this.apply(id, 'filter')}>${icon('filter', 13)}</button>
            ${
              // A filter layer draws nothing, so there is nothing to fit to.
              drawn
                ? html`<button part="fit" type="button" aria-label=${`Fit the map to ${text}`} title="Fit the map to it"
                    @click=${() => emit(this, 'mosaica-artifactfit', {id: idString(id)})}>${icon('fit', 13)}</button>`
                : nothing
            }
          </span>
          <span part="counts" aria-hidden="true">
            ${node.row.matchedCount !== null
              ? html`<mosaica-count part="count-matched" .masked=${masked(node.row.matchedCount)}></mosaica-count>/`
              : nothing}<mosaica-count part="count-masked" .masked=${masked(node.row.maskedCount)}></mosaica-count>
          </span><span class="sr-only">${spokenCounts(node.row)}</span>
        </span>
        ${clauses.map(
            (verb) => html`<button part="dismiss" type="button" data-verb=${verb} aria-label=${verb === 'highlight' ? `Stop highlighting ${text}` : `Stop filtering to ${text}`}
              title=${verb === 'highlight' ? 'Stop highlighting' : 'Stop filtering'} @click=${() => this.apply(id, verb)}>${icon('close', 12)}</button>`
        )}
      </div>
      ${also.length > 0 ? html`<div part="also" style=${`--depth:${depth}`}>also under ${calledList(also)}</div>` : nothing}
      ${node.refusal ? html`<div part="also" style=${`--depth:${depth}`} data-code=${node.refusal.code}>Children unavailable</div>` : nothing}
      ${open
        ? html`<ul part="children" aria-label=${`Children of ${text}`}>
            ${node.children!.map((c) => this.renderNode(c, depth + 1, layer))}
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
