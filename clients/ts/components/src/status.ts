import {css, html, nothing, type PropertyValues, type TemplateResult} from 'lit';
import {property} from 'lit/decorators.js';
import {NO_COUNT, NO_MASKED, formatMasked, type HeldRegion, type MemberClause, type StatusProjection, type ViewProjection} from '@mosaicajs/client';
import {MosaicaElement, emit, idString, shapeDetail} from './base.js';
import {attachContextRoot, defineOnce} from './define.js';
import {ActionFocus, EmptyAnswer, NOTHING_VISIBLE, NamedQuery, UNSAID, liveLine, seesNothing, showsContent, stateLine, stateOf, stateWord, type PanelState, type StateLine} from './states.js';
import {chrome, tokens} from './tokens.js';
import './count.js';

/**
 * Use the status line with a map so that viewers know whether what they see is current and how much
 * of the data it covers. A typical line reads `● | 16,822,190 of 21,406,522 match | 5,390 shown`,
 * where the first figure is the items the filters match out of those the viewer may see. The
 * figures count the camera's box, or the selected region while there is one. While a highlight is
 * set, the counts cell reads `N highlighted of M`.
 *
 * While the view is up to date, the first cell is a dot alone. When something goes wrong, the cell
 * names the state and offers a button if the viewer can act, such as Retry when the server is busy.
 * Sign in appears when the session expires only if you have set `reauthorise`. If the viewer may see
 * no item in the view at all, the cell says so.
 *
 * Where the strip is short of room, its cells wrap onto further lines.
 *
 * After a change of filters, highlight or selection, the cell reads Running query once the answer
 * has taken more than a quarter of a second, until the map, the layers and the counts have all
 * loaded. Its Cancel button goes back to the filters, highlight and selection that last finished
 * loading.
 *
 * Screen readers announce the states the cell names in words, because the words are a live region.
 * A pan announces nothing: up to date and updating are read but not announced, the counts sit
 * outside the region, and Nothing in view is announced only as the answer to a change of filters,
 * highlight or selection, not where a pan went. After such a change the region announces the counts
 * once they have loaded, such as `3,923 of 50,000 match`, or that the view is up to date where there
 * are no counts to say. After any state it announced, it announces that the view is up to date
 * again. A state's button sits outside the region. When the button goes with its state while it has
 * the focus, the state cell takes the focus. With `quiet`, the strip shows the same and announces
 * nothing.
 *
 * @summary A status line showing whether the map is up to date and how many items it shows.
 * @tagname mosaica-status
 * @category Elements
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-statechange']>} mosaica-statechange - The panel
 *   state changed, including on the first render.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-filterchange']>} mosaica-filterchange - Cancel
 *   went back to other filters or another highlight, once for each that changed, with `column` null.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-clausechange']>} mosaica-clausechange - Cancel
 *   put back or took away a `member_of` clause, once for each.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-selectchange']>} mosaica-selectchange - Cancel
 *   went back to another selected region, or to none, and once with `view` for each other view
 *   whose held region it put back or took away.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-expired']>} mosaica-expired - The session expired.
 *   Fires once each time.
 * @csspart strip - The strip of cells.
 * @csspart state - The state's dot and words, with `data-state` set to the panel state.
 * @csspart refusal - The words "View refused", or "Server busy", with `data-code` set to the
 *   refusal's code.
 * @csspart reason - The server's reason for a refusal, under "View refused".
 * @csspart refresh - The Refresh button, shown when the data has changed.
 * @csspart retry - The Retry button, after a refusal, no answer or a busy server.
 * @csspart cancel - The Cancel button, while a query runs.
 * @csspart spinner - The spinner beside the words Running query. The state part carries
 *   `data-querying` while it shows.
 * @csspart reauthorise - The Sign in button, shown on expiry if `reauthorise` is set.
 * @csspart count-shown - The `<mosaica-count>` of marks shown.
 * @csspart count-matched - The `<mosaica-count>` of items the filters match.
 * @csspart count-highlighted - The `<mosaica-count>` of highlighted items, while a highlight is set.
 * @csspart count-of - The matched count the highlighted count is out of, while a highlight is set.
 * @csspart count-visible - The `<mosaica-count>` of items the viewer may see here, which the matched
 *   count is out of.
 * @csspart card - The card of figures, when `expanded` is set.
 */
export class MosaicaStatus extends MosaicaElement {
  static override styles = [
    tokens,
    chrome,
    css`
      :host {
        display: inline-block;
        max-width: 100%;
        vertical-align: top;
      }
      /* The cells wrap onto further lines where the strip is short of room, and a cell wider than
         the strip wraps its own words. */
      [part='strip'] {
        display: inline-flex;
        flex-wrap: wrap;
        align-items: stretch;
        max-width: 100%;
        min-height: 32px;
        overflow: hidden;
        background: var(--_mosaica-surface);
        border: 1px solid var(--_mosaica-line);
        border-radius: var(--_mosaica-radius);
        box-shadow: var(--_mosaica-shadow);
        font-size: 12px;
        color: var(--_mosaica-ink-2);
        font-variant-numeric: tabular-nums;
      }
      [part='strip'][data-state='detached'] {
        display: none;
      }
      /* A rule on each cell's leading edge, drawn one pixel outside the cell, so the strip's own
         edge covers it where the cell starts a line. */
      .cell {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: 0 4px;
        min-height: 30px;
        padding: 7px 12px;
        flex: 0 1 auto;
        min-width: 0;
        margin-left: -1px;
        border-left: 1px solid var(--_mosaica-line-2);
      }
      .cell mosaica-count {
        font-size: inherit;
      }
      .cell.dim {
        opacity: 0.4;
      }
      .cell.state {
        padding: 0 5px 0 12px;
      }
      .cell.state.bare {
        padding: 0 12px;
      }
      .cell.state:focus-visible {
        outline-offset: -2px;
      }
      [part='state'] {
        gap: 7px;
        min-width: 0;
        color: inherit;
      }
      [part='state'] .skel {
        display: none;
      }
      [part='state'] .btn {
        margin-left: 1px;
      }
      [part='state']:not(:has(.btn)) {
        padding-right: 7px;
      }

      .cell mosaica-count::part(count) {
        font-weight: 600;
      }
      .cell mosaica-count[part~='count-visible']::part(count) {
        color: inherit;
        font-weight: 400;
      }
      .cell mosaica-count[part~='count-shown']::part(count) {
        color: inherit;
        font-weight: 400;
      }
      .cell mosaica-count[part~='count-of']::part(count) {
        color: inherit;
        font-weight: 400;
      }
      .cell mosaica-count.lit::part(count) {
        color: var(--_mosaica-highlight);
      }
      .cell mosaica-count::part(label) {
        margin-left: 0.3em;
      }
      :host([compact]) .cell {
        margin-left: 0;
        border-left: 0;
      }
      :host([compact]) .cell + .cell {
        padding-left: 0;
      }
      :host([compact]) .cell.state {
        padding-right: 7px;
      }
      [part='card'] {
        margin-top: 8px;
        padding: 10px 12px;
        background: var(--_mosaica-surface);
        border: 1px solid var(--_mosaica-line);
        border-radius: var(--_mosaica-radius);
        box-shadow: var(--_mosaica-shadow);
        font-size: 12px;
      }
      [part='card'] dl,
      [part='card'] dd {
        margin: 0;
      }
    `
  ];

  /** Adds a card of the figures below the strip. */
  @property({type: Boolean}) accessor expanded = false;
  /** Shortens the figures to one decimal (`16.8M`) and drops the shown count. */
  @property({type: Boolean, reflect: true}) accessor compact = false;
  /** A function the Sign in button calls when the session has expired. Without it there is no Sign in button. */
  @property({attribute: false}) accessor reauthorise: (() => void) | null = null;
  /**
   * Announces nothing: every state and result is still shown, and none is put in a live region. Set
   * it when another element on the page says the same things.
   */
  @property({type: Boolean, reflect: true}) accessor quiet = false;

  private readonly query = new NamedQuery(this);
  private readonly actionFocus = new ActionFocus();
  private readonly emptyAnswer = new EmptyAnswer();
  private lastState: PanelState | null = null;
  private expiryFired = false;
  /** What the strip shows now, decided before each render: `bare` is a dot alone, and `unsaid` what {@link liveLine} takes. */
  private shown: {state: PanelState; running: boolean; bare: boolean; line: StateLine; unsaid: {heard: unknown} | null} = {state: 'detached', running: false, bare: false, line: stateLine('detached', null), unsaid: null};
  /** What the live region says while the strip shows a dot alone: a query's result, or that the view is back up to date. */
  private heard = '';
  /** The dot-alone state `heard` was last decided under. */
  private bareState: PanelState | null = null;
  private wasQuerying = false;
  /** Whether a query has finished and its result is still to be said. */
  private resultDue = false;
  /** Whether the strip has left a state it announced and has not yet said the view is up to date again. */
  private recoveryDue = false;

  private get status(): StatusProjection | null {
    return this.resolvedStore?.get('status') ?? null;
  }

  private get view(): ViewProjection | null {
    return this.resolvedStore?.get('view') ?? null;
  }

  override disconnectedCallback(): void {
    this.query.stop();
    super.disconnectedCallback();
  }

  protected override willUpdate(changed: PropertyValues<this>): void {
    super.willUpdate(changed);
    this.actionFocus.note(this.renderRoot);
    const status = this.status;
    const state = stateOf(status);
    const querying = status?.querying === true;
    const refresh = () => this.resolvedStore?.refresh();
    const nothingVisible = seesNothing(this.resolvedStore, state);
    // A query that has run long enough to name. The map may have drawn its answer while the
    // counts still load, and a refusal, an expiry or a lost server is said in its own words.
    const running = querying && this.query.named && (state === 'loading' || state === 'shown' || state === 'empty' || state === 'stale');
    // Up to date and updating, the two states a pan moves between, are each a dot alone, so the
    // strip keeps its width through a gesture; every other state names itself beside its dot.
    const bare = !running && !nothingVisible && (state === 'shown' || (state === 'loading' && status?.sessionWarm === true));
    // Nothing in view is announced as a query's answer. Where a pan went, it is shown and not said.
    const answered = this.emptyAnswer.note(state, querying);
    const panned = state === 'empty' && !running && !nothingVisible && !answered;
    this.hear(state, bare, panned);
    const words = stateWord(state, status);
    const line: StateLine = running
      ? {dot: null, words: 'Running query', action: html`<button part="cancel" class="btn small" type="button" @click=${() => this.cancel()}>Cancel</button>`}
      : bare
        ? // Read and not drawn, and left out while the live region is saying the same words.
          {dot: state === 'shown' ? 'dot' : 'dot quiet', title: words, words: this.heard === words && !this.quiet ? nothing : html`<span class="sr-only">${words}</span>`, action: nothing}
        : state === 'empty'
          ? {dot: 'dot quiet', words: nothingVisible ? NOTHING_VISIBLE : words, action: nothing}
          : stateLine(state, status, {onRefresh: refresh, onRetry: refresh, onReauthorise: this.reauthorise});
    this.shown = {state, running, bare, line, unsaid: this.quiet || panned ? UNSAID : bare ? {heard: this.heard} : null};
  }

  /**
   * Decide what the live region says while the strip shows a dot alone. A pan moves between up to
   * date and updating and says nothing. A query that finishes says its result once the counts are
   * in, or that the view is up to date where there are no counts to say, and a return from a
   * state the strip announced says the view is up to date. Either is taken away at the next
   * change, so that it is said again when it next holds.
   */
  private hear(state: PanelState, bare: boolean, panned: boolean): void {
    const querying = this.status?.querying === true;
    if (querying) this.resultDue = false;
    else if (this.wasQuerying) this.resultDue = true;
    this.wasQuerying = querying;
    if (!bare) {
      // The words shown are the answer. Where a pan went is not announced, so leaving it is not.
      this.heard = '';
      this.bareState = null;
      this.resultDue = false;
      if (!panned) this.recoveryDue = state !== 'detached';
      return;
    }
    if (state !== this.bareState || querying) this.heard = '';
    this.bareState = state;
    if (state !== 'shown' || querying) return;
    if (this.resultDue) {
      // Counts still loading are waited for; refused or absent, the state is said in their place.
      if (this.view?.inView?.status === 'loading') return;
      this.heard = this.result() ?? stateWord(state, this.status);
    } else if (this.recoveryDue) this.heard = stateWord(state, this.status);
    this.resultDue = false;
    this.recoveryDue = false;
  }

  /** The counts as the cells show them, in full figures, or `null` where there are none to show. */
  private result(): string | null {
    const v = this.view;
    const n = v?.inView;
    if (!v || !n || n.status !== 'shown') return null;
    const match = `${formatMasked(n.matched)} of ${formatMasked(n.visible)} match`;
    if (!v.highlighting) return match;
    const lit = `${formatMasked(n.highlighted)} highlighted of ${formatMasked(n.matched)}`;
    return this.resolvedStore?.requestFilters() != null ? `${match}, ${lit}` : lit;
  }

  protected override updated(): void {
    this.actionFocus.keep(this.renderRoot.querySelector<HTMLElement>('.cell.state'));
    this.query.track(this.status?.querying ?? false);
    const state = stateOf(this.status);
    if (state !== this.lastState) {
      const from = this.lastState;
      this.lastState = state;
      emit(this, 'mosaica-statechange', {from, to: state, refusal: this.status?.refusal ?? null});
    }
    if (state === 'expired' && !this.expiryFired) {
      this.expiryFired = true;
      emit(this, 'mosaica-expired', {refusal: this.status?.refusal ?? null});
    } else if (state !== 'expired') {
      this.expiryFired = false;
    }
  }

  /** Cancel the query, and tell the page what it went back to, as the controls that set those do. */
  private cancel(): void {
    const s = this.resolvedStore;
    if (!s) return;
    const was = {...s.get('filters'), shape: s.get('region')?.shape ?? null};
    s.cancelQuery();
    const now = s.get('filters');
    const differ = (a: unknown, b: unknown) => JSON.stringify(a) !== JSON.stringify(b);
    if (differ(was.expr, now.expr)) emit(this, 'mosaica-filterchange', {column: null, verb: 'filter', expr: now.expr});
    if (differ(was.highlight, now.highlight)) emit(this, 'mosaica-filterchange', {column: null, verb: 'highlight', expr: now.highlight});
    const keyed = (clauses: readonly MemberClause[]) => new Map(clauses.map((c) => [`${c.verb} ${c.outside} ${c.artifact} ${c.layer}`, c]));
    const [before, after] = [keyed(was.members), keyed(now.members)];
    const told = (c: MemberClause, on: boolean) => emit(this, 'mosaica-clausechange', {id: idString(c.artifact), layer: c.layer, outside: c.outside, verb: c.verb, on});
    for (const [key, clause] of before) if (!after.has(key)) told(clause, false);
    for (const [key, clause] of after) if (!before.has(key)) told(clause, true);
    const shape = s.get('region')?.shape ?? null;
    if (shape !== was.shape) emit(this, 'mosaica-selectchange', {shape: shapeDetail(shape), status: shape ? 'loading' : 'cleared'});
    const heldIn = (regions: readonly HeldRegion[]) => new Map(regions.map((r) => [r.view, r.shape]));
    const [heldBefore, heldAfter] = [heldIn(was.regions), heldIn(now.regions)];
    for (const view of heldBefore.keys()) if (!heldAfter.has(view)) emit(this, 'mosaica-selectchange', {shape: null, view, status: 'cleared'});
    for (const [view, held] of heldAfter) if (heldBefore.get(view) !== held) emit(this, 'mosaica-selectchange', {shape: shapeDetail(held), view, status: 'loading'});
  }

  override render(): TemplateResult | typeof nothing {
    const {state, running, bare, line, unsaid} = this.shown;
    const stale = this.status?.stale ?? false;
    const v = this.view;
    // One template for every state, so the live element stays and each change of its words is
    // announced. The state's button sits outside it, and the cell takes the focus when the
    // button goes.
    const first = html`<div class=${`cell state${bare ? ' bare' : ''}`} tabindex="-1">
      <span part="state" data-state=${state} ?data-querying=${running}>${running ? html`<span part="spinner" class="spinner" aria-hidden="true"></span>` : nothing}${liveLine(line, unsaid)}</span>
    </div>`;
    return html`<div part="strip" data-state=${state}>${first}${this.counts(state, v, stale)}</div>
      ${this.expanded && showsContent(state) && v ? this.card(v, stale) : nothing}`;
  }

  /**
   * The count cells: current in the shown state, greyed out while a new answer is awaited or the
   * last one was refused, and skeletons where there is no answer yet. Nothing where there is no
   * view to count, or where the view is empty.
   */
  private counts(state: PanelState, v: ViewProjection | null, stale: boolean): TemplateResult | typeof nothing {
    if (state === 'detached' || state === 'empty' || !v) return nothing;
    if (state === 'loading' && this.status?.sessionWarm === false) return nothing;
    const n = v.inView;
    const dim = state !== 'shown' || n?.status !== 'shown';
    const cls = `cell${dim ? ' dim' : ''}`;
    if (!n || stale) {
      const skel = html`<span class="skel" aria-hidden="true"></span>`;
      return html`<div class=${cls}>${skel}<span>match</span></div>${this.compact ? nothing : html`<div class=${cls}>${skel}<span>shown</span></div>`}`;
    }
    // Under a highlight with nothing narrowing, the match cell would read "M of M match" beside
    // the highlight's, so the highlight's cell stands alone and names M.
    const narrowed = this.resolvedStore?.requestFilters() != null;
    const match = html`<div class=${cls}>
      <mosaica-count part="count-matched" .masked=${n.matched} .compact=${this.compact}></mosaica-count><span>of</span><mosaica-count
        part="count-visible" .masked=${n.visible} .compact=${this.compact}></mosaica-count><span>match</span>
    </div>`;
    return html`${v.highlighting && !narrowed ? nothing : match}
      ${v.highlighting
        ? html`<div class=${cls}><mosaica-count part="count-highlighted" class="lit" .masked=${n.highlighted} .compact=${this.compact}></mosaica-count><span>highlighted of</span><mosaica-count
              part="count-of" .masked=${n.matched} .compact=${this.compact}></mosaica-count></div>`
        : nothing}
      ${this.compact ? nothing : html`<div class=${cls}><mosaica-count part="count-shown" .count=${{shown: n.shown, total: n.matched.value, exact: true}} figure="shown" label="shown"></mosaica-count></div>`}`;
  }

  private card(v: ViewProjection, stale: boolean) {
    const row = (label: string, value: unknown) => html`<dt class="k">${label}</dt><dd class="v">${value}</dd>`;
    const n = v.inView;
    return html`<div part="card"><dl class="kv">
      ${row('Shown', html`<mosaica-count .count=${n ? {shown: n.shown, total: n.matched.value, exact: true} : NO_COUNT} .stale=${stale}></mosaica-count>`)}
      ${row('Match the filters', html`<mosaica-count .masked=${n?.matched ?? NO_MASKED} .stale=${stale}></mosaica-count>`)}
      ${v.highlighting ? row('Highlighted', html`<mosaica-count .masked=${n?.highlighted ?? NO_MASKED} .stale=${stale}></mosaica-count>`) : nothing}
      ${row('In this view', html`<mosaica-count .masked=${n?.visible ?? NO_MASKED} .stale=${stale}></mosaica-count>`)}
    </dl></div>`;
  }
}

attachContextRoot();
defineOnce('mosaica-status', MosaicaStatus);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-status': MosaicaStatus;
  }
}
