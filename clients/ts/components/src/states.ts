import {html, nothing, type TemplateResult} from 'lit';
import type {AggregateSpec, Refusal, Store, StatusProjection} from '@mosaicajs/client';
import {isBusy, isUnanswered} from '@mosaicajs/client/internal';
import {HeldAggregate} from './aggregate.js';

/**
 * The state of a panel. Every element shows its state in its `part="state"` element, with
 * `data-state` set to one of these, so one CSS rule can style the same state across every panel.
 *
 * | State | Meaning |
 * |---|---|
 * | `detached` | The element has no store. |
 * | `loading` | The store is connecting or fetching. |
 * | `retrying` | The server cannot be reached, or the store is asking again. What is drawn stays. |
 * | `shown` | The panel shows the server's answer. |
 * | `empty` | The server answered, with nothing to show. |
 * | `refused` | The server refused the request. |
 * | `unanswered` | The server failed with a `5xx` that is not a Mosaica error, such as a gateway's `504`. |
 * | `busy` | The server is too busy (`429`), even after the store's retries. |
 * | `expired` | The session expired: the token is past its expiry, or the server no longer accepts it. |
 * | `stale` | The panel shows an answer, but the data has changed since it was fetched. Refresh fetches it again. |
 *
 * In the `unanswered` and `busy` states the store does not ask again until the viewer presses
 * Retry.
 */
export type PanelState = 'detached' | 'loading' | 'retrying' | 'shown' | 'empty' | 'refused' | 'unanswered' | 'busy' | 'expired' | 'stale';

export function stateOf(status: StatusProjection | null | undefined): PanelState {
  if (!status) return 'detached';
  if (status.expired) return 'expired';
  if (status.unreachable) return 'retrying';
  switch (status.status) {
    case 'idle':
    case 'loading':
      return 'loading';
    case 'retrying':
      return 'retrying';
    case 'empty':
      return 'empty';
    case 'refused':
      return isUnanswered(status.refusal) ? 'unanswered' : isBusy(status.refusal) ? 'busy' : 'refused';
    case 'shown':
      return status.stale ? 'stale' : 'shown';
  }
}

/** Whether a panel in this state shows its content: the numbers, the list, the card. */
export function showsContent(state: PanelState): boolean {
  return state === 'shown' || state === 'stale';
}

export type StateActions = {
  /** The Refresh button, present when stale. */
  onRefresh?: (() => void) | null;
  /** The Retry button, present when refused, unanswered or busy. */
  onRetry?: (() => void) | null;
  /** The Sign in button, shown on expiry when the host gave the element a renewal. */
  onReauthorise?: (() => void) | null;
  /** Whether the panel has shown nothing yet, which loading then names Loading rather than Updating. */
  first?: boolean;
};

/**
 * The words for a state as the status strip says it: two or three plain words. A refusal's reason
 * is shown under them by {@link refusedState}; the rest of what went wrong stays in
 * `status.refusal`, which a host reads.
 */
export function stateWord(state: PanelState, status: StatusProjection | null | undefined): string {
  switch (state) {
    case 'detached':
      return 'Not connected';
    case 'loading':
      return status && !status.sessionWarm ? 'Connecting' : 'Updating';
    case 'retrying':
      return status?.unreachable ? 'Server unreachable' : 'Reconnecting';
    case 'shown':
      return 'Up to date';
    case 'empty':
      return 'Nothing in view';
    case 'refused':
      return 'View refused';
    case 'unanswered':
      return 'Server did not answer';
    case 'busy':
      return 'Server busy';
    case 'expired':
      return 'Session expired';
    case 'stale':
      return 'Data updated';
  }
}

/** The refusal's words, with its code on `data-code` for a host's rules and tests. */
export function refusalText(words: string, code: string | null | undefined): TemplateResult {
  return html`<span part="refusal" data-code=${code ?? nothing}>${words}</span>`;
}

/**
 * The sentence a server gave for refusing, as it sent it, or `null` where the refusal is not the
 * server's, such as an error from a request that reached no server.
 */
export function reasonOf(refusal: Refusal | null | undefined): string | null {
  if (!refusal || refusal.status === null || refusal.code === 'fetch-failed') return null;
  return refusal.detail.trim() || null;
}

/** The Retry button where `onRetry` is given. */
function retryButton(onRetry: (() => void) | null | undefined): TemplateResult | typeof nothing {
  return onRetry ? html`<button part="retry" class="btn small" type="button" @click=${onRetry}>Retry</button>` : nothing;
}

/**
 * What a state's line is made of: the class of the dot before its words, the words, and the button
 * the state offers. An element whose state is a live region puts the words inside it and the
 * button beside it, so the button's name is not read out as part of the state.
 */
export type StateLine = {
  dot: string | null;
  words: TemplateResult | string | typeof nothing;
  action: TemplateResult | typeof nothing;
  /** The dot's tooltip, for a line whose words are not drawn. */
  title?: string;
};

/**
 * The refused state's line: its words with the refusal's code, the server's reason on a line under
 * them where `reason` is given, and Retry where `onRetry` is given.
 */
export function refusedLine(words: string, code: string | null | undefined, onRetry?: (() => void) | null, reason?: string | null): StateLine {
  const said = reason ? html`<span class="said">${refusalText(words, code)}<span part="reason">${reason}</span></span>` : refusalText(words, code);
  return {dot: 'dot refuse', words: said, action: retryButton(onRetry)};
}

/** The line each state needs; `detached` and `shown` need none. */
export function stateLine(state: PanelState, status: StatusProjection | null | undefined, actions: StateActions = {}): StateLine {
  const refusal = status?.refusal ?? null;
  const words = stateWord(state, status);
  switch (state) {
    // Empty: "empty" and "refused" are answers, and a detached panel has none.
    case 'detached':
    case 'shown':
      return {dot: null, words: nothing, action: nothing};
    case 'loading':
      return {dot: 'dot quiet', words: actions.first && status?.sessionWarm ? 'Loading' : words, action: nothing};
    case 'retrying':
      return {dot: 'dot warn', words, action: nothing};
    case 'unanswered':
      return {dot: 'dot warn', words, action: retryButton(actions.onRetry)};
    case 'busy':
      return {dot: 'dot warn', words: refusalText(words, refusal?.code), action: retryButton(actions.onRetry)};
    case 'empty':
      return {dot: null, words, action: nothing};
    case 'refused':
      return refusedLine(words, refusal?.code, actions.onRetry, reasonOf(refusal));
    case 'expired':
      return {
        dot: 'dot refuse',
        words,
        action: actions.onReauthorise ? html`<button part="reauthorise" class="btn small" type="button" @click=${actions.onReauthorise}>Sign in</button>` : nothing
      };
    case 'stale':
      return {dot: 'dot warn', words, action: html`<button part="refresh" class="btn small" type="button" @click=${actions.onRefresh ?? undefined}>Refresh</button>`};
  }
}

/** The dot before a line's words. */
export function stateDot(line: StateLine): TemplateResult | typeof nothing {
  return line.dot ? html`<span class=${line.dot} title=${line.title ?? nothing}></span>` : nothing;
}

/**
 * A line for an element that announces its state: the dot, the words in a `role="status"` element,
 * `after`, and the line's button beside them. An element renders this in the same place in every
 * state, so the live element stays and each change of its words is announced. With `unsaid`, the
 * words sit beside the live element instead, read and not announced, and the live element holds
 * `unsaid.heard`, undrawn.
 */
export function liveLine(line: StateLine, unsaid: {heard: unknown} | null = null, after: unknown = nothing): TemplateResult {
  return html`${stateDot(line)}${unsaid ? line.words : nothing}<span class=${unsaid ? 'sr-only' : nothing} role="status">${unsaid ? unsaid.heard : line.words}</span>${after}${line.action}`;
}

/** What {@link liveLine} takes for words that are shown and never announced. */
export const UNSAID = {heard: nothing};

/**
 * Whether the view's emptiness is a query's answer, which an element announces, or where a pan
 * went, which it does not. An element calls {@link note} once for each update.
 */
export class EmptyAnswer {
  private wasQuerying = false;
  private answered = false;

  /** Whether `state` is `empty` and a query ended in it. It stays so until the view is no longer empty. */
  note(state: PanelState, querying: boolean): boolean {
    if (state !== 'empty') this.answered = false;
    else if (this.wasQuerying && !querying) this.answered = true;
    this.wasQuerying = querying;
    return this.answered;
  }
}

/** The element with the focus in `doc`, looked for through the shadow roots that hold it. */
export function focusedElement(doc: Document): Element | null {
  let focused = doc.activeElement;
  while (focused?.shadowRoot?.activeElement) focused = focused.shadowRoot.activeElement;
  return focused;
}

/** A line as a `part="state"` carrying `data-state`, with `after` between its words and its button. */
function stateRegion(state: PanelState, line: StateLine, after: TemplateResult | typeof nothing = nothing): TemplateResult {
  return html`<span part="state" data-state=${state}>${stateDot(line)}${line.words}${after}${line.action}</span>`;
}

/**
 * The refused state: its dot, its words with the refusal's code, the server's reason on a line
 * under them where `reason` is given, and Retry where `onRetry` is given.
 */
export function refusedState(words: string, code: string | null | undefined, onRetry?: (() => void) | null, reason?: string | null): TemplateResult {
  return stateRegion('refused', refusedLine(words, code, onRetry, reason));
}

/**
 * The state region every panel renders: a `part="state"` carrying `data-state`, and the line the
 * state needs, such as a skeleton while loading or the refresh control when stale.
 */
export function renderState(
  state: PanelState,
  status: StatusProjection | null | undefined,
  actions: StateActions = {}
): TemplateResult | typeof nothing {
  const skeleton = state === 'loading' && !(status && !status.sessionWarm) ? html`<span class="skel" aria-hidden="true"></span>` : nothing;
  return stateRegion(state, stateLine(state, status, actions), skeleton);
}

/**
 * Where focus goes when a state's button leaves with its state. An element calls {@link note} from
 * `willUpdate` and {@link keep} from `updated`, naming the element that takes the focus.
 */
export class ActionFocus {
  private held: Element | null = null;

  /** Remember the button with focus in a state line inside `root`, if there is one. */
  note(root: DocumentFragment | HTMLElement | null): void {
    const focused = root ? focusedElement(root.ownerDocument) : null;
    this.held = focused instanceof HTMLButtonElement && root!.contains(focused) && focused.closest('[part~="state"]') ? focused : null;
  }

  /** Focus `to` if the remembered button has left the page. */
  keep(to: HTMLElement | null): void {
    if (this.held && !this.held.isConnected) to?.focus();
    this.held = null;
  }
}

/** How long a query has run before an element names it, so one answered at once does not flicker it. */
const QUERY_NAMED_AFTER_MS = 250;

/**
 * Whether a query has run long enough to name. An element calls {@link track} from `updated` with
 * whether its query is running, reads {@link named} when it renders, and is asked to update when
 * the query has run for {@link QUERY_NAMED_AFTER_MS}.
 */
export class NamedQuery {
  named = false;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(private readonly host: {requestUpdate(): void}) {}

  track(running: boolean): void {
    if (!running) {
      this.stop();
      this.named = false;
      return;
    }
    if (this.named || this.timer !== null) return;
    this.timer = setTimeout(() => {
      this.timer = null;
      this.named = true;
      this.host.requestUpdate();
    }, QUERY_NAMED_AFTER_MS);
  }

  /** Forget the wait, as an element leaving the page does. */
  stop(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }
}

/** The words for a viewer who can see no item in the view at all, where `empty` would read as "pan elsewhere". */
export const NOTHING_VISIBLE = 'Nothing visible to you';

const VISIBLE_TOTAL: AggregateSpec = {groupings: [{count: true}], subject: 'visible'};
const visibleTotals = new WeakMap<Store, HeldAggregate>();

/**
 * Whether the viewer of `store` can see no item in the view at all, whatever the camera and the
 * filters. It is counted, once per store however many elements ask, from the first time `state` is
 * `empty`, and the registration is kept, so the store asks again only where it asks every
 * registered aggregate again: for another viewer, view or state of the corpus. `false` until the
 * count answers, and wherever `state` is not `empty`.
 */
export function seesNothing(store: Store | null, state: PanelState): boolean {
  if (!store) return false;
  let held = visibleTotals.get(store);
  if (!held) {
    if (state !== 'empty') return false;
    held = new HeldAggregate('visible-total');
    visibleTotals.set(store, held);
  }
  held.set(store, VISIBLE_TOTAL);
  return state === 'empty' && held.entryOf(store, VISIBLE_TOTAL)?.result?.tables[0]?.total === 0;
}
