import {css, html, nothing, type TemplateResult} from 'lit';
import {property} from 'lit/decorators.js';
import {NO_COUNT, NO_MASKED, type StatusProjection, type ViewProjection} from '@mosaicajs/client';
import {MosaicaElement, emit} from './base.js';
import {attachContextRoot, defineOnce} from './define.js';
import {NOTHING_VISIBLE, renderState, seesNothing, showsContent, stateOf, type PanelState} from './states.js';
import {chrome, tokens} from './tokens.js';
import './count.js';

/**
 * Use `<mosaica-status>` to show on one line whether the map is up to date and how many items it
 * covers, such as `● | 16,822,190 of 21,406,522 match | 5,390 shown`. The counts cover the
 * camera's box, or the selected region while there is one. While a highlight is set, a cell reads
 * `N highlighted of M`.
 *
 * While the view is up to date, the first cell is a dot alone. Otherwise it names the state in a few
 * words, such as Reconnecting, Server busy, View refused or Session expired, with a button if there
 * is something to do: Refresh when the data has changed, Retry after a refusal, no answer or a
 * busy server, and Sign in when the session expires and `reauthorise` is set. The counts grey out while they
 * are out of date.
 *
 * `compact` shortens the figures and drops the shown count, and `expanded` adds a card of the
 * figures below the strip. The state cell is an `aria-live` region, so screen readers announce a
 * refusal, an expiry or a change of data, but not the counts as the map pans.
 *
 * @summary One line with the view's state and counts.
 * @tagname mosaica-status
 * @category Elements
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-statechange']>} mosaica-statechange - The panel
 *   state changed, including on the first render.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-expired']>} mosaica-expired - The session expired.
 *   Fires once each time.
 * @csspart strip - The one-line strip.
 * @csspart state - The state's dot and words, with `data-state` set to the panel state.
 * @csspart refusal - The words "View refused", or "Server busy", with `data-code` set to the
 *   refusal's code.
 * @csspart reason - The server's reason for a refusal, under "View refused".
 * @csspart refresh - The Refresh button, shown when the data has changed.
 * @csspart retry - The Retry button, after a refusal, no answer or a busy server.
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
      [part='strip'] {
        display: inline-flex;
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
        white-space: nowrap;
      }
      [part='strip'][data-state='detached'] {
        display: none;
      }
      .cell {
        display: flex;
        align-items: center;
        gap: 4px;
        padding: 7px 12px;
        flex: none;
      }
      .cell mosaica-count {
        font-size: inherit;
      }
      .cell + .cell {
        border-left: 1px solid var(--_mosaica-line-2);
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
      [part='state'] {
        gap: 7px;
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
      :host([compact]) .cell + .cell {
        border-left: 0;
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
    `
  ];

  /** Adds a card of the figures below the strip. */
  @property({type: Boolean}) accessor expanded = false;
  /** Shortens the figures to one decimal (`16.8M`) and drops the shown count. */
  @property({type: Boolean, reflect: true}) accessor compact = false;
  /** A function the Sign in button calls when the session has expired. Without it there is no Sign in button. */
  @property({attribute: false}) accessor reauthorise: (() => void) | null = null;

  private lastState: PanelState | null = null;
  private expiryFired = false;

  private get status(): StatusProjection | null {
    return this.resolvedStore?.get('status') ?? null;
  }

  private get view(): ViewProjection | null {
    return this.resolvedStore?.get('view') ?? null;
  }

  protected override updated(): void {
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

  override render(): TemplateResult | typeof nothing {
    const status = this.status;
    const state = stateOf(status);
    const stale = status?.stale ?? false;
    const v = this.view;
    const refresh = () => this.resolvedStore?.refresh();
    // Up to date and updating, the two states a pan moves between, are each a dot alone, so the
    // strip keeps its width through a gesture; every other state names itself beside its dot.
    const bare = (cls: string, words: string) => html`<span part="state" data-state=${state}><span class=${cls} role="img" aria-label=${words} title=${words}></span></span>`;
    // A viewer who can see nothing is told so once, by the map; the strip keeps its dot.
    const nothingVisible = seesNothing(this.resolvedStore, state);
    const dotAlone = state === 'shown' || (state === 'loading' && status?.sessionWarm === true) || nothingVisible;
    const inner =
      state === 'shown'
        ? bare('dot', 'Up to date')
        : nothingVisible
          ? bare('dot quiet', NOTHING_VISIBLE)
          : dotAlone
            ? bare('dot quiet', 'Updating')
            : state === 'empty'
              ? html`<span part="state" data-state="empty"><span class="dot quiet"></span>Nothing in view</span>`
              : renderState(state, status, {onRefresh: refresh, onRetry: refresh, onReauthorise: this.reauthorise});
    // One live element across every state, so each change of state is announced from it.
    const first = html`<div class=${`cell state${dotAlone ? ' bare' : ''}`} role="status" aria-live="polite">${inner}</div>`;
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
    const row = (label: string, value: unknown) => html`<div class="k">${label}</div><div class="v">${value}</div>`;
    const n = v.inView;
    return html`<div part="card"><div class="kv">
      ${row('Shown', html`<mosaica-count .count=${n ? {shown: n.shown, total: n.matched.value, exact: true} : NO_COUNT} .stale=${stale}></mosaica-count>`)}
      ${row('Match the filters', html`<mosaica-count .masked=${n?.matched ?? NO_MASKED} .stale=${stale}></mosaica-count>`)}
      ${v.highlighting ? row('Highlighted', html`<mosaica-count .masked=${n?.highlighted ?? NO_MASKED} .stale=${stale}></mosaica-count>`) : nothing}
      ${row('In this view', html`<mosaica-count .masked=${n?.visible ?? NO_MASKED} .stale=${stale}></mosaica-count>`)}
    </div></div>`;
  }
}

attachContextRoot();
defineOnce('mosaica-status', MosaicaStatus);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-status': MosaicaStatus;
  }
}
