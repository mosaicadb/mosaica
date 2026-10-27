/**
 * The soft backing behind a cluster's name: a pill of the ground colour, opaque over the text and
 * fading to nothing a little past it, so the name reads over points however dense without a
 * hard-edged plate. The pills are drawn from one small atlas of white pills of a range of widths,
 * tinted to the ground and stretched to each name's height, so many names cost one draw.
 */

/** A pill's height as a multiple of the name's size: the text, and the fade above and below it. */
export const BACKING_HEIGHT_EM = 2.1;
/** How far the fade reaches past the text on every side, as a multiple of the name's size. */
export const BACKING_FADE_EM = 0.55;

/** Each pill's height in the atlas, in pixels. */
const ICON_PX = 24;
/** The widest pill, as width over height, and the ratio between neighbouring widths. */
const WIDEST = 40;
const STEP = 1.1;

/** The atlas's pills, by width over height, narrowest first. */
const ASPECTS: number[] = [];
for (let a = 1; a <= WIDEST; a *= STEP) ASPECTS.push(a);

/** The key of the atlas pill nearest `aspect`, width over height. */
export function backingIcon(aspect: number): string {
  const i = Math.round(Math.log(Math.max(1, aspect)) / Math.log(STEP));
  return `p${Math.min(ASPECTS.length - 1, i)}`;
}

/** The pill's width over height for a name `textWidth` pixels wide at `size` pixels. */
export function backingAspect(textWidth: number, size: number): number {
  return (textWidth + 2 * BACKING_FADE_EM * size) / (BACKING_HEIGHT_EM * size);
}

export type BackingAtlas = {
  width: number;
  height: number;
  /** RGBA, white, the pill in the alpha channel. */
  data: Uint8ClampedArray<ArrayBuffer>;
  mapping: Record<string, {x: number; y: number; width: number; height: number; mask: true}>;
};

let built: BackingAtlas | null = null;

/**
 * The atlas: one pill per width, one per row. Each is a stadium whose straight sides and round ends
 * lie the fade's width inside the pill's edge; inside it the alpha is full, and outside it falls
 * along a smoothstep to zero at the edge.
 */
export function backingAtlas(): BackingAtlas {
  if (built) return built;
  const h = ICON_PX;
  const fade = (h * BACKING_FADE_EM) / BACKING_HEIGHT_EM;
  const radius = h / 2 - fade;
  const widths = ASPECTS.map((a) => Math.round(h * a));
  const width = Math.max(...widths);
  const height = h * ASPECTS.length;
  const data = new Uint8ClampedArray(width * height * 4);
  const mapping: BackingAtlas['mapping'] = {};
  widths.forEach((w, row) => {
    const top = row * h;
    mapping[`p${row}`] = {x: 0, y: top, width: w, height: h, mask: true};
    const x0 = h / 2;
    const x1 = Math.max(x0, w - h / 2);
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const px = x + 0.5;
        const dy = y + 0.5 - h / 2;
        const dx = px < x0 ? px - x0 : px > x1 ? px - x1 : 0;
        const d = Math.hypot(dx, dy) - radius;
        const t = d <= 0 ? 1 : Math.max(0, 1 - d / fade);
        const at = ((top + y) * width + x) * 4;
        data[at] = data[at + 1] = data[at + 2] = 255;
        data[at + 3] = Math.round(255 * t * t * (3 - 2 * t));
      }
    }
  });
  built = {width, height, data, mapping};
  return built;
}
