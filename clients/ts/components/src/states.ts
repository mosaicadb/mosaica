import {html, nothing, type TemplateResult} from 'lit';
import type {AggregateSpec, Store, StatusProjection} from '@mosaicajs/client';
import {isUnanswered} from '@mosaicajs/client/internal';
import {HeldAggregate} from './aggregate.js';

/**
 * The nine panel states. Every panel shows its state in one `part="state"` region, with
 * `data-state` set to it, so one rule restyles every panel. Each follows from the store's `status`
 * projection:
 *
 * | state | from `status` |
 * |---|---|
 * | detached | no store |
 * | loading | `idle` or `loading` |
 * | retrying | `retrying`, or `unreachable` |
 * | shown | `shown` |
 * | empty | `empty` |
 * | refused | `refused` |
 * | unanswered | `refused` with a `5xx` whose body is not a Mosaica error, such as a gateway's `504` |
 * | expired | the `expired` flag: `expired-token`, a `bad-credential` on a token this store used, or `expiresAt` passed |
 * | stale | `shown` with `stale` |
 *
 * `expired` is a named refusal and wins over `refused`. While the server cannot be reached the state
 * is `retrying` whatever the last answer was, since the store is asking again; what is drawn stays.
 * `unanswered` is a server error that says nothing about this viewer, so it is not shown as a
 * refusal; the store does not ask again until Retry.
 * `stale` is `shown` with the picture older than the numbers.
 */
export type PanelState = 'detached' | 'loading' | 'retrying' | 'shown' | 'empty' | 'refused' | 'unanswered' | 'expired' | 'stale';

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
      return isUnanswered(status.refusal) ? 'unanswered' : 'refused';
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
  /** The Retry button, present when refused or unanswered. */
  onRetry?: (() => void) | null;
  /** The Sign in button, shown on expiry when the host gave the element a renewal. */
  onReauthorise?: (() => void) | null;
  /** Whether the panel has shown nothing yet, which loading then names Loading rather than Updating. */
  first?: boolean;
};

/**
 * The words for a state as the status strip says it: two or three plain words. What went wrong in
 * detail stays in `status.refusal`, which a host reads, and is not put on screen.
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

/** The refused state: its dot, its words with the refusal's code, and Retry where `onRetry` is given. */
export function refusedState(words: string, code: string | null | undefined, onRetry?: (() => void) | null): TemplateResult {
  return html`<span part="state" data-state="refused"><span class="dot refuse"></span>${refusalText(words, code)}${onRetry
      ? html`<button part="retry" class="btn small" type="button" @click=${onRetry}>Retry</button>`
      : nothing}</span>`;
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
      return html`<span part="state" data-state="unanswered"><span class="dot warn"></span>${stateWord(state, status)}${actions.onRetry
          ? html`<button part="retry" class="btn small" type="button" @click=${actions.onRetry}>Retry</button>`
          : nothing}</span>`;
    case 'shown':
      return html`<span part="state" data-state="shown"></span>`;
    case 'empty':
      return html`<span part="state" data-state="empty">${stateWord(state, status)}</span>`;
    case 'refused':
      return refusedState(stateWord(state, status), refusal?.code, actions.onRetry);
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
