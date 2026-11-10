import {css, html, nothing, type ReactiveController, type ReactiveElement, type TemplateResult} from 'lit';
import {hexOf, lighter, rgbOfHex} from '@mosaicajs/deck/internal';
import {hsvOf, rgbOfHsv, type Hsv} from './hsv.js';
import {icon} from './icons.js';

/** The width of the hue knob, which its travel along the bar allows for. */
const HUE_KNOB = 12;
/** How far each lighter colour is taken towards white. */
const LIGHTER = 0.45;
/** The picker's width before it is measured. */
const WIDTH = 264;

/** What the picker changes the colour of. */
export type PickerTarget = {
  /** The name the picker is headed and labelled with. */
  title: string;
  /** The palette whose colours, and a lighter row of each, the picker offers. */
  palette: {title: string; colours: readonly (readonly number[])[]};
  /** The colour Reset gives back, `#rrggbb`. */
  own: string;
  /** The colour drawn now, `#rrggbb`. */
  current: () => string;
  /** Give the target `hex`, or its own colour back where `hex` is null. `final` is false while a custom colour is dragged. */
  apply: (hex: string | null, final: boolean) => void;
  /** Note what the target is coloured now, returning what puts it back, for a drag given up before it ends. */
  hold: () => () => void;
};

/** An element the picker renders inside. */
type Host = ReactiveElement & {renderRoot: HTMLElement | DocumentFragment};

/**
 * The colour picker a swatch opens: the palette's colours and a lighter row of them, a custom area
 * with a saturation and brightness square, a hue bar and a hex field, and Reset. A choice applies
 * at once. It is shown in the top layer beside the rectangle `beside` gives, level with the swatch
 * it was opened from, or under or over that swatch's row where the window has no room beside, and
 * focus goes into it; Escape, Tab past either end of it, or a press outside
 * it and its swatch, closes it, the keys giving focus back to the swatch. A custom colour shows on
 * the map as it is dragged and is reported as it is let go; a drag cancelled, or ended by closing
 * the picker, puts back the colour from before it. Text in the hex field that is not a colour is
 * kept and marked invalid, and the line under the field says what to write.
 *
 * The host renders {@link render} where the picker belongs in its tree and adds
 * {@link pickerStyles} to its styles. The parts are the host's: `colour-popover`, `choice`, `sv`,
 * `hue`, `hex` and `reset`.
 */
export class ColourPicker implements ReactiveController {
  private picking: PickerTarget | null = null;
  private hsv: Hsv = [0, 0, 0];
  /** The swatch the picker was opened from, which it is placed beside and gives focus back to. */
  private from: HTMLElement | null = null;
  /** Whether focus goes into the picker at the next update, as it opens. */
  private focusNext = false;
  /** A colour waiting for the next frame, while a custom colour is dragged. */
  private live: {hex: string; frame: number} | null = null;
  /** What puts back the colour from before a drag, while one shows a colour not yet reported. */
  private restore: (() => void) | null = null;
  /** What was typed in the hex field, while it is not a colour. */
  private typed: string | null = null;

  constructor(
    private readonly host: Host,
    private readonly beside: () => DOMRect
  ) {
    host.addController(this);
  }

  open(from: HTMLElement, target: PickerTarget): void {
    this.from = from;
    this.hsv = hsvOf(rgbOfHex(target.current()) ?? [0, 0, 0]);
    this.picking = target;
    this.typed = null;
    this.focusNext = true;
    document.addEventListener('pointerdown', this.onOutside, true);
    this.host.requestUpdate();
  }

  close(refocus: boolean): void {
    this.abandon();
    if (this.picking === null) return;
    this.picking = null;
    document.removeEventListener('pointerdown', this.onOutside, true);
    if (refocus) this.from?.focus();
    this.host.requestUpdate();
  }

  hostDisconnected(): void {
    this.close(false);
  }

  hostUpdated(): void {
    this.place();
  }

  /** A press outside the picker and its swatch closes it. */
  private onOutside = (e: PointerEvent): void => {
    const path = e.composedPath();
    const pop = this.host.renderRoot.querySelector('.pop');
    if ((pop && path.includes(pop)) || (this.from && path.includes(this.from))) return;
    this.close(false);
  };

  /** Give up a drag that has not ended: put back the colour from before it. */
  private abandon(): void {
    if (this.live) cancelAnimationFrame(this.live.frame);
    this.live = null;
    const restore = this.restore;
    this.restore = null;
    restore?.();
  }

  /** {@link PickerTarget.apply} for a drag: at most once a frame, and the final colour at once. */
  private applyLive(hex: string, final: boolean): void {
    if (this.live) cancelAnimationFrame(this.live.frame);
    this.live = null;
    const target = this.picking;
    if (!target) return;
    if (final) {
      this.restore = null;
      target.apply(hex, true);
      return;
    }
    this.restore ??= target.hold();
    if (typeof requestAnimationFrame === 'undefined') {
      target.apply(hex, false);
      return;
    }
    this.live = {
      hex,
      frame: requestAnimationFrame(() => {
        this.live = null;
        target.apply(hex, false);
      })
    };
  }

  private choose(hex: string | null): void {
    const target = this.picking;
    if (!target) return;
    this.hsv = hsvOf(rgbOfHex(hex ?? target.own) ?? [0, 0, 0]);
    this.typed = null;
    this.restore = null;
    target.apply(hex, true);
    this.host.requestUpdate();
  }

  render(): TemplateResult | typeof nothing {
    const p = this.picking;
    if (!p) return nothing;
    const current = p.current();
    const n = p.palette.colours.length;
    const rgb = (c: readonly number[]) => [c[0]!, c[1]!, c[2]!] as [number, number, number];
    const choices = [
      ...p.palette.colours.map((c, i) => ({hex: hexOf(rgb(c)), label: `${p.palette.title}, colour ${i + 1} of ${n}`})),
      ...p.palette.colours.map((c, i) => ({hex: hexOf(lighter(rgb(c), LIGHTER)), label: `${p.palette.title}, lighter colour ${i + 1} of ${n}`}))
    ];
    const [h, sat, val] = this.hsv;
    const custom = hexOf(rgbOfHsv(this.hsv));
    const setHsv = (next: Hsv, final: boolean) => {
      this.hsv = next;
      this.typed = null;
      this.host.requestUpdate();
      this.applyLive(hexOf(rgbOfHsv(next)), final);
    };
    const svAt = (e: PointerEvent): Hsv => {
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      const x = r.width > 0 ? Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)) : sat;
      const y = r.height > 0 ? Math.min(1, Math.max(0, (e.clientY - r.top) / r.height)) : 1 - val;
      return [h, x, 1 - y];
    };
    const hueAt = (e: PointerEvent): Hsv => {
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      const run = r.width - HUE_KNOB;
      return [run > 0 ? Math.min(359, Math.max(0, ((e.clientX - r.left - HUE_KNOB / 2) / run) * 360)) : h, sat, val];
    };
    const drag = (read: (e: PointerEvent) => Hsv) => ({
      down: (e: PointerEvent) => {
        (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
        setHsv(read(e), false);
      },
      move: (e: PointerEvent) => {
        if ((e.currentTarget as HTMLElement).hasPointerCapture?.(e.pointerId)) setHsv(read(e), false);
      },
      up: (e: PointerEvent) => setHsv(read(e), true),
      cancel: () => {
        this.abandon();
        this.hsv = hsvOf(rgbOfHex(p.current()) ?? [0, 0, 0]);
        this.host.requestUpdate();
      }
    });
    const sv = drag(svAt);
    const hue = drag(hueAt);
    const clamp = (x: number) => Math.min(1, Math.max(0, x));
    const svKey = (e: KeyboardEvent) => {
      const step = e.shiftKey ? 0.1 : 0.01;
      const next: Hsv | undefined = {
        ArrowLeft: [h, clamp(sat - step), val] as Hsv,
        ArrowRight: [h, clamp(sat + step), val] as Hsv,
        ArrowDown: [h, sat, clamp(val - step)] as Hsv,
        ArrowUp: [h, sat, clamp(val + step)] as Hsv
      }[e.key];
      if (!next) return;
      e.preventDefault();
      setHsv(next, true);
    };
    const hueKey = (e: KeyboardEvent) => {
      const step = e.shiftKey ? 10 : 1;
      const next = {ArrowLeft: h - step, ArrowDown: h - step, ArrowRight: h + step, ArrowUp: h + step, Home: 0, End: 359}[e.key];
      if (next === undefined) return;
      e.preventDefault();
      setHsv([(next + 360) % 360, sat, val], true);
    };
    const invalid = this.typed !== null;
    const hexShown = this.typed ?? custom.toUpperCase();
    /** Take the hex field's text as the colour, or keep it as typed where it is not one. */
    const hexChanged = (input: HTMLInputElement) => {
      const text = input.value.trim();
      const c = rgbOfHex(text.startsWith('#') ? text : `#${text}`);
      if (c) return this.choose(hexOf(c));
      this.typed = text;
      this.host.requestUpdate();
    };
    const onKey = (e: KeyboardEvent) => {
      const stops = Array.from((e.currentTarget as HTMLElement).querySelectorAll('button, input, [tabindex="0"]'));
      const leaves = e.key === 'Tab' && e.target === (e.shiftKey ? stops[0] : stops.at(-1));
      if (e.key !== 'Escape' && !leaves) return;
      e.stopPropagation();
      e.preventDefault();
      // Tab out of the hex field takes its text first, and stays where that is not a colour.
      if (leaves && e.target instanceof HTMLInputElement && e.target.value.trim().toUpperCase() !== hexShown.toUpperCase()) {
        hexChanged(e.target);
        if (this.typed !== null) return;
      }
      this.close(true);
    };
    const choice =({hex, label}: {hex: string; label: string}) =>
      html`<button part="choice" type="button" style=${`background:${hex}`} aria-label=${`${label}, ${hex}`} title=${`${label}, ${hex}`}
        aria-pressed=${hex === current ? 'true' : 'false'} @click=${() => this.choose(hex)}>${hex === current ? icon('check', 14, 2) : nothing}</button>`;
    return html`<div part="colour-popover" class="pop" popover="manual" role="dialog" aria-label=${`Colour of ${p.title}`}
      @keydown=${onKey}
      @focusout=${(e: FocusEvent) => {
        const to = e.relatedTarget as Node | null;
        if (to && !(e.currentTarget as HTMLElement).contains(to)) this.close(false);
      }}>
      <div class="top"><span class="t">${p.title}</span><button part="reset" class="quiet" type="button" @click=${() => this.choose(null)}>Reset</button></div>
      <div class="choices" role="group" aria-label="Palette colours">
        <div class="set">${choices.slice(0, n).map(choice)}</div>
        <div class="set">${choices.slice(n).map(choice)}</div>
      </div>
      <div class="hd" id="picker-custom">Custom</div>
      <div class="custom" role="group" aria-labelledby="picker-custom">
        <div part="sv" role="slider" tabindex="0" aria-label="Saturation and brightness" aria-valuemin="0" aria-valuemax="100" aria-valuenow=${Math.round(sat * 100)}
          aria-valuetext=${`Saturation ${Math.round(sat * 100)}%, brightness ${Math.round(val * 100)}%`}
          style=${`background:linear-gradient(to top, #000000, rgba(0, 0, 0, 0)), linear-gradient(to right, #ffffff, hsl(${h.toFixed(0)}, 100%, 50%))`}
          @pointerdown=${sv.down} @pointermove=${sv.move} @pointerup=${sv.up} @pointercancel=${sv.cancel} @keydown=${svKey}>
          <span class="knob" style=${`left:${(sat * 100).toFixed(1)}%;top:${((1 - val) * 100).toFixed(1)}%`}></span>
        </div>
        <div part="hue" role="slider" tabindex="0" aria-label="Hue" aria-valuemin="0" aria-valuemax="359" aria-valuenow=${Math.round(h)} aria-valuetext=${`${Math.round(h)} degrees`}
          @pointerdown=${hue.down} @pointermove=${hue.move} @pointerup=${hue.up} @pointercancel=${hue.cancel} @keydown=${hueKey}>
          <span class="knob" style=${`left:calc(${HUE_KNOB / 2}px + (100% - ${HUE_KNOB}px) * ${(h / 360).toFixed(4)})`}></span>
        </div>
        <label>Hex<input part="hex" type="text" spellcheck="false" .value=${hexShown}
          aria-invalid=${invalid ? 'true' : 'false'} aria-describedby="picker-hex-format"
          @change=${(e: Event) => hexChanged(e.target as HTMLInputElement)} /></label>
        <span id="picker-hex-format" class="format" role="status" ?data-invalid=${invalid}>${invalid ? 'Not a colour. Write # and six hex digits, such as #1B9E77.' : '# and six hex digits, such as #1B9E77'}</span>
      </div>
    </div>`;
  }

  /**
   * Show the picker in the top layer, beside {@link beside} and level with its swatch, or, where
   * neither side has room, under or over its swatch's row; focus goes into it as it opens.
   */
  private place(): void {
    const pop = this.host.renderRoot.querySelector<HTMLElement>('.pop');
    const from = this.from;
    if (!pop || !from || !this.picking) return;
    if (typeof pop.showPopover === 'function' && !pop.matches(':popover-open')) {
      try {
        pop.showPopover();
      } catch {
        // Shown already; the picker is in the page either way.
      }
    }
    const a = from.getBoundingClientRect();
    const own = this.beside();
    pop.style.maxHeight = '';
    const width = pop.offsetWidth || WIDTH;
    let height = pop.offsetHeight || 0;
    const vw = typeof innerWidth === 'number' ? innerWidth : 1024;
    const vh = typeof innerHeight === 'number' ? innerHeight : 768;
    let left = own.right + 12;
    if (left + width > vw - 8) left = own.left - width - 12;
    let top = a.top - 8;
    if (left < 8) {
      // No room on either side: under the row it was opened from, or over it where that has more
      // room, and no taller than the room, so the row stays in sight.
      const row = (from.closest('[part~="row"]') ?? from).getBoundingClientRect();
      const under = vh - row.bottom - 16;
      const over = row.top - 16;
      const below = under >= height || under >= over;
      height = Math.min(height, Math.max(0, below ? under : over));
      pop.style.maxHeight = `${height}px`;
      left = a.left;
      top = below ? row.bottom + 8 : row.top - 8 - height;
    }
    pop.style.left = `${Math.round(Math.max(8, Math.min(left, vw - width - 8)))}px`;
    pop.style.top = `${Math.round(Math.max(8, Math.min(top, vh - height - 8)))}px`;
    if (this.focusNext) {
      this.focusNext = false;
      (pop.querySelector<HTMLElement>('[aria-pressed="true"]') ?? pop.querySelector<HTMLElement>('button'))?.focus();
    }
  }
}

/** The picker's styles, for the element that renders it. */
export const pickerStyles = css`
  .pop {
    position: fixed;
    inset: auto;
    margin: 0;
    padding: 0;
    width: ${WIDTH}px;
    box-sizing: border-box;
    background: var(--_mosaica-surface);
    color: var(--_mosaica-ink);
    border: 1px solid var(--_mosaica-line);
    border-radius: var(--_mosaica-radius);
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.1);
    font-size: 13px;
    max-height: calc(100vh - 16px);
    overflow-y: auto;
  }
  .pop .top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
    padding: 12px 14px 8px;
  }
  .pop .top .t {
    min-width: 0;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .pop [part='reset'] {
    flex: none;
    min-width: 24px;
    min-height: 24px;
  }
  .pop .choices {
    display: grid;
    gap: 6px;
    padding: 0 14px 10px;
  }
  /* A palette's colours, in as many rows as keep each at least 24 px square. */
  .pop .set {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(24px, 1fr));
    gap: 6px;
  }
  [part='choice'] {
    display: grid;
    place-items: center;
    aspect-ratio: 1;
    width: 100%;
    border-radius: 5px;
    forced-color-adjust: none;
  }
  /* The ring stands clear of the colour, on the surface, whatever the colour is. */
  [part='choice']:focus-visible {
    outline-offset: 2px;
  }
  [part='choice'][aria-pressed='true'] {
    box-shadow:
      0 0 0 2px var(--_mosaica-surface),
      0 0 0 3.5px var(--_mosaica-ink);
  }
  /* The chosen colour's tick, light with a dark edge, so it shows on any colour. */
  [part='choice'] svg {
    color: #ffffff;
    filter: drop-shadow(0 0 1px #000000) drop-shadow(0 0 1px #000000);
  }
  .pop .hd {
    margin: 0;
    padding: 10px 14px 4px;
    border-top: 1px solid var(--_mosaica-line-2);
  }
  .pop .custom {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px 14px 14px;
  }
  [part='sv'],
  [part='hue'] {
    position: relative;
    touch-action: none;
    cursor: crosshair;
    forced-color-adjust: none;
  }
  [part='sv'] {
    height: 120px;
    border-radius: var(--_mosaica-radius-control);
  }
  [part='hue'] {
    height: 10px;
    border-radius: 5px;
    background: linear-gradient(90deg, #ff0000, #ffff00, #00ff00, #00ffff, #0000ff, #ff00ff, #ff0000);
  }
  /* The bar's target is 24 px high around it. */
  [part='hue']::before {
    content: '';
    position: absolute;
    inset: -7px 0;
  }
  .pop .knob {
    position: absolute;
    width: 12px;
    height: 12px;
    margin: -6px 0 0 -6px;
    border: 2px solid #ffffff;
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.3);
    pointer-events: none;
  }
  [part='hue'] .knob {
    top: 50%;
  }
  .pop label {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--_mosaica-ink-2);
  }
  [part='hex'] {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  [part='hex'][aria-invalid='true'] {
    border-color: var(--_mosaica-refuse);
  }
  .pop .format {
    font-size: 12px;
    color: var(--_mosaica-ink-2);
  }
  .pop .format[data-invalid] {
    color: var(--_mosaica-refuse);
    font-weight: 500;
  }
`;
