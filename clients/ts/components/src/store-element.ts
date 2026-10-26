import {ContextProvider} from '@lit/context';
import {css, html, nothing, type TemplateResult} from 'lit';
import type {Store} from '@mosaicajs/client';
import {MosaicaElement} from './base.js';
import {storeContext} from './context.js';
import {attachContextRoot, defineOnce} from './define.js';

/**
 * Use `<mosaica-store>` when you lay out the elements yourself, so that the map, the filter panel
 * and the cards all show the same view and filters. The elements inside it find its store through
 * context, which means you do not pass the store to each one. Put each map of a different view in
 * its own wrapper. `<mosaica-explorer>` provides its own store, so it needs no wrapper.
 *
 * The wrapper uses the store in its `store` property. When that is not set, it builds one from
 * `viewer-url` and `token` or `authorise`.
 *
 * @summary A wrapper that shares one store between the elements inside it.
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
