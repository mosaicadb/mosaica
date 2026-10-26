import {ContextProvider} from '@lit/context';
import {css, html, nothing, type TemplateResult} from 'lit';
import type {Store} from '@mosaicajs/client';
import {MosaicaElement} from './base.js';
import {storeContext} from './context.js';
import {attachContextRoot, defineOnce} from './define.js';

/**
 * `<mosaica-store>` holds one store for the elements inside it, so that a map, a filter panel and
 * an item card you lay out yourself all show the same view and filters. The elements find the store
 * through context, which means you do not pass it to each one. You need one wrapper for each map of
 * a different view, and none inside `<mosaica-explorer>`, which provides its own store.
 *
 * The wrapper uses the store in its `store` property, or builds one from `viewer-url` and `token`
 * or `authorise` when that property is not set. It draws nothing itself, because its display is
 * `contents`.
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
