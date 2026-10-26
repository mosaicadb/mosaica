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
 * | `stale` | The panel shows an answer, but the picture is older than the numbers. |
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
 * The refused state: its dot, its words with the refusal's code, the server's reason on a line
 * under them where `reason` is given, and Retry where `onRetry` is given.
 */
export function refusedState(words: string, code: string | null | undefined, onRetry?: (() => void) | null, reason?: string | null): TemplateResult {
  const said = reason ? html`<span class="said">${refusalText(words, code)}<span part="reason">${reason}</span></span>` : refusalText(words, code);
  return html`<span part="state" data-state="refused"><span class="dot refuse"></span>${said}${retryButton(onRetry)}</span>`;
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
  const refusal = status?.refusal ?? null;
  switch (state) {
    case 'detached':
      // Empty: "empty" and "refused" are answers, and a detached panel has none.
      return html`<span part="state" data-state="detached"></span>`;
    case 'loading': {
      const words = actions.first && status?.sessionWarm ? 'Loading' : stateWord(state, status);
      return html`<span part="state" data-state="loading"><span class="dot quiet"></span>${words}${status && !status.sessionWarm ? nothing : html`<span class="skel" aria-hidden="true"></span>`}</span>`;
    }
    case 'retrying':
      return html`<span part="state" data-state="retrying"><span class="dot warn"></span>${stateWord(state, status)}</span>`;
    case 'unanswered':
      return html`<span part="state" data-state="unanswered"><span class="dot warn"></span>${stateWord(state, status)}${retryButton(actions.onRetry)}</span>`;
    case 'busy':
      return html`<span part="state" data-state="busy"><span class="dot warn"></span>${refusalText(stateWord(state, status), refusal?.code)}${retryButton(actions.onRetry)}</span>`;
    case 'shown':
      return html`<span part="state" data-state="shown"></span>`;
    case 'empty':
      return html`<span part="state" data-state="empty">${stateWord(state, status)}</span>`;
    case 'refused':
      return refusedState(stateWord(state, status), refusal?.code, actions.onRetry, reasonOf(refusal));
    case 'expired':
      return html`<span part="state" data-state="expired"><span class="dot refuse"></span>${stateWord(state, status)}${actions.onReauthorise
          ? html`<button part="reauthorise" class="btn small" type="button" @click=${actions.onReauthorise}>Sign in</button>`
          : nothing}</span>`;
    case 'stale':
      return html`<span part="state" data-state="stale"><span class="dot warn"></span>${stateWord(state, status)}<button part="refresh" class="btn small" type="button" @click=${actions.onRefresh ?? undefined}>Refresh</button></span>`;
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
