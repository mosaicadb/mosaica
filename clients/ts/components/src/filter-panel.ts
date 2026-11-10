import {css, html, nothing, type PropertyValues, type TemplateResult} from 'lit';
import {property, state} from 'lit/decorators.js';
import {repeat} from 'lit/directives/repeat.js';
import {CLUSTER_PREFIX, activeCount, artifactName, emptyDraft, isPopulated, withoutClause, withoutMember, type ClauseVerb, type ColumnDraft, type FilterDraft, type HeldRegion, type Layer, type MemberClause, type Meta, type Store} from '@mosaicajs/client';
import {OPERATOR_WORDS} from './filter.js';
import {HeldAggregate} from './aggregate.js';
import {BOOL_WORDS, MosaicaElement, UNNAMED, boolKeys, columnCaption, dateRangeText, countText, emit, keyTitle, shortCount} from './base.js';
import {attachContextRoot, defineOnce} from './define.js';
import type {MosaicaFieldCard} from './field-card.js';
import {icon} from './icons.js';
import {exportparts, forwarded} from './parts.js';
import {switchView} from './view-switch.js';
import {FloatingList} from './float.js';
import {renderState, stateOf} from './states.js';
import {chrome, tokens} from './tokens.js';
import './field-card.js';

/** Each card's parts, forwarded as `field-card-<part>`, and the parts of its search box as it forwards them. */
const CARD_PARTS = exportparts('field-card', [...forwarded('filter'), ...forwarded('cluster-filter')]);

/** The layers whose clusters a viewer can filter by in `view`: those `meta` lists for it that attach to no other. */
export function clusterFieldLayers(meta: Meta, view: string): Layer[] {
  return meta.layers.filter((l) => l.depsOn.length === 0 && (view === '' || l.views.includes(view)));
}

/** A layer's field as the panel keys it, apart from any column's. */
const layerField = (layer: string) => `${CLUSTER_PREFIX}${layer}`;

const VERBS: readonly ClauseVerb[] = ['filter', 'highlight'];

/** A view's name as the view picker shows it, else its id. */
const viewTitle = (meta: Meta, id: string) => meta.views.find((v) => v.id === id)?.displayName ?? id;

/** A held region's chip text: its shape and the view it was drawn in. */
function regionText(shape: HeldRegion['shape'], view: string): string {
  const kind = shape.kind === 'box' ? 'Box' : shape.kind === 'lasso' ? 'Lasso' : 'Shape';
  return `${shape.outside ? `Outside ${kind.toLowerCase()}` : kind} in ${view}`;
}

/** A figure in the subject row: whole, or shortened in the compact layout. */
const figure = (n: number, compact: boolean) => (compact ? shortCount(n) : countText(n));

/**
 * Use the filter panel as the place where viewers narrow the map down, as `<mosaica-explorer>` does
 * in its sidebar. Each filter or highlight in force shows as a chip above the cards, a highlight's
 * with the word on it, and pressing a chip opens its field's card. When a chip is removed, focus
 * moves to the × of the chip after it, else of the one before, else to Add field. Under
 * `chips-only` there is no Add field, so when the last chip is removed the panel shows the words
 * No filters, which take focus and go as focus leaves them.
 *
 * A region selected in another view keeps filtering after the viewer switches away from it, and
 * shows as a chip naming its view, such as "Lasso in Topics". Pressing the chip switches back to
 * that view, where the region is selected again, and its × removes it.
 *
 * The row at the top says what the cards' solid bars count. That is the items in view that the
 * filters match, or the highlighted items while there is a highlight. The pale bars count
 * everything the filters match, so that viewers can compare what is in view with the whole.
 *
 * The panel shows a card for each field in `pinned`, for each field that holds a filter or
 * highlight, and for the field the map is coloured by. Viewers add other fields with Add field.
 * Taking a card off clears that field's filters and highlights.
 *
 * Set `chips-only` on one panel and `controls-only` on another when you want the chips apart from
 * the cards.
 *
 * @summary A panel of field cards for filtering and highlighting the map by any field.
 * @tagname mosaica-filter-panel
 * @category Elements
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-filterchange']>} mosaica-filterchange - A
 *   column's chip was removed, Clear all was pressed, the highlight was cleared from the subject
 *   row, or a card was taken off in Add field. Removing a `member_of` chip fires nothing. Each card
 *   fires its own as well.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-selectchange']>} mosaica-selectchange - The chip
 *   of a region held in another view was removed, or Clear all removed one, with that `view`.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-viewswitch']>} mosaica-viewswitch - The chip of
 *   a region held in another view was pressed, and the panel switched to that view.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-chipopen']>} mosaica-chipopen - A column's chip
 *   was pressed while `chips-only` is set, so you can show its card.
 * @csspart state - The state line, with `data-state`.
 * @csspart refusal - The words "View refused", with `data-code`, in the refused state.
 * @csspart reason - The server's reason for a refusal, under "View refused".
 * @csspart subject - The subject row.
 * @csspart subject-key - The colour of the subject's bars.
 * @csspart subject-name - In view, or Highlighted while a highlight is set.
 * @csspart subject-count - The number in view, or highlighted.
 * @csspart clear-highlight - The × beside Highlighted, which clears the highlight.
 * @csspart all - All matching and its number.
 * @csspart all-count - The number matching.
 * @csspart chips - The line of chips, while any filter or highlight is in force, and under
 *   `chips-only` the words that stand in it after the last chip is removed.
 * @csspart chip - One chip, with `data-verb` (`filter` or `highlight`) and `data-column`,
 *   `data-artifact`, or `data-view` for a region held in another view.
 * @csspart edit - A column chip's text, a button that opens its card; on a held region's chip, a
 *   button that switches to its view.
 * @csspart clear - The Clear all button.
 * @csspart card - One `<mosaica-field-card>`, with `data-field`.
 * @csspart add - The Add field button, with `aria-expanded`.
 * @csspart add-note - How many fields are not listed.
 * @csspart add-list - The list of fields to add, while it is open. A press outside it, or focus
 *   leaving it, closes it.
 * @csspart add-search - The search box over that list.
 * @csspart add-option - One field in that list, with `data-column` or `data-layer`, `aria-checked`
 *   while its card is shown, and `aria-disabled` if it is pinned and cannot be taken off.
 * @csspart field-card-<part> - A part of a card.
 * @csspart filter-<part> - A part of a card's `<mosaica-filter>` search box.
 * @csspart cluster-filter-<part> - A part of a card's `<mosaica-cluster-filter>` search box.
 */
export class MosaicaFilterPanel extends MosaicaElement {
  /** Shows only the chips, and nothing while no filter or highlight is in force. */
  @property({type: Boolean, attribute: 'chips-only'}) accessor chipsOnly = false;
  /** Shows the cards and Add field without the subject row and the chips. */
  @property({type: Boolean, attribute: 'controls-only'}) accessor controlsOnly = false;
  /**
   * The columns whose cards always show, even when they hold no filter, separated by spaces or
   * commas. Write a layer as `cluster:<layer>`. Unset, a card shows only once its field is filtered
   * or the viewer adds it.
   */
  @property() accessor pinned = '';
  /** Keeps one card open at a time, and shortens the subject row's figures. */
  @property({type: Boolean, reflect: true}) accessor compact = false;
  /** The level that cards for layers with levels count at. `null` is the coarsest level. */
  @property({type: Number, attribute: 'cluster-level'}) accessor clusterLevel: number | null = null;

  /** The fields the user added, which stay listed. @internal */
  @state() accessor opened: ReadonlySet<string> = new Set();
  /** The cards the user folded, outside the compact layout. @internal */
  @state() accessor folded: ReadonlySet<string> = new Set();
  /** The one card open in the compact layout. @internal */
  @state() accessor openCard: string | null = null;
  /** Whether the Add field list is open, and its search. @internal */
  @state() accessor adding = false;
  /** @internal */
  @state() accessor addSearch = '';
  /** The Add field option in the tab order, the one the arrow keys reached last. @internal */
  @state() accessor addActive = 0;

  static override styles = [
    tokens,
    chrome,
    css`
      :host {
        display: block;
      }
      [part='subject'] {
        display: flex;
        align-items: center;
        flex-wrap: wrap;
        gap: 4px 14px;
        padding: 8px var(--_mosaica-panel-inline, 14px);
        background: color-mix(in srgb, var(--_mosaica-surface-2) 40%, var(--_mosaica-surface));
        border-top: 1px solid var(--_mosaica-line-2);
        border-bottom: 1px solid var(--_mosaica-line-2);
        font-size: 12px;
        color: var(--_mosaica-ink-2);
        font-variant-numeric: tabular-nums;
      }
      :host([compact]) [part='subject'] {
        gap: 4px 12px;
        padding: 6px var(--_mosaica-panel-inline, 12px);
      }
      .key {
        display: flex;
        align-items: center;
        gap: 6px;
        white-space: nowrap;
      }
      :host([compact]) .key {
        gap: 5px;
      }
      .swatch {
        flex: none;
        width: 10px;
        height: 6px;
        border-radius: 1px;
        background: var(--_mosaica-bar);
        forced-color-adjust: none;
      }
      .swatch.lit {
        background: var(--_mosaica-bar-highlight);
      }
      .swatch.pale {
        background: var(--_mosaica-bar-match);
      }
      [part='subject-name'] {
        font-weight: 500;
        color: var(--_mosaica-ink);
      }
      [part='subject-name'].lit {
        color: var(--_mosaica-highlight);
      }
      [part='subject-count'] {
        font-weight: 600;
        color: var(--_mosaica-ink);
      }
      /* A 24 px target the row does not grow for. */
      [part='clear-highlight'] {
        width: 24px;
        height: 24px;
        margin: -4px -2px;
        display: grid;
        place-items: center;
        border-radius: 4px;
        color: var(--_mosaica-ink-2);
      }
      [part='clear-highlight']:hover {
        background: var(--_mosaica-surface-3);
      }
      [part='chips'] {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: 6px;
        padding: 8px var(--_mosaica-panel-inline, 14px);
        border-bottom: 1px solid var(--_mosaica-line-2);
      }
      [part='chip'] {
        min-height: 0;
        padding: 2px 4px 2px 8px;
        gap: 4px;
      }
      /* A region held as the viewer leaves its view arrives here from the map's corner. */
      [part='chip'][data-view] {
        animation: arrive 1.2s ease-out 1;
      }
      @keyframes arrive {
        from {
          box-shadow: 0 0 0 3px var(--_mosaica-accent);
        }
        to {
          box-shadow: 0 0 0 0 transparent;
        }
      }
      @media (prefers-reduced-motion: reduce) {
        [part='chip'][data-view] {
          animation: none;
        }
      }
      .chip .edit {
        display: inline-flex;
        flex: 0 1 auto;
        align-items: center;
        color: inherit;
        font: inherit;
        text-align: left;
      }
      [part='clear'] {
        margin-left: auto;
        padding: 0;
        font-size: 12px;
        color: var(--_mosaica-ink-2);
      }
      .adder {
        position: relative;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 8px;
        padding: 10px var(--_mosaica-panel-inline, 14px) 12px;
      }
      :host([compact]) .adder {
        padding: 8px var(--_mosaica-panel-inline, 12px);
      }
      [part='add'] {
        height: auto;
        padding: 5px 10px;
      }
      :host([compact]) [part='add'] {
        padding: 4px 9px;
      }
      [part='add-note'] {
        font-size: 12px;
        color: var(--_mosaica-ink-3);
      }
      /* The list opens over what sits below it, in the top layer. */
      [part='add-list'] {
        position: fixed;
        inset: auto;
        margin: 0;
        box-sizing: border-box;
        overflow-y: auto;
        background: var(--_mosaica-surface);
        color: var(--_mosaica-ink);
        font-size: 13px;
        padding: 4px;
        border: 1px solid var(--_mosaica-line);
        border-radius: var(--_mosaica-radius-control);
        box-shadow: 0 6px 18px rgba(0, 0, 0, 0.08);
      }
      [part='add-list'] .input {
        margin: 2px 2px 6px;
      }
      [part~='add-option'] {
        display: flex;
        justify-content: space-between;
        gap: 8px;
        width: 100%;
        padding: 6px 8px;
        border-radius: 4px;
        text-align: left;
      }
      [part~='add-option']:hover,
      [part~='add-option']:focus-visible {
        background: var(--_mosaica-surface-2);
      }
      [part~='add-option']:focus-visible {
        outline-offset: -2px;
      }
      [part~='add-option'][aria-disabled='true'] {
        cursor: default;
      }
      [part~='add-option'] .kind {
        font-size: 12px;
        font-weight: 400;
        color: var(--_mosaica-ink-3);
      }
      .none {
        display: block;
        padding: 6px 8px;
        font-size: 12px;
        color: var(--_mosaica-ink-3);
      }
      .emptied {
        padding: 2px 4px;
        border-radius: 4px;
        font-size: 12px;
        color: var(--_mosaica-ink-2);
      }
      @media (forced-colors: active) {
        .swatch {
          background: CanvasText;
        }
        .swatch.lit {
          background: Highlight;
        }
        .swatch.pale {
          background: GrayText;
        }
      }
    `
  ];

  /** Every item the filters admit, which All matching counts. */
  private readonly matching = new HeldAggregate('fields-match');
  /** The card to open, scroll to and focus once the cards are drawn. */
  private editing: string | null = null;
  /** A pressed control that its own action removes, and the control that takes focus once it has gone. */
  private leaving: {gone: HTMLElement; to: HTMLElement | null} | null = null;
  /** Whether the last chip was removed from a panel with nothing else to focus, which then says so. @internal */
  @state() accessor emptied = false;
  /** The keys of the fields shown, in the order they were first shown. */
  private listOrder: string[] = [];
  private readonly floating = new FloatingList(() => {
    const list = this.renderRoot.querySelector<HTMLElement>('[part="add-list"]');
    const anchor = this.renderRoot.querySelector<HTMLElement>('.adder');
    return list && anchor ? {list, anchor} : null;
  });

  /** What the column last drew from, so a publish that changes none of it draws nothing. */
  private drawnFrom: readonly unknown[] = [];

  /**
   * Draw again only when what the column reads changed: the clauses, the counts, the colouring, the
   * view and the clusters served, whose names a chip may carry. The store publishes as each frame
   * arrives, which changes none of these.
   */
  protected override onStoreChange(): void {
    const s = this.resolvedStore;
    const view = s?.get('view');
    const now = s && view ? [s.get('filters'), s.get('aggregates'), s.get('legend').colourBy, s.get('meta'), s.get('status').status, view.id, view.inView?.matched.value, view.inView?.highlighted.value, s.get('artifacts').served] : [];
    if (now.length === this.drawnFrom.length && now.every((v, i) => v === this.drawnFrom[i])) return;
    this.drawnFrom = now;
    super.onStoreChange();
  }

  protected override onStoreAdopted(): void {
    this.drawnFrom = [];
  }

  override disconnectedCallback(): void {
    this.floating.stop();
    this.matching.set(null, null);
    document.removeEventListener('pointerdown', this.onPressOutsideAdd, true);
    super.disconnectedCallback();
  }

  /** A press outside the open Add field list closes it, as Escape does. */
  private onPressOutsideAdd = (e: PointerEvent): void => {
    const adder = this.renderRoot.querySelector('.adder');
    if (!adder || !e.composedPath().includes(adder)) this.adding = false;
  };

  /**
   * Before an action that removes the pressed control `gone`, note where focus goes once it has.
   * From a chip's ×, to the × of the chip after it, else of the chip before, as each stands then.
   * From there, and from Clear all and the clear-highlight ×, to Add field: a × that took focus
   * unasked would remove a filter on the next press. Under `chips-only`, which has no Add field, to
   * the words that say no filter is left.
   */
  private leave(gone: HTMLElement): void {
    const closes = Array.from(this.renderRoot.querySelectorAll<HTMLElement>('[part="chip"] > button:last-child'));
    const at = closes.indexOf(gone);
    this.leaving = {gone, to: at < 0 ? null : (closes[at + 1] ?? closes[at - 1] ?? null)};
  }

  /** Move focus as {@link leave} noted, once the pressed control has gone and taken focus with it. */
  private settleFocus(): void {
    const leaving = this.leaving;
    if (!leaving || leaving.gone.isConnected) return;
    this.leaving = null;
    if (document.activeElement && document.activeElement !== document.body) return;
    const to = (leaving.to?.isConnected ? leaving.to : null) ?? this.renderRoot.querySelector<HTMLElement>('[part="add"]');
    if (to) return to.focus();
    this.emptied = true;
    void this.updateComplete.then(() => this.renderRoot.querySelector<HTMLElement>('.emptied')?.focus());
  }

  /** Show `field`'s card, open it, and scroll to and focus it. Call it after `mosaica-chipopen` to show the chip's card. */
  show(field: string): void {
    this.opened = new Set([...this.opened, field]);
    this.unfold(field);
    this.editing = field;
    this.requestUpdate();
  }

  /** Moves focus to the first card's search box or plot, else Add field. */
  override focus(options?: FocusOptions): void {
    const card = this.renderRoot.querySelector<HTMLElement>('[part~="card"]');
    const target = card?.shadowRoot?.querySelector<HTMLElement>('mosaica-filter, mosaica-cluster-filter, [part="plot"], [part="fold"]') ?? this.renderRoot.querySelector<HTMLElement>('[part="add"]');
    if (target) target.focus(options);
    else super.focus(options);
  }

  private unfold(field: string): void {
    if (this.compact) this.openCard = field;
    else if (this.folded.has(field)) {
      const next = new Set(this.folded);
      next.delete(field);
      this.folded = next;
    }
  }

  /** A card's own fold button: in the compact layout, opening one folds the one open before. */
  private onToggle(field: string, card: MosaicaFieldCard): void {
    if (this.compact) {
      this.openCard = card.folded ? null : field;
      return;
    }
    const next = new Set(this.folded);
    if (card.folded) next.add(field);
    else next.delete(field);
    this.folded = next;
  }

  /** A card the user has changed stays listed, even as its clause empties under them. */
  private keepListed(field: string): void {
    if (!this.opened.has(field)) this.opened = new Set([...this.opened, field]);
  }

  private edit(column: string, verb: ClauseVerb): void {
    if (this.chipsOnly) emit(this, 'mosaica-chipopen', {column, verb});
    else this.show(column);
  }

  protected override updated(changed: PropertyValues<this>): void {
    super.updated(changed);
    this.floating.update();
    this.settleFocus();
    if (this.emptied && this.renderRoot.querySelector('[part="chip"]')) this.emptied = false;
    if (changed.has('adding')) {
      if (this.adding) document.addEventListener('pointerdown', this.onPressOutsideAdd, true);
      else document.removeEventListener('pointerdown', this.onPressOutsideAdd, true);
    }
    const s = this.resolvedStore;
    this.matching.set(s, this.isConnected && !this.chipsOnly && !this.controlsOnly && s?.get('meta') ? {groupings: [{count: true}]} : null);
    if (!this.editing || this.chipsOnly) return;
    const card = Array.from(this.renderRoot.querySelectorAll<MosaicaFieldCard>('[part~="card"]')).find((c) => c.field === this.editing);
    if (!card) return;
    this.editing = null;
    void card.updateComplete.then(() => {
      card.scrollIntoView({block: 'nearest'});
      (card.shadowRoot?.querySelector<HTMLElement>('mosaica-filter, mosaica-cluster-filter, [part="plot"]') ?? card.shadowRoot?.querySelector<HTMLElement>('[part="fold"]'))?.focus();
    });
  }

  private chipText(column: string, draft: ColumnDraft): string {
    const caption = columnCaption(column);
    switch (draft.family) {
      case 'text':
        return `${caption}: ${draft.expr !== undefined ? JSON.stringify(draft.expr) : draft.query.trim().replace(/"([^"]*)"?/g, '“$1”')}`;
      case 'keyword':
        return `${caption} ${OPERATOR_WORDS[draft.op]} ${draft.needle}`;
      case 'category':
        return `${caption}: ${draft.keys.map((k) => keyTitle(this.resolvedStore, column, k)).join(', ')}`;
      case 'numeric': {
        const type = this.resolvedStore?.get('meta')?.declaredScalars.find((c) => c.name === column)?.arrowType;
        if (type === 'timestamp_us') return `${caption} ${dateRangeText(draft.gte, draft.lte)}`;
        const kept = type === 'bool' ? boolKeys(draft) : [];
        if (kept.length > 0) return `${caption}: ${kept.map((k) => BOOL_WORDS[k]).join(', ')}`;
        const f = (v: number) => v.toLocaleString('en-GB');
        if (draft.lte === null) return `${caption} ≥ ${f(draft.gte!)}`;
        if (draft.gte === null) return `${caption} ≤ ${f(draft.lte)}`;
        return `${caption} ${f(draft.gte)} – ${f(draft.lte)}`;
      }
    }
  }

  /**
   * A `member_of` chip's text: the clause's own label (the only name a filter layer's artifact has,
   * since it is not served), else the served name, else unnamed.
   */
  private memberText(clause: MemberClause): string {
    const artifacts = this.resolvedStore?.get('artifacts');
    const served = artifacts?.served.find((a) => a.mosaicaId === clause.artifact && a.layer === clause.layer);
    const name = clause.label ?? (served ? artifactName(served, artifacts!.attached) : null) ?? UNNAMED;
    return clause.outside ? `Outside ${name}` : name;
  }

  /** Switch to the view a held region is in, where the store selects it again. */
  private showRegion(meta: Meta, view: string): void {
    const s = this.resolvedStore;
    if (s) switchView(this, s, meta, view);
  }

  private clearRegion(view: string): void {
    const s = this.resolvedStore;
    if (!s) return;
    s.setRegions(s.get('filters').regions.filter((r) => r.view !== view));
    emit(this, 'mosaica-selectchange', {shape: null, view, status: 'cleared'});
  }

  private clearMember(clause: MemberClause): void {
    const s = this.resolvedStore;
    if (!s) return;
    s.setMembers(withoutMember(s.get('filters').members, clause.layer, clause.artifact, clause.verb));
  }

  private clearColumn(column: string, verb: ClauseVerb): void {
    const s = this.resolvedStore;
    if (!s) return;
    s.setFilters(withoutClause(s.get('filters').draft, column, verb));
    emit(this, 'mosaica-filterchange', {column, verb, expr: null});
  }

  private clearAll(): void {
    const s = this.resolvedStore;
    const meta = s?.get('meta');
    if (!s || !meta) return;
    const held = s.get('filters').regions;
    s.setFilters(emptyDraft(meta.filterOperands));
    s.setMembers([]);
    s.setRegions([]);
    emit(this, 'mosaica-filterchange', {column: null, expr: null});
    for (const {view} of held) emit(this, 'mosaica-selectchange', {shape: null, view, status: 'cleared'});
  }

  /** Empty every clause in the highlight position, so the cards count what is in view again. */
  private clearHighlight(): void {
    const s = this.resolvedStore;
    const meta = s?.get('meta');
    if (!s || !meta) return;
    const {draft, members} = s.get('filters');
    s.setFilters({...draft, highlight: emptyDraft(meta.filterOperands).highlight});
    if (members.some((m) => m.verb === 'highlight')) s.setMembers(members.filter((m) => m.verb !== 'highlight'));
    emit(this, 'mosaica-filterchange', {column: null, verb: 'highlight', expr: null});
  }

  private add(field: string): void {
    this.adding = false;
    this.addSearch = '';
    this.show(field);
  }

  /** Take a field off the column, emptying its clauses in both positions. */
  private takeOff(field: string): void {
    this.adding = false;
    this.addSearch = '';
    const opened = new Set(this.opened);
    opened.delete(field);
    this.opened = opened;
    const s = this.resolvedStore;
    if (s && field.startsWith(CLUSTER_PREFIX)) {
      const layer = field.slice(CLUSTER_PREFIX.length);
      const members = s.get('filters').members;
      if (members.some((m) => m.layer === layer)) s.setMembers(members.filter((m) => m.layer !== layer));
    } else if (s) {
      const draft = s.get('filters').draft;
      const held = VERBS.filter((verb) => {
        const d = draft[verb][field];
        return d !== undefined && isPopulated(d);
      });
      if (held.length > 0) {
        s.setFilters(held.reduce((d, verb) => withoutClause(d, field, verb), draft));
        for (const verb of held) emit(this, 'mosaica-filterchange', {column: field, verb, expr: null});
      }
    }
    void this.updateComplete.then(() => this.renderRoot.querySelector<HTMLElement>('[part="add"]')?.focus());
  }

  /** The arrow keys, Home and End move among the Add field options; Up from the first goes to the search box. */
  private addKeys(e: KeyboardEvent, i: number | null): void {
    const items = Array.from(this.renderRoot.querySelectorAll<HTMLElement>('[part~="add-option"]'));
    if (items.length === 0) return;
    const last = items.length - 1;
    const next = i === null ? {ArrowDown: 0}[e.key] : {ArrowDown: Math.min(i + 1, last), ArrowUp: i - 1, Home: 0, End: last}[e.key];
    if (next === undefined) return;
    e.preventDefault();
    if (next < 0) {
      this.renderRoot.querySelector<HTMLElement>('[part="add-search"]')?.focus();
      return;
    }
    this.addActive = next;
    items[next]?.focus();
  }

  override render(): TemplateResult | typeof nothing {
    const s = this.resolvedStore;
    const meta = s?.get('meta') ?? null;
    if (!s || !meta) return html`<div class="panel">${renderState(stateOf(s?.get('status')), s?.get('status'))}</div>`;
    const operands = meta.filterOperands;
    const layers = clusterFieldLayers(meta, s.get('view').id);
    if (operands.length === 0 && layers.length === 0) return html`<div class="panel"><span part="state" data-state="empty">Nothing to filter</span></div>`;
    const {draft, members, regions} = s.get('filters');
    const holds = (column: string, verb: ClauseVerb) => {
      const d = draft[verb][column];
      return d !== undefined && isPopulated(d);
    };
    const chips = this.chipLine(meta, draft, members, regions, holds);
    if (this.chipsOnly) return chips;

    const fields = [
      ...operands.map((o) => ({key: o.column, title: columnCaption(o.column), layer: null as Layer | null})),
      ...layers.map((l) => ({key: layerField(l.name), title: l.title || l.name, layer: l as Layer | null}))
    ];
    const pinned = new Set(this.pinned.split(/[\s,]+/).filter(Boolean));
    const holdsField = (f: (typeof fields)[number], verb: ClauseVerb) => (f.layer ? members.some((m) => m.layer === f.layer!.name && m.verb === verb) : holds(f.key, verb));
    // The field the map is coloured by has a card, which says what its colours mean.
    const colourBy = s.get('legend').colourBy;
    const shown = fields.filter((f) => pinned.has(f.key) || this.opened.has(f.key) || f.key === colourBy || holdsField(f, 'filter') || holdsField(f, 'highlight'));
    // A field joins the end of the list, so the cards already shown do not move.
    const keys = new Set(shown.map((f) => f.key));
    this.listOrder = [...this.listOrder.filter((k) => keys.has(k)), ...shown.map((f) => f.key).filter((k) => !this.listOrder.includes(k))];
    const listed = this.listOrder.map((k) => shown.find((f) => f.key === k)!);
    const open = this.compact ? (listed.some((f) => f.key === this.openCard) ? this.openCard : null) : null;
    const card = (f: (typeof fields)[number]) =>
      html`<mosaica-field-card part="card" exportparts=${CARD_PARTS} data-field=${f.key} field=${f.key} .store=${s} .level=${this.clusterLevel}
        ?compact=${this.compact} .folded=${this.compact ? f.key !== open : this.folded.has(f.key)}
        @mosaica-fold=${(e: Event) => this.onToggle(f.key, e.currentTarget as MosaicaFieldCard)}
        @mosaica-filterchange=${() => this.keepListed(f.key)} @mosaica-clausechange=${() => this.keepListed(f.key)}></mosaica-field-card>`;
    const head = this.controlsOnly ? nothing : html`${this.subjectRow(s)}${chips}`;
    return html`${head}<span part="state" data-state="shown"></span>${repeat(listed, (f) => f.key, card)}${this.adder(fields, listed, pinned)}`;
  }

  /** In view or Highlighted and its number, then All matching and its number. */
  private subjectRow(s: Store): TemplateResult {
    const {draft, members} = s.get('filters');
    const lit = activeCount(draft, 'highlight') > 0 || members.some((m) => m.verb === 'highlight');
    const inView = s.get('view').inView;
    const n = inView ? (lit ? inView.highlighted.value : inView.matched.value) : null;
    const total = this.matching.entry()?.result?.tables[0]?.total ?? null;
    const skel = html`<span class="skel" aria-hidden="true"></span>`;
    return html`<div part="subject">
      <span class="key"><span part="subject-key" class=${`swatch${lit ? ' lit' : ''}`}></span><span part="subject-name" class=${lit ? 'lit' : ''}>${lit ? 'Highlighted' : 'In view'}</span><span part="subject-count">${n === null ? skel : figure(n, this.compact)}</span>${lit
        ? html`<button part="clear-highlight" type="button" aria-label="Clear highlight" title="Clear highlight" @click=${(e: Event) => {
            this.leave(e.currentTarget as HTMLElement);
            this.clearHighlight();
          }}>${icon('close', 11, 2.4)}</button>`
        : nothing}</span>
      <span part="all" class="key"><span class="swatch pale"></span><span>${this.compact ? 'All' : 'All matching'}</span><span part="all-count">${total === null ? skel : figure(total, this.compact)}</span></span>
    </div>`;
  }

  /** Every clause as a chip, then Clear all; nothing while none is applied. */
  private chipLine(
    meta: Meta,
    draft: FilterDraft,
    members: readonly MemberClause[],
    regions: readonly HeldRegion[],
    holds: (column: string, verb: ClauseVerb) => boolean
  ): TemplateResult | typeof nothing {
    const columns = [...new Set([...Object.keys(draft.filter), ...Object.keys(draft.highlight)])];
    const chips = VERBS.flatMap((verb) => columns.flatMap((c) => (holds(c, verb) ? [{c, verb, d: draft[verb][c]!}] : [])));
    if (chips.length === 0 && members.length === 0 && regions.length === 0) {
      return this.emptied && this.chipsOnly ? html`<div part="chips"><span class="emptied" tabindex="-1" @blur=${() => (this.emptied = false)}>No filters</span></div>` : nothing;
    }
    // The word on a highlight's chip; a filter's has none.
    const word = (verb: ClauseVerb) => (verb === 'highlight' ? html`<span class="verb">highlight</span> ` : nothing);
    const pressed = (e: Event) => this.leave(e.currentTarget as HTMLElement);
    return html`<div part="chips">
      ${repeat(
        chips,
        ({c, verb}) => `${verb} ${c}`,
        ({c, verb, d}) => html`<span part="chip" class="chip" data-verb=${verb} data-column=${c}
          >${word(verb)}<button part="edit" class="edit" type="button" title="Show its card" @click=${() => this.edit(c, verb)}>${this.chipText(c, d)}</button
          ><button type="button" aria-label=${`Remove the ${columnCaption(c)} ${verb}`} @click=${(e: Event) => {
            pressed(e);
            this.clearColumn(c, verb);
          }}>${icon('close', 10, 2.4)}</button></span
        >`
      )}
      ${repeat(
        [...members].sort((a, b) => VERBS.indexOf(a.verb) - VERBS.indexOf(b.verb)),
        (m) => `${m.verb} ${m.layer} ${m.artifact}`,
        (m) => html`<span part="chip" class="chip" data-verb=${m.verb} data-artifact=${String(m.artifact)}
          >${word(m.verb)}${this.memberText(m)}<button type="button" aria-label=${`Remove the ${this.memberText(m)} ${m.verb}`} @click=${(e: Event) => {
            pressed(e);
            this.clearMember(m);
          }}>${icon('close', 10, 2.4)}</button></span
        >`
      )}
      ${repeat(
        regions,
        (r) => r.view,
        (r) => {
          const view = viewTitle(meta, r.view);
          const text = regionText(r.shape, view);
          return html`<span part="chip" class="chip" data-verb="filter" data-view=${r.view}
            ><button part="edit" class="edit" type="button" title=${`Show in ${view}`} @click=${() => this.showRegion(meta, r.view)}
              >${icon(r.shape.kind === 'lasso' ? 'lasso' : 'box', 12)}${text}</button
            ><button type="button" aria-label=${`Remove the ${text} filter`} @click=${(e: Event) => {
              pressed(e);
              this.clearRegion(r.view);
            }}>${icon('close', 10, 2.4)}</button></span
          >`;
        }
      )}
      <button part="clear" type="button" @click=${(e: Event) => {
        pressed(e);
        this.clearAll();
      }}>Clear all</button>
    </div>`;
  }

  /** Add field, its list of every field, and how many are not listed. */
  private adder(fields: {key: string; title: string; layer: Layer | null}[], listed: {key: string}[], pinned: Set<string>): TemplateResult {
    const q = this.addSearch.trim().toLowerCase();
    const offered = fields.filter((f) => q === '' || f.key.toLowerCase().includes(q) || f.title.toLowerCase().includes(q));
    const tabbed = this.addActive < offered.length ? this.addActive : 0;
    const isListed = (key: string) => listed.some((f) => f.key === key);
    const choose = (key: string) => {
      if (!isListed(key)) this.add(key);
      else if (!pinned.has(key)) this.takeOff(key);
    };
    const rest = fields.length - listed.length;
    return html`<div class="adder" @focusout=${(e: FocusEvent) => {
        // Focus that goes to nothing, as on a press on the list's own padding or scrollbar, has not left.
        const to = e.relatedTarget as Node | null;
        if (to && !(e.currentTarget as HTMLElement).contains(to)) this.adding = false;
      }}>
      <button part="add" class="btn" type="button" aria-expanded=${this.adding ? 'true' : 'false'} aria-controls="add-list"
        @click=${() => {
          this.adding = !this.adding;
          this.addSearch = '';
          this.addActive = 0;
        }}>${icon('plus', 12, 1.6)}Add field</button>
      ${rest > 0 && !this.compact ? html`<span part="add-note">${rest} more ${rest === 1 ? 'field' : 'fields'}</span>` : nothing}
      ${this.adding
        ? html`<div part="add-list" id="add-list" popover="manual" @keydown=${(e: KeyboardEvent) => {
            if (e.key !== 'Escape') return;
            e.stopPropagation();
            this.adding = false;
            this.renderRoot.querySelector<HTMLElement>('[part="add"]')?.focus();
          }}>
            <div class="input">${icon('search', 14)}<input part="add-search" type="search" autocomplete="off" placeholder="Find a field" aria-label="Find a field" .value=${this.addSearch}
              @input=${(e: Event) => {
                this.addSearch = (e.target as HTMLInputElement).value;
                this.addActive = 0;
              }}
              @keydown=${(e: KeyboardEvent) => {
                // Enter adds the first match not yet listed. Taking a field off is always a press on its row.
                if (e.key !== 'Enter') return this.addKeys(e, null);
                e.preventDefault();
                const unlisted = offered.find((o) => !isListed(o.key));
                if (unlisted) this.add(unlisted.key);
              }} /></div>
            <div role="status">${offered.length > 0
              ? html`<span class="sr-only">${offered.length} ${offered.length === 1 ? 'field' : 'fields'} listed</span>`
              : html`<span class="none">No field by that name</span>`}</div>
            ${offered.length > 0
              ? html`<div role="menu" aria-label="Fields">${offered.map((o, i) => {
                  const added = isListed(o.key);
                  const kept = added && pinned.has(o.key);
                  return html`<button part="add-option" type="button" role="menuitemcheckbox" data-column=${o.layer ? nothing : o.key} data-layer=${o.layer ? o.layer.name : nothing}
                    aria-checked=${added ? 'true' : 'false'}
                    aria-disabled=${kept ? 'true' : nothing} title=${kept ? 'Always shown here' : added ? 'Remove from the column' : nothing} tabindex=${i === tabbed ? '0' : '-1'}
                    @click=${() => choose(o.key)} @keydown=${(e: KeyboardEvent) => this.addKeys(e, i)}><span>${o.title}</span>${added ? html`<span class="kind">Shown</span>` : o.layer ? html`<span class="kind">Clusters</span>` : nothing}</button>`;
                })}</div>`
              : nothing}
          </div>`
        : nothing}
    </div>`;
  }

  protected override willUpdate(changed: PropertyValues<this>): void {
    super.willUpdate(changed);
    // The search box takes focus as the list opens.
    if (changed.has('adding') && this.adding) void this.updateComplete.then(() => this.renderRoot.querySelector<HTMLElement>('[part="add-search"]')?.focus());
  }
}

attachContextRoot();
defineOnce('mosaica-filter-panel', MosaicaFilterPanel);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-filter-panel': MosaicaFilterPanel;
  }
}
