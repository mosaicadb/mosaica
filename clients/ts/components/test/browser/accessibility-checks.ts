// What `accessibility-page.ts` measures in the page for `accessibility.browser.ts`: text that is cut
// off, pointer targets that are small and crowded, the control that has focus, and the contrast of
// a piece of text against what is drawn behind it. Each walks the page as it is laid out, through
// shadow roots and slots.

/** An element's parent as it is drawn: its slot, else its parent, else its shadow root's host. */
function drawnParent(node: Node): Node | null {
  return (node instanceof Element ? node.assignedSlot : null) ?? node.parentNode ?? (node instanceof ShadowRoot ? node.host : null);
}

/** Whether `node` is `container` or drawn inside it. */
function inside(node: Node | null, container: Node): boolean {
  for (let at = node; at; at = drawnParent(at)) if (at === container) return true;
  return false;
}

/** The elements `node` is drawn inside, nearest first. */
function* drawnAncestors(node: Node): Generator<Element> {
  for (let at = drawnParent(node); at; at = drawnParent(at)) if (at instanceof Element) yield at;
}

/** Every element under `root`, through shadow roots. */
function* elementsUnder(root: ParentNode): Generator<Element> {
  for (const el of root.querySelectorAll('*')) {
    yield el;
    if (el.shadowRoot) yield* elementsUnder(el.shadowRoot);
  }
}

/** The innermost element drawn at a point, through shadow roots. */
function topAt(x: number, y: number): Element | null {
  let el = document.elementFromPoint(x, y);
  for (;;) {
    const inner = el?.shadowRoot?.elementFromPoint(x, y);
    if (!inner || inner === el) return el;
    el = inner;
  }
}

/** The element with focus, through shadow roots. */
function deepActive(): Element | null {
  let el = document.activeElement;
  while (el?.shadowRoot?.activeElement) el = el.shadowRoot.activeElement;
  return el;
}

/** Where an element is and what it is: the custom elements around it, then its part, class or tag. */
export function place(el: Element): string {
  const hosts: string[] = [];
  for (let root = el.getRootNode(); root instanceof ShadowRoot; root = root.host.getRootNode()) hosts.unshift(root.host.localName.replace(/^mosaica-/, ''));
  const part = el.getAttribute('part');
  const own = part ? `[${part}]` : el.id ? `#${el.id}` : typeof el.className === 'string' && el.className ? `.${el.className.trim().split(/\s+/).join('.')}` : '';
  return `${hosts.join(' > ')}${hosts.length ? ' > ' : ''}${el.localName}${own}`;
}

/** A control's accessible name, near enough to tell one from another in a report. */
function nameOf(el: Element): string {
  const labelled = el.getAttribute('aria-labelledby');
  const root = el.getRootNode() as Document | ShadowRoot;
  const byId = labelled
    ?.split(/\s+/)
    .map((id) => root.getElementById(id)?.textContent?.trim() ?? '')
    .join(' ');
  return (el.getAttribute('aria-label') ?? byId ?? el.textContent?.trim() ?? '').replace(/\s+/g, ' ').slice(0, 60);
}

const drawn = (el: Element) => el.checkVisibility({visibilityProperty: true}) && el.getClientRects().length > 0;

/** Whether no ancestor clips an element: it is in the top layer, or fixed to the window. */
const unclipped = (el: Element) => el.matches(':popover-open') || (el instanceof HTMLDialogElement && el.matches(':modal')) || getComputedStyle(el).position === 'fixed';

type Edges = {left: number; top: number; right: number; bottom: number};

/** The box an element clips its content to: inside its borders. */
function clipBox(el: Element): Edges {
  const r = el.getBoundingClientRect();
  return {left: r.left + el.clientLeft, top: r.top + el.clientTop, right: r.left + el.clientLeft + el.clientWidth, bottom: r.top + el.clientTop + el.clientHeight};
}

/**
 * The text under `scope` that is cut off or that only a sideways scroll reaches, each as where it
 * is and how it starts. Text a viewer reaches by scrolling down is whole. Text hidden for screen
 * readers alone, a canvas's, and a native control's own are left out.
 */
export function clippedText(scope: Element): string[] {
  const found = new Set<string>();
  const roots: ParentNode[] = [scope, ...[...elementsUnder(scope)].flatMap((el) => (el.shadowRoot ? [el.shadowRoot] : []))];
  for (const root of roots) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const text = node.textContent?.trim() ?? '';
      const parent = node.parentElement;
      if (!text || !parent || !drawn(parent)) continue;
      if (parent.closest('.sr-only, canvas, select, option, style, script, svg')) continue;
      const range = document.createRange();
      range.selectNodeContents(node);
      const rects = [...range.getClientRects()].filter((r) => r.width > 0 && r.height > 0);
      if (rects.length === 0) continue;
      const box: Edges = {left: Math.min(...rects.map((r) => r.left)), top: Math.min(...rects.map((r) => r.top)), right: Math.max(...rects.map((r) => r.right)), bottom: Math.max(...rects.map((r) => r.bottom))};
      let cut = '';
      for (const ancestor of unclipped(parent) ? [] : drawnAncestors(node)) {
        const style = getComputedStyle(ancestor);
        // An inline box clips nothing.
        if (style.display === 'inline') continue;
        const clip = clipBox(ancestor);
        if (style.overflowX !== 'visible' && (box.right > clip.right + 1 || box.left < clip.left - 1)) {
          cut = `${style.overflowX === 'hidden' || style.overflowX === 'clip' ? 'is cut off at the side by' : 'scrolls sideways in'} ${place(ancestor)}`;
          break;
        }
        if (style.overflowY !== 'visible' && (box.bottom > clip.bottom + 1 || box.top < clip.top - 1)) {
          if (style.overflowY === 'hidden' || style.overflowY === 'clip') {
            cut = `is cut off at the top or bottom by ${place(ancestor)}`;
            break;
          }
          // Reached by scrolling down: what is drawn of it is what the ancestors beyond see.
          box.top = Math.max(box.top, clip.top);
          box.bottom = Math.min(box.bottom, clip.bottom);
          if (box.bottom <= box.top) break;
        }
        if (unclipped(ancestor)) break;
      }
      if (!cut && (box.right > innerWidth + 1 || box.left < -1) && box.bottom > box.top) cut = 'is outside the window at the side';
      if (cut) found.add(`${place(parent)} "${text.slice(0, 40)}" ${cut}`);
    }
  }
  return [...found].sort();
}

/**
 * The elements under `scope` whose content is wider than they are, so it scrolls sideways or is cut
 * off. The map's canvas, text fields and text hidden for screen readers alone are left out.
 */
export function wideContent(scope: Element): string[] {
  const found: string[] = [];
  for (const el of [scope, ...elementsUnder(scope)]) {
    if (!drawn(el) || el.matches('input, select, textarea, canvas, .sr-only') || el.closest('[part~="canvas"], svg')) continue;
    const style = getComputedStyle(el);
    if (style.overflowX === 'visible' || el.scrollWidth <= el.clientWidth + 1) continue;
    found.push(`${place(el)} holds more than its width and ${style.overflowX === 'hidden' || style.overflowX === 'clip' ? 'cuts it off' : 'scrolls sideways'}`);
  }
  if (document.documentElement.scrollWidth > innerWidth + 1) found.push('the page scrolls sideways');
  return found.sort();
}

let spaced: HTMLStyleElement[] = [];

/** Force wider text spacing on every element in the page and in every shadow root, or take it off. */
export function spaceText(on: boolean): void {
  for (const style of spaced) style.remove();
  spaced = [];
  if (!on) return;
  const roots: (Document | ShadowRoot)[] = [document, ...[...elementsUnder(document)].flatMap((el) => (el.shadowRoot ? [el.shadowRoot] : []))];
  for (const root of roots) {
    const style = document.createElement('style');
    style.textContent = '* { line-height: 1.5 !important; letter-spacing: 0.12em !important; word-spacing: 0.16em !important; }';
    (root instanceof Document ? root.head : root).append(style);
    spaced.push(style);
  }
}

/** What a pointer can press: the native controls, and elements with a widget's role. */
const TARGETS = [
  'button',
  'a[href]',
  'input:not([type="hidden"])',
  'select',
  'textarea',
  'summary',
  ...['button', 'link', 'checkbox', 'radio', 'switch', 'slider', 'tab', 'menuitem', 'menuitemradio', 'menuitemcheckbox', 'option', 'combobox', 'spinbutton', 'textbox', 'application'].map((role) => `[role="${role}"]`)
].join(', ');

/** The target a press at a point reaches: the innermost one drawn there, or the control of a label there. */
function targetAt(x: number, y: number): Element | null {
  for (let at: Node | null = topAt(x, y); at; at = drawnParent(at)) {
    if (at instanceof HTMLLabelElement && at.control) return at.control;
    if (at instanceof Element && at.matches(TARGETS) && !at.matches(':disabled, [aria-disabled="true"]')) return at;
  }
  return null;
}

type Target = {el: Element; name: string; cx: number; cy: number; small: boolean};

/**
 * The pointer targets under `scope` that are smaller than 24 by 24 CSS px and closer to another
 * than the spacing that makes up for it: a press inside a 24 px circle centred on the small one
 * reaches another target, or the circle meets another small one's. A target's size is what a press
 * reaches around its centre, so a pseudo-element or a label that enlarges it counts, and so does
 * whatever is drawn over it. A target and one drawn inside it are not compared, and a target scrolled
 * out of sight is not measured.
 */
export function crowdedTargets(scope: Element): string[] {
  const targets: Target[] = [];
  for (const el of elementsUnder(scope)) {
    if (!el.matches(TARGETS) || !drawn(el) || el.matches(':disabled, [aria-disabled="true"]') || el.closest('[inert]')) continue;
    const r = el.getBoundingClientRect();
    const [cx, cy] = [r.left + r.width / 2, r.top + r.height / 2];
    // Covered at its centre, or outside the window, it is not what a press there reaches.
    if (cx < 0 || cy < 0 || cx > innerWidth || cy > innerHeight || targetAt(cx, cy) !== el) continue;
    /** How many whole pixels from the centre a press still reaches the target, up to 12, along one direction. */
    const reach = (dx: number, dy: number) => {
      let d = 0;
      while (d < 12 && targetAt(cx + dx * (d + 0.5), cy + dy * (d + 0.5)) === el) d += 1;
      return d;
    };
    const width = r.width >= 24 ? r.width : reach(-1, 0) + reach(1, 0);
    const height = r.height >= 24 ? r.height : reach(0, -1) + reach(0, 1);
    targets.push({el, name: `${place(el)} "${nameOf(el)}" ${Math.round(width)}x${Math.round(height)}`, cx, cy, small: width < 23.5 || height < 23.5});
  }
  const found: string[] = [];
  const nested = (a: Element, b: Element) => inside(a, b) || inside(b, a);
  for (const a of targets) {
    if (!a.small) continue;
    const near = new Map<Element, number>();
    for (let dx = -11; dx <= 11; dx += 2)
      for (let dy = -11; dy <= 11; dy += 2) {
        const apart = Math.hypot(dx, dy);
        if (apart > 11.5) continue;
        const at = targetAt(a.cx + dx, a.cy + dy);
        if (at && at !== a.el && !nested(a.el, at)) near.set(at, Math.min(near.get(at) ?? 12, apart));
      }
    for (const b of targets) if (b !== a && b.small && !nested(a.el, b.el) && Math.hypot(a.cx - b.cx, a.cy - b.cy) < 23.5) near.set(b.el, Math.min(near.get(b.el) ?? 12, Math.hypot(a.cx - b.cx, a.cy - b.cy) / 2));
    for (const [el, apart] of near) found.push(`${a.name} has ${targets.find((t) => t.el === el)?.name ?? place(el)} ${Math.round(apart)} px from its centre`);
  }
  return found.sort();
}

export type Stop = {name: string; faults: string[]; again: boolean};

let visited = new WeakSet<Element>();

/** Forget the stops a walk has made, before another walk. */
export function forgetStops(): void {
  visited = new WeakSet();
}

/**
 * The control with focus, if it is under `scope`: whether this walk has stopped at it before, and
 * what is wrong with it as a place for the focus to be: not drawn, outside the window, under
 * something else at its centre, or showing no focus indicator of its own or on the box around it.
 */
export function focusStop(scope: Element): Stop | null {
  const el = deepActive();
  if (!el || !inside(el, scope)) return null;
  const again = visited.has(el);
  visited.add(el);
  const faults: string[] = [];
  const r = el.getBoundingClientRect();
  if (!drawn(el) || r.width === 0 || r.height === 0) faults.push('is not drawn');
  else {
    if (r.left < -1 || r.top < -1 || r.right > innerWidth + 1 || r.bottom > innerHeight + 1) faults.push(`is outside the window, from ${Math.round(r.left)},${Math.round(r.top)} to ${Math.round(r.right)},${Math.round(r.bottom)}`);
    const [cx, cy] = [r.left + r.width / 2, r.top + r.height / 2];
    const covering = topAt(cx, cy);
    const onTop = (x: number, y: number) => inside(topAt(x, y), el) || targetAt(x, y) === el;
    // A stop as large as the map may hold a message or a card at its centre: it is covered where
    // less than half of it shows, over a grid of 25 points.
    const fifths = [0.1, 0.3, 0.5, 0.7, 0.9];
    const showing = Math.min(r.width, r.height) < 100 ? 0 : fifths.flatMap((fx) => fifths.map((fy) => onTop(r.left + fx * r.width, r.top + fy * r.height))).filter(Boolean).length;
    if (covering && !onTop(cx, cy) && showing <= 12) faults.push(`is under ${place(covering)}`);
  }
  // The indicator is an outline or a shadow the control, or the box around a field and its icon,
  // does not have without the focus: an empty copy beside it is styled as it is, unfocused.
  const ringed = (at: Element) => {
    const copy = at.cloneNode(false) as Element;
    copy.removeAttribute('id');
    at.after(copy);
    const [now, plain] = [getComputedStyle(at), getComputedStyle(copy)];
    const outline = (s: CSSStyleDeclaration) => (s.outlineStyle === 'none' || parseFloat(s.outlineWidth) === 0 ? 'none' : `${s.outlineStyle} ${s.outlineWidth} ${s.outlineColor}`);
    const shown = (outline(now) !== 'none' && outline(now) !== outline(plain)) || (now.boxShadow !== 'none' && now.boxShadow !== plain.boxShadow);
    copy.remove();
    return shown;
  };
  let ring = ringed(el);
  let hops = 0;
  for (const ancestor of drawnAncestors(el)) {
    if (ring || hops++ >= 2) break;
    ring = ancestor.matches(':focus-within') && ringed(ancestor);
  }
  if (!ring) faults.push('shows no focus indicator');
  return {name: `${place(el)} "${nameOf(el)}"`, faults, again};
}

type Rgb = [number, number, number];
type Rgba = [number, number, number, number];

/** A CSS colour as red, green, blue and alpha, each from 0 to 1, read back from a canvas. */
function rgba(colour: string): Rgba {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 1;
  const ctx = canvas.getContext('2d', {willReadFrequently: true})!;
  const read = (fill: string) => {
    ctx.clearRect(0, 0, 1, 1);
    ctx.fillStyle = fill;
    ctx.fillRect(0, 0, 1, 1);
    return ctx.getImageData(0, 0, 1, 1).data;
  };
  const alpha = read(colour)[3]! / 255;
  // A translucent colour loses precision on the way back, so its channels are read at full alpha.
  const [r, g, b] = read(alpha === 1 || alpha === 0 ? colour : `rgb(from ${colour} r g b / 1)`);
  return [r! / 255, g! / 255, b! / 255, alpha];
}

const over = (top: Rgba, under: Rgb): Rgb => [0, 1, 2].map((i) => top[i]! * top[3] + under[i]! * (1 - top[3])) as Rgb;

function luminance([r, g, b]: Rgb): number {
  const linear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
}

/** The colour a page with no background of its own is drawn on, in the colour scheme in force. */
function pageGround(): Rgb {
  const probe = document.createElement('div');
  probe.style.backgroundColor = 'Canvas';
  document.documentElement.append(probe);
  const [r, g, b] = rgba(getComputedStyle(probe).backgroundColor);
  probe.remove();
  return [r, g, b];
}

/**
 * What is wrong with the contrast of an element's text against the plain colours drawn behind it,
 * or `null` where it reaches the ratio its size needs: 3 to 1 for large text, 4.5 to 1 otherwise.
 * What is behind is each background from the page down to the element's own, composed with each
 * element's opacity. The map's canvas is transparent over the map's background, so text over the
 * map is measured against that. A background that is an image or a gradient cannot be measured,
 * and is reported, unless the element matches `shaded`: one whose images only shade its edges,
 * which is measured against its background colour.
 */
export function contrastFault(el: Element, shaded: string): string | null {
  const style = getComputedStyle(el);
  const size = parseFloat(style.fontSize);
  const needs = size >= 24 || (Number(style.fontWeight) >= 700 && size >= 18.66) ? 3 : 4.5;
  const chain = [el, ...drawnAncestors(el)].reverse();
  const said = `${place(el)} "${(el.textContent ?? '').trim().replace(/\s+/g, ' ').slice(0, 40)}"`;
  const image = chain.find((at) => getComputedStyle(at).backgroundImage !== 'none' && !at.matches(shaded));
  if (image) return `${said} is over the background image of ${place(image)}`;
  /** The colours of a pixel of the text and of one beside it, with `chain` from `i` on drawn over `behind`. */
  const draw = (i: number, behind: Rgb): [Rgb, Rgb] => {
    const s = getComputedStyle(chain[i]!);
    const ground = over(rgba(s.backgroundColor), behind);
    const [text, beside] = i === chain.length - 1 ? [over(rgba(style.color), ground), ground] : draw(i + 1, ground);
    const faded = (c: Rgb) => over([...c, Number(s.opacity)], behind);
    return [faded(text), faded(beside)];
  };
  const [text, beside] = draw(0, pageGround());
  const [a, b] = [luminance(text), luminance(beside)];
  const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
  if (ratio >= needs) return null;
  const fader = chain.find((at) => Number(getComputedStyle(at).opacity) < 1);
  return `${said} is ${ratio.toFixed(2)} to 1 and needs ${needs}${fader ? `, faded by ${place(fader)}` : ''}`;
}

/** The elements under `scope` that hold text of their own and are drawn, apart from text for screen readers alone. */
export function textElements(scope: ParentNode): Element[] {
  const found = new Set<Element>();
  const roots: ParentNode[] = [scope, ...[...elementsUnder(scope)].flatMap((el) => (el.shadowRoot ? [el.shadowRoot] : []))];
  for (const root of roots) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const parent = node.parentElement;
      if (node.textContent?.trim() && parent && drawn(parent) && !parent.closest('.sr-only, canvas, select, option, style, script, svg')) found.add(parent);
    }
  }
  return [...found];
}
