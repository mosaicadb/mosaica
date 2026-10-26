import type {ArtifactDetail, ClauseVerb, Count, FilterExpr, ItemDetail, Masked, PaletteName, RegionProjection, Refusal, SelectionShape} from '@mosaicajs/client';
import type {CategoryPaletteName, DensityColours, DensityMode, DensityScale, RampName, RampScale, SizeScale} from '@mosaicajs/deck';
import type {PanelState} from './states.js';

/** A selection shape as an event carries it: an artifact's `mosaica_id` as a decimal string. */
export type SelectionShapeDetail = Exclude<SelectionShape, {kind: 'artifact'}> | {kind: 'artifact'; id: string; outside?: boolean};

/**
 * Every event the elements fire, by name, with the type of its `detail`. Each event bubbles and is
 * composed, so you can listen on any ancestor, including one outside `<mosaica-explorer>`. A
 * `mosaica_id` is sent as a decimal string, since it is a 64-bit integer.
 */
export type MosaicaEventDetails = {
  /**
   * The panel state changed from `from` to `to`; `from` is null on the first render. If `to` is
   * `refused`, `unanswered`, `busy` or `expired`, `refusal` says what went wrong; otherwise it is
   * null.
   */
  'mosaica-statechange': {from: PanelState | null; to: PanelState; refusal: Refusal | null};
  /** The session expired. `refusal` is the server's refusal that ended it, if there was one. */
  'mosaica-expired': {refusal: Refusal | null};
  /**
   * The camera moved. `bbox` is the area shown, `[x0, y0, x1, y1]` in the view's data coordinates.
   * `zoom` is 0 when the 512-unit world is 512 pixels wide, and 1 more per doubling. `width` and
   * `height` are the canvas size in CSS pixels.
   */
  'mosaica-viewchange': {bbox: [number, number, number, number]; zoom: number; width: number; height: number};
  /** A point was clicked. `id` is its `mosaica_id`. Its record follows in `mosaica-itemopen`. */
  'mosaica-pick': {id: string};
  /**
   * The record of the selected item `id` arrived, whether it was selected by a click or by
   * `Store.pick`. If the server refuses the record, this does not fire, and the refusal is in the
   * store's `selection.itemRefusal`.
   */
  'mosaica-itemopen': {id: string; record: ItemDetail};
  /** A click on the map found no point and no artifact. */
  'mosaica-miss': Record<string, never>;
  /** The pointer is over the point `id`, at `x`, `y` in canvas pixels. */
  'mosaica-hover': {id: string; x: number; y: number};
  /**
   * The artifact `id` was opened and its details arrived in `detail`, with `maskedCount` as a
   * decimal string.
   */
  'mosaica-artifactopen': {id: string; detail: Omit<ArtifactDetail, 'maskedCount'> & {maskedCount: string}};
  /** Fit was pressed for the artifact `id`. `<mosaica-explorer>` fits its map to it. */
  'mosaica-artifactfit': {id: string};
  /**
   * The selected region changed. `shape` is the new shape in data coordinates, or null if the
   * selection was cleared. It fires with `status` `loading` when a shape is selected or `cleared`
   * when it is cleared, then again with `shown` or `refused` when the server answers. Then `visible`, `matched` and
   * `served` carry the region's counts, and `verdict` says whether they are exact for the shape.
   */
  'mosaica-selectchange': {
    shape: SelectionShapeDetail | null;
    status?: RegionProjection['status'] | 'cleared';
    visible?: Masked | null;
    matched?: Masked;
    served?: Count;
    verdict?: RegionProjection['verdict'];
  };
  /** The chosen layers are now `layers`. The map also draws the layers each depends on, which the list leaves out. */
  'mosaica-layerchange': {layers: string[]};
  /** The points are now coloured by `colourBy`: a column, `cluster:<layer>`, or null for a single colour. */
  'mosaica-colourchange': {colourBy: string | null};
  /**
   * A size setting changed in the explorer's Layers popover. The detail holds all four as they now
   * stand: `sizeBy` is the number column that sizes the points, or null for a single size, `min`
   * and `max` are the radii in pixels of its smallest and largest value, and `scale` is how values
   * are placed between them.
   */
  'mosaica-sizechange': {sizeBy: string | null; min: number; max: number; scale: SizeScale};
  /** A field card's fold button folded it to one line (`folded` true) or opened it. */
  'mosaica-fold': {field: string; folded: boolean};
  /** The Most points slider was released. The store now aims to draw `budget` marks on screen. */
  'mosaica-budgetchange': {budget: number};
  /** The Most clusters slider was released. A `nested` or `dag` layer is now cut to at most `budget` clusters. */
  'mosaica-clusterbudgetchange': {budget: number};
  /** The level that clusters are coloured and labelled at is now `level`. Null is the default level. */
  'mosaica-levelchange': {level: number | null};
  /**
   * The viewer chose or reset colours for values of the category column `column`, in a colour
   * picker or in Edit colours. Each change names a category key and its new colour as `#rrggbb`, or
   * `null` if it was reset to its palette colour. The map already draws the new colours. To restore
   * them in a later session, save them and set the map's or explorer's `valueColours`.
   */
  'mosaica-valuecolour': {column: string; changes: {value: string; colour: string | null}[]};
  /**
   * The viewer chose or reset colours for clusters of the layer `layer`. Each change names a cluster
   * by its `mosaica_id` and its new colour as `#rrggbb`, or `null` if it was reset to its palette
   * colour. The map already draws the new colours. To restore them in a later session, save them
   * and set the map's or explorer's `clusterColours`.
   */
  'mosaica-clustercolour': {layer: string; changes: {mosaicaId: string; colour: string | null}[]};
  /** The palette, ramp, ramp scale or ramp direction was changed in the explorer's Colour section. The detail holds all four as they now stand. */
  'mosaica-palettechange': {palette: CategoryPaletteName; ramp: RampName; scale: RampScale; reverse: boolean};
  /** A palette was chosen for a layer's clusters in the explorer's Colour section and set on the store (`Store.setPalette`). */
  'mosaica-clusterpalettechange': {palette: PaletteName};
  /**
   * A display setting changed in the explorer's Layers popover. The detail holds every setting as it
   * now stands. `points` is the opposite of the map's `noPoints`; the other fields match the map's
   * properties of the same name. `radius`, `pointOpacity` and `densityColours` are null while the
   * map chooses them itself.
   */
  'mosaica-displaychange': {
    points: boolean;
    radius: number | null;
    pointOpacity: number | null;
    density: DensityMode;
    densityColours: DensityColours | null;
    densityStrength: number;
    densityResolution: number;
    densityScale: DensityScale;
  };
  /**
   * A filter or highlight changed. `column` is the column whose control changed, or null for Clear
   * all and for clearing the highlight. `verb` is `filter` or `highlight` (`filter` if absent).
   * After an edit, `expr` is the combined expression of every control's clause for that `verb`, or
   * null if there are none. When a chip is removed, or Clear all is pressed, `expr` is null.
   * `member_of` clauses are reported by `mosaica-clausechange`.
   */
  'mosaica-filterchange': {column: string | null; expr?: FilterExpr | null; verb?: ClauseVerb};
  /** A column's chip was pressed in a filter panel with `chips-only`, so you can show its card. `verb` is `filter` or `highlight`. */
  'mosaica-chipopen': {column: string; verb: ClauseVerb};
  /**
   * Open was pressed on the item `id`, whose record's fields are `fields`. It fires both when Open
   * calls the card's `open` and when it follows `open-url`.
   */
  'mosaica-open': {id: string; fields: Record<string, unknown>};
  /**
   * The view changed from `from` to `to`. `sameFrame` is true if both views share one extent, in
   * which case the camera stays where it is; otherwise the map refits.
   */
  'mosaica-viewswitch': {from: string; to: string; sameFrame: boolean};
  /**
   * A view chip on the item card asked to show the item in the view `view`, where it sits at `x`,
   * `y` in that view's data coordinates. `<mosaica-explorer>` switches to the view and centres the
   * map there.
   */
  'mosaica-viewfollow': {view: string; x: number; y: number};
  /**
   * The map's message about its basemap changed. `refusal` says why the `basemap` value cannot be
   * drawn under its view, and is shown over the map. `note` says why nothing is drawn under the
   * current view, and is shown in a corner. Each is null if there is none.
   */
  'mosaica-basemapchange': {refusal: string | null; note: string | null};
  /** A card's close button was pressed. `what` says which card. */
  'mosaica-close': {what: 'item' | 'artifact'};
  /**
   * A `member_of` clause, which keeps the map to the members of the artifact `id` in layer `layer`,
   * was added (`on` true) or removed. `verb` is `filter` or `highlight`, and `outside` is true if the
   * clause keeps everything outside the artifact instead.
   */
  'mosaica-clausechange': {id: string; layer: string; outside: boolean; verb: ClauseVerb; on: boolean};
};

/** Every event the elements fire, as the `CustomEvent` a listener receives. */
export type MosaicaEventMap = {[K in keyof MosaicaEventDetails]: CustomEvent<MosaicaEventDetails[K]>};

declare global {
  interface HTMLElementEventMap extends MosaicaEventMap {}
}
