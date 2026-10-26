import {ContextProvider} from '@lit/context';
import {css, html, nothing, type TemplateResult} from 'lit';
import type {Store} from '@mosaicajs/client';
import {MosaicaElement} from './base.js';
import {storeContext} from './context.js';
import {attachContextRoot, defineOnce} from './define.js';

/**
 * Use `<mosaica-store>` to share one store among elements you lay out yourself, such as a map, a
 * filter panel and an item card. The elements inside it find the store through context. For two
 * maps of different views side by side, use two `<mosaica-store>` elements. `<mosaica-explorer>`
 * provides its own store and does not need one.
 *
 * It uses the store in its `store` property, or builds one from `viewer-url` and `token` or
 * `authorise`. It renders only its children (`display: contents`).
 *
 * @summary Shares one store with the elements inside it.
 * @tagname mosaica-store
 * @category Elements
 * @slot - The elements that use the store.
 */
export class MosaicaStore extends MosaicaElement {
  static override styles = css`
    :host {
      display: contents;
    }
  `;

  protected override canBuildOwn = true;
  private provider = new ContextProvider(this, {context: storeContext, initialValue: null});

  protected override onStoreAdopted(store: Store | null): void {
    this.provider.setValue(store);
  }

  override render(): TemplateResult | typeof nothing {
    return html`<slot></slot>`;
  }
}

attachContextRoot();
defineOnce('mosaica-store', MosaicaStore);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-store': MosaicaStore;
  }
}
