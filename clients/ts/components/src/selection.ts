import {css, html, nothing, type PropertyValues, type TemplateResult} from 'lit';
import {property} from 'lit/decorators.js';
import {formatMasked, type RegionProjection, type StatusProjection} from '@mosaicajs/client';
import {isBusy, isUnanswered} from '@mosaicajs/client/internal';
import {MosaicaElement, emit, idString, shapeDetail} from './base.js';
import {attachContextRoot, defineOnce} from './define.js';
import {icon} from './icons.js';
import {ActionFocus, UNSAID, liveLine, reasonOf, refusedLine, stateLine, stateOf, type PanelState, type StateLine} from './states.js';
import {chrome, tokens} from './tokens.js';
import './count.js';

/**
 * Use the selection card beside a map so that viewers can see what is inside the region they have
 * drawn. It reads the store's selected region, or the `region` you set. It counts the items inside
 * that the filters match, the items the viewer may see there and the marks drawn there. The drawn
 * marks are listed below the counts, each a button that picks it.
 *
 * The region acts as a filter, which means the map and every other count narrow to it once it
 * settles. Outside flips it to everything outside the shape, and Clear removes it. The counts are
 * approximate when the server had to count an area around the shape, and the state line's tooltip
 * says when that is so. Export and Save as artifact are shown disabled because they are not
 * available yet.
 *
 * Screen readers announce the state line as it changes: that the selection is being counted, a
 * refusal, and, once, when the query has finished loading, the matched and visible counts with
 * whether they are exact for the shape. The counts and that verdict are read out and not drawn on
 * the line, since the figures are below it. With `quiet`, the card shows the same and announces
 * nothing.
 *
 * @summary A card showing the counts and marks inside the selected region.
 * @tagname mosaica-selection
 * @category Elements
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-selectchange']>} mosaica-selectchange - Clear,
 *   Outside or Inside was pressed.
 * @csspart title - The heading, with the shape's kind.
 * @csspart state - The state line, with `data-state`. Its tooltip says whether the counts are exact
 *   for the shape.
 * @csspart refusal - The words "Selection refused", or "Server busy", with `data-code` set to the
 *   refusal's code.
 * @csspart reason - The server's reason for the refusal, under "Selection refused".
 * @csspart retry - The Retry button, shown if the server was too busy to count the selection.
 * @csspart counts - The counts.
 * @csspart count-served - The `<mosaica-count>` of marks shown inside.
 * @csspart count-matched - The `<mosaica-count>` of items the filters match inside (or outside).
 * @csspart count-visible - The `<mosaica-count>` of items the viewer may see inside (or outside).
 * @csspart label - The heading above the list.
 * @csspart items - The list of drawn marks inside.
 * @csspart item - One drawn mark's row, holding a button named by its `mosaica_id`.
 * @csspart actions - The action buttons.
 * @csspart action - One action button.
 */
export class MosaicaSelection extends MosaicaElement {
  static override styles = [
    tokens,
    chrome,
    css`
      :host {
        display: block;
      }
      [part='counts'] {
        margin: 0 0 4px;
      }
      [part='counts'] dd {
        margin: 0;
      }
      [part='counts'] mosaica-count::part(count) {
        font-weight: 500;
      }
      [part='counts'] mosaica-count::part(label) {
        display: none;
      }
      .items-label {
        margin: 10px 0 4px;
      }
      [part='items'] {
        list-style: none;
        margin: 0;
        padding: 0;
        max-height: 160px;
        overflow-y: auto;
      }
      [part='item'] {
        min-height: 26px;
      }
      /* The row is its button, which fills it and takes the row's own padding. */
      .list > [part='item'] {
        padding: 0;
      }
      [part='item'] button {
        flex: 1 1 0;
        min-width: 0;
        align-self: stretch;
        padding: 0 6px;
        border-radius: var(--_mosaica-radius-control);
        font-family: var(--_mosaica-font-mono);
        font-size: 12px;
        text-align: left;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }
      [part='item'] button:focus-visible {
        outline-offset: -2px;
      }
      [part='actions'] {
        display: flex;
        flex-wrap: wrap;
        gap: 6px;
        margin-top: 12px;
      }
    `
  ];

  /** A region to show instead of the store's `region` projection. */
  @property({attribute: false}) accessor region: RegionProjection | null = null;
  /**
   * Announces nothing: the state line and the counts are still shown, and none is put in a live
   * region. Set it when another element on the page says the same things.
   */
  @property({type: Boolean, reflect: true}) accessor quiet = false;

  private get shown(): RegionProjection | null {
    return this.region ?? this.resolvedStore?.get('region') ?? null;
  }

  private readonly actionFocus = new ActionFocus();

  protected override willUpdate(changed: PropertyValues<this>): void {
    super.willUpdate(changed);
    this.actionFocus.note(this.renderRoot);
  }

  protected override updated(): void {
    this.actionFocus.keep(this.renderRoot.querySelector<HTMLElement>('[part="state"]'));
  }

  /**
   * The state and its line for a region: what the server said of its count, or how the count
   * stands. Once it is counted, `verdict` says whether the counts are exact for the shape.
   */
  private stateOfRegion(r: RegionProjection, status: StatusProjection | null): {state: PanelState; line: StateLine; verdict?: string} {
    // A server that cannot be reached, did not answer or is too busy is named as the status strip
    // names it, since nothing about the selection was refused.
    const unreachable = stateOf(status) === 'retrying' && status?.unreachable === true;
    const unanswered = (r.status === 'refused' && isUnanswered(r.refusal)) || stateOf(status) === 'unanswered';
    if (r.status !== 'shown' && (unreachable || unanswered)) {
      const state = unreachable ? 'retrying' : 'unanswered';
      return {state, line: stateLine(state, status)};
    }
    // Busy only where the selection's own count was shed, so the code shown is its own.
    if (r.status === 'refused' && isBusy(r.refusal) && status) return {state: 'busy', line: stateLine('busy', {...status, refusal: r.refusal}, {onRetry: () => this.resolvedStore?.refresh()})};
    // Counting until the query has finished loading. The figures a frame gives first are an
    // estimate, shown below as they stand with no line above them; the line goes on saying
    // Counting, undrawn, from the same element, so the counts that follow are said once.
    const estimate = r.status === 'shown' && status?.querying === true;
    const counting = html`<span class=${estimate ? 'sr-only' : nothing}>Counting</span>`;
    if (r.status === 'loading') return {state: 'loading', line: {dot: 'dot quiet', words: counting, action: nothing}};
    if (r.status === 'refused') return {state: 'refused', line: refusedLine('Selection refused', r.refusal?.code, null, reasonOf(r.refusal))};
    const verdict = r.verdict === null ? 'Not counted yet' : r.verdict.exact ? 'Exact for the shape' : 'Approximate: counted over cells around the shape';
    if (status?.stale) return {state: 'stale', line: {dot: 'dot warn', words: 'Data updated', action: nothing}, verdict};
    if (estimate) return {state: 'shown', line: {dot: null, words: counting, action: nothing}, verdict};
    // The counts in words and whether they are exact for the shape: heard as they arrive and not
    // drawn, since the figures are below and the verdict is the line's tooltip.
    const side = r.shape.outside ? 'outside' : 'inside';
    const counted = `${formatMasked(r.matched)} matched ${side}${r.visible ? `, ${formatMasked(r.visible)} visible ${side}` : ''}.`;
    return {state: 'shown', line: {dot: null, words: html`<span class="sr-only">${this.quiet ? nothing : counted} ${verdict}</span>`, action: nothing}, verdict};
  }

  override render(): TemplateResult | typeof nothing {
    const r = this.shown;
    const status = this.resolvedStore?.get('status') ?? null;
    const stale = status?.stale ?? false;
    const {state, line, verdict} = r ? this.stateOfRegion(r, status) : {state: 'detached' as const, line: stateLine('detached', null)};
    const pair = (label: string, count: unknown) => html`<dt class="k">${label}</dt><dd class="v">${count}</dd>`;
    const counts =
      r?.status === 'shown'
        ? html`<dl part="counts" class="kv">
            ${pair('Shown inside', html`<mosaica-count part="count-served" .count=${r.served} .stale=${stale} figure="shown"></mosaica-count>`)}
            ${pair(r.shape.outside ? 'Matched outside' : 'Matched inside', html`<mosaica-count part="count-matched" .masked=${r.matched} .stale=${stale}></mosaica-count>`)}
            ${r.visible ? pair(r.shape.outside ? 'Visible outside' : 'Visible inside', html`<mosaica-count part="count-visible" .masked=${r.visible} .stale=${stale}></mosaica-count>`) : nothing}
          </dl>`
        : nothing;
    const ids = r ? Array.from(r.held.ids, idString) : [];
    const unavailable = 'Not available yet';
    const waiting = (label: string) => html`<button part="action" class="btn off" type="button" disabled title=${unavailable} aria-describedby="unavailable">${label}</button>`;
    // One template whether or not there is a region, so the live element inside the state line is
    // there before its words are. The state's button sits beside it, and the line takes the focus
    // when the button goes.
    return html`<div class="panel">
      <h2 part="title">Selection <span class="summary">${r ? (r.shape.outside ? `outside ${r.shape.kind}` : r.shape.kind) : nothing}</span></h2>
      <span part="state" data-state=${state} tabindex="-1" title=${verdict ?? nothing}>${liveLine(line, this.quiet ? UNSAID : null, state === 'loading' ? html`<span class="skel" aria-hidden="true"></span>` : nothing)}</span>
      ${counts}
      ${r && ids.length > 0
        ? html`<div part="label" id="items-label" class="xs muted items-label">Shown items</div>
          <ul part="items" class="list" aria-labelledby="items-label">
            ${ids.map((id) => html`<li part="item" class="item"><button type="button" @click=${() => void this.resolvedStore?.pick(BigInt(id))}>${id}</button></li>`)}
            ${r.held.count > ids.length ? html`<li class="muted sm">${(r.held.count - ids.length).toLocaleString('en-GB')} more not listed</li>` : nothing}
          </ul>`
        : nothing}
      ${r
        ? html`<div part="actions">
            <button part="action" class="btn" type="button" @click=${() => {
              this.resolvedStore?.select(null);
              emit(this, 'mosaica-selectchange', {shape: null});
            }}>${icon('close', 13)}Clear</button>
            <button part="action" class="btn" type="button" title=${r.shape.outside ? 'Filter to the inside of the shape' : 'Filter to the outside of the shape'} @click=${() => {
              const next = {...r.shape, outside: !r.shape.outside};
              this.resolvedStore?.select(next);
              emit(this, 'mosaica-selectchange', {shape: shapeDetail(next), status: 'loading'});
            }}>${icon(r.shape.outside ? 'filter' : 'outside', 13)}${r.shape.outside ? 'Inside' : 'Outside'}</button>
            ${waiting('Export')}${waiting('Save as artifact')}
            <span id="unavailable" class="sr-only">${unavailable}</span>
          </div>`
        : nothing}
    </div>`;
  }
}

attachContextRoot();
defineOnce('mosaica-selection', MosaicaSelection);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-selection': MosaicaSelection;
  }
}
