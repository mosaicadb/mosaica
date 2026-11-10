import {css, html, nothing, type TemplateResult} from 'lit';
import {property} from 'lit/decorators.js';
import {artifactName, isFilterLayer, withMember, withoutMember, type Artifact, type ArtifactDetail, type ClauseVerb, type Layer, type Masked, type Refusal} from '@mosaicajs/client';
import {MosaicaElement, UNNAMED, emit, focusLost, idString} from './base.js';
import {attachContextRoot, defineOnce} from './define.js';
import {icon} from './icons.js';
import {reasonOf, refusedState, renderState} from './states.js';
import {chrome, tokens} from './tokens.js';
import './count.js';

/** A parent's or child's name, or {@link UNNAMED}. */
const nameSpan = (name: string | null) => html`<span part="name" class="name" ?data-unnamed=${name === null}>${name ?? UNNAMED}</span>`;

/** What each kind of drawn shape says on the card. */
const SHAPE_TEXT: Record<'derived' | 'predicate' | 'authored', string> = {
  derived: 'Drawn around the members you can see',
  predicate: 'The same for everyone',
  authored: 'The same for everyone'
};

/**
 * Use the artifact card to show an artifact the viewer has opened on the map, such as a cluster. An
 * artifact is a named group of items in an annotation layer. The card's count covers every member
 * the viewer may see, so it does not change as the map moves. Pressing one of the artifact's
 * parents or children opens that one instead, and moves focus to its name.
 *
 * The card reads the store's opened artifact, or the `artifact` you set.
 *
 * The card's buttons filter or highlight the map by the artifact's members wherever they are, or
 * filter to everything outside it. Fit to cluster fires `mosaica-artifactfit`, and
 * `<mosaica-explorer>` responds by fitting its map to the artifact. A filter layer draws nothing,
 * so its artifacts have no Fit button.
 *
 * @summary A card showing an artifact's details, parents and children.
 * @tagname mosaica-artifact-card
 * @category Elements
 * @slot actions - Buttons beside the close button, such as the explorer's Pin.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-artifactfit']>} mosaica-artifactfit - Fit to
 *   cluster was pressed. `<mosaica-explorer>` fits its map to the artifact.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-clausechange']>} mosaica-clausechange - A filter
 *   or highlight button added or removed a `member_of` clause.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-close']>} mosaica-close - The close button was
 *   pressed, with `what` set to `artifact`.
 * @csspart title - The heading: the level's or layer's title.
 * @csspart close - The close button.
 * @csspart state - The state line, with `data-state`, inside a `role="status"` region.
 * @csspart refusal - The words "Not available", with `data-code` set to the refusal's code.
 * @csspart reason - The server's reason for the refusal, under "Not available".
 * @csspart headline - The artifact's name, an `<h3>`, with `data-unnamed` if it has none.
 * @csspart count - The count of items the viewer may see.
 * @csspart content - The artifact's description, or its attached topic.
 * @csspart value - The layer and the key.
 * @csspart shape - How the layer's shapes are made, with `data-kind` (`derived`, `predicate` or
 *   `authored`).
 * @csspart label - The headings above the parents and the children, each an `<h3>`.
 * @csspart parents - The list of parents.
 * @csspart parent - One parent, a button, with `data-id`.
 * @csspart children - The list of children in this view.
 * @csspart child - One child, a button, with `data-id`.
 * @csspart name - A parent's or child's name, with `data-unnamed` if it has none.
 * @csspart fit - The Fit to cluster button.
 * @csspart verbs - The clause buttons.
 * @csspart filter - The Filter to this button, with `aria-pressed` while its clause is on.
 * @csspart highlight - The Highlight this button, with `aria-pressed` while its clause is on.
 * @csspart outside - The Outside this button, with `aria-pressed` while its clause is on.
 */
export class MosaicaArtifactCard extends MosaicaElement {
  static override styles = [
    tokens,
    chrome,
    css`
      :host {
        display: block;
      }
      .head {
        display: flex;
        align-items: center;
        gap: 8px;
        margin: 0 0 10px;
      }
      [part='title'] {
        flex: 1;
        min-width: 0;
        margin: 0;
      }
      h3[part='headline'] {
        display: block;
        margin: 0 0 6px;
        letter-spacing: 0;
        text-transform: none;
        color: var(--_mosaica-ink);
      }
      /* The name takes focus when the card opens a parent or child in its place; it is no control, so it shows no ring. */
      [part='headline']:focus {
        outline: none;
      }
      [part='count'] {
        display: flex;
        align-items: baseline;
        gap: 6px;
        margin-bottom: 10px;
      }
      [part='count'] mosaica-count::part(count) {
        font-size: 20px;
        font-weight: 600;
      }
      [part='count'] mosaica-count::part(label) {
        font-size: 13px;
      }
      [part='content'] {
        margin: 0 0 10px;
        font-size: 12px;
        color: var(--_mosaica-ink-2);
      }
      dl,
      dd {
        margin: 0;
      }
      h3[part='label'] {
        display: block;
        margin: 12px 0 4px;
        font-weight: 400;
        letter-spacing: 0;
        text-transform: none;
      }
      [part='parents'],
      [part='children'] {
        list-style: none;
        margin: 0;
        padding: 0;
      }
      /* A parent or child: its name on one line, and its count at the right. */
      [part='parent'],
      [part='child'] {
        display: flex;
        align-items: center;
        gap: 8px;
        width: 100%;
        min-height: 30px;
        padding: 0 6px 0 22px;
        border-radius: var(--_mosaica-radius-control);
        text-align: left;
      }
      [part='parent']:hover,
      [part='child']:hover {
        background: var(--_mosaica-surface-2);
      }
      [part='name'] {
        flex: 1 1 0;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: var(--_mosaica-long-text);
        overflow-wrap: anywhere;
      }
      [part='parent'] mosaica-count::part(count),
      [part='child'] mosaica-count::part(count) {
        color: var(--_mosaica-ink-2);
        font-size: 12px;
        font-weight: 400;
      }
      [part='fit'] {
        margin-top: 12px;
      }
      [part='verbs'] {
        display: flex;
        flex-wrap: wrap;
        gap: 6px;
        margin-top: 8px;
      }
      [part='verbs'] .btn[aria-pressed='true'] {
        border-color: currentColor;
        background: var(--_mosaica-surface-2);
        box-shadow: inset 0 -2px 0 currentColor;
      }
      [part='verbs'] [data-verb='highlight'][aria-pressed='true'] {
        background: var(--_mosaica-highlight-soft);
        color: var(--_mosaica-highlight);
      }
      [part='close'] {
        flex: none;
        width: 24px;
        height: 24px;
        margin: -4px -4px -4px 0;
        display: grid;
        place-items: center;
        border-radius: 5px;
        color: var(--_mosaica-ink-2);
      }
      [part='close']:hover {
        background: var(--_mosaica-surface-2);
      }
    `
  ];

  /** The artifact to show, if you fetch its details yourself. It replaces the store's opened artifact. */
  @property({attribute: false}) accessor artifact: {id: bigint; detail: ArtifactDetail} | null = null;
  /** A refusal to show instead of an artifact, if you fetch its details yourself. */
  @property({attribute: false}) accessor refusal: Refusal | null = null;

  private get shown(): {artifact: {id: bigint; detail: ArtifactDetail} | null; refusal: Refusal | null} {
    if (this.artifact || this.refusal) return {artifact: this.artifact, refusal: this.refusal};
    const sel = this.resolvedStore?.get('selection');
    return {artifact: sel?.artifact ?? null, refusal: sel?.artifactRefusal ?? null};
  }

  /**
   * One of the card's clause buttons: a `member_of` clause on this artifact, as a filter or a
   * highlight, inside or outside. The button shows whether the clause is on; clicking a pressed
   * button withdraws it. The artifact holds one clause in each position, so Filter to this and
   * Outside this replace each other, and a highlight is kept beside either.
   */
  private verb(layer: string, artifact: bigint, outside: boolean, verb: ClauseVerb, label: string, name: string | null) {
    const s = this.resolvedStore;
    const held = s?.get('filters').members ?? [];
    const on = held.some((c) => c.layer === layer && c.artifact === artifact && c.outside === outside && c.verb === verb);
    const title =
      verb === 'filter'
        ? outside
          ? 'Narrow the map and every count to what is not in this artifact'
          : "Narrow the map and every count to this artifact's members"
        : "Keep the map and light this artifact's members";
    return html`<button
      part=${outside ? 'outside' : verb === 'filter' ? 'filter' : 'highlight'}
      class="btn"
      type="button"
      data-verb=${verb}
      aria-pressed=${on ? 'true' : 'false'}
      title=${title}
      @click=${() => {
        if (!s) return;
        const members = on
          ? withoutMember(s.get('filters').members, layer, artifact, verb)
          : withMember(s.get('filters').members, {layer, artifact, outside, verb, ...(name === null ? {} : {label: name})});
        s.setMembers(members);
        emit(this, 'mosaica-clausechange', {id: idString(artifact), layer, outside, verb, on: !on});
      }}
    >
      ${icon(outside ? 'outside' : verb === 'filter' ? 'filter' : 'highlight', 14)}${label}
    </button>`;
  }

  /** The parent or child pressed, its button, and the artifact it was pressed on, until the card shows another. */
  private opening: {id: bigint; from: bigint | null; button: HTMLElement} | null = null;

  /** Open a parent or child in the card's place. */
  private openRow(id: bigint, button: HTMLElement): void {
    this.opening = {id, from: this.shown.artifact?.id ?? null, button};
    void this.resolvedStore?.openArtifact(id);
  }

  /**
   * Once the card shows the parent or child that was pressed, focus goes to its name, since the
   * row that had it is gone; where it was refused, to the close button. Focus that has gone
   * anywhere else meanwhile stays there.
   */
  protected override updated(): void {
    const opening = this.opening;
    if (!opening) return;
    const {artifact, refusal} = this.shown;
    if (!refusal && artifact?.id === opening.from) return;
    this.opening = null;
    if (this.shadowRoot?.activeElement !== opening.button && !(focusLost() && !opening.button.isConnected)) return;
    if (refusal) this.renderRoot.querySelector<HTMLElement>('[part="close"]')?.focus();
    else if (artifact?.id === opening.id) this.renderRoot.querySelector<HTMLElement>('[part="headline"]')?.focus();
  }

  /** The parents or the children, each a button that opens it, under a heading that names the list. */
  private rows(children: boolean, rows: readonly Artifact[], attached: ReadonlyMap<bigint, string>, stale: boolean) {
    if (rows.length === 0) return nothing;
    const id = children ? 'children' : 'parents';
    return html`<h3 part="label" class="xs muted" id=${id}>${children ? 'Children in this view' : 'Parents'}</h3>
      <ul part=${children ? 'children' : 'parents'} aria-labelledby=${id}>
        ${rows.map(
          (a) => html`<li><button part=${children ? 'child' : 'parent'} type="button" data-id=${idString(a.mosaicaId)} @click=${(e: Event) => this.openRow(a.mosaicaId, e.currentTarget as HTMLElement)}>
            ${nameSpan(artifactName(a, attached))}
            <mosaica-count .masked=${{value: Number(a.maskedCount), exact: true} as Masked} .stale=${stale}></mosaica-count>
          </button></li>`
        )}
      </ul>`;
  }

  override render(): TemplateResult | typeof nothing {
    const {artifact, refusal} = this.shown;
    // The heading is the level's title on a levelled layer (County, Admin 2), else the layer's.
    const metaLayers = this.resolvedStore?.get('meta')?.layers ?? [];
    const row = artifact ? this.resolvedStore?.get('artifacts')?.served.find((a) => a.mosaicaId === artifact.id) : undefined;
    // A filter layer's artifacts are never served, so the layer falls back to the drill-down's;
    // otherwise the card would offer Fit on a layer with nothing drawn.
    const declaredLayer = row?.layer ?? artifact?.detail.layer;
    const decl = declaredLayer === undefined ? undefined : metaLayers.find((l) => l.name === declaredLayer);
    const what = (row && decl?.levels.find((lv) => lv.level === row.rung)?.title) || decl?.title || decl?.name || 'Selected';
    const state = refusal
      ? refusedState('Not available', refusal.code, null, reasonOf(refusal))
      : artifact
        ? html`<span part="state" data-state="shown"></span>`
        : this.resolvedStore
          ? html`<span part="state" data-state="empty">Nothing selected</span>`
          : renderState('detached', null);
    // One template whatever the card shows, so the state's region is in the page before its words
    // change. The close button appears only while the card shows something to close.
    return html`<div class="panel">
      <div class="head">
        <h2 part="title">${what}</h2>
        ${refusal || artifact
          ? html`<slot name="actions"></slot><button part="close" type="button" aria-label="Close" @click=${() => emit(this, 'mosaica-close', {what: 'artifact'})}>${icon('close', 14)}</button>`
          : nothing}
      </div>
      <div role="status">${state}</div>
      ${artifact && !refusal ? this.body(artifact, decl) : nothing}
    </div>`;
  }

  /** What the card shows of an artifact, under its heading. */
  private body(artifact: {id: bigint; detail: ArtifactDetail}, decl: Layer | undefined): TemplateResult {
    const s = this.resolvedStore;
    const artifacts = s?.get('artifacts');
    const served = artifacts?.served ?? [];
    const attached = artifacts?.attached ?? new Map<bigint, string>();
    const bySize = (a: Artifact, b: Artifact) => (a.maskedCount < b.maskedCount ? 1 : a.maskedCount > b.maskedCount ? -1 : 0);
    // The artifact's row and children are read from the served set now, on every render.
    const here = served.find((a) => a.mosaicaId === artifact.id);
    // On a `dag` layer a child appears on the card of each served parent.
    const children = served.filter((a) => a.parentIds.includes(artifact.id)).sort(bySize);
    // Parents from the served set, which holds every parent the map draws. The hierarchy panel
    // asks `POST /v1/artifacts/browse` for the rest.
    const parents = here ? served.filter((a) => here.parentIds.includes(a.mosaicaId)).sort(bySize) : [];
    const stale = s?.get('status').stale ?? false;
    const count: Masked = {value: Number(artifact.detail.maskedCount), exact: true};
    const id = idString(artifact.id);
    // The kind of shape the layer draws, so a reader knows whether the outline depends on what
    // they may see.
    const shape = decl?.shape ? {kind: decl.shape, text: SHAPE_TEXT[decl.shape]} : null;
    // A clause made here carries the drawn name as its label, since nothing downstream can resolve it.
    const clauseName = here ? artifactName(here, attached) : null;
    return html`<h3 part="headline" class="card-title" tabindex="-1" ?data-unnamed=${clauseName === null}>${clauseName ?? UNNAMED}</h3>
      <div part="count"><mosaica-count .masked=${count} .stale=${stale} label="members"></mosaica-count></div>
      ${here && here.content.length > 0 && attached.has(here.mosaicaId) ? html`<p part="content">${attached.get(here.mosaicaId)}</p>` : here && here.content.length > 1 ? html`<p part="content">${here.content.slice(1).join(' · ')}</p>` : nothing}
      <dl class="field">
        <dt class="k">Layer</dt><dd part="value" class="v">${decl?.title || artifact.detail.layer}</dd>
        ${artifact.detail.key ? html`<dt class="k">Key</dt><dd part="value" class="v mono">${artifact.detail.key}</dd>` : nothing}
        ${shape ? html`<dt class="k">Shape</dt><dd part="shape" class="v" data-kind=${shape.kind}>${shape.text}</dd>` : nothing}
      </dl>
      ${this.rows(false, parents, attached, stale)}
      ${this.rows(true, children, attached, stale)}
      ${
        // A filter layer draws nothing, so there is nothing to fit to.
        decl && isFilterLayer(decl)
          ? nothing
          : html`<button part="fit" class="btn" type="button" @click=${() => emit(this, 'mosaica-artifactfit', {id})}>${icon('fit', 14)}Fit to cluster</button>`
      }
      <div part="verbs" role="group" aria-label="This artifact">
        ${this.verb(artifact.detail.layer, artifact.id, false, 'filter', 'Filter to this', clauseName)}
        ${this.verb(artifact.detail.layer, artifact.id, false, 'highlight', 'Highlight this', clauseName)}
        ${this.verb(artifact.detail.layer, artifact.id, true, 'filter', 'Outside this', clauseName)}
      </div>`;
  }
}

attachContextRoot();
defineOnce('mosaica-artifact-card', MosaicaArtifactCard);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-artifact-card': MosaicaArtifactCard;
  }
}
