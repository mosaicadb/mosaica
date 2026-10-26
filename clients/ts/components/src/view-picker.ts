import {css, html, nothing, type PropertyValues, type TemplateResult} from 'lit';
import {type Meta} from '@mosaicajs/client';
import {enterGroup, hasOneLayout, viewPickerEntries} from '@mosaicajs/client/internal';
import {MosaicaElement} from './base.js';
import {attachContextRoot, defineOnce} from './define.js';
import {icon} from './icons.js';
import {switchView} from './view-switch.js';
import {chrome, tokens} from './tokens.js';

/**
 * The view picker is a menu for switching between the views a viewer may see, for a bundle that
 * lays its items out in more than one way. It lists each view that belongs to no group, then each
 * group as a single entry, and shows nothing when there is only one entry to choose.
 *
 * Choosing a group opens the view the viewer last left that group on, or the group's first view if
 * they have not been in it. When the current view's group shares keys with the chosen group, the
 * picker opens the view under the current key instead, so the viewer stays on the same year or
 * quarter. To step between the views inside a group, use `<mosaica-key-picker>`.
 *
 * @summary A drop-down menu for switching views.
 * @tagname mosaica-view-picker
 * @category Elements
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-viewswitch']>} mosaica-viewswitch - The view
 *   changed.
 * @csspart field - The select and its chevron, shown as the view's title in bold.
 * @csspart select - The select.
 */
export class MosaicaViewPicker extends MosaicaElement {
  static override styles = [
    tokens,
    chrome,
    css`
      :host {
        display: block;
      }
      [part='field'] {
        font-size: 15px;
        font-weight: 600;
        line-height: 1.3;
      }
    `
  ];

  /** The key this element last left each group on, which `enterGroup` prefers. */
  private lastKey = new Map<string, string>();
  private chosen = '';

  /**
   * Writes the chosen option to the select after its options exist. Toggling `selected` on a
   * re-render does not move a select whose value has been set once.
   */
  protected override updated(_changed: PropertyValues<this>): void {
    const select = this.renderRoot.querySelector('select');
    if (select && select.value !== this.chosen) select.value = this.chosen;
  }

  private choose(meta: Meta, value: string): void {
    const s = this.resolvedStore;
    if (!s) return;
    const currentId = s.get('view').id;
    // Leaving a group records the key it was left on.
    const roster = meta.views.find((v) => v.id === currentId)?.roster ?? null;
    if (roster) this.lastKey.set(roster.group, roster.key);
    const group = value.startsWith('g:') ? value.slice(2) : null;
    const target = group === null ? value.slice(2) : enterGroup(meta, group, currentId, this.lastKey.get(group));
    // A choice that switches nothing (a group with no view this session may reach, or the view
    // already current) puts the select back to the layout the map is drawing.
    if (target === null || target === currentId) {
      this.restoreSelect();
      return;
    }
    switchView(this, s, meta, target);
  }

  /** Put the select back to the entry the store is in. */
  private restoreSelect(): void {
    const select = this.renderRoot.querySelector('select');
    if (select && select.value !== this.chosen) select.value = this.chosen;
  }

  override render(): TemplateResult | typeof nothing {
    const s = this.resolvedStore;
    const meta = s?.get('meta') ?? null;
    if (!s || !meta || hasOneLayout(meta)) return nothing;
    const entries = viewPickerEntries(meta, s.get('view').id);
    const value = (entry: {kind: 'view' | 'group'; id: string}) => `${entry.kind === 'group' ? 'g' : 'v'}:${entry.id}`;
    const current = entries.find((e) => e.current);
    this.chosen = current ? value(current) : '';
    return html`<div part="field" class="choice">
      <select part="select" aria-label="View" @change=${(e: Event) => this.choose(meta, (e.target as HTMLSelectElement).value)}>
        ${entries.map(
          (entry) => html`<option value=${value(entry)} data-kind=${entry.kind} ?selected=${entry.current}>${entry.text}</option>`
        )}
      </select>${icon('chev', 14, 1.4)}
    </div>`;
  }
}

attachContextRoot();
defineOnce('mosaica-view-picker', MosaicaViewPicker);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-view-picker': MosaicaViewPicker;
  }
}
