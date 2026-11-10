import {LitElement, css, html} from 'lit';
import '../../src/map.js';
import {chrome, tokens} from '../../src/tokens.js';

// Built by `forced-colours.browser.ts` with Vite from the packages' sources. The shared theme on
// controls that are pressed, checked, selected or chosen, each drawing its focus ring the way an
// element does: a menu item and a row's own button on `:focus-visible`, inside or outside the
// control's edge, and a suggestion the arrow keys reached, ringed while a box keeps the focus. A
// bare map stands beside them for its pressed tool.
class ThemeProbe extends LitElement {
  static override styles = [
    tokens,
    chrome,
    css`
      :host {
        display: flex;
        flex-direction: column;
        align-items: flex-start;
        gap: 12px;
        padding: 12px;
      }
      .inset:focus-visible {
        outline-offset: -2px;
      }
      .reached {
        outline: 2px solid var(--_mosaica-focus);
        outline-offset: -2px;
      }
      @media (forced-colors: active) {
        .reached {
          outline-color: Highlight;
        }
      }
    `
  ];

  override render() {
    return html`<button id="checked" class="btn inset" role="menuitemradio" aria-checked="true">Checked, ringed inside</button>
      <button id="pressed" class="btn" aria-pressed="true">Pressed, ringed outside</button>
      <div class="list"><div id="row" class="item on"><button id="inner" class="btn small">A button in a chosen row</button></div></div>
      <button id="plain" class="btn inset">Neither</button>
      <div id="reached" class="reached" data-active role="option" aria-selected="true">Selected, and reached by the arrow keys</div>
      <div id="passed" class="reached" data-active role="option" aria-selected="false">Reached by the arrow keys</div>`;
  }
}
customElements.define('theme-probe', ThemeProbe);

document.body.innerHTML = '<theme-probe></theme-probe><mosaica-map style="display:block;width:300px;height:200px"></mosaica-map>';
await (document.querySelector('theme-probe') as ThemeProbe).updateComplete;
await (document.querySelector('mosaica-map') as unknown as {updateComplete: Promise<unknown>}).updateComplete;
(window as unknown as {ready: boolean}).ready = true;
