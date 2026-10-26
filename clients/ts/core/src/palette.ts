/**
 * The cluster palettes. The server gives each cluster a slot below the palette's size, chosen so
 * that clusters drawn beside each other differ, and the same at any zoom, box, budget or filter.
 * The client maps a slot to the palette's colour at that index. A request names the palette's size
 * as `palette_size`, so the slots of one size mean nothing under another, and a slot is coloured
 * from the palette of the size it was served for.
 */

/**
 * A colour as red, green, blue and alpha bytes, each from 0 to 255.
 *
 * @category Coordinates and colour
 */
export type Rgba = readonly [number, number, number, number];

/**
 * The name of a palette for colouring clusters: `okabe-ito` (8 colours that stay distinct under
 * the common forms of colour blindness), `tableau10` (10 colours), `tableau20` (20 colours in light
 * and dark pairs) or `kelly` (Kelly's 22 colours of maximum contrast).
 *
 * @category Coordinates and colour
 */
export type PaletteName = 'okabe-ito' | 'tableau10' | 'tableau20' | 'kelly';

/**
 * A palette for colouring clusters, with a title and description for a menu. The server gives
 * each cluster a slot in the palette, chosen so that neighbouring clusters differ, and the cluster
 * takes the colour at that slot.
 *
 * @category Coordinates and colour
 */
export type Palette = {
  /** The palette's name for a menu. */
  title: string;
  /** A short description for a menu, such as `8 colours · colour-blind safe`. */
  description: string;
  /** The colours, in slot order. */
  colours: readonly Rgba[];
};

const ALPHA = 220;

function hex(...colours: string[]): Rgba[] {
  return colours.map((c) => [parseInt(c.slice(1, 3), 16), parseInt(c.slice(3, 5), 16), parseInt(c.slice(5, 7), 16), ALPHA]);
}

/**
 * The cluster palettes, by name, in the order a menu lists them.
 *
 * @category Coordinates and colour
 */
export const PALETTES: Readonly<Record<PaletteName, Palette>> = {
  'okabe-ito': {
    title: 'Okabe-Ito',
    description: '8 colours · colour-blind safe',
    colours: hex('#e69f00', '#56b4e9', '#009e73', '#f0e442', '#0072b2', '#d55e00', '#cc79a7', '#000000')
  },
  tableau10: {
    title: 'Tableau 10',
    description: '10 colours',
    colours: hex('#4e79a7', '#f28e2b', '#e15759', '#76b7b2', '#59a14f', '#edc948', '#b07aa1', '#ff9da7', '#9c755f', '#bab0ac')
  },
  tableau20: {
    title: 'Tableau 20',
    description: '20 colours, in light and dark pairs',
    colours: hex(
      '#4e79a7', '#a0cbe8', '#f28e2b', '#ffbe7d', '#59a14f', '#8cd17d', '#b6992d', '#f1ce63', '#499894', '#86bcb6',
      '#e15759', '#ff9d9a', '#79706e', '#bab0ac', '#d37295', '#fabfd2', '#b07aa1', '#d4a6c8', '#9d7660', '#d7b5a6'
    )
  },
  kelly: {
    title: 'Kelly',
    description: '22 colours, most distinct',
    colours: hex(
      '#f2f3f4', '#222222', '#f3c300', '#875692', '#f38400', '#a1caf1', '#be0032', '#c2b280', '#848482', '#008856', '#e68fac',
      '#0067a5', '#f99379', '#604e97', '#f6a600', '#b3446c', '#dcd300', '#882d17', '#8db600', '#654522', '#e25822', '#2b3d26'
    )
  }
};

/**
 * The palette a store colours clusters with unless you choose another: `tableau10`.
 *
 * @category Coordinates and colour
 */
export const DEFAULT_PALETTE: PaletteName = 'tableau10';

/**
 * How many colours `palette` has. Requests for palette slots send it as `palette_size`.
 *
 * @category Coordinates and colour
 */
export function paletteSize(palette: PaletteName): number {
  return PALETTES[palette].colours.length;
}

/**
 * The grey used for an artifact with no colour of its own, such as one whose palette slot is not
 * known yet.
 *
 * @category Coordinates and colour
 */
export const NEUTRAL: Rgba = [118, 126, 140, 200];

/**
 * A cluster's colour: `chosen` if given, otherwise the colour at `slot` in `palette`. Returns
 * {@link NEUTRAL} if `slot` is `null` or past the end of the palette.
 *
 * @category Coordinates and colour
 */
export function artifactColour(palette: PaletteName, slot: number | null, chosen?: Rgba): Rgba {
  if (chosen) return chosen;
  return (slot === null ? undefined : PALETTES[palette].colours[slot]) ?? NEUTRAL;
}

/**
 * `#rrggbb` as a colour chosen for a cluster, as opaque as the palettes' colours; `null` for any
 * other text.
 *
 * @internal
 */
export function chosenColour(text: string): Rgba | null {
  return /^#[0-9a-f]{6}$/i.test(text) ? hex(text)[0]! : null;
}

/**
 * The palette with `size` colours, or `null` where none has that many. Each palette has a size of
 * its own, so a slot's palette is known from the size it was served under.
 *
 * @category Coordinates and colour
 */
export function paletteOfSize(size: number | null): PaletteName | null {
  return (Object.keys(PALETTES) as PaletteName[]).find((name) => PALETTES[name].colours.length === size) ?? null;
}

/**
 * Colours for particular artifacts, to use in place of their palette colours, keyed by layer and
 * then by `mosaica_id`. Pass it to {@link Store.setArtifactColours}.
 *
 * @category Coordinates and colour
 */
export type ChosenColours = ReadonlyMap<string, ReadonlyMap<bigint, Rgba>>;

/**
 * An artifact a colour is wanted for: its ordinal in the session table, its layer, its
 * `mosaica_id`, its slot and the palette size the slot was served under.
 *
 * @internal
 */
export type Slotted = {ordinal: number; layer: string; mosaicaId: bigint; slot: number | null; paletteSize: number | null};

/**
 * A colour per artifact, by ordinal, with the colours `chosen` for its layer and `mosaica_id` in
 * place of the palette's. Each slot is coloured from the palette it was served for, so a slot served before a
 * change of palette keeps its colour until the slot of the new size arrives.
 *
 * @internal
 */
export function artifactColours(artifacts: readonly Slotted[], chosen: ChosenColours = new Map()): Map<number, Rgba> {
  const out = new Map<number, Rgba>();
  for (const a of artifacts) out.set(a.ordinal, slottedColour(a, chosen));
  return out;
}

/** One artifact's colour, as {@link artifactColours} gives it. @internal */
export function slottedColour(a: Omit<Slotted, 'ordinal'>, chosen: ChosenColours): Rgba {
  const palette = paletteOfSize(a.paletteSize);
  const own = chosen.get(a.layer)?.get(a.mosaicaId);
  return palette === null ? (own ?? NEUTRAL) : artifactColour(palette, a.slot, own);
}
