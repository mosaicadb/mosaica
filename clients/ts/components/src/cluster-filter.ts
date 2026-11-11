import {css, html, nothing, type PropertyValues, type TemplateResult} from 'lit';
import {property, state} from 'lit/decorators.js';
import {repeat} from 'lit/directives/repeat.js';
import {artifactName, withMember, withoutMember, type BrowseRow, type Layer, type MemberClause, type Refusal} from '@mosaicajs/client';
import {refusalOf} from '@mosaicajs/client/internal';
import {HeldAggregate, artifactGroupings, countsByKey} from './aggregate.js';
import {MosaicaElement, called, emit, idString, labelOf} from './base.js';
import {FloatingList} from './float.js';
import {attachContextRoot, defineOnce} from './define.js';
import {icon} from './icons.js';
import {ClusterPaths, type Path} from './paths.js';
import {NamedQuery} from './states.js';
import {chrome, tokens} from './tokens.js';

/** How long typing must be quiet before a name search is sent. */
const SEARCH_DEBOUNCE_MS = 250;
/** The top-level clusters asked for, to rank by count while the box is empty. */
const TOP_LEVEL_ROWS = 50;
/**
 * A path of `n` of its nearest parents, nearest last, led by an ellipsis where parents further up
 * are left out.
 */
function pathText(path: Path, n: number): string {
  return `${path.beyond || n < path.steps.length ? '… › ' : ''}${path.steps.slice(path.steps.length - n).join(' › ')}`;
}

/** How many parents a match's path is walked up, nearest first. */
const PATH_DEPTH = 3;

/** One cluster the list offers. */
type Offer = {row: BrowseRow};

/**
 * Use `<mosaica-cluster-filter>` when you want to search a layer's clusters without the field card
 * around it. Set `layer` to a layer `meta.layers` lists.
 *
 * With the box empty, the list offers the layer's top-level clusters, largest first. Typing
 * searches the cluster names at every level. The arrow keys move between the rows, as do Home and
 * End once one is reached. Enter chooses the row they reached, and Escape closes the list. A line
 * under the box says how many clusters are listed, or why none is, and that they are loading once
 * that has taken a quarter of a second.
 *
 * Each row counts the items the filters match in that cluster without applying this layer's own
 * filter, so a cluster the viewer has filtered out still shows its count. Choosing a row filters
 * the map to the cluster's members, and choosing it again removes that filter.
 *
 * @summary A search box that filters the map by a layer's clusters.
 * @tagname mosaica-cluster-filter
 * @category Elements
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-clausechange']>} mosaica-clausechange - Choosing a
 *   row added or removed a `member_of` clause.
 * @csspart entry - The search box.
 * @csspart values - The list, while it is open.
 * @csspart option - One cluster in the list, with `data-id`, `aria-selected` while its clause is
 *   on, and `data-active` on the one the arrow keys reached.
 * @csspart name - A row's name. Where the cluster has none it is the cluster's key, with
 *   `data-key-name`, and where it has no key either it is the word Unnamed, with `data-unnamed`.
 * @csspart path - A row's path of its parents' names, each step naming every served parent of the
 *   step below it, by its key where it has no name.
 * @csspart value-count - A row's exact count.
 * @csspart more - The note under the box when there is nothing to list, or when more match than
 *   are listed.
 * @csspart refusal - The words "Clusters unavailable", with `data-code`, if the list was refused.
 */
export class MosaicaClusterFilter extends MosaicaElement {
  static override styles = [
    tokens,
    chrome,
    css`
      :host {
        display: block;
      }
      .input {
        height: 28px;
        gap: 6px;
        padding: 0 8px;
        font-size: 12px;
      }
      .input input {
        font-size: 12px;
      }
      .combo {
        position: relative;
      }
      /* The list opens over what sits below the box, in the top layer. */
      [part='values'] {
        position: fixed;
        inset: auto;
        margin: 0;
        box-sizing: border-box;
        overflow-y: auto;
        display: flex;
        flex-direction: column;
        padding: 4px;
        border: 1px solid var(--_mosaica-line);
        border-radius: var(--_mosaica-radius-control);
        background: var(--_mosaica-surface);
        box-shadow: 0 6px 18px rgba(0, 0, 0, 0.08);
      }
      [part~='option'] {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        align-items: center;
        column-gap: 10px;
        padding: 5px 8px;
        border-radius: 4px;
        text-align: left;
      }
      [part~='option']:hover,
      [part~='option'][data-active] {
        background: var(--_mosaica-surface-2);
      }
      [part~='option'][aria-selected='true'] {
        background: var(--_mosaica-surface-2);
        font-weight: 600;
      }
      /* The box keeps focus, so the row the arrow keys reached is ringed as if it had it. */
      [part~='option'][data-active] {
        outline: 2px solid var(--_mosaica-focus);
        outline-offset: -2px;
      }
      .opt {
        display: flex;
        flex-direction: column;
        min-width: 0;
      }
      [part='name'],
      [part='path'] {
        white-space: var(--_mosaica-long-text, nowrap);
        overflow: hidden;
        text-overflow: ellipsis;
        overflow-wrap: anywhere;
      }
      /* On one line, shortened from the left a whole name at a time, so the nearest parents stay; see pathText. */
      [part='path'] {
        font-size: 12px;
        font-weight: 400;
        color: var(--_mosaica-ink-3);
      }
      [part='value-count'] {
        font-size: 12px;
        font-weight: 400;
        font-variant-numeric: tabular-nums;
        color: var(--_mosaica-ink-2);
      }
      [part='more'],
      [part='refusal'] {
        display: block;
        margin-top: 6px;
        font-size: 12px;
        color: var(--_mosaica-ink-3);
      }
      .skel {
        margin-top: 8px;
      }
      @media (forced-colors: active) {
        [part~='option'][data-active] {
          outline-color: Highlight;
        }
      }
    `
  ];

  /** The layer whose clusters are offered, a name `meta.layers` lists. Unset or unknown, the box shows nothing. */
  @property() accessor layer = '';
  /** Placeholder text for the box, in place of the default. */
  @property() accessor placeholder = '';
  /**
   * Turns the box into a cluster picker. Choosing a row calls this with the cluster's `mosaica_id`
   * (as a decimal string), its name, its parents' names as the list shows them, and its level
   * (`rung`), then empties the box. No filter changes.
   */
  @property({attribute: false}) accessor choose: ((cluster: {id: string; name: string | null; path: string; rung: number}) => void) | null = null;

  /** @internal */
  @state() accessor search = '';
  /** Whether the list is open. @internal */
  @state() accessor listOpen = false;
  /** The row the arrow keys reached, by `mosaicaId`; `null` before they reach one. @internal */
  @state() accessor activeId: bigint | null = null;
  /** The top-level clusters, once fetched. @internal */
  @state() accessor tops: Offer[] | null = null;
  /** The matches for the search asked last, once they land. @internal */
  @state() accessor found: {q: string; offers: Offer[]; more: boolean} | null = null;
  /** @internal */
  @state() accessor refusal: Refusal | null = null;

  private readonly counts = new HeldAggregate('clusters');
  /** Whether the open list's clusters have been awaited long enough to say so. */
  private readonly awaited = new NamedQuery(this);
  private readonly floating = new FloatingList(() => {
    const list = this.renderRoot.querySelector<HTMLElement>('[part="values"]');
    const anchor = this.renderRoot.querySelector<HTMLElement>('.combo');
    return list && anchor ? {list, anchor} : null;
  });
  /** The names of the offered clusters' parents, walked {@link PATH_DEPTH} steps up. */
  private readonly paths = new ClusterPaths(() => this.requestUpdate());
  /** The layer and view the top-level clusters were fetched for. */
  private fetchedFor = '';
  private searchTimer: ReturnType<typeof setTimeout> | null = null;
  /** Moved by {@link resetServerData}, so a page asked for before is dropped. */
  private epoch = 0;

  protected override resetServerData(): void {
    this.epoch += 1;
    this.tops = null;
    this.found = null;
    this.refusal = null;
    this.paths.reset();
    this.fetchedFor = '';
  }

  override disconnectedCallback(): void {
    if (this.searchTimer) clearTimeout(this.searchTimer);
    this.searchTimer = null;
    this.counts.set(null, null);
    this.floating.stop();
    this.awaited.stop();
    super.disconnectedCallback();
  }

  private declared(): Layer | null {
    return this.resolvedStore?.get('meta')?.layers.find((l) => l.name === this.layer) ?? null;
  }

  /** The first declared level of a levelled layer, whose clusters are the top level; `undefined` on any other. */
  private firstLevel(layer: Layer): number | undefined {
    const levelled = layer.hierarchy.kind === 'stacked' || layer.hierarchy.kind === 'tiered';
    return levelled ? layer.levels[0]?.level : undefined;
  }

  /** One browse page of the layer. Counts come from the aggregate, so the page is asked unfiltered. */
  private browse(req: {q?: string; parent?: bigint; level?: number; limit: number}) {
    const s = this.resolvedStore;
    if (!s) return Promise.reject(new Error('no store'));
    const ceiling = s.get('meta')?.selection.maxBrowseRows ?? req.limit;
    const {level, ...rest} = req;
    return s.browse({...rest, layer: this.layer, filters: null, limit: Math.max(1, Math.min(req.limit, ceiling)), ...(level === undefined ? {} : {level})});
  }

  /** Offer the rows of a page, and walk the parents of each. */
  private offer(rows: BrowseRow[], layer: Layer): Offer[] {
    const s = this.resolvedStore;
    this.paths.meet(rows);
    if (s) this.paths.ask(s, layer, rows.map((r) => r.mosaicaId), PATH_DEPTH);
    return rows.map((row) => ({row}));
  }

  /** An offer's path of parents, once walked; empty for a cluster with none. */
  private pathOf(o: Offer): Path | null {
    return this.paths.path(o.row.mosaicaId, PATH_DEPTH);
  }

  /** Fetch the top-level clusters, once per layer and view. */
  private fetchTops(): void {
    const s = this.resolvedStore;
    const layer = this.declared();
    if (!s || !layer) return;
    const key = `${layer.name}|${s.get('view').id}`;
    if (key === this.fetchedFor) return;
    this.fetchedFor = key;
    const epoch = this.epoch;
    const level = this.firstLevel(layer);
    this.browse({limit: TOP_LEVEL_ROWS, ...(level === undefined ? {} : {level})})
      .then((page) => {
        if (epoch !== this.epoch) return;
        this.tops = this.offer(page.artifacts, layer);
        this.refusal = null;
      })
      .catch((error: unknown) => {
        if (epoch !== this.epoch) return;
        this.refusal = refusalOf(error);
        this.tops = [];
        // Asked again the next time the list opens.
        this.fetchedFor = '';
      });
  }

  /**
   * Search the layer's names for `q`, and walk each match's path. One request with no level
   * searches every level of a levelled layer.
   */
  private searchFor(q: string): void {
    const layer = this.declared();
    if (!layer) return;
    const epoch = this.epoch;
    const limit = this.resolvedStore?.get('meta')?.selection.maxBrowseRows ?? 50;
    this.browse({q, limit: Math.min(limit, 50)})
      .then((page) => {
        if (epoch !== this.epoch || this.search.trim() !== q) return;
        this.found = {q, offers: this.offer(page.artifacts, layer), more: page.next !== null};
        this.refusal = null;
      })
      .catch((error: unknown) => {
        if (epoch !== this.epoch) return;
        this.refusal = refusalOf(error);
        this.found = {q, offers: [], more: false};
      });
  }

  /** The clauses this field holds: the layer's, in its position. */
  private held(): MemberClause[] {
    return (this.resolvedStore?.get('filters').members ?? []).filter((m) => m.layer === this.layer && m.verb === 'filter');
  }

  private toggle(offer: Offer): void {
    const s = this.resolvedStore;
    if (!s) return;
    const row = offer.row;
    if (this.choose) {
      this.search = '';
      this.found = null;
      this.listOpen = false;
      const path = this.pathOf(offer);
      this.choose({id: idString(row.mosaicaId), name: artifactName(row), path: path && path.steps.length > 0 ? pathText(path, path.steps.length) : '', rung: row.rung});
      return;
    }
    const members = s.get('filters').members;
    const on = this.held().some((m) => m.artifact === row.mosaicaId && !m.outside);
    s.setMembers(
      on
        ? withoutMember(members, this.layer, row.mosaicaId, 'filter')
        : withMember(members, {layer: this.layer, artifact: row.mosaicaId, outside: false, verb: 'filter', ...labelOf(called(row.name, row.key))})
    );
    emit(this, 'mosaica-clausechange', {id: idString(row.mosaicaId), layer: this.layer, outside: false, verb: 'filter', on: !on});
    this.search = '';
    this.found = null;
    this.listOpen = false;
  }

  private type(text: string): void {
    this.search = text;
    this.activeId = null;
    this.listOpen = true;
    if (this.searchTimer) clearTimeout(this.searchTimer);
    const q = text.trim();
    if (q === '') {
      this.found = null;
      return;
    }
    this.searchTimer = setTimeout(() => {
      this.searchTimer = null;
      this.searchFor(q);
    }, SEARCH_DEBOUNCE_MS);
  }

  /** The offers the list shows now: the matches for the box, or the top-level clusters while it is empty. */
  private listed(): Offer[] | null {
    const q = this.search.trim();
    if (q === '') return this.tops;
    return this.found && this.found.q === q ? this.found.offers : null;
  }

  /** Moves focus to the search box. */
  override focus(options?: FocusOptions): void {
    const target = this.renderRoot.querySelector<HTMLElement>('#ctl');
    if (target) target.focus(options);
    else super.focus(options);
  }

  /** How many of each offer's parents its path names, where all of them do not fit; by `mosaica_id`. */
  private pathFits = new Map<bigint, number>();
  /** The rows and width {@link fitPaths} last measured. */
  private pathsMeasured = '';

  /**
   * Where a path is kept to one line, name as many of each listed path's nearest parents as fit
   * it, dropping whole names from the left. Measured with the line's own font, after the list is
   * placed. A path that wraps names them all.
   */
  private fitPaths(): void {
    // Only while the list shows, and only when its rows or its width changed since last measured.
    const shown = this.listOpen ? Array.from(this.renderRoot.querySelectorAll<HTMLElement>('[part="path"][data-id]')) : [];
    const oneLine = shown[0] ? getComputedStyle(shown[0]).whiteSpace === 'nowrap' : false;
    const key = `${oneLine}|${shown[0]?.clientWidth ?? 0}|${shown.map((el) => `${el.dataset.id}:${el.title}`).join(',')}`;
    if (shown.length === 0 || key === this.pathsMeasured) return;
    this.pathsMeasured = key;
    const context = !oneLine || typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
    let changed = false;
    for (const el of shown) {
      const id = BigInt(el.dataset.id!);
      const o = this.listed()?.find((x) => x.row.mosaicaId === id);
      const path = o ? this.pathOf(o) : null;
      if (!path) continue;
      let n = path.steps.length;
      if (context && el.clientWidth > 0) {
        context.font = getComputedStyle(el).font;
        while (n > 1 && context.measureText(pathText(path, n)).width > el.clientWidth) n -= 1;
      }
      if ((this.pathFits.get(id) ?? path.steps.length) !== n) {
        this.pathFits.set(id, n);
        changed = true;
      }
    }
    if (changed) this.requestUpdate();
  }

  protected override updated(changed: PropertyValues<this>): void {
    super.updated(changed);
    this.floating.update();
    this.fitPaths();
    if (changed.has('activeId')) this.renderRoot.querySelector('[part~="option"][data-active]')?.scrollIntoView({block: 'nearest'});
    this.awaited.track(this.listOpen && this.isConnected && this.refusal === null && this.listed() === null);
    // Counted only while the list is open: a closed list shows no counts, so asks for none.
    const layer = this.declared();
    const limits = this.resolvedStore?.get('meta')?.selection;
    const offers = this.listOpen && this.isConnected ? (this.listed() ?? []) : [];
    this.counts.set(
      this.resolvedStore,
      layer && limits && offers.length > 0
        ? {groupings: artifactGroupings(layer, offers.map((o) => o.row), limits), withoutMembersOf: layer.name}
        : null
    );
  }

  override render(): TemplateResult | typeof nothing {
    const s = this.resolvedStore;
    const layer = this.declared();
    if (!s || !layer) return nothing;
    const title = layer.title || layer.name;
    const on = new Set(this.choose ? [] : this.held().filter((m) => !m.outside).map((m) => m.artifact));
    const counts = countsByKey(this.counts.entry());
    const countOf = (o: Offer) => counts?.get(o.row.mosaicaId.toString());
    const listed = this.listed();
    // Largest first once the counts land; the server's order until then.
    const offers = listed === null ? [] : counts === null ? listed : [...listed].sort((a, b) => (countOf(b) ?? -1) - (countOf(a) ?? -1));
    // The row Enter acts on: the one the arrow keys reached. Until they reach one there is none, so
    // Enter does not take whichever row the counts put first as they land.
    const active = offers.find((o) => o.row.mosaicaId === this.activeId) ?? null;
    const at = active ? offers.indexOf(active) : -1;
    const reach = (i: number) => {
      if (offers.length > 0) this.activeId = offers[(i + offers.length) % offers.length]!.row.mosaicaId;
    };
    const open = this.listOpen && offers.length > 0;
    const option = (o: Offer) => {
      const name = called(o.row.name, o.row.key);
      const count = countOf(o);
      const walked = this.pathOf(o);
      const full = walked && walked.steps.length > 0 ? pathText(walked, walked.steps.length) : '';
      const path = walked && walked.steps.length > 0 ? pathText(walked, this.pathFits.get(o.row.mosaicaId) ?? walked.steps.length) : '';
      return html`<button type="button" part="option" role="option" id=${`c-${o.row.mosaicaId}`} tabindex="-1" data-id=${idString(o.row.mosaicaId)} ?data-active=${o === active}
        aria-selected=${on.has(o.row.mosaicaId) ? 'true' : 'false'}
        @mousedown=${(e: Event) => e.preventDefault()} @click=${() => this.toggle(o)}>
        <span class="opt"><span part="name" ?data-key-name=${name.from === 'key'} ?data-unnamed=${name.from === 'none'}>${name.text}</span>${path ? html`<span part="path" data-id=${idString(o.row.mosaicaId)} title=${full}><bdi>${path}</bdi></span>` : nothing}</span>
        ${count === undefined ? nothing : html`<span part="value-count">${count.toLocaleString('en-GB')}</span>`}
      </button>`;
    };
    const shown = this.placeholder || 'Type a name';
    const box = html`<div class="combo">
      <div class="input">${icon('search', 12)}<input id="ctl" part="entry" type="search" autocomplete="off" placeholder=${shown}
        role="combobox" aria-autocomplete="list" aria-expanded=${open ? 'true' : 'false'} aria-controls="values" aria-activedescendant=${open && active ? `c-${active.row.mosaicaId}` : nothing}
        aria-label=${`${title}: ${shown}`} .value=${this.search}
        @focus=${() => {
          this.fetchTops();
          this.listOpen = true;
        }}
        @blur=${() => (this.listOpen = false)}
        @input=${(e: Event) => this.type((e.target as HTMLInputElement).value)}
        @keydown=${(e: KeyboardEvent) => {
          if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            this.listOpen = true;
            reach(e.key === 'ArrowDown' ? at + 1 : Math.max(at, 0) - 1);
          } else if ((e.key === 'Home' || e.key === 'End') && open && active) {
            // Before the arrow keys reach a row, Home and End move in the text.
            e.preventDefault();
            reach(e.key === 'Home' ? 0 : -1);
          } else if (e.key === 'Enter') {
            if (open && active) this.toggle(active);
          } else if (e.key === 'Escape' && this.listOpen) {
            e.stopPropagation();
            this.listOpen = false;
          }
        }} /></div>
      ${open ? html`<div part="values" id="values" popover="manual" role="listbox" aria-multiselectable=${this.choose ? nothing : 'true'} aria-label=${title}>${repeat(offers, (o) => o.row.mosaicaId, option)}</div>` : nothing}
    </div>`;
    const typed = this.search.trim() !== '';
    // How many clusters are listed, or why none is, for a reader as the list changes under the box.
    const said = this.refusal
      ? html`<span part="refusal" data-code=${this.refusal.code}>Clusters unavailable</span>`
      : listed === null
        ? this.listOpen
          ? html`<span class="skel" aria-hidden="true"></span>${this.awaited.named ? html`<span class="sr-only">Loading clusters</span>` : nothing}`
          : nothing
        : offers.length === 0
          ? typed
            ? html`<span part="more">No cluster by that name</span>`
            : this.listOpen
              ? html`<span part="more">No clusters</span>`
              : nothing
          : !open
            ? nothing
            : html`<span class="sr-only">${offers.length} ${offers.length === 1 ? 'cluster' : 'clusters'} listed. </span>${typed && this.found?.more ? html`<span part="more">Type more to narrow the list</span>` : nothing}`;
    return html`${box}<div role="status">${said}</div>`;
  }
}

attachContextRoot();
defineOnce('mosaica-cluster-filter', MosaicaClusterFilter);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-cluster-filter': MosaicaClusterFilter;
  }
}
