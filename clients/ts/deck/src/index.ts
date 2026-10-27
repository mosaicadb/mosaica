/**
 * `@mosaicajs/deck` draws a Mosaica map with deck.gl, for an app that builds its own `Deck`. If
 * you want a ready-made map, the `<mosaica-map>` element in `@mosaicajs/components` wraps this
 * package and handles the camera for you.
 *
 * {@link MosaicaLayer} is a deck.gl layer that draws what a store from `@mosaicajs/client`
 * publishes: the points, density, the artifacts' names and outlines, and the selected region. It
 * fetches nothing itself. Your app owns the camera. Each time the view state changes, pass the
 * camera to {@link viewInputOf} and give the result to the store's `setView`. The store fetches
 * what the new view needs, and the layer redraws when the store's projections change.
 *
 * The rest of the package works alongside the layer. {@link resolvePick} turns a deck.gl pick into
 * the item or artifact under the pointer. {@link frameMatches} and {@link cameraFitting} find the
 * camera that shows the filtered items. {@link basemapLayers} draws a world outline, map tiles or
 * an image under the points. {@link DensityCounter} fetches the counts that density is drawn from,
 * and the layer keeps one of its own unless you pass yours. {@link Colouring}, {@link Sizing} and
 * the named palettes and ramps are the choices for colouring and sizing points by a column.
 *
 * @example
 * ```ts
 * import {Deck, OrthographicView} from '@deck.gl/core';
 * import {createStore} from '@mosaicajs/client';
 * import {MosaicaLayer, viewInputOf} from '@mosaicajs/deck';
 *
 * type Camera = {target: [number, number, number]; zoom: number};
 *
 * const store = createStore({viewerUrl: 'http://localhost:8080', token});
 * let viewState: Camera = {target: [256, 256, 0], zoom: 0};
 *
 * // Tell the store where the camera is. viewInputOf returns null until /v1/meta has arrived.
 * function tell() {
 *   const input = viewInputOf(store, viewState, deck.width, deck.height);
 *   if (input) store.setView(input);
 * }
 *
 * const deck = new Deck<OrthographicView>({
 *   parent: document.getElementById('map') as HTMLDivElement,
 *   views: new OrthographicView({flipY: true}),
 *   viewState,
 *   controller: true,
 *   layers: [new MosaicaLayer({id: 'mosaica', store})],
 *   onViewStateChange: ({viewState: next}) => {
 *     viewState = next as Camera;
 *     deck.setProps({viewState});
 *     tell();
 *   }
 * });
 *
 * store.subscribe('meta', (meta) => {
 *   if (meta) tell();
 * });
 * ```
 *
 * @module @mosaicajs/deck
 */
export {MosaicaLayer, type MosaicaLayerProps, type LayerTimings} from './layer.js';
export {
  CATEGORY_PALETTES,
  DEFAULT_COLOURING,
  RAMPS,
  type CategoryPalette,
  type CategoryPaletteName,
  type Colouring,
  type Ramp,
  type RampName,
  type RampScale,
  type Rgb
} from './colour.js';
export {DEFAULT_SIZING, type SizeScale, type Sizing} from './size.js';
export {DEFAULT_DENSITY_SCALE, densityCountAt, densityPosition, type DensityCell, type DensityColours, type DensityCounts, type DensityMode, type DensityScale} from './density.js';
export {
  DEFAULT_DENSITY_CELL_PX,
  DENSITY_CELL_SIZES,
  DENSITY_SETTLE_MS,
  DensityCounter,
  cellDepth,
  nearestStop,
  resolutionStops,
  type DensityCamera,
  type DensitySettings,
  type ResolutionStop
} from './density-counter.js';
export {cameraFitting, viewInputOf, type OrthographicCamera} from './camera.js';
export {frameMatches} from './frame.js';
export {resolvePick, type Picked, type PickInfo} from './pick.js';
export {basemapLayers, basemapMisfit, type BasemapOptions, type BasemapValue, type WorldColours} from './basemap.js';
