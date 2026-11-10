import {css, html, nothing, type TemplateResult} from 'lit';
import {isFilterLayer, layerEntries, type LayerEntry} from '@mosaicajs/client';
import {MosaicaElement, emit} from './base.js';
import {attachContextRoot, defineOnce} from './define.js';
import {renderState, stateOf} from './states.js';
import {chrome, tokens} from './tokens.js';

/**
 * Put the layer picker beside a map when the bundle has annotation layers that viewers should be
 * able to turn on and off. A layer that others depend on, such as a clustering and its labels, has
 * one checkbox, so that the whole group is turned on together.
 *
 * Filter layers draw nothing, so the picker names them in a note instead of giving them checkboxes.
 * Viewers filter by a filter layer from its card in `<mosaica-filter-panel>`, or browse it in
 * `<mosaica-hierarchy>` when its artifacts form a hierarchy.
 *
 * @summary A list of checkboxes for choosing which annotation layers the map draws.
 * @tagname mosaica-layer-picker
 * @category Elements
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-layerchange']>} mosaica-layerchange - A checkbox
 *   changed which layers are drawn.
 * @csspart title - The heading.
 * @csspart state - The state line, with `data-state`.
 * @csspart refusal - The words "View refused", with `data-code`, in the refused state.
 * @csspart reason - The server's reason for a refusal, under "View refused".
 * @csspart entry - One layer that draws, with `data-layer`.
 * @csspart name - A layer's title, or its name if it has no title.
 * @csspart linked - Under a layer's name, the layers drawn with it, such as "with Labels".
 * @csspart note - The note naming the filter layers, such as "Venues can be used as a filter
 *   only.", with `data-layers` listing their names.
 */
export class MosaicaLayerPicker extends MosaicaElement {
  static override styles = [
    tokens,
    chrome,
    css`
      :host {
        display: block;
      }
      [part='entry'] {
        display: flex;
        align-items: center;
        gap: 8px;
        min-height: 28px;
        cursor: pointer;
      }
      [part='note'] {
        margin: 8px 0 0;
        font-size: 12px;
        color: var(--_mosaica-ink-3);
      }
      [part='entry'] .col {
        min-width: 0;
        padding: 4px 0;
      }
      [part='name'] {
        overflow-wrap: anywhere;
      }
      [part='linked'] {
        font-size: 12px;
        color: var(--_mosaica-ink-2);
        overflow-wrap: anywhere;
      }
    `
  ];

  private entries(): LayerEntry[] {
    return layerEntries(this.resolvedStore?.get('meta')?.layers ?? []);
  }

  /** The entries that draw, and the filter layers, in `meta` order within each. */
  private split(): {drawn: LayerEntry[]; filters: LayerEntry[]} {
    const entries = this.entries();
    return {
      drawn: entries.filter((e) => !isFilterLayer(e.root)),
      filters: entries.filter((e) => isFilterLayer(e.root))
    };
  }

  private toggle(entry: LayerEntry, on: boolean): void {
    const s = this.resolvedStore;
    if (!s) return;
    const current = new Set(s.get('artifacts').layers);
    const roots = this.entries().filter((e) => (e.root.name === entry.root.name ? on : current.has(e.root.name))).map((e) => e.root.name);
    s.setLayers(roots);
    emit(this, 'mosaica-layerchange', {layers: roots});
  }

  override render(): TemplateResult | typeof nothing {
    const s = this.resolvedStore;
    const heading = html`<h2 part="title">Layers</h2>`;
    const meta = s?.get('meta') ?? null;
    if (!s || !meta) return html`<div class="panel">${heading}${renderState(stateOf(s?.get('status')), s?.get('status'))}</div>`;
    const {drawn, filters} = this.split();
    if (drawn.length + filters.length === 0) return html`<div class="panel">${heading}<span part="state" data-state="empty">No layers</span></div>`;
    const on = new Set(s.get('artifacts').layers);
    const titled = (name: string) => meta.layers.find((l) => l.name === name)?.title || name;
    return html`<div class="panel">${heading}
      <span part="state" data-state="shown"></span>
      <div class="col">
        ${drawn.map(
          (e) => html`<label part="entry" class="check" data-layer=${e.root.name}>
            <input type="checkbox" .checked=${on.has(e.root.name)} @change=${(ev: Event) => this.toggle(e, (ev.target as HTMLInputElement).checked)} />
            <span class="col">
              <span part="name">${e.root.title || e.root.name}</span>
              ${e.closure.length > 1 ? html`<span part="linked" data-layers=${linkedTo(e).join(' ')}>with ${linkedTo(e).map(titled).join(', ')}</span>` : nothing}
            </span>
          </label>`
        )}
      </div>
      ${filters.length > 0
        ? html`<p part="note" data-layers=${filters.map((e) => e.root.name).join(' ')}>${filterNote(filters.map((e) => e.root.title || e.root.name))}</p>`
        : nothing}
    </div>`;
  }
}

/** The layers an entry draws with its own. */
function linkedTo(entry: LayerEntry): string[] {
  return entry.closure.filter((name) => name !== entry.root.name);
}

/** "Venues can be used as a filter only.", or for several, "Venues and Countries can be used as filters only." */
function filterNote(titles: string[]): string {
  if (titles.length === 1) return `${titles[0]} can be used as a filter only.`;
  return `${titles.slice(0, -1).join(', ')} and ${titles.at(-1)} can be used as filters only.`;
}

attachContextRoot();
defineOnce('mosaica-layer-picker', MosaicaLayerPicker);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-layer-picker': MosaicaLayerPicker;
  }
}
