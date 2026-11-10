import {css, html, nothing, type PropertyValues, type TemplateResult} from 'lit';
import {property, state} from 'lit/decorators.js';
import type {DeclaredScalar, ItemDetail, ItemViewPosition, Meta, Quantisation, Refusal, Store} from '@mosaicajs/client';
import {GRID32} from '@mosaicajs/client';
import type {MosaicaEventDetails} from './events.js';
import {BOOL_WORDS, MosaicaElement, columnCaption, emit, idString, keyTitle, timestampText, type PickOutcome} from './base.js';
import {attachContextRoot, defineOnce} from './define.js';
import {icon} from './icons.js';
import {reasonOf, refusedState, renderState, stateOf} from './states.js';
import {chrome, tokens} from './tokens.js';

/**
 * Use the item card to show the record of the item a viewer has picked on the map, so that they can
 * read its fields and open it elsewhere. It reads the store's selected item, or the `item` you set
 * when you fetch records yourself.
 *
 * The headline is the `title-field` field, or the item's `mosaica_id` when that field is unset or
 * empty. To render a field your own way, as a link for example, put an element in its
 * `field-<name>` slot. The card is headed "Item" for a screen reader, with the headline a heading
 * under it.
 *
 * A value over 60 characters is kept to six lines, with a Show more button under it where that
 * cuts it short. A `compact` card also keeps its headline to three lines and its subtitle to two,
 * and its Show all button shows every field and all of each.
 *
 * A field named in `image-fields` whose value is an `http:` or `https:` address, or a path
 * resolved against the page, is drawn as the image at that address, across the card's width. The viewer's browser loads it from the host
 * that serves it, without a referrer. Its alternative text is the item's `title-field` value, or
 * the field's name where the item has none. A value that is not such an address, and an image that
 * does not load, are shown as text.
 *
 * The Open button appears only when you set `open-url` or `open`. `open-url` is a link template
 * such as `https://www.geonames.org/{geonameid}`, in which each `{name}` is replaced by the item's
 * value for that field. Because the values are URL-encoded, a template cannot open an address that
 * a field already holds, so set `open` to a function for that. An item gets no Open button when it
 * has no value for a field the template names. The link opens in a new tab. A function in `open`
 * is called with the item's `mosaica_id` and fields, and it takes precedence over `open-url`.
 *
 * @summary A card showing an item's fields, with an optional Open button.
 * @tagname mosaica-item-card
 * @category Elements
 * @slot field-<name> - Replaces the value of the field `<name>`, in the grid or the headline.
 * @slot actions - Buttons beside the close button, such as the explorer's Pin.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-open']>} mosaica-open - Open was pressed.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-close']>} mosaica-close - The close button was
 *   pressed, with `what` set to `item`.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-viewfollow']>} mosaica-viewfollow - A view chip
 *   was pressed, asking to show the item in that view.
 * @csspart title - The header row: the headline or the state, and the close button.
 * @csspart close - The close button.
 * @csspart state - The state line, with `data-state`: `shown`, `empty` (no item, or a click that
 *   found nothing), `refused` or `detached`. It is inside a `role="status"` region.
 * @csspart refusal - The words "Item unavailable", with `data-code` set to the refusal's code if
 *   the server refused the record. For a broken pick, the details are in the map's `lastPick`.
 * @csspart reason - The server's reason for refusing the record, under "Item unavailable".
 * @csspart headline - The headline, an `<h3>`, with `data-name` set to the field it shows.
 * @csspart subtitle - The `subtitle-field` value under the headline, when `compact` is set.
 * @csspart show-all - The "Show all N fields" button, when `compact` is set, with `aria-expanded`.
 *   It reads "Show all" where every field is shown and only text is cut short.
 * @csspart view-chip - One view the item is in, with `data-view`, and `aria-current` and a bar
 *   under the current view.
 * @csspart field - One field, with `data-name`, and `data-prose` on a value over 60 characters.
 * @csspart label - A field's name, a `<dt>`, and the heading above the view chips.
 * @csspart value - A field's value, a `<dd>`.
 * @csspart more - The Show more button under a long value that is cut short, with `aria-expanded`.
 * @csspart image - A field's value drawn as an image, under `image-fields`.
 * @csspart scoped - The group-scoped values, grouped by key.
 * @csspart key - One key's heading among the group-scoped values, with `data-key`.
 * @csspart open - The Open button.
 * @csspart copy - The Copy id button, which copies the `mosaica_id` to the clipboard and says to a
 *   screen reader whether it did.
 */
export class MosaicaItemCard extends MosaicaElement {
  static override styles = [
    tokens,
    chrome,
    css`
      :host {
        display: block;
      }
      .head[part='title'] {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: 0;
        margin: 0 0 10px;
        font-size: 13px;
        font-weight: 400;
        letter-spacing: 0;
        text-transform: none;
        color: var(--_mosaica-ink);
      }
      /* The state's region is in the row whether or not it says anything, and takes room only when it does. */
      .said {
        min-width: 0;
      }
      .said.alone,
      .head [part='headline'],
      .head .headline {
        flex: 1 1 auto;
        min-width: 0;
      }
      .headline {
        display: flex;
        flex-direction: column;
        gap: 3px;
      }
      h3[part='headline'] {
        display: block;
        margin: 0;
        letter-spacing: 0;
        text-transform: none;
        color: var(--_mosaica-ink);
      }
      [part='subtitle'] {
        font-size: 12px;
        color: var(--_mosaica-ink-2);
      }
      :host([compact]) [part='title'] {
        margin-bottom: 6px;
      }
      :host([compact]) .card-title {
        font-size: 14px;
      }
      /* Text kept to a few lines until Show all, or its own Show more, shows it whole: a compact
         card's headline to three and its subtitle to two, a long value to six. */
      .clamp {
        display: -webkit-box;
        -webkit-line-clamp: 6;
        line-clamp: 6;
        -webkit-box-orient: vertical;
        overflow: hidden;
        overflow-wrap: anywhere;
      }
      h3[part='headline'].clamp {
        display: -webkit-box;
        -webkit-line-clamp: 3;
        line-clamp: 3;
      }
      [part='subtitle'].clamp {
        -webkit-line-clamp: 2;
        line-clamp: 2;
      }
      /* Each is drawn as a line of text, with a target 24 px high and at least as wide around it. */
      [part='show-all'],
      [part='more'] {
        position: relative;
      }
      [part='show-all']::before,
      [part='more']::before {
        content: '';
        position: absolute;
        inset: -4px 0;
        min-width: 24px;
      }
      dl,
      dd {
        margin: 0;
      }
      :host([compact]) .foot {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 8px;
        margin: 10px calc(-1 * var(--_mosaica-panel-inline, 14px)) 0;
        padding: 8px var(--_mosaica-panel-inline, 14px) 0;
        border-top: 1px solid var(--_mosaica-line-2);
      }
      a[part='open'] {
        color: inherit;
        text-decoration: none;
      }
      :host([compact]) [part='open'] {
        height: 24px;
        padding: 0 8px;
        font-size: 12px;
      }
      ::slotted([slot='actions']) {
        flex: none;
        margin-left: 10px;
      }
      .head [part='state'] {
        min-height: 24px;
      }
      [part='field'][data-prose],
      [part='field'][data-image] {
        grid-column: 1 / -1;
      }
      [part='image'] {
        display: block;
        max-width: 100%;
        max-height: 240px;
        margin-top: 4px;
        border-radius: 6px;
        object-fit: contain;
      }
      .field .v.mono {
        font-size: 12px;
      }
      .field .v {
        font-variant-numeric: tabular-nums;
      }
      .actions {
        margin-top: 12px;
      }
      [part='close'] {
        flex: none;
        width: 24px;
        height: 24px;
        margin: -2px -4px 0 10px;
        display: grid;
        place-items: center;
        border-radius: 5px;
        color: var(--_mosaica-ink-2);
      }
      [part='close']:hover {
        background: var(--_mosaica-surface-2);
      }
      .chips {
        display: flex;
        flex-wrap: wrap;
        gap: 6px;
        margin: 4px 0 10px;
      }
      /* The bar marks the current view by more than its colour, and takes no room. */
      [part='view-chip'][aria-current='true'] {
        background: var(--_mosaica-accent);
        color: var(--_mosaica-accent-ink);
        box-shadow: inset 0 -2px 0 currentColor;
      }
      [part='scoped'] {
        margin-top: 12px;
      }
      [part='key'] {
        margin: 10px 0 4px;
        text-transform: none;
        letter-spacing: 0;
        font-size: 12px;
      }
    `
  ];

  /** The item to show, if you fetch the record yourself. It replaces the store's selection. */
  @property({attribute: false}) accessor item: {id: bigint; detail: ItemDetail} | null = null;
  /** A refusal to show instead of an item, if you fetch the record yourself. */
  @property({attribute: false}) accessor refusal: Refusal | null = null;
  /**
   * What the map's last click found (`<mosaica-map>`'s `lastPick`), shown while no item is
   * selected. A miss shows "No item here" and a broken pick shows "Item unavailable".
   */
  @property({attribute: false}) accessor pick: PickOutcome = null;
  /** The server's `/v1/meta`, for the field order and view names, if the card has no store. */
  @property({attribute: false}) accessor meta: Meta | null = null;
  /** The field the headline shows. Unset, or if the item has no value for it, the headline is the `mosaica_id`. */
  @property({attribute: 'title-field'}) accessor titleField = '';
  /** Shows a shorter card: the headline, the subtitle and three fields, with the rest under "Show all N fields". */
  @property({type: Boolean, reflect: true}) accessor compact = false;
  /** The field shown under the headline when `compact` is set. Unset, there is no subtitle. */
  @property({attribute: 'subtitle-field'}) accessor subtitleField = '';
  /**
   * The fields a `compact` card shows before "Show all", separated by spaces or commas, in order.
   * Unset, the first three. The full list shows these fields first, then the rest.
   */
  @property({attribute: 'card-fields'}) accessor cardFields = '';
  /**
   * The fields whose value is the address of an image, separated by spaces or commas. Each is drawn
   * as its image rather than as text.
   */
  @property({attribute: 'image-fields'}) accessor imageFields = '';
  /**
   * The link template for Open, with `{name}` for the value of the field `name` and `{mosaica_id}`
   * for the item's id. Without it or `open`, there is no Open button.
   */
  @property({attribute: 'open-url'}) accessor openUrl = '';
  /** A function called with the item's `mosaica_id` and fields when Open is pressed. Takes precedence over `open-url`. */
  @property({attribute: false}) accessor open: ((item: MosaicaEventDetails['mosaica-open']) => void) | null = null;
  /** Whether "Show all" was pressed, for the item it was pressed on. @internal */
  @state() accessor expanded = false;
  private expandedFor: bigint | null = null;
  /** The long values shown whole on the full card, by field, for the item shown. @internal */
  @state() accessor opened: ReadonlySet<string> = new Set();
  /** The fields whose headline, subtitle or value is cut short where it is drawn. @internal */
  @state() accessor clipped: ReadonlySet<string> = new Set();
  /** What the last press of Copy id came to, for the item shown. @internal */
  @state() accessor copied = '';
  private resized: ResizeObserver | null = null;
  /** The image addresses that failed to load, shown as text. @internal */
  @state() accessor broken: ReadonlySet<string> = new Set();
  /** `image-fields` as a list, kept in step by `willUpdate`. */
  private imageNames: string[] = [];
  /** A category value's title as the store holds it, else from the legend or a suggestion page, else its key. */
  private titleOf(field: string, key: string): string {
    return this.resolvedStore?.heldTitle(field, key) ?? keyTitle(this.resolvedStore, field, key);
  }

  /** A field's value as text: a category value by its title, anything else by its declared type. */
  private text(name: string, value: unknown, column: DeclaredScalar | undefined): string {
    return column?.category && typeof value === 'string' ? this.titleOf(name, value) : present(value, column);
  }

  /**
   * Another item starts folded again, and its category values are what the titles are asked for;
   * set before the render, so the render changes nothing it reads.
   */
  protected override willUpdate(changed: PropertyValues<this>): void {
    super.willUpdate(changed);
    if (changed.has('imageFields')) this.imageNames = listed(this.imageFields);
    const {item, meta} = this.shown;
    const id = item?.id ?? null;
    if (id !== this.expandedFor) {
      this.expandedFor = id;
      this.expanded = false;
      this.opened = new Set();
      this.clipped = new Set();
      this.copied = '';
    }
    const store = this.resolvedStore;
    if (!store || !store.get('meta') || !meta || !this.isConnected) return;
    // Ask for each category value the store holds no title for. The store holds an answer only
    // under the `meta` it was asked under; a refused ask holds nothing, so it is made again at the
    // next render.
    const values = item?.detail.fields ?? {};
    for (const column of meta.declaredScalars) {
      const key = values[column.name];
      if (!column.category || typeof key !== 'string' || store.heldTitle(column.name, key) !== undefined) continue;
      void store.titleOf(column.name, key).then((title) => {
        if (title !== null) this.requestUpdate();
      });
    }
  }

  override connectedCallback(): void {
    super.connectedCallback();
    if (typeof ResizeObserver === 'undefined') return;
    this.resized ??= new ResizeObserver(() => this.measure());
    this.resized.observe(this);
  }

  override disconnectedCallback(): void {
    this.resized?.disconnect();
    super.disconnectedCallback();
  }

  protected override updated(changed: PropertyValues<this>): void {
    super.updated(changed);
    this.measure();
  }

  /**
   * Note which clamped texts are cut short at the card's width, so each is offered whole. A text
   * shown whole keeps what was noted while it was clamped, so the button that shows it stays.
   */
  private measure(): void {
    const clipped = new Set(this.clipped);
    for (const el of Array.from(this.renderRoot.querySelectorAll<HTMLElement>('.clamp'))) {
      const name = el.closest<HTMLElement>('[data-name]')?.dataset.name;
      if (name === undefined) continue;
      if (el.scrollHeight > el.clientHeight + 1) clipped.add(name);
      else clipped.delete(name);
    }
    if (clipped.size !== this.clipped.size || [...clipped].some((name) => !this.clipped.has(name))) this.clipped = clipped;
  }

  /** Copy the id to the clipboard and say in the status region how that went. */
  private async copy(id: string): Promise<void> {
    const shown = this.expandedFor;
    this.copied = '';
    await this.updateComplete;
    let copied = false;
    try {
      await navigator.clipboard.writeText(id);
      copied = true;
    } catch {
      // No clipboard on a page that is not secure, or the browser refused.
    }
    if (shown === this.expandedFor) this.copied = copied ? 'Copied' : 'Could not copy';
  }

  private get shown(): {item: {id: bigint; detail: ItemDetail} | null; refusal: Refusal | null; meta: Meta | null} {
    // A card fed by property still takes the schema from an adopted store: declaration order and
    // view names and frames belong to the bundle.
    if (this.item || this.refusal) return {item: this.item, refusal: this.refusal, meta: this.meta ?? this.resolvedStore?.get('meta') ?? null};
    const s = this.resolvedStore;
    if (!s) return {item: null, refusal: null, meta: this.meta};
    const sel = s.get('selection');
    return {item: sel.item, refusal: sel.itemRefusal, meta: this.meta ?? s.get('meta')};
  }

  override render(): TemplateResult | typeof nothing {
    const {item, refusal, meta} = this.shown;
    // The close button appears only where there is something to close.
    let closable = true;
    let state: unknown = html`<span part="state" data-state="shown"></span>`;
    if (refusal) state = refusedState('Item unavailable', refusal.code, null, reasonOf(refusal));
    else if (!item) {
      const pick = this.pick;
      // A broken pick is a fault in the layer; its details stay on the map's `lastPick`.
      if (pick?.kind === 'broken') state = refusedState('Item unavailable', null, null, null);
      else if (pick?.kind === 'miss') state = html`<span part="state" data-state="empty">No item here</span>`;
      else {
        closable = false;
        state = stateOf(this.resolvedStore?.get('status')) === 'detached' ? renderState('detached', null) : html`<span part="state" data-state="empty">No item selected</span>`;
      }
    }
    const card = item && !refusal ? this.card(item, meta) : null;
    // One template whatever the card shows, so the state's region is in the page before its words change.
    return html`<div class="panel">
      <div part="title" class="head">
        <h2 class="sr-only">Item</h2>
        <div class=${card ? 'said' : 'said alone'} role="status">${state}</div>
        ${card?.headline ?? nothing}
        ${closable
          ? html`<slot name="actions"></slot><button part="close" type="button" aria-label="Close" @click=${() => emit(this, 'mosaica-close', {what: 'item'})}>${icon('close', 14)}</button>`
          : nothing}
      </div>
      ${card?.body ?? nothing}
    </div>`;
  }

  /** An item's headline, for the header row, and what the card shows under it: the full card's, or under `compact` the shorter one's. */
  private card(item: {id: bigint; detail: ItemDetail}, meta: Meta | null): {headline: TemplateResult; body: TemplateResult} {
    const declared = meta?.declaredScalars ?? [];
    const column = (name: string) => declared.find((c) => c.name === name);
    const {fields} = item.detail;
    const names = Object.keys(fields);
    const ordered = [...declared.map((c) => c.name).filter((n) => n in fields), ...names.filter((n) => !declared.some((c) => c.name === n))];
    const id = idString(item.id);
    const has = (name: string) => fields[name] !== undefined && fields[name] !== null;
    const titleName = this.titleField && has(this.titleField) ? this.titleField : null;
    const title = titleName ? this.text(titleName, fields[titleName], column(titleName)) : null;
    const rest = this.namedFirst(ordered.filter((n) => n !== titleName));
    const whole = !this.compact || this.expanded;
    const h3 = titleName
      ? html`<h3 part="headline" class=${whole ? 'card-title' : 'card-title clamp'} data-name=${titleName}><slot name=${`field-${titleName}`}><span part="value">${title}</span></slot></h3>`
      : html`<h3 part="headline" class="card-title mono" data-name="mosaica_id">${id}</h3>`;
    const idField = titleName
      ? html`<div part="field" data-name="mosaica_id" style="display:contents"><dt part="label" class="k">mosaica_id</dt><dd part="value" class="v mono">${id}</dd></div>`
      : nothing;
    const field = (name: string) => this.field(name, fields[name], column(name), title);
    const open = this.openButton(id, fields, this.compact ? nothing : icon('open', 14));
    if (!this.compact) {
      return {
        headline: h3,
        body: html`${this.views(item.detail.views, meta)}
          <dl class="field">${rest.map(field)}${idField}</dl>
          ${this.scoped(item.detail.scoped)}
          <div class="row actions">
            ${open}
            <button part="copy" class="btn" type="button" @click=${() => void this.copy(id)}>Copy id</button>
            <span class="sr-only" role="status">${this.copied}</span>
          </div>`
      };
    }
    const subtitleName = this.subtitleField && this.subtitleField !== titleName && has(this.subtitleField) ? this.subtitleField : null;
    const others = rest.filter((n) => n !== subtitleName);
    const total = others.length + (titleName ? 1 : 0);
    const few = this.cardFields ? this.named(others) : others.slice(0, 3);
    const more = total > few.length;
    // Show all is there for the fields left out, and for any text the card cuts short.
    const showAll =
      more || this.expanded || this.clipped.size > 0
        ? html`<button part="show-all" class="more-link" type="button" aria-expanded=${this.expanded ? 'true' : 'false'} @click=${() => (this.expanded = !this.expanded)}>${this.expanded ? (more ? 'Show fewer' : 'Show less') : more ? `Show all ${total.toLocaleString('en-GB')} fields` : 'Show all'}</button>`
        : null;
    const subtitle = subtitleName
      ? html`<div part="subtitle" class=${whole ? '' : 'clamp'} data-name=${subtitleName}><slot name=${`field-${subtitleName}`}>${this.text(subtitleName, fields[subtitleName], column(subtitleName))}</slot></div>`
      : nothing;
    return {
      headline: html`<div class="headline">${h3}${subtitle}</div>`,
      body: html`<div class="body">
          ${this.expanded ? this.views(item.detail.views, meta) : nothing}
          <dl class="field">${(this.expanded ? others : few).map(field)}${this.expanded ? idField : nothing}</dl>
          ${this.expanded ? this.scoped(item.detail.scoped) : nothing}
        </div>
        ${showAll || open !== nothing ? html`<div class="foot">${showAll || html`<span></span>`}${open}</div>` : nothing}`
    };
  }

  /**
   * Open, where the host says what it does: a button that calls `open`, else a link to `open-url`
   * filled from the item, which opens in a new tab and says so to a screen reader, else nothing.
   * Each fires `mosaica-open`. `mark` is drawn before the word.
   */
  private openButton(id: string, fields: Record<string, unknown>, mark: TemplateResult | typeof nothing): TemplateResult | typeof nothing {
    const pressed = () => emit(this, 'mosaica-open', {id, fields});
    const open = this.open;
    if (open)
      return html`<button part="open" class="btn" type="button" @click=${() => {
        pressed();
        open({id, fields});
      }}>${mark}Open</button>`;
    const href = this.openUrl ? openAddress(this.openUrl, {mosaica_id: id, ...fields}) : null;
    if (!href) return nothing;
    // A middle-click opens the link too, and is a press.
    return html`<a part="open" class="btn" href=${href} target="_blank" rel="noopener noreferrer" @click=${pressed} @auxclick=${(e: MouseEvent) => e.button === 1 && pressed()}>${mark}Open<span class="sr-only"> (opens in a new tab)</span></a>`;
  }

  /** The fields of `fields` that `card-fields` names, in its order. */
  private named(fields: string[]): string[] {
    const names = listed(this.cardFields);
    return names.filter((n, i) => fields.includes(n) && names.indexOf(n) === i);
  }

  /** The address of the image a field's value names, where `image-fields` lists the field. */
  private imageOf(name: string, value: unknown): string | null {
    if (typeof value !== 'string' || !this.imageNames.includes(name)) return null;
    const address = webAddress(value);
    return address && !this.broken.has(address) ? address : null;
  }

  /** `fields` with those `card-fields` names first, in its order. */
  private namedFirst(fields: string[]): string[] {
    const named = this.named(fields);
    return [...named, ...fields.filter((n) => !named.includes(n))];
  }

  /**
   * The views this item is in that this session may reach, as chips under the title with the
   * current one marked. Clicking another follows the item there; the detail already holds its
   * position, so no request is made. An empty array draws nothing: the server does not distinguish
   * "in no view" from "in no view you can reach".
   */
  private views(positions: ItemViewPosition[], meta: Meta | null) {
    if (positions.length === 0) return nothing;
    const current = this.resolvedStore?.get('view').id ?? '';
    return html`<span part="label" class="xs muted" id="views">In views</span>
      <div class="chips" role="group" aria-labelledby="views">
        ${positions.map((p) => {
          const view = meta?.views.find((v) => v.id === p.id) ?? null;
          const here = p.id === current;
          return html`<button part="view-chip" class="chip" type="button" data-view=${p.id} aria-current=${here ? 'true' : 'false'} @click=${() => this.follow(p, view?.quantisation ?? null)}>
            ${view?.displayName ?? p.id}
          </button>`;
        })}
      </div>`;
  }

  /**
   * Follow the item into another view: emit `mosaica-viewfollow` with the position dequantised
   * under that view's frame, in data coordinates.
   */
  private follow(position: ItemViewPosition, frame: Quantisation | null): void {
    if (!frame) return;
    emit(this, 'mosaica-viewfollow', {
      view: position.id,
      x: frame.xMin + (position.x / GRID32) * (frame.xMax - frame.xMin),
      y: frame.yMin + (position.y / GRID32) * (frame.yMax - frame.yMin)
    });
  }

  /**
   * The group-scoped attribute values, headed by key in the order served. Two views that share a
   * key through a `members` group share a heading.
   */
  private scoped(scoped: Record<string, Record<string, unknown>>) {
    const keys: string[] = [];
    for (const family of Object.keys(scoped)) for (const key of Object.keys(scoped[family] ?? {})) if (!keys.includes(key)) keys.push(key);
    if (keys.length === 0) return nothing;
    return html`<div part="scoped">
      ${keys.map(
        (key) => html`<h4 part="key" class="hd" data-key=${key}>${key}</h4>
          <dl class="field" data-key=${key}>
            ${Object.keys(scoped)
              .filter((family) => key in (scoped[family] ?? {}))
              .map((family) => html`<div part="field" data-name=${family} data-key=${key} style="display:contents"><dt part="label" class="k">${columnCaption(family)}</dt><dd part="value" class="v">${present(scoped[family]![key], undefined)}</dd></div>`)}
          </dl>`
      )}
    </div>`;
  }

  /**
   * One field, presented by its declared type; prose and images span the grid. An image's
   * alternative text is the item's `headline` where it has one, since its caption is the label
   * above it. On the full card, prose a clamp cuts short has a button that shows it whole.
   */
  private field(name: string, value: unknown, column: DeclaredScalar | undefined, headline: string | null) {
    const caption = column ? columnCaption(name) : name;
    const image = this.imageOf(name, value);
    if (image) {
      const failed = () => (this.broken = new Set([...this.broken, image]));
      return html`<div part="field" data-name=${name} data-image>
        <dt part="label" class="k">${caption}</dt>
        <dd part="value" class="v"><slot name=${`field-${name}`}><img part="image" src=${image} alt=${headline ?? caption} loading="lazy" referrerpolicy="no-referrer" @error=${failed} /></slot></dd>
      </div>`;
    }
    const text = this.text(name, value, column);
    const prose = text.length > 60;
    const whole = this.compact ? this.expanded : this.opened.has(name);
    const toggle = () => (this.opened = new Set(whole ? [...this.opened].filter((n) => n !== name) : [...this.opened, name]));
    return html`<div part="field" data-name=${name} ?data-prose=${prose} style=${prose ? nothing : 'display:contents'}>
      <dt part="label" class="k">${caption}</dt>
      <dd part="value" class="v" title=${prose ? text : nothing}><slot name=${`field-${name}`}>${prose && !whole ? html`<span class="clamp">${text}</span>` : text}</slot></dd>
      ${prose && !this.compact && (whole || this.clipped.has(name))
        ? html`<dd><button part="more" class="more-link" type="button" aria-expanded=${whole ? 'true' : 'false'} aria-label=${`${whole ? 'Show less' : 'Show more'} of ${caption}`} @click=${toggle}>${whole ? 'Show less' : 'Show more'}</button></dd>`
        : nothing}
    </div>`;
  }
}

/**
 * `template` with each `{name}` replaced by `values[name]`, URL-encoded, and resolved against the
 * page; `null` where a name has no value or an empty one, a value cannot be encoded, or the address
 * is not `http:` or `https:`.
 */
function openAddress(template: string, values: Record<string, unknown>): string | null {
  let missing = false;
  try {
    const filled = template.replace(/\{([^{}]+)\}/g, (_, name: string) => {
      const value = Object.hasOwn(values, name) ? values[name] : undefined;
      if (value === undefined || value === null || value === '') missing = true;
      return encodeURIComponent(String(value));
    });
    return missing ? null : webAddress(filled);
  } catch {
    return null;
  }
}

/** `text` resolved against the page, where it is an `http:` or `https:` address. */
function webAddress(text: string): string | null {
  try {
    const url = new URL(text, document.baseURI);
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : null;
  } catch {
    return null;
  }
}

/** The names in a list separated by spaces or commas. */
function listed(names: string): string[] {
  return names.split(/[\s,]+/).filter(Boolean);
}

/** A value as text, by the column's declared type. */
function present(value: unknown, column: DeclaredScalar | undefined): string {
  if (value === null || value === undefined) return '—';
  if (column?.arrowType === 'timestamp_us' && (typeof value === 'number' || typeof value === 'bigint')) return timestampText(value);
  if (typeof value === 'number') return Number.isInteger(value) ? value.toLocaleString('en-GB') : String(value);
  if (typeof value === 'boolean') return BOOL_WORDS[value ? 'true' : 'false'];
  return String(value);
}

attachContextRoot();
defineOnce('mosaica-item-card', MosaicaItemCard);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-item-card': MosaicaItemCard;
  }
}
