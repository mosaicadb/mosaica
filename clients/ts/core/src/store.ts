import {Aggregates, isDrawn, isPaletteDrawn, joinedAggregate, type AggregateBody, type AggregateSpec, type AggregatesProjection} from './aggregates.js';
import {ArtifactChannel, requestLevels, servedLineage, type ArtifactChannelState, type ServedLineage} from './artifactChannel.js';
import {SessionArtifactTable, type ArtifactTable} from './artifactTable.js';
import {BandBudget, bandKey, type Band, type BandKey} from './bands.js';
import {tileRectOfBbox, type DepthChoice} from './budget.js';
import {MosaicaClient, MosaicaError, type MosaicaClientOptions} from './client.js';
import {ArtifactColours} from './colours.js';
import type {Composition} from './compose.js';
import {dataToWorldXY, gridToWorld, MAX_DEPTH, rectToRequestBbox, WORLD_SIZE} from './coords.js';
import {NO_COUNT, NO_MASKED, type Count, type Masked} from './counts.js';
import {RETRY_DEFAULTS, type Clock, type DriverOptions, type ViewState as DriverViewState} from './driver.js';
import {composeFilters, emptyDraft, withoutClause, type ClauseVerb, type FilterDraft} from './filters.js';
import {HeldRecords, HeldShapes} from './held.js';
import {HeldViews, type ViewMachinery} from './heldViews.js';
import {colourLayers, isFilterLayer, layerClosure} from './layers.js';
import {CLUSTER_PREFIX, EMPTY_LEGEND, Legend, type LegendProjection} from './legend.js';
import {withMembers, type MemberClause} from './members.js';
import {attachedTextOf} from './names.js';
import {DEFAULT_PALETTE, PALETTES, paletteSize, type ChosenColours, type PaletteName, type Rgba} from './palette.js';
import {worldBbox} from './prefetch.js';
import {rectContainsTile} from './rects.js';
import {Presenter, defaultFrameScheduler, isUnanswered, refusalOf, type FrameScheduler, type Presented, type PresentedStatus, type Refusal} from './presented.js';
import {insideBox, regionOperand, withRegion} from './region.js';
import type {RegionOperand} from './types.js';
import type {RecordsRead} from './records.js';
import {DEFAULT_CACHE_BYTES, Replica, type ReplicaOptions} from './replica.js';
import {SelectedRegion, type RegionProjection, type SelectionShape} from './selectedRegion.js';
import {Suggestions, type SuggestState} from './suggestions.js';
import {TokenSupply, type TokenSupplier} from './token.js';
import type {
  AggregateResult,
  AggregateTable,
  Artifact,
  ArtifactDetail,
  BrowsePage,
  BrowseRequest,
  FilterExpr,
  Grouping,
  ItemDetail,
  ItemsHead,
  ItemsRequest,
  Meta,
  Quantisation,
  Shape,
  ShapeKind,
  Timings,
  ViewportResult
} from './types.js';

/**
 * The headless store: told where the viewer is looking, it hands back what to draw, from its
 * cache, on its own schedule. It holds no camera; the host calls `setView`. It hands the vis side
 * a `Composition` by reference and typed counts, never pixels.
 *
 * The frame scheduler and the clock are injected, defaulting to `requestAnimationFrame` and
 * `setTimeout`, so the store runs in node against a fake client.
 *
 * The store composes parts that each own their state: the token ({@link TokenSupply}), each view's
 * replica, presenter and artifact channel ({@link HeldViews}), the typeahead ({@link Suggestions}),
 * shapes and hovered records ({@link HeldShapes}, {@link HeldRecords}), the selected region
 * ({@link SelectedRegion}), artifact colours ({@link ArtifactColours}) and
 * the legend ({@link Legend}). What remains here is what they share: `meta`, the current view id,
 * the camera, the layers, the filters and the projections.
 */

export type {Count, Masked} from './counts.js';
export {formatCount, formatMasked} from './counts.js';
export type {TokenSupplier} from './token.js';
export {CLUSTER_PREFIX, type LegendProjection} from './legend.js';
export {REGION_HELD_LIMIT, type RegionProjection, type SelectionShape} from './selectedRegion.js';
export type {AggregateEntry, AggregateSpec, AggregateSpecGrouping, AggregatesProjection, DrawnLayerBy, FieldSummary} from './aggregates.js';

/**
 * The camera position, as {@link Store.setView} takes it.
 *
 * @category Store
 */
export type ViewInput = {
  /**
   * The area the canvas shows, `[x0, y0, x1, y1]` in the current view's data coordinates. It may be
   * clamped to the view's extent, as `viewInputOf` in `@mosaicajs/deck` clamps it.
   */
  bbox: [number, number, number, number];
  /**
   * The camera's zoom, as deck.gl's `OrthographicView` counts it: `2 ** zoom` pixels per world unit,
   * so at 0 the 512-unit world is 512 pixels wide. It is passed separately because a clamped `bbox`
   * does not give the true zoom.
   */
  zoom: number;
  /** The canvas width in pixels. */
  width: number;
  /** The canvas height in pixels. */
  height: number;
};

/**
 * Options for {@link createStore}. Pass `viewerUrl` or `client`, and `token` or `authorise`. A
 * store with neither `token` nor `authorise` sends no credentials, which only an open server
 * (`serve.open`) accepts.
 *
 * @category Store
 */
export type StoreOptions = {
  /**
   * The viewer server's base URL, as for {@link MosaicaClientOptions.viewerUrl}. An empty string
   * means the page's own origin. Required unless `client` is given.
   */
  viewerUrl?: string;
  /** A viewer token, used for every request and never renewed. Takes precedence over `authorise`. */
  token?: string;
  /**
   * A function that returns a viewer token. The store calls it at the start and again before the
   * token expires; see {@link TokenSupplier}.
   */
  authorise?: TokenSupplier;
  /** The view to open with. Defaults to the first view in `meta.views`. */
  view?: string;
  /** How many marks the store aims to draw on screen. Defaults to {@link DEFAULT_BUDGET}. */
  budget?: number;
  /** The palette clusters are coloured from (see {@link PaletteName}). Defaults to `tableau10`. */
  palette?: PaletteName;
  /**
   * Whether to fetch ahead while the camera is still: the tiles around the view and one zoom level
   * deeper. Defaults to `true`.
   */
  prefetch?: boolean;
  /** Injected for tests; browser defaults otherwise. @internal */
  scheduler?: FrameScheduler;
  /** @internal */
  clock?: Clock;
  /** @internal */
  driver?: DriverOptions;
  /**
   * The tile cache, which every view shares. `cacheBytes` is the most it holds, evicting the tiles
   * drawn least recently; it defaults to 512 MiB. `cache: false` holds nothing, so every camera move
   * makes a request, which is useful for measuring the cache. `revalidateAfterMs` is how long the
   * store may answer the camera from the cache alone before it asks the server whether the counts
   * have changed; it defaults to `60000`. `onPhase(kind, ms, n)` reports how long each step inside
   * the cache took, for instrumentation.
   */
  replica?: Pick<ReplicaOptions, 'cacheBytes' | 'cache' | 'revalidateAfterMs' | 'onPhase'>;
  /**
   * How the drawn layers' artifacts are fetched. Artifacts come in tiles 128 to 256 pixels across,
   * and `perTile` is the most one level shows in a tile, largest first. It defaults to 50, or to
   * `meta.selection.maxArtifactsPerTile` if that is lower; a larger value is sent as given and the
   * server refuses it. `budget` is the starting cluster budget ({@link Store.setClusterBudget});
   * by default there is no limit. `heldTiles` is how many tiles of artifacts to keep per view; it
   * defaults to `4096`.
   */
  artifacts?: {perTile?: number; budget?: number; heldTiles?: number};
  /**
   * A `/v1/meta` response you have already fetched, so the store need not fetch it again. It must
   * have been fetched with this viewer's token.
   */
  meta?: Meta;
  /**
   * A {@link MosaicaClient} to send requests through, such as one shared between stores. By default
   * the store creates one from `viewerUrl` and `clientOptions`. You close a client you pass in;
   * {@link Store.dispose} closes only one the store created.
   */
  client?: MosaicaClient;
  /**
   * Options for the client the store creates, such as `headers`, `fetch` or `decoder`. Ignored if
   * `client` is given.
   */
  clientOptions?: Omit<MosaicaClientOptions, 'viewerUrl'>;
  /** Measurements the projections do not carry, for a demo's instrument panels. @internal */
  instruments?: {
    onFrame?(info: {plan: {choice: DepthChoice}; timings: Timings | null; bytes: number; held: number; fetched: number; calibration: {mTarget: number; visibleInView: number | undefined}; replica: {bytes: number; points: number; bands: number}}): void;
    onTrace?(kind: string, fields: Record<string, number | string>): void;
  };
};

/**
 * The store's projections, by name. A projection is a read-only snapshot of one part of the map's
 * state, such as the points to draw (`marks`) or the filters in force (`filters`). When one changes
 * the store replaces it with a new object, so comparing by identity tells you whether it changed.
 * {@link Store.subscribe} reports each replacement.
 *
 * @category Store
 */
export type Projections = {
  /**
   * The `/v1/meta` the store read or was given, or `null` before it has arrived, and after
   * {@link Store.clear} or an answer under another identity key until it is read again.
   */
  meta: Meta | null;
  /** Whether the current view's map is loading, shown, empty or refused. */
  status: StatusProjection;
  /** The current view's id, the frame on screen and the frame's counts. */
  view: ViewProjection;
  /** The marks on screen, for a renderer. */
  marks: MarksProjection;
  /** The tiles of the frame on screen. */
  tiles: TilesProjection;
  /** The artifacts served for the drawn layers, with their colours and shapes. */
  artifacts: ArtifactsProjection;
  /** The picked item and the opened artifact. */
  selection: SelectionProjection;
  /** The selected region and its counts, or `null` while nothing is selected. */
  region: RegionProjection | null;
  /** The filter controls, the expressions composed from them, the `member_of` clauses and the typeahead. */
  filters: FiltersProjection;
  /** The legend of the colour and size columns. */
  legend: LegendProjection;
  /** How much the store's tile cache holds. */
  replica: ReplicaProjection;
  /** Each aggregate registered with {@link Store.setAggregate}, by its id. */
  aggregates: AggregatesProjection;
};

/**
 * The name of one projection, a key of {@link Projections}. The names of map projections are a
 * different type, `ViewInfo['projection']`.
 *
 * @category Store
 */
export type ProjectionName = keyof Projections;

/**
 * The state of the current view's map, for a status line or an overlay.
 *
 * @category Projections
 */
export type StatusProjection = {
  /**
   * `idle` before the first request and after {@link Store.clear}. `loading` while a request is out,
   * or from a view switch until the incoming view draws. `retrying` while a request the server shed
   * (`429`) or answered as still starting (`503`) waits to be sent again. `shown` when a frame
   * answered. `empty` when the answer holds no item this viewer can see. `refused` when the request,
   * or the store's read of `/v1/meta`, was refused. Show counts only while it is `shown`, and do
   * not show `empty` and `refused` alike: `refused` says nothing about what exists.
   */
  status: PresentedStatus;
  /** Whether a frame has been shown or has drawn marks since the store was made or last cleared. */
  sessionWarm: boolean;
  /**
   * The refusal while `status` is `refused`, else `null`. A `5xx` whose body names no Mosaica code,
   * such as a gateway's `504`, has the code `http-error`: the server did not answer, and nothing
   * about this viewer was refused.
   */
  refusal: Refusal | null;
  /**
   * Whether a response since the frame on screen was fetched reported that the data has changed,
   * so the counts on screen may be out of date. It clears when a frame fetched after the change is
   * drawn. Pass it to {@link formatCount} and {@link formatMasked} as `stale`.
   */
  stale: boolean;
  /**
   * Whether the refusal means the session ended: an `expired-token`, or a `bad-credential` for a
   * token this store has already used. A new token is needed.
   */
  expired: boolean;
  /** Whether `status` is `retrying`. */
  retrying: boolean;
  /**
   * Whether the server cannot be reached: a request failed to connect, or a proxy in front of it
   * answered `502`. While it is set the store asks `/v1/meta` again, waiting 1 s, then twice as
   * long each time up to 30 s, and an outage within a minute of the last one goes on waiting as
   * long. Any answer clears it, and the store asks again for what is drawn with the token it holds.
   * The store asks `authorise` for a new token only where the server answers `401` to one that had
   * worked. A store given a `client` does not watch its requests, and this stays `false`.
   */
  unreachable: boolean;
};

/**
 * The current view and the frame on screen, with the frame's counts. Each count is summed over the
 * frame's exact tiles and taken from within this viewer's visible set.
 *
 * @category Projections
 */
export type ViewProjection = {
  /**
   * The view the store answers from: `''` before `/v1/meta` arrives, then an id `meta.views` lists.
   * A component compares it to learn that a switch happened.
   */
  id: string;
  /** The composition on screen, by reference; `null` before the view's first frame. */
  composition: Composition | null;
  /** The tile depth the frame was composed at; `0` before the first frame. */
  depth: number;
  /** How many items this viewer can see in the frame's exact tiles. */
  visible: Masked;
  /**
   * How many of `visible` the request's `filters` admit (see {@link Store.requestFilters}). Equal to
   * `visible` where no filter is set.
   */
  matched: Masked;
  /**
   * How many of `matched` the request's `highlight` also admits. Equal to `matched` where no
   * highlight is set, and also where a highlight admits everything the filter does, so
   * `highlighting` says whether one is set.
   */
  highlighted: Masked;
  /** Whether a `highlight` was set when this frame was presented. */
  highlighting: boolean;
  /** The marks the server sent for the exact tiles (`shown`) against `visible` (`total`). */
  served: Count;
  /**
   * How many stand-in marks are on screen, drawn from another depth while the frame's own tiles
   * arrive. A plain mark count, not a masked count.
   */
  provisional: number;
  /** The counts over the camera's box or the selected region; `null` before the view's first. */
  inView: InViewCounts | null;
};

/**
 * Counts over the camera's box, or over the selected region while one is selected, from `POST
 * /v1/aggregate`. They are asked for once the camera has rested for 250 ms, and again when the
 * filters, the highlight or the selection change. The frame's own counts cover whole tiles, which
 * reach past the box by an amount that changes with the depth drawn; these count the area itself,
 * so `visible` does not move when only a filter changes.
 *
 * @category Projections
 */
export type InViewCounts = {
  /**
   * `loading` while a count for the current camera, filters and selection is out, the previous
   * figures standing until it lands; `shown` once it has; `refused` where it was refused.
   */
  status: 'loading' | 'shown' | 'refused';
  /** The items this viewer can see in the area. */
  visible: Masked;
  /** How many of `visible` the filters admit. */
  matched: Masked;
  /** How many of `matched` the highlight also admits; equal to `matched` where none is set. */
  highlighted: Masked;
  /** The held marks of the frame's exact tiles that lie in the area. */
  shown: number;
};

/**
 * The marks on screen, for a renderer.
 *
 * @category Projections
 */
export type MarksProjection = {
  /** The frame's exact bands: the points served for the frame's own tiles, by reference. */
  bands: Composition['exact'];
  /**
   * Stand-in pieces, coarsest first: points from bands of another depth, drawn where the frame's own
   * tiles have not arrived. Each draws its band's `indices`, or its first `limit` points.
   */
  standIn: Composition['standIn'];
  /** The marks drawn from exact bands (`shown`) against the frame's `visible` count (`total`). */
  count: Count;
};

/**
 * The tiles of the frame on screen.
 *
 * @category Projections
 */
export type TilesProjection = {
  /**
   * One entry per contributing tile: its address, whether it is exact, how many marks it draws and,
   * for an exact tile or one whose counts have landed before its points, the server's counts.
   */
  tiles: Composition['tiles'];
};

/**
 * The artifacts served for the current view's drawn layers, with what a renderer needs to colour
 * and outline them. An artifact this viewer may not see is absent, as one that does not exist is.
 *
 * @category Projections
 */
export type ArtifactsProjection = {
  /** The first layer drawn, or `null` for none. */
  layer: string | null;
  /**
   * Every layer drawn, with its closure, and without filter layers. The colour layer is here only
   * when it is drawn too.
   */
  layers: string[];
  /** The served artifacts of the drawn layers, for the current camera. */
  served: Artifact[];
  /**
   * The served artifacts of the layer `setColourBy('cluster:<layer>')` names, whether it is drawn
   * or not; else empty.
   */
  colourServed: Artifact[];
  /** The forest `served` forms through its parent links. */
  lineage: ServedLineage;
  /**
   * The text of each attached artifact the channel serves (a clustering's topic labels), keyed by
   * the artifact it is attached to, over the drawn layers and the colour layer. The names
   * {@link artifactName} gives the artifacts in `served` and `colourServed`.
   */
  attached: ReadonlyMap<bigint, string>;
  /**
   * `idle` before the first answer and while no layer is asked for; `loading` while a request is
   * out; `shown` when `served` answers the current camera; `refused` when the request was refused,
   * `served` then being empty.
   */
  status: ArtifactChannelState['status'];
  /** The refusal while `status` is `refused`, else `null`. */
  refusal: Refusal | null;
  /** A number that increases each time `served` is replaced. */
  version: number;
  /** How many artifact payloads the session holds: the served set and those kept from earlier answers. */
  held: number;
  /**
   * The session artifact table, shared by every view, which resolves the ordinals in a band's
   * membership columns to artifacts.
   */
  table: ArtifactTable;
  /** The ordinals of `served` in `table`. */
  servedOrdinals: ReadonlySet<number>;
  /**
   * Shapes by `mosaicaId`: those {@link Store.needShape} fetched, simplified for the view's zoom,
   * and those {@link Store.openArtifact} fetched, at full detail. An artifact with no entry has not
   * been asked for, has not answered or was refused, and is drawn as its `box`. A view switch,
   * {@link Store.clear} and an answer under another identity key empty it.
   */
  shapes: ReadonlyMap<bigint, Shape>;
  /**
   * A colour for every ordinal `table` holds. A band fetched under a coarser cut names artifacts
   * outside `servedOrdinals`, and its points take those artifacts' colours. An ordinal not here
   * has no colour of its own.
   */
  colours: ReadonlyMap<number, Rgba>;
  /** The palette `colours` was built under. Its size is the `palette_size` the artifacts are asked with. */
  palette: PaletteName;
  /**
   * The colours set by layer and `mosaica_id` with {@link Store.setArtifactColours}, which
   * `colours` holds in place of the palette's.
   */
  overrides: ChosenColours;
  /**
   * How many bands in view have a colour for every point under each layer asked for (`current`),
   * and how many do not and are being fetched again (`stale`). Both are `0` unless `status` is
   * `shown` and a layer is asked for.
   */
  coverage: {current: number; stale: number};
};

/**
 * The picked item and the opened artifact, for a card. An id this viewer cannot see is refused as
 * an id that does not exist is.
 *
 * @category Projections
 */
export type SelectionProjection = {
  /**
   * The item {@link Store.pick} last fetched: its `mosaica_id` and its record. `null` before a
   * pick, after a refused one, and once {@link Store.openArtifact} succeeds.
   */
  item: {id: bigint; detail: ItemDetail} | null;
  /** The refusal of the last pick, else `null`. */
  itemRefusal: Refusal | null;
  /**
   * The artifact {@link Store.openArtifact} last fetched: its `mosaica_id` and its detail. `null`
   * before one is opened and after a refusal.
   */
  artifact: {id: bigint; detail: ArtifactDetail} | null;
  /** The refusal of the last `openArtifact`, else `null`. */
  artifactRefusal: Refusal | null;
};

/**
 * The filter controls, the expressions composed from them, the `member_of` clauses and the
 * category typeahead.
 *
 * @category Projections
 */
export type FiltersProjection = {
  /**
   * The controls as `setFilters` last set them. Until it is called, the store seeds one empty
   * control per filterable column ({@link emptyDraft}) when `/v1/meta` arrives.
   */
  draft: FilterDraft;
  /**
   * The controls in the `filter` position, composed; `null` for none. The `member_of` clauses and
   * the region are not here; {@link Store.requestFilters} gives the whole expression.
   */
  expr: FilterExpr | null;
  /**
   * The controls in the `highlight` position, composed; `null` for none. The `member_of` clauses in
   * that position are not here, though a request's `highlight` carries them.
   */
  highlight: FilterExpr | null;
  /** The `member_of` clauses, in either position, as `setMembers` last set them. */
  members: MemberClause[];
} & SuggestState;

/**
 * How much the store's tile cache holds.
 *
 * @category Projections
 */
export type ReplicaProjection = {
  /** Bytes held across every view's bands, the figure `replica.cacheBytes` bounds. */
  bytes: number;
  /** Points held for the current view. */
  points: number;
  /** Bands held for the current view. A band is one tile's points. */
  bands: number;
  /** How many views hold any band. */
  views: number;
  /**
   * For the last frame composed in full, rather than refreshed from the frame before: how many of
   * the tiles it wanted were already held (`held`) and how many were not (`fetched`). `null` before
   * the first such frame and after `clear`.
   */
  lastPlan: {held: number; fetched: number} | null;
};

type Listener = () => void;

/**
 * The request {@link Store.items} takes: an {@link ItemsRequest} whose `view` and `filters` default
 * to the store's, with `inView` to read only the area in view.
 *
 * @category Store
 */
export type StoreItemsRequest = Omit<ItemsRequest, 'view' | 'filters'> & {
  /** The view to read. Defaults to the current view. */
  view?: string;
  /**
   * Defaults to {@link Store.requestFilters}, so the read returns the items the map counts as
   * matching. `null` reads every item the viewer may see.
   */
  filters?: FilterExpr | null;
  /**
   * Read only the area the in-view counts cover: the selected region if there is one, otherwise the
   * camera's box. Current view only.
   */
  inView?: boolean;
};

/**
 * The store is the client-side state of a map. You give it the camera position, filters and layer
 * settings, and it fetches, caches and composes the data. It publishes the result as
 * {@link Projections}, read-only snapshots of one part of the map's state each. Read a projection
 * with `get` or `projections`, and hear of changes with `subscribe`.
 *
 * Every count, artifact and record the store publishes covers only what the viewer may see.
 * Methods that need the server's metadata (`/v1/meta`) wait for it to load.
 *
 * A store holds data for one viewer. To show the map to a different viewer, call
 * {@link Store.clear} or create a new store. The store calls `authorise` only to renew its token,
 * so if `authorise` starts returning another viewer's token without a `clear`, the previous
 * viewer's data stays on screen until the next renewal. The store also discards what it holds and
 * loads again when the server's data is compacted or rebuilt.
 *
 * @category Store
 */
export interface Store {
  /**
   * The current value of every projection. The store replaces this object's fields as they change,
   * so read a field each time you need it instead of keeping it.
   */
  readonly projections: Projections;
  /** The current value of one projection. */
  get<K extends ProjectionName>(name: K): Projections[K];
  /**
   * Call `fn` whenever any projection changes. An error thrown by `fn` is logged to the console and
   * does not stop the other subscribers.
   *
   * @returns A function that unsubscribes `fn`.
   */
  subscribe(fn: Listener): () => void;
  /**
   * Call `fn` with the new value whenever projection `name` changes. It is not called with the
   * current value; use `get` for that. An error thrown by `fn` is logged to the console and does not
   * stop the other subscribers.
   *
   * @returns A function that unsubscribes `fn`.
   */
  subscribe<K extends ProjectionName>(name: K, fn: (value: Projections[K]) => void): () => void;

  /**
   * Tell the store where the camera is. Call it whenever the camera moves. The store waits briefly
   * for the camera to settle, then fetches what the view needs and publishes each frame as it is
   * composed.
   */
  setView(input: ViewInput): void;
  /** Replace the filter controls with `draft`. Publishes `filters` and fetches the map again. */
  setFilters(draft: FilterDraft): void;
  /**
   * Replace the `member_of` clauses, which keep the map to items inside or outside given
   * artifacts: a card's "filter to this" and "outside this", or the nodes ticked in a hierarchy
   * panel. Pass the whole list; {@link withMember} and {@link withoutMember} build it. Publishes
   * `filters.members` and fetches the map again.
   */
  setMembers(clauses: readonly MemberClause[]): void;
  /**
   * Read one page of a layer's hierarchy: its roots, an artifact's children and parents, or the
   * results of a name search. Each row carries its count of visible items. An artifact the store
   * has not been served returns an empty page.
   *
   * @param req - The page to read. `filters` defaults to {@link Store.requestFilters}, so the counts
   *   match the map's; pass `null` to count without filters. `view` defaults to the current view.
   * @throws {@link MosaicaError} if the server refuses the request, for example with `422` for an
   *   unknown layer.
   */
  browse(req: Omit<BrowseRequest, 'filters' | 'view'> & {filters?: FilterExpr | null; view?: string}): Promise<BrowsePage>;
  /**
   * Read the items that match the map's filters, one page at a time. This is
   * {@link MosaicaClient.items} with the store's token, view and filters filled in. It resolves when
   * the first page's header arrives; call `next()` on the result for each page, as an Arrow table.
   * Pages are fetched only as you ask for them.
   *
   * ```ts
   * const read = await store.items({fields: ['title'], pageRows: 50, count: true, inView: true});
   * const first = await read.next(); // read.head.matched counts every match
   * const second = await read.next(); // the next 50
   * ```
   *
   * A read keeps the filters it started with, so start a new one after the filters change. With
   * `inView`, a call made before the map has set the camera or a region has been selected waits for
   * one, then reads that area.
   *
   * @param req - The read. `view` and `filters` default to the store's; see {@link StoreItemsRequest}.
   * @param signal - Aborts the read.
   * @throws {@link MosaicaError} if the server refuses the first request. `Error` if `inView` is set
   *   with a view other than the current one. A `DOMException` named `AbortError` if the store is
   *   disposed while the call waits for the camera, or `signal`'s reason if it is aborted.
   */
  items(req: StoreItemsRequest, signal?: AbortSignal): Promise<RecordsRead<ItemsHead>>;
  /**
   * The filter expression the store sends with every request: the filter controls, the
   * `member_of` clauses and the selected region, combined with `all_of`. Controls and clauses set
   * to highlight are not included. `null` when nothing is set. Artifact ids in the expression are
   * decimal strings, so it can be serialised as JSON.
   */
  requestFilters(): FilterExpr | null;
  /**
   * Keep a live aggregate of the current view under `id`, published in the `aggregates`
   * projection. Pass `null` to drop it.
   *
   * The aggregate counts what the map counts ({@link Store.requestFilters}). `spec.without` leaves
   * one column's filter out, so a filter control can still show the values its own clause
   * excludes; `spec.withoutMembersOf` does the same for a layer's `member_of` clauses.
   * `spec.subject: 'view'` limits the counts to the area in view, and `'visible'` ignores the
   * filters and counts everything the viewer may see.
   *
   * The store asks again whenever the filters, highlight, selected region or view change, and for
   * a `view` aggregate once the camera comes to rest. Each call sends a new request, so call it
   * when the spec changes, not on every render.
   *
   * @example A field card's histogram of `year`, in view and over everything the filters match,
   * each leaving out the card's own clause, and the field's figures:
   * ```ts
   * const bins: Grouping = {count: true, by: {field: 'year', bins: 24, sample: 100_000}};
   * store.setAggregate('year:subject', {groupings: [bins], subject: 'view', without: 'year', highlighted: true});
   * store.setAggregate('year:match', {groupings: [bins], without: 'year'});
   * store.setAggregate('year:summary', {groupings: [{summary: ['year']}], subject: 'visible'});
   * ```
   */
  setAggregate(id: string, spec: AggregateSpec | null): void;
  /**
   * Fetch typeahead suggestions for the category column `column` that match `q`, for a control set
   * to `verb`. The page lands in `filters.suggestions[column]`, each value with its count in the
   * current view, and an error lands in `filters.suggestErrors[column]`.
   *
   * Under `filter`, the counts leave out the column's own clause, so each shows what choosing that
   * value as well would add. Under `highlight` they use every filter, so a value the filters
   * exclude counts 0.
   *
   * Calls are debounced by 120 ms. The store fetches the suggestions again when the filters or the
   * region change, until {@link Store.forgetSuggestions} is called. A busy server is asked up to
   * five times before the error is `backpressure`.
   */
  suggest(column: string, q: string, verb: ClauseVerb): void;
  /**
   * Stop fetching suggestions for `column` and drop its page and error. Call it when a search box is
   * emptied or removed.
   */
  forgetSuggestions(column: string): void;
  /**
   * Draw the artifacts of these layers, each together with the layers it is linked to by
   * `depends_on` ({@link layerClosure}). Pass `[]` to draw none. Filter layers are ignored.
   * Publishes `artifacts`.
   */
  setLayers(names: string[]): void;
  /**
   * Colour points by a declared column, by a layer's clusters with `cluster:<layer>` for a layer
   * {@link colourLayers} lists, or all alike with `null`. Publishes `legend`. Colouring by a layer
   * fetches its artifacts and labels without drawing them. A layer that cannot colour leaves the
   * points uniform. Points already held are fetched again with the new column.
   */
  setColourBy(column: string | null): void;
  /**
   * Size points by a declared number column, or give every point the same size with `null`. With
   * `rank: true` points are sized by rank instead of value. Publishes `legend`, whose `domains` and
   * `missing` fill in from the points drawn. A column that is not a rendered number column sizes
   * nothing.
   */
  setSizeBy(column: string | null, options?: {rank?: boolean}): void;
  /**
   * Ask for extra render columns on every point on behalf of `id`, such as the fields a hover reads
   * from {@link Band.scalars}. The server sends only the render columns that have been asked for
   * here or by `setColourBy` and `setSizeBy`. Pass `[]` to withdraw `id`'s request. Names that are
   * not render columns of the current view are ignored. Adding a column fetches the held points
   * again, and they stay drawn until the new ones arrive. The request applies to every view.
   */
  setPointColumns(id: string, columns: readonly string[]): void;
  /**
   * Colour clusters from `palette`. Publishes `artifacts.colours` and `artifacts.palette`. A name
   * {@link PALETTES} does not list is ignored. Changing to a palette of a different size fetches the
   * artifacts again.
   */
  setPalette(palette: PaletteName): void;
  /**
   * Give particular artifacts their own colours, keyed by layer and then by `mosaica_id`. Every
   * other artifact keeps its palette colour. Each call replaces the last, and an empty map clears
   * them. Publishes `artifacts.colours` and `artifacts.overrides`.
   */
  setArtifactColours(colours: ChosenColours): void;
  /**
   * Set how many marks the store aims to draw on screen in every view, and fetch the map again.
   * Anything but a positive finite number is ignored.
   */
  setBudget(budget: number): void;
  /** How many marks the store aims to draw. Set by `budget` in the options or by `setBudget`. */
  readonly budget: number;
  /**
   * Limit every `nested` and `dag` layer to at most `budget` artifacts by drawing ancestors in place
   * of their descendants. `null` removes the limit. The store fetches the affected layers again,
   * and any aggregate that ranks a layer at `cut: 'drawn'`. Anything but a positive whole number or
   * `null` is ignored.
   */
  setClusterBudget(budget: number | null): void;
  /**
   * The most artifacts a `nested` or `dag` layer is drawn with, or `null` for no limit. Set by
   * `artifacts.budget` in the options or by `setClusterBudget`.
   */
  readonly clusterBudget: number | null;
  /**
   * Switch to view `id`. An id `meta.views` does not list is ignored. Called before `/v1/meta` has
   * loaded, it sets the view to open with.
   *
   * Between views that share an extent, the camera and the selected region carry over and the new
   * view draws what it already holds at once. A view with another extent drops both, and the next
   * `setView` sets the camera. A switch clears the marks, the tiles, the typeahead pages and the
   * fetched shapes, and publishes `status` as `loading`.
   */
  setCurrentView(id: string): void;
  /**
   * The extent the current view's positions are quantised against, in data coordinates. `null`
   * until `/v1/meta` has loaded.
   */
  frame(): Quantisation | null;
  /**
   * Fetch one item's record and publish it as `selection.item`. An opened artifact stays open. If
   * the request is refused, the refusal is published as `selection.itemRefusal` and the promise
   * still resolves. An item the viewer cannot see is refused as `unknown`, the same as one that
   * does not exist.
   */
  pick(id: bigint): Promise<void>;
  /**
   * Fetch one item's fields for a hover. Each id is fetched once and the answer kept. Resolves to
   * `null` if the request was refused. Publishes nothing; {@link Store.pick} opens a card.
   */
  describe(id: bigint): Promise<Record<string, unknown> | null>;
  /**
   * The display title of `key`, a value of the category column `column`. Resolves to `null` if the
   * value has no title, if `column` is not a category column a filter can name, or if the request
   * was refused. Every value of a `public` vocabulary has its title. A value of a `derived`
   * vocabulary has one only if the viewer may see an item that carries it. Titles are kept once
   * fetched, and lookups made together go as one request.
   *
   * @example The title of an opened item's archive
   * ```ts
   * map.addEventListener('mosaica-itemopen', async (e) => {
   *   const title = await store.titleOf('archive', String(e.detail.record.fields.archive));
   * });
   * ```
   */
  titleOf(column: string, key: string): Promise<string | null>;
  /**
   * The title {@link Store.titleOf} has already fetched, without a request. `null` if the value has
   * no title. `undefined` if none is held, including while a lookup is in flight and after a
   * refusal.
   */
  heldTitle(column: string, key: string): string | null | undefined;
  /**
   * Fetch one artifact's details and publish them as `selection.artifact`, clearing
   * `selection.item`. Its full-detail shape goes into `artifacts.shapes`. If the request is refused,
   * the refusal is published as `selection.artifactRefusal` and the promise still resolves. An
   * artifact the viewer cannot reach is refused the same as one that does not exist.
   */
  openArtifact(id: bigint): Promise<void>;
  /**
   * Fetch one artifact's outline into `artifacts.shapes`, simplified for the current zoom. Map tiles
   * carry only each artifact's centre and box. A shape is fetched at most once, so this can be
   * called on every pointer move.
   */
  needShape(id: bigint): void;
  /**
   * Clear the picked item, the opened artifact and their refusals. A pick or open still in flight is
   * not shown. Publishes `selection`.
   */
  clearSelection(): void;
  /**
   * Select a region, or clear the selection with `null` or a lasso of fewer than three points. The
   * region becomes part of every request's filters, so the map is fetched again. Publishes `region`.
   */
  select(shape: SelectionShape | null): void;
  /**
   * A served artifact's box in the current view's data coordinates, `[x0, y0, x1, y1]`. `null` if
   * the artifact is not in `artifacts.served` or has no box.
   */
  extentOf(artifactId: bigint): [number, number, number, number] | null;
  /**
   * Convert a world position to the current view's data coordinates. World space runs from `0` to
   * {@link WORLD_SIZE} on each axis across the view's extent.
   *
   * @throws `Error` if `/v1/meta` has not loaded.
   */
  dataXY(worldX: number, worldY: number): [number, number];
  /**
   * Forget the current viewer, so the map can be shown to another. The store drops its token, its
   * metadata, everything it has fetched, the `member_of` clauses, the region and the selection, and
   * abandons every request in flight. It publishes empty projections with `status` as `idle`, then
   * asks `authorise` for a new token and loads again. A store created with a fixed `token` keeps it.
   *
   * The filters, layers, colouring, point columns, current view and camera are kept. Any that name
   * a view, layer or column the new metadata does not list are dropped, and if the current view is
   * not listed the store opens on the first one.
   */
  clear(): void;
  /**
   * Fetch the world outline for a basemap ({@link MosaicaClient.basemapWorld}). The outline is the
   * same for every viewer, so the stores on a page that use one server share one download. A failed
   * fetch is not kept, so the next call tries again.
   */
  basemapWorld(): Promise<ArrayBuffer>;
  /**
   * Fetch the current view again, as after a refusal. The artifacts are fetched afresh, and the
   * points wherever the held tiles do not cover the view.
   */
  refresh(): void;
  /**
   * Stop every timer and request. Closes the client if the store created it; a client passed as
   * `client` is left for you to close.
   */
  dispose(): void;
}

/** The world outline by server: the client a host shared between stores, else the URL a store's own client reaches, as {@link serverOf} names it. */
const worldOutlines = new WeakMap<object, Promise<ArrayBuffer>>();
const servers = new Map<string, object>();

/** One object per server URL, so a URL keys {@link worldOutlines} as a shared client does. */
function serverOf(url: string): object {
  let server = servers.get(url);
  if (!server) servers.set(url, (server = {}));
  return server;
}

/** The `view` projection before a frame has been drawn in view `id`. */
function noFrame(id: string): ViewProjection {
  return {id, composition: null, depth: 0, visible: NO_MASKED, matched: NO_MASKED, highlighted: NO_MASKED, highlighting: false, served: NO_COUNT, provisional: 0, inView: null};
}

/**
 * The camera's centre on one axis, from the box it shows `[lo, hi]` in world units and half the
 * canvas at its zoom. Where the box is narrower than the canvas it was clamped to the world on the
 * side that meets an edge; where it meets both, the world's middle shows the same box.
 */
function cameraCentre(lo: number, hi: number, half: number): number {
  const clamped = hi - lo < 2 * half * (1 - 1e-9);
  if (clamped && lo <= 0 && hi < WORLD_SIZE) return hi - half;
  if (clamped && hi >= WORLD_SIZE && lo > 0) return lo + half;
  return (lo + hi) / 2;
}

/** How long the camera rests before the counts in view are asked for. */
const IN_VIEW_REST_MS = 250;

/** How many of a colour column's commonest values the legend ranks by count: more than any category palette holds. */
const RANKED_VALUES = 32;

/**
 * How many marks a store aims to draw on screen when {@link StoreOptions.budget} is not given.
 *
 * @category Store
 */
export const DEFAULT_BUDGET = 250_000;

/** The first wait before `/v1/meta` is asked again while the server cannot be reached, and the longest. */
const PROBE_FIRST_MS = 1_000;
const PROBE_MAX_MS = 30_000;

/** The keys of `table`'s listed groups, in its order. */
function listedKeys(table: AggregateTable | undefined): string[] {
  const rows = table?.rows;
  const group = rows?.getChild('group');
  const key = rows?.getChild('key');
  const keys: string[] = [];
  for (let i = 0; rows && group && key && i < rows.numRows; i++) if (group.get(i) === 'listed') keys.push(String(key.get(i)));
  return keys;
}

/** The title of the listed group `key` in `table`, or `null` where it is not listed or has none. */
function listedTitle(table: AggregateTable | undefined, key: string): string | null {
  const rows = table?.rows;
  const group = rows?.getChild('group');
  const keys = rows?.getChild('key');
  const titles = rows?.getChild('title');
  for (let i = 0; rows && group && keys && titles && i < rows.numRows; i++) {
    if (group.get(i) !== 'listed' || String(keys.get(i)) !== key) continue;
    const title = titles.get(i) as unknown;
    return typeof title === 'string' ? title : null;
  }
  return null;
}

/** The bearer token in a request's headers, or `null`. */
function bearerOf(headers: HeadersInit | undefined): string | null {
  const value = headers instanceof Headers ? headers.get('authorization') : Array.isArray(headers) ? (headers.find(([k]) => k.toLowerCase() === 'authorization')?.[1] ?? null) : (headers?.['authorization'] ?? null);
  return value?.startsWith('Bearer ') ? value.slice(7) : null;
}

/** How many tiles of artifacts a view keeps where `artifacts.heldTiles` names no number. */
const DEFAULT_HELD_TILES = 4096;
/** The `perTile` asked for where the caller gives none, when the deployment allows that many. */
const DEFAULT_ARTIFACTS_PER_TILE = 50;

const NO_STATUS: StatusProjection = {
  status: 'idle',
  sessionWarm: false,
  refusal: null,
  stale: false,
  expired: false,
  retrying: false,
  unreachable: false
};

/**
 * Create a store and start loading the server's metadata (`/v1/meta`), unless `options.meta`
 * provides it. If the server refuses, `status` is published as `refused`, and the store tries again
 * on the next call that needs the metadata or when `authorise` supplies a new token.
 *
 * @throws `Error` if `options` has neither `viewerUrl` nor `client`.
 * @category Store
 */
export function createStore(options: StoreOptions): Store {
  const scheduler = options.scheduler ?? defaultFrameScheduler();
  const clock: Clock =
    options.clock ??
    ({
      now: () => (typeof performance !== 'undefined' ? performance.now() : Date.now()),
      after: (ms, fire) => setTimeout(fire, ms),
      cancel: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>)
    } as Clock);

  const ownsClient = !options.client;
  if (ownsClient && options.viewerUrl === undefined) throw new Error('createStore needs `viewerUrl`, or a `client` to send requests through.');
  const client =
    options.client ?? new MosaicaClient({viewerUrl: options.viewerUrl!, ...options.clientOptions, fetch: watched(options.clientOptions?.fetch)});

  const table = new SessionArtifactTable();

  let meta: Meta | null = null;
  let budget = options.budget ?? DEFAULT_BUDGET;
  let clusterBudget: number | null = options.artifacts?.budget ?? null;
  let contentKeyAtFrame = '';
  /** The content key the registered aggregates were last asked under, by {@link observeCorpus}. */
  let corpusKey = '';

  /** One byte budget across every view's bands, evicted least recently drawn across them. */
  let bandBudget = new BandBudget(options.replica?.cacheBytes ?? DEFAULT_CACHE_BYTES);
  const views = new HeldViews(clock, buildView, options.view ?? '');

  /** The layers drawn, with their closure, as `setLayers` last named them. */
  let layersOn: string[] = [];
  let lastView: {input: ViewInput} | null = null;
  /** A `setView` before meta. */
  let queuedView: ViewInput | null = null;
  /** A `setCurrentView` before meta, applied at warm-up in place of `options.view`. */
  let queuedCurrentView: string | null = null;
  /**
   * A switch published `loading`, and a frame drawn from the incoming view's held bands ends it,
   * since no request's status will. Cleared by {@link setView}, after which a cold switch stays at
   * `loading` until its request answers.
   */
  let awaitingSwitchFrame = false;

  const tokens = new TokenSupply(options.authorise, options.token, clock, (changed) => {
    // A renewal keeps what is held. Until an answer under the new token confirms a key held, `admit`
    // does not trust an answer on a view that holds no key, and one ask goes out at once so the key
    // is seen without waiting for the camera. A store holding `meta` and no key yet has nothing to
    // compare the next answer with, so it reads `meta` again.
    if (changed && identities.size > 0) {
      unconfirmed = tokens.current;
      void askIdentity();
    } else if (changed && meta !== null) {
      forgetAnswers();
    }
    // A warm-up that failed for want of a token runs again now there is one.
    if (meta === null) void ready().catch(() => {});
  });

  /** See {@link StatusProjection.unreachable}. */
  let unreachable = false;
  /** The next ask of `/v1/meta` while the server cannot be reached, and how many have gone before it. */
  let probe: unknown = null;
  let probes = 0;
  /** When the last outage ended, by the store's clock. An outage soon after it waits on as the last one did. */
  let recoveredAt = -Infinity;
  /** The token a request last succeeded with. A `401` on it means its session ended. */
  let confirmedToken: string | null = null;

  /**
   * `fetch` for the client the store builds, reporting each request's outcome: a request that
   * could not reach the server, and the status of one that did.
   */
  function watched(base: typeof fetch | undefined): typeof fetch {
    const send = base ?? ((input: RequestInfo | URL, init?: RequestInit) => fetch(input, init));
    return async (input, init) => {
      const token = bearerOf(init?.headers);
      let response: Response;
      try {
        response = await send(input, init);
      } catch (error) {
        if (!init?.signal?.aborted && !(error instanceof DOMException && error.name === 'AbortError')) lost();
        throw error;
      }
      if (response.status === 502) lost();
      else answered(response.status, token);
      return response;
    };
  }

  /** A request could not reach the server: say so, and ask `/v1/meta` again after a wait. */
  function lost(): void {
    if (disposed) return;
    if (!unreachable) {
      unreachable = true;
      if (clock.now() - recoveredAt > 2 * PROBE_MAX_MS) probes = 0;
      replaceProjection('status', {...projections.status, unreachable: true});
    }
    if (probe === null) probe = clock.after(Math.min(PROBE_FIRST_MS * 2 ** probes, PROBE_MAX_MS), askAgain);
  }

  /**
   * Ask `/v1/meta` while the server cannot be reached, or read it where the store never has. Any
   * answer ends the outage; a failure to reach the server again has {@link lost} wait longer.
   */
  function askAgain(): void {
    probe = null;
    probes += 1;
    if (disposed || !unreachable) return;
    const asked: Promise<unknown> = meta === null ? ready() : client.meta(tokens.current ?? '');
    asked.then(recover, () => {
      if (probe === null) recover();
    });
  }

  /** The server answers again: say so, and ask again for what is drawn. */
  function recover(): void {
    if (disposed || !unreachable) return;
    unreachable = false;
    recoveredAt = clock.now();
    if (probe !== null) clock.cancel(probe);
    probe = null;
    replaceProjection('status', {...projections.status, unreachable: false});
    if (meta !== null) refresh();
  }

  /**
   * The server answered `status` to a request made with `token`. A `401` on a token that had
   * worked ends its session, as a restart that keeps no sessions does: a store with `authorise`
   * asks it for a new token, once per token that had worked, and asks again for what is drawn.
   */
  function answered(status: number, token: string | null): void {
    if (disposed || token === null) return;
    if (status >= 200 && status < 300 && token === tokens.current) confirmedToken = token;
    if (status !== 401 || token !== confirmedToken || options.authorise === undefined || options.token !== undefined) return;
    confirmedToken = null;
    tokens.forget();
    if (meta === null) void ready().catch(() => {});
    else refresh();
  }

  /** The identity key of the answers held, by view; see {@link Store} for what it derives from. */
  const identities = new Map<string, string>();
  /** A token a renewal brought, until an answer under it matches a key in {@link identities}. */
  let unconfirmed: string | null = null;
  /** The next {@link askIdentity} after one that failed for a reason that may pass. */
  let identityRetry: unknown = null;

  const colours = new ArtifactColours(table, options.palette ?? DEFAULT_PALETTE, (map, palette, overrides) =>
    replaceProjection('artifacts', {...projections.artifacts, colours: map, palette, overrides})
  );
  /** The served-set version each colour-stale band was last fetched again under. */
  const colourAsked = new Map<BandKey, number>();
  /** The render columns each caller of {@link setPointColumns} asked for, by its id. */
  const pointColumnAsks = new Map<string, readonly string[]>();
  /** What the last {@link checkPointColumns} looked at, so a frame that changed none of it costs nothing. */
  let pointColumnsChecked: {replica: Replica; version: number; columns: string} | null = null;

  const legend = new Legend(
    async (column, codes) => client.categories(await tokens.get(), column, {codes, view: views.id}),
    (value) => replaceProjection('legend', value),
    (column) => rankingOf(column)
  );

  const suggestions = new Suggestions(
    clock,
    (column, verb) => (verb === 'highlight' ? requestFilters() : filtersWithout(column)),
    async (column, q, filters, signal) => {
      const asked = await viewed();
      return client.suggest(asked.token, column, q, {view: asked.view, counts: true, ...(filters === null ? {} : {filters}), signal});
    },
    (state) => replaceProjection('filters', {...projections.filters, ...state})
  );

  const shapes = new HeldShapes(
    async (id) => {
      const asked = await viewed();
      // Generalised to the pixel of the view's own zoom.
      const zoom = views.current?.presenter.view?.view.zoom;
      const detail = await client.artifact(asked.token, id, {view: asked.view, ...(zoom === undefined ? {} : {zoom})});
      return detail.shape;
    },
    shapeKindOf,
    (held) => replaceProjection('artifacts', {...projections.artifacts, shapes: held})
  );

  const region = new SelectedRegion({
    clock,
    frame: frameOrNull,
    extentOf,
    covered: (box, depth) => {
      const replica = views.current?.replica;
      return replica !== undefined && meta !== null && replica.novelIn(tileRectOfBbox(box, depth), depth, meta.selection.kMaxMarks) === 0;
    },
    publish: (value) => replaceProjection('region', value),
    trace: (kind, fields) => options.instruments?.onTrace?.(kind, fields)
  });

  /** `client.aggregate`, with the requests asked for in one task over the same set joined. */
  const aggregate = joinedAggregate(
    (token, req, signal) => client.aggregate(token, req, signal),
    () => meta?.selection.maxAggregateGroupings ?? 1
  );

  /** One registered aggregate's request, in the current view under the current token. */
  async function askRegistered(_spec: AggregateSpec, body: AggregateBody, signal: AbortSignal): Promise<{result: AggregateResult; view: string}> {
    const asked = await viewed();
    const result = await aggregate(asked.token, {view: asked.view, ...body}, signal);
    // A response cancelled before its first page carries no key.
    if (result.identityKey !== '' && !admit(asked.view, result.identityKey, asked.token)) throw identityChanged();
    return {result, view: asked.view};
  }

  const aggregates = new Aggregates(ready, askRegistered, (entries) => replaceProjection('aggregates', entries), clock, {...RETRY_DEFAULTS, ...options.driver}, aggregateBody);

  /**
   * The legend's asks for a colour column's commonest values over everything this viewer may see,
   * by column, sent and retried as registered aggregates are, and settled as each answer lands or
   * its refusal stands.
   */
  const rankingAsks = new Map<string, {resolve: (keys: string[]) => void; reject: (refusal: Refusal | null) => void}>();
  const ranking = new Aggregates(
    ready,
    askRegistered,
    (entries) => {
      for (const [column, entry] of entries) {
        const ask = rankingAsks.get(column);
        if (!ask || (entry.status !== 'shown' && entry.status !== 'refused')) continue;
        rankingAsks.delete(column);
        if (entry.status === 'shown' && entry.result) ask.resolve(listedKeys(entry.result.tables[0]));
        else ask.reject(entry.refusal);
        queueMicrotask(() => ranking.set(column, null));
      }
    },
    clock,
    {...RETRY_DEFAULTS, ...options.driver},
    aggregateBody
  );

  function rankingOf(column: string): Promise<string[]> {
    return new Promise((resolve, reject) => {
      rankingAsks.set(column, {resolve, reject});
      const top = Math.min(RANKED_VALUES, meta?.selection.maxAggregateTop ?? RANKED_VALUES);
      ranking.set(column, {groupings: [{count: true, by: {field: column, top}}], subject: 'visible'});
    });
  }

  /**
   * The titles of category values {@link Store.titleOf} was answered under the current `meta`, by
   * column and key; `null` for a value with no title. Emptied when the store forgets.
   */
  const titles = new Map<string, string | null>();
  /** The asks for a title in flight, by column and key, each with the `meta` it was asked under. */
  const titleAsks = new Map<string, {under: Meta; answer: Promise<string | null>; settle: (title: string | null) => void}>();
  const titling = new Aggregates(
    ready,
    askRegistered,
    (entries) => {
      for (const [id, entry] of entries) {
        const ask = titleAsks.get(id);
        if (!ask || (entry.status !== 'shown' && entry.status !== 'refused')) continue;
        titleAsks.delete(id);
        const kept = entry.status === 'shown' && meta === ask.under;
        const title = kept ? listedTitle(entry.result?.tables[0], id.slice(id.indexOf('\u0000') + 1)) : null;
        if (kept) titles.set(id, title);
        ask.settle(title);
        queueMicrotask(() => {
          if (!titleAsks.has(id)) titling.set(id, null);
        });
      }
    },
    clock,
    {...RETRY_DEFAULTS, ...options.driver},
    aggregateBody
  );

  /** Settled by `dispose`, so a title asked for while `/v1/meta` is awaited resolves. */
  let stopTitles: () => void = () => {};
  const titlesStopped = new Promise<void>((resolve) => (stopTitles = resolve));

  async function titleOf(column: string, key: string): Promise<string | null> {
    try {
      await Promise.race([ready(), titlesStopped]);
    } catch {
      return null;
    }
    if (disposed) return null;
    const under = meta;
    if (!under || !under.filterOperands.some((o) => o.column === column && o.family === 'category' && !o.scope)) return null;
    const id = `${column}\u0000${key}`;
    if (titles.has(id)) return titles.get(id)!;
    const asked = titleAsks.get(id);
    if (asked) return asked.answer;
    let settle: (title: string | null) => void = () => {};
    const answer = new Promise<string | null>((resolve) => (settle = resolve));
    titleAsks.set(id, {under, answer, settle});
    titling.set(id, {groupings: [{count: true, by: {field: column, values: [key]}}], subject: 'visible'});
    return answer;
  }

  /** Drop the titles held, settle every title asked for and not yet answered with `null`, and stop asking. */
  function dropTitles(): void {
    titles.clear();
    const asks = [...titleAsks];
    titleAsks.clear();
    for (const [id, ask] of asks) {
      ask.settle(null);
      titling.set(id, null);
    }
  }

  /**
   * The counts in view: `counts` is the set under the filters against the area as its reference,
   * `highlighted` the set under the highlight as well, registered only while one is set.
   */
  const inView = new Aggregates(
    ready,
    async (_spec, body, signal) => {
      const asked = await viewed();
      const result = await aggregate(asked.token, {view: asked.view, ...body}, signal);
      const box = boxOf.get(body);
      if (box) boxOf.set(result, box);
      if (result.identityKey !== '' && !admit(asked.view, result.identityKey, asked.token)) throw identityChanged();
      return {result, view: asked.view};
    },
    (entries) => publishInView(entries),
    clock,
    {...RETRY_DEFAULTS, ...options.driver},
    (spec) => {
      const area = countedArea();
      if (area === null) return null;
      const groupings = drawnGroupings(spec);
      if (groupings === null) return null;
      const body = {groupings, filters: withArea(aggregateFilters(spec), area)!, reference: withArea(null, area)!};
      if (!area.selected) boxOf.set(body, lastView!.input.bbox);
      return body;
    }
  );
  /** The pending ask after the camera moved. */
  let inViewTimer: unknown = null;
  /** The camera's box each request and answer in view was taken over; a selected region's has none. */
  const boxOf = new WeakMap<object, [number, number, number, number]>();
  /** The box the counts in view on show were taken over, which the shown count is taken over too. */
  let countedBox: [number, number, number, number] | null = null;
  /** Whether `inView` holds its two specs, and the highlighted one. */
  let inViewRegistered = false;
  let inViewHighlighted = false;

  const records = new HeldRecords((id) => tokens.get().then((t) => client.item(t, id)).then((detail) => detail.fields));

  const projections: Projections = {
    meta: null,
    status: NO_STATUS,
    view: noFrame(''),
    marks: {bands: [], standIn: [], count: NO_COUNT},
    tiles: {tiles: []},
    artifacts: {layer: null, layers: [], served: [], colourServed: [], lineage: servedLineage([]), attached: new Map(), status: 'idle', refusal: null, version: 0, held: 0, table, servedOrdinals: new Set(), shapes: new Map(), colours: new Map(), palette: colours.palette, overrides: colours.overrides, coverage: {current: 0, stale: 0}},
    selection: {item: null, itemRefusal: null, artifact: null, artifactRefusal: null},
    region: null,
    filters: {draft: emptyDraft([]), expr: null, highlight: null, members: [], suggestions: {}, suggestErrors: {}, suggestEpoch: 0},
    legend: EMPTY_LEGEND,
    replica: {bytes: 0, points: 0, bands: 0, views: 0, lastPlan: null},
    aggregates: new Map()
  };

  const all: Set<Listener> = new Set();
  const perName = new Map<ProjectionName, Set<() => void>>();

  /**
   * Publish one projection to every subscriber. A subscriber that throws is reported to the console
   * and to `onTrace`, and the subscribers after it are still told.
   */
  function replaceProjection<K extends ProjectionName>(name: K, value: Projections[K]): void {
    projections[name] = value;
    perName.get(name)?.forEach(deliver);
    all.forEach(deliver);
  }

  function deliver(fn: () => void): void {
    try {
      fn();
    } catch (error) {
      console.error('a store subscriber threw; the other subscribers were still told', error);
      options.instruments?.onTrace?.('subscriber-fault', {message: error instanceof Error ? error.message : String(error)});
    }
  }

  /** The token and the view a verb naming a view asks under, once the store has read its meta. */
  async function viewed(): Promise<{token: string; view: string}> {
    await ready();
    return {token: await tokens.get(), view: views.id};
  }

  /** The store's first `/v1/meta`, read once and shared. A failed warm-up is forgotten, so the next call runs it again. */
  let warming: Promise<void> | null = null;

  function ready(): Promise<void> {
    if (warming) return warming;
    const attempt: Promise<void> = warm().catch((error: unknown) => {
      // A `clear` may have started another warm-up since.
      if (warming === attempt) {
        warming = null;
        if (!disposed) {
          const refusal = refusalOf(error);
          replaceProjection('status', {...projections.status, status: 'refused', refusal, expired: tokens.isExpiry(refusal)});
        }
      }
      throw error;
    });
    warming = attempt;
    return attempt;
  }

  /** The current view's frame, or `null` before `meta`. Each view of a bundle may declare its own. */
  function frameOrNull(): Quantisation | null {
    return quantisationOf(views.id);
  }

  /** One named view's frame, or `null` where the bundle declares no such view. */
  function quantisationOf(id: string): Quantisation | null {
    return meta?.views.find((v) => v.id === id)?.quantisation ?? null;
  }

  /** Whether two views quantise against one extent, compared by value. The camera carries between them. */
  function sameFrame(a: Quantisation | null, b: Quantisation | null): boolean {
    return (
      a !== null && b !== null && a.xMin === b.xMin && a.xMax === b.xMax && a.yMin === b.yMin && a.yMax === b.yMax
    );
  }

  /** {@link frameOrNull} for a caller that holds `meta`. There is no default extent to fall back to. */
  function frame(): Quantisation {
    const q = frameOrNull();
    if (!q) throw new Error(`the bundle declares no view '${views.id}'`);
    return q;
  }

  /**
   * Build one view's machinery, bound to that view's id and frame. Its events reach the projections
   * only while it is the current view. The session artifact table is shared, so an artifact served
   * in two views has one ordinal and one colour.
   */
  function buildView(id: string): ViewMachinery {
    const m = meta;
    if (!m) throw new Error('a view is built after meta');
    const q = quantisationOf(id);
    if (!q) throw new Error(`the bundle declares no view '${id}'`);
    /** Assigned below; the fetch reads it. */
    let ownPresenter: Presenter | null = null;
    /** Assigned below. Machinery {@link forgetAnswers} dropped is never current again. */
    let own: ViewMachinery | null = null;
    const current = () => own !== null && views.current === own;
    /**
     * Whether an answer asked for under `token` may be held: while the store still holds this
     * machinery, and {@link admit} keeps it.
     */
    const admitted = (identityKey: string, token: string): boolean => own !== null && views.holds(own) && admit(id, identityKey, token);

    const built = new Replica(
      async (req, signal, background, onPart, onCounts) => {
        const tok = await tokens.use();
        // The layers named put a membership column on each band, and the columns named are the
        // render columns it carries. A counts-only revalidation (`k = 0`) absorbs no points, so it
        // names neither. The budget and levels are the channel's, so a point's membership names an
        // artifact of the cut the panels show.
        const zoom = ownPresenter?.view?.view.zoom ?? 0;
        const layers = req.k === 0 ? [] : pointLayers();
        const columns = req.k === 0 ? undefined : pointColumns(id);
        const cut = clusterBudget;
        const response = await client.viewport(
          tok,
          {
            ...req,
            view: id,
            filters: requestFilters(),
            highlight: requestHighlight(),
            layers,
            ...(columns === undefined ? {} : {pointRows: columns}),
            ...(layers.length === 0 || cut === null ? {} : {artifactBudget: cut}),
            ...(layers.length === 0 || requestLevels(m.layers, layers, zoom) === undefined ? {} : {levels: requestLevels(m.layers, layers, zoom)})
          },
          {
            signal,
            background,
            onPart:
              onPart &&
              ((part) => {
                if (!admitted(part.identityKey, tok)) throw identityChanged();
                return onPart(part);
              }),
            onCounts:
              onCounts &&
              ((counts) => {
                if (!admitted(counts.identityKey, tok)) throw identityChanged();
                onCounts(counts);
              })
          }
        );
        if (!admitted(response.identityKey, tok)) throw identityChanged();
        return response;
      },
      q,
      {
        view: id,
        table,
        budget: bandBudget,
        cache: options.replica?.cache,
        revalidateAfterMs: options.replica?.revalidateAfterMs,
        onPhase: (kind, ms, n) => {
          options.replica?.onPhase?.(kind, ms, n);
          // A stored slice is drawable, so the first marks arrive with the first slice.
          if (kind === 'piece' || kind === 'store') ownPresenter?.absorbed();
        },
        // Counts are drawable as density and figures before the points that follow them.
        onCounts: () => ownPresenter?.absorbed(),
        now: () => clock.now()
      }
    );

    ownPresenter = new Presenter(
      built,
      {
        kMaxMarks: m.selection.kMaxMarks,
        maxTilesPerRequest: m.maxTilesPerRequest,
        thetaTargetMarks: m.selection.thetaTargetMarks,
        kMin: m.selection.kMin,
        filtered: () => requestFilters() !== null
      },
      clock,
      scheduler,
      {
        onPresented: (p) => {
          if (current()) onPresented(p);
        },
        onStatus: (status, refusal) => {
          if (current()) onStatus(status, refusal);
        },
        onTrace: (kind, fields) => {
          if (current()) onTrace(kind, fields);
        }
      },
      {budget, ...options.driver},
      options.prefetch ?? true
    );

    const viewChannel = new ArtifactChannel(client, {
      clock,
      view: id,
      quantisation: q,
      projection: m.views.find((v) => v.id === id)?.projection ?? 'none',
      perTile: options.artifacts?.perTile ?? Math.min(DEFAULT_ARTIFACTS_PER_TILE, m.selection.maxArtifactsPerTile),
      ...(clusterBudget === null ? {} : {budget: clusterBudget}),
      paletteSize: paletteSize(colours.palette),
      heldTiles: options.artifacts?.heldTiles ?? DEFAULT_HELD_TILES,
      prefetch: options.prefetch ?? true,
      token: () => tokens.get(),
      maxTiles: m.maxTilesPerRequest,
      table,
      // The point path's filters, region included, so an artifact's `matched` bit counts members
      // inside the selection.
      filters: () => requestFilters(),
      declarations: m.layers,
      onChange: (state) => {
        if (current()) onArtifacts(state);
      },
      admit: admitted
    });
    // Layers set before this view existed, including before meta.
    viewChannel.setLayers(layersAsked());

    own = {replica: built, presenter: ownPresenter, channel: viewChannel};
    return own;
  }

  /**
   * Every viewport answer, on the point path and the artifact channel, passes through here before
   * anything in it is held, with the token it was asked under. The store runs
   * {@link forgetAnswers} where the answer's identity key differs from the one held for its view,
   * and where a renewal brought a token no answer has yet matched to a held key and the answer's
   * view holds none, since that key could be another viewer's.
   *
   * @returns Whether the answer may be held. False drops it: the machinery that asked is gone.
   */
  function admit(view: string, identityKey: string, token: string): boolean {
    const held = identities.get(view);
    const unchecked = held === undefined && unconfirmed !== null && identities.size > 0;
    if ((held !== undefined && held !== identityKey) || unchecked) {
      forgetAnswers();
      return false;
    }
    if (held !== undefined && token === unconfirmed) unconfirmed = null;
    identities.set(view, identityKey);
    return true;
  }

  /**
   * Asks for the current view's identity key under a token a renewal brought: counts only, one tile
   * at depth 0, with no filter, highlight or layer, so nothing set for the previous viewer can have
   * it refused. The server refusing the view the previous token reached shows another viewer, and
   * the store forgets as {@link admit} does. An expired token leaves the next answer to
   * {@link admit}. A shed request (`429`), a server still starting (`503`), a server error that
   * refused nothing, such as a gateway's `504`, and a failed fetch ask again while the token is
   * unconfirmed, after 1 s, then twice as long each time up to 30 s, or the server's
   * `Retry-After` where that is longer: one ask at a time, until an answer confirms the key or
   * the store forgets.
   */
  async function askIdentity(attempt = 0): Promise<void> {
    if (identityRetry !== null) clock.cancel(identityRetry);
    identityRetry = null;
    const current = views.current;
    const q = frameOrNull();
    if (!current || !q) return;
    const view = views.id;
    let tok: string | null = null;
    try {
      tok = await tokens.get();
      const response = await client.viewport(tok, {view, zoom: 0, bbox: rectToRequestBbox({x0: 0, y0: 0, x1: 0, y1: 0}, 0, q), k: 0, layers: []});
      if (!disposed && views.current === current) admit(view, response.identityKey, tok);
    } catch (error) {
      if (disposed || tok === null || unconfirmed !== tok) return;
      const refusal = refusalOf(error);
      if (tokens.isExpiry(refusal)) return;
      const named = error instanceof MosaicaError ? error : null;
      if (named && named.status !== 429 && named.status !== 503 && !isUnanswered(refusal)) {
        forgetAnswers();
        return;
      }
      const wait = Math.min(Math.max(PROBE_FIRST_MS * 2 ** attempt, (named?.retryAfterS ?? 0) * 1000), PROBE_MAX_MS);
      identityRetry = clock.after(wait, () => {
        identityRetry = null;
        if (!disposed && unconfirmed === tok) void askIdentity(attempt + 1);
      });
    }
  }

  async function warm(): Promise<void> {
    // A warm-up overtaken by a `clear` hands over to the one the clear started.
    const epoch = clears;
    let read: Meta;
    try {
      const t = await tokens.use();
      // `options.meta` was read for the viewer the store was made for.
      read = (epoch === 0 ? options.meta : undefined) ?? (await client.meta(t));
    } catch (error) {
      if (!disposed && epoch !== clears) return ready();
      throw error;
    }
    if (disposed) return;
    if (epoch !== clears) return ready();
    meta = read;
    // A `setCurrentView` before meta names the view to open with, in place of `options.view`.
    if (queuedCurrentView !== null) {
      const wanted = queuedCurrentView;
      queuedCurrentView = null;
      if (meta.views.some((v) => v.id === wanted)) views.name(wanted);
      else onTrace('view-switch', {refused: 1, id: wanted});
    }
    if (!meta.views.some((v) => v.id === views.id)) {
      if (views.id) {
        // A view this viewer is not served, named by `options.view` or kept through a `clear`. The
        // camera was in its data coordinates.
        onTrace('view-switch', {refused: 1, id: views.id});
        lastView = null;
        queuedView = null;
        region.drop();
      }
      views.name(meta.views[0]?.id ?? '');
    }
    if (epoch > 0) dropUnoffered(meta);
    replaceProjection('meta', meta);
    replaceProjection('view', {...projections.view, id: views.id});
    // One empty control per operand set the bundle publishes.
    const held = projections.filters.draft;
    if (Object.keys(held.filter).length === 0 && Object.keys(held.highlight).length === 0) {
      const draft = emptyDraft(meta.filterOperands);
      replaceProjection('filters', {...projections.filters, draft, expr: composeFilters(draft, 'filter'), highlight: composeFilters(draft, 'highlight')});
    }

    layersOn = drawnOnly(layerClosure(meta.layers, layersOn));
    traceUnknownColourLayer();
    // The layers as `meta` resolved them, published with the view's artifacts.
    onArtifacts(views.enter(views.id).channel.current);

    if (queuedView) {
      const q = queuedView;
      queuedView = null;
      setView(q);
    } else if (lastView) {
      setView(lastView.input);
    }
  }

  /**
   * After `meta` is read again, drop the host's inputs that name what it no longer offers: filter
   * controls on columns it does not list, layers it does not list, a colouring by a column or
   * layer it does not list, and a sizing by a column it does not list.
   */
  function dropUnoffered(m: Meta): void {
    const columns = new Set(m.filterOperands.map((f) => f.column));
    const layerNames = new Set(m.layers.map((l) => l.name));
    const {draft} = projections.filters;
    const kept = (controls: FilterDraft['filter']) => Object.fromEntries(Object.entries(controls).filter(([column]) => columns.has(column)));
    const keptDraft: FilterDraft = {filter: kept(draft.filter), highlight: kept(draft.highlight)};
    const count = (d: FilterDraft) => Object.keys(d.filter).length + Object.keys(d.highlight).length;
    if (count(keptDraft) !== count(draft)) {
      replaceProjection('filters', {
        ...projections.filters,
        draft: keptDraft,
        expr: composeFilters(keptDraft, 'filter'),
        highlight: composeFilters(keptDraft, 'highlight')
      });
    }
    layersOn = layersOn.filter((name) => layerNames.has(name));
    const colourBy = legend.colourBy;
    const offered =
      colourBy === null ||
      (colourBy.startsWith(CLUSTER_PREFIX)
        ? colourLayers(m.layers).some((l) => CLUSTER_PREFIX + l.name === colourBy)
        : m.declaredScalars.some((c) => c.name === colourBy));
    if (!offered) legend.setColourBy(null);
    const sizeBy = legend.sizeBy;
    if (sizeBy !== null && !m.declaredScalars.some((c) => c.name === sizeBy)) legend.setSizeBy(null);
  }

  function onStatus(status: PresentedStatus, refusal: Refusal | null): void {
    const expired = tokens.isExpiry(refusal);
    replaceProjection('status', {
      status,
      sessionWarm: projections.status.sessionWarm || status === 'shown',
      refusal,
      stale: projections.status.stale,
      expired,
      retrying: status === 'retrying',
      unreachable
    });
    if (status === 'refused' && refusal) region.refuse(refusal);
  }

  /**
   * `status.stale`: the content key the replica last observed differs from the one the presented
   * marks were derived under. A revalidation or an artifact response can observe a new key without
   * a redraw. Not built yet: the counts from a revalidation are not published, so the numbers stay
   * as drawn until the next derive.
   */
  function recomputeStale(): void {
    const observed = views.current?.replica.currentContentKey ?? '';
    observeCorpus(observed);
    const stale = contentKeyAtFrame !== '' && observed !== '' && observed !== contentKeyAtFrame;
    if (stale !== projections.status.stale) {
      replaceProjection('status', {...projections.status, stale, sessionWarm: true});
    }
  }

  /**
   * Ask every registered aggregate and the counts in view again, their requests unchanged, once the point path observes a
   * content key other than the one it observed before: an ingest, a deletion or a suppression
   * this viewer can see.
   */
  function observeCorpus(observed: string): void {
    if (observed === '') return;
    const before = corpusKey;
    corpusKey = observed;
    if (before === '' || before === observed) return;
    aggregates.refresh(false);
    askInView(false);
  }

  function onTrace(kind: string, fields: Record<string, number | string>): void {
    // A revalidation observed a content key without redrawing the marks.
    if (kind === 'revalidate') {
      recomputeStale();
      observeArtifactRotation();
    }
    options.instruments?.onTrace?.(kind, fields);
  }

  /**
   * Tell the artifact channel the content key the point path observed. A channel answering from
   * scopes it holds whole makes no request, so this is how it learns that the key moved.
   */
  function observeArtifactRotation(): void {
    const current = views.current;
    const observed = current?.replica.currentContentKey;
    if (observed) current?.channel.observeContentKey(observed);
  }

  function onPresented(p: Presented): void {
    const current = views.current;
    const replica = current?.replica;
    const frame = p.frame;
    // A derive (`p.fetched` set) redrew the marks under the current content key; a fold did not.
    const observed = replica?.currentContentKey ?? '';
    if (p.fetched) observeCorpus(observed);
    if (p.fetched) contentKeyAtFrame = observed;
    const stale = contentKeyAtFrame !== '' && observed !== '' && observed !== contentKeyAtFrame;

    let visible = 0n;
    let matched = 0n;
    let highlighted = 0n;
    let served = 0;
    for (const tile of frame.tiles) {
      if (!tile.counts) continue;
      visible += tile.counts.visible;
      matched += tile.counts.matched;
      highlighted += tile.counts.highlighted;
      // Marks on screen: a tile counted before its points draws none.
      served += tile.drawn;
    }

    // A frame with marks means the session answered, while the request may still be streaming.
    if (!projections.status.sessionWarm && frame.exactDrawn + frame.provisional > 0) replaceProjection('status', {...projections.status, sessionWarm: true});
    // A switch published `loading`; a frame drawn from the view's own bands, with no request
    // behind it, ends that.
    if (awaitingSwitchFrame) {
      awaitingSwitchFrame = false;
      if (projections.status.status === 'loading') replaceProjection('status', {...projections.status, status: 'shown'});
    }
    replaceProjection('view', {
      id: views.id,
      composition: frame,
      depth: frame.depth,
      visible: {value: Number(visible), exact: true},
      matched: {value: Number(matched), exact: true},
      highlighted: {value: Number(highlighted), exact: true},
      highlighting: requestHighlight() !== null,
      served: {shown: served, total: Number(visible), exact: true},
      provisional: frame.provisional,
      inView: projections.view.inView && {...projections.view.inView, shown: shownInArea(frame.exact)}
    });
    replaceProjection('marks', {
      bands: frame.exact,
      standIn: frame.standIn,
      count: {shown: frame.exactDrawn, total: Number(visible), exact: true}
    });
    replaceProjection('tiles', {tiles: frame.tiles});
    region.answer({
      marks: {bands: frame.exact, standIn: frame.standIn},
      depth: frame.depth,
      verdict: replica?.lastRegionVerdict ?? null,
      fetched: p.fetched !== null,
      matched: Number(matched),
      narrowed: filtersBesideRegion(projections.filters.draft) !== null
    });
    const counted = projections.view.inView;
    if (counted && region.selected && counted.shown !== shownInArea(frame.exact)) {
      replaceProjection('view', {...projections.view, inView: {...counted, shown: shownInArea(frame.exact)}});
    }
    legend.accumulate(frame, meta?.declaredScalars ?? []);
    colours.refresh();
    checkPointColumns();
    checkColourCoverage();

    if (replica) {
      const fetched = p.fetched;
      publishReplica(
        fetched
          ? {held: fetched.plan.wanted - fetched.plan.novel, fetched: fetched.plan.novel}
          : projections.replica.lastPlan
      );
    }
    if (stale !== projections.status.stale) {
      replaceProjection('status', {...projections.status, stale, sessionWarm: true});
    }
    if (p.fetched) observeArtifactRotation();
    if (options.instruments?.onFrame && replica && p.fetched) {
      options.instruments.onFrame({
        plan: {choice: p.plan.choice},
        timings: p.fetched.response?.timings ?? null,
        bytes: p.fetched.plan.bytes,
        held: p.fetched.plan.wanted - p.fetched.plan.novel,
        fetched: p.fetched.plan.novel,
        calibration: p.calibration,
        replica: {bytes: replica.bytes, points: replica.points, bands: replica.bandCount}
      });
    }
  }

  /** `bytes` and `views` count every view's bands; `points` and `bands` the current view's. */
  function publishReplica(lastPlan: ReplicaProjection['lastPlan']): void {
    const replica = views.current?.replica;
    replaceProjection('replica', {
      bytes: replica?.bytes ?? 0,
      points: replica?.points ?? 0,
      bands: replica?.bandCount ?? 0,
      views: replica?.heldViews ?? 0,
      lastPlan
    });
  }

  function onArtifacts(state: ArtifactChannelState): void {
    recomputeStale();
    // The channel asks once the view has settled, and the cut it draws at may have moved with it.
    aggregates.refresh(false, {which: drawsCut, changed: true});
    // The channel is asked for the colour layer as well as the drawn ones. Only the drawn layers'
    // rows reach `served`; the colour layer's reach `colourServed`.
    const drawn = new Set(layersOn);
    const served = state.artifacts.every((a) => drawn.has(a.layer)) ? state.artifacts : state.artifacts.filter((a) => drawn.has(a.layer));
    const coloured = colourLayer();
    const servedOrdinals = new Set<number>();
    for (const a of served) {
      const ordinal = table.ordinalOf(a.layer, a.mosaicaId);
      if (ordinal !== 0) servedOrdinals.add(ordinal);
    }
    replaceProjection('artifacts', {
      ...projections.artifacts,
      layer: layersOn[0] ?? null,
      layers: layersOn,
      served,
      colourServed: coloured === null ? [] : state.artifacts.filter((a) => a.layer === coloured),
      lineage: servedLineage(served),
      attached: attachedTextOf(state.artifacts, meta?.layers ?? []),
      status: state.status,
      refusal: state.refusal,
      version: state.version,
      held: state.held,
      table,
      servedOrdinals,
      shapes: shapes.shapes,
      colours: colours.current(),
      palette: colours.palette,
      overrides: colours.overrides
    });
    checkColourCoverage();
  }

  /**
   * Publish the colour coverage of the bands in view, and fetch the colour-stale ones again, once
   * per served-set version. A band is colour-stale when it has no membership column for a layer a
   * point request names ({@link pointLayers}), or names an ordinal that resolves to no colour. It
   * resolves against the colours, which cover every artifact the table holds, so a band fetched
   * under a coarser cut is still coloured. In view is the visible box: the point path also fetches a margin, whose bands name
   * artifacts the channel did not serve for this view.
   */
  function checkColourCoverage(): void {
    const a = projections.artifacts;
    const layers = pointLayers();
    const machinery = views.current;
    // During a switch, until the marks are cleared, the marks on screen are the outgoing view's.
    if (projections.view.id !== views.id) return;
    if (!machinery || a.status !== 'shown' || layers.length === 0) {
      if (a.coverage.stale !== 0 || a.coverage.current !== 0) replaceProjection('artifacts', {...a, coverage: {current: 0, stale: 0}});
      return;
    }
    const started = clock.now();
    const v = machinery.presenter.view;
    const depth = projections.view.depth;
    const visible = v ? tileRectOfBbox(worldBbox({target: [v.view.target[0], v.view.target[1]], zoom: v.view.zoom, width: v.width, height: v.height}, 1), depth) : null;
    const bands = projections.marks.bands;
    const staleBands: Band[] = [];
    let current = 0;
    // The tags in view, so those no held tile carries are read by identifier.
    const tags = new Set<number>();
    for (const band of bands) {
      if (visible && (band.depth !== depth || !rectContainsTile(visible, band.x, band.y))) continue;
      // A tile that serves no point has nothing to colour.
      if (band.ids.length === 0) continue;
      for (const layer of layers) for (const ordinal of band.membership[layer]?.distinct ?? []) tags.add(ordinal);
      if (layers.every((layer) => resolves(band, layer, a.colours))) current++;
      else staleBands.push(band);
    }
    if (tags.size > 0) void machinery.channel.lookUp(tags);
    const stale = staleBands.length;
    // While a switch settles nothing is asked for or recorded: asking reschedules the driver, which
    // would request a view the slider is passing through.
    const toAsk = views.settling ? [] : staleBands.filter((b) => colourAsked.get(bandKey(b.depth, b.prefix)) !== a.version);
    for (const b of toAsk) colourAsked.set(bandKey(b.depth, b.prefix), a.version);
    if (toAsk.length > 0) {
      machinery.replica.retract(toAsk);
      machinery.presenter.reschedule();
    }
    options.instruments?.onTrace?.('coverage', {ms: clock.now() - started, bands: bands.length, stale, asked: toAsk.length});
    if (a.coverage.current !== current || a.coverage.stale !== stale) {
      replaceProjection('artifacts', {...projections.artifacts, coverage: {current, stale}});
    }
  }

  /**
   * The render columns of `view` a point request names, in the order `meta` lists them: the colour
   * column, the size column and those {@link setPointColumns} asked for. A name that is not a
   * render column of the view is left out.
   */
  function pointColumns(view: string): string[] {
    const wanted = new Set<string>();
    const colourBy = legend.colourBy;
    if (colourBy !== null && !colourBy.startsWith(CLUSTER_PREFIX)) wanted.add(colourBy);
    if (legend.sizeBy !== null) wanted.add(legend.sizeBy);
    for (const names of pointColumnAsks.values()) for (const name of names) wanted.add(name);
    if (!meta || wanted.size === 0) return [];
    const rendered = [
      ...meta.declaredScalars.filter((c) => c.render).map((c) => c.name),
      ...meta.scopedScalars.filter((c) => c.render && c.views.includes(view)).map((c) => c.name)
    ];
    return rendered.filter((name) => wanted.has(name));
  }

  /**
   * Fetch again every band the current view holds, in view or not, that lacks a column
   * {@link pointColumns} names and its request did not, so panning does not reveal a band without
   * it. A band asked for every column, or for this one, is not fetched again for lacking it: the
   * server served what there is. A band stays drawn until its replacement arrives, and is withdrawn
   * again if the piece it came in marks it covered after. A band with no points has nothing to
   * carry.
   */
  function checkPointColumns(): void {
    const machinery = views.current;
    // As in the colour coverage, asking while a switch settles would request a passing view.
    if (!machinery || views.settling) return;
    const replica = machinery.replica;
    const columns = pointColumns(views.id);
    const checked = {replica, version: replica.version, columns: `${views.id}\0${columns.join('\0')}`};
    const last = pointColumnsChecked;
    if (last && last.replica === checked.replica && last.version === checked.version && last.columns === checked.columns) return;
    pointColumnsChecked = checked;
    const unasked = (b: Band, c: string) => !(c in b.scalars) && b.columnsAsked != null && !b.columnsAsked.includes(c);
    const lacking = replica
      .heldBands()
      .filter((b) => b.ids.length > 0 && columns.some((c) => unasked(b, c)));
    if (replica.retract(lacking) > 0) machinery.presenter.reschedule();
  }

  function resolves(band: Band, layer: string, colourMap: ReadonlyMap<number, Rgba>): boolean {
    const m = band.membership[layer];
    // A layer the band's request named and the band carries no column for tagged none of its
    // points: absent is all null, not unasked.
    if (!m) return band.layersAsked === 'all' || (band.layersAsked?.includes(layer) ?? false);
    for (let i = 0; i < m.distinct.length; i++) {
      if (table.resolve(m.distinct[i]!, colourMap) === 0) return false;
    }
    return true;
  }

  /**
   * The camera as the driver and the channel take it: the host's zoom, and the centre whose canvas,
   * clamped to the world, is `input.bbox`. A box narrower than the canvas at that zoom was clamped
   * on the side it meets the world's edge, so the centre is half a canvas in from its other side.
   */
  function toDriverView(input: ViewInput): DriverViewState & {width: number; height: number} {
    const q = frame();
    const [wx0, wy0] = dataToWorldXY(input.bbox[0], input.bbox[1], q);
    const [wx1, wy1] = dataToWorldXY(input.bbox[2], input.bbox[3], q);
    const zoom = Math.min(MAX_DEPTH, input.zoom);
    const scale = 2 ** zoom;
    return {
      target: [cameraCentre(Math.min(wx0, wx1), Math.max(wx0, wx1), input.width / 2 / scale), cameraCentre(Math.min(wy0, wy1), Math.max(wy0, wy1), input.height / 2 / scale), 0],
      zoom,
      width: input.width,
      height: input.height
    };
  }

  /**
   * Make `id` the view the store answers from. Between views that share a frame the camera and the
   * selection carry over: the incoming view draws what it holds, and asks after the settle. Across
   * frames both are dropped, and the map's refit supplies the next `setView`. An id `meta.views`
   * does not list is ignored and traced.
   */
  function setCurrentView(id: string): void {
    if (!meta) {
      queuedCurrentView = id;
      return;
    }
    if (id === views.id) return;
    if (!meta.views.some((v) => v.id === id)) {
      onTrace('view-switch', {refused: 1, id});
      return;
    }

    const from = views.id;
    const kept = sameFrame(quantisationOf(from), quantisationOf(id));
    // A suggestion page answers one view.
    suggestions.reset();

    // Current before anything below publishes, so a subscriber's `setView` reaches the incoming view.
    const incoming = views.enter(id);
    // A `setView` from a subscriber clears this, and its request then answers for the status.
    awaitingSwitchFrame = true;
    // The camera is in the outgoing frame's data coordinates. Dropped before anything below publishes,
    // so a subscriber that refits the camera on a publish keeps its refit.
    if (!kept) lastView = null;
    // The shared settings reach a view as it becomes current, before a refit can ask. Set on a held
    // view, they would make it ask.
    incoming.presenter.setBudget(budget);
    incoming.channel.setLayers(layersAsked());
    // The content key, the bands asked for again and the shapes were the outgoing view's.
    contentKeyAtFrame = '';
    corpusKey = '';
    colourAsked.clear();
    shapes.forget('all');

    // The selection is in the outgoing frame's data coordinates too.
    if (!kept) region.drop();

    // No marks until the incoming view presents, and the incoming channel's artifacts. A subscriber
    // that refits the camera here reaches the incoming view with its settings in place.
    replaceProjection('view', noFrame(id));
    replaceProjection('marks', {...projections.marks, bands: [], standIn: [], count: NO_COUNT});
    replaceProjection('tiles', {tiles: []});
    onArtifacts(incoming.channel.current);
    replaceProjection('status', {...projections.status, status: 'loading', refusal: null, stale: false});
    publishReplica(projections.replica.lastPlan);

    if (kept && lastView) {
      const v = toDriverView(lastView.input);
      // Draws what the view holds on the next scheduler tick, without a request.
      incoming.presenter.redraw({target: v.target, zoom: v.zoom}, v.width, v.height);
      views.settle(() => {
        if (lastView) setView(lastView.input);
      });
    }

    aggregates.refresh(true);
    askInView(false, true);
    onTrace('view-switch', {from, to: id, sameFrame: kept ? 1 : 0});
  }

  function setView(input: ViewInput): void {
    if (!Number.isFinite(input.zoom)) throw new TypeError(`setView was given zoom ${String(input.zoom)}: ViewInput.zoom is required, the camera's zoom as a finite number`);
    lastView = {input};
    areaChanged();
    // From here the request's own status transitions answer for the status.
    awaitingSwitchFrame = false;
    const current = views.current;
    if (!meta || !current) {
      queuedView = input;
      return;
    }
    const v = toDriverView(input);
    current.presenter.schedule({target: v.target, zoom: v.zoom}, v.width, v.height);
    current.channel.schedule({target: v.target, zoom: v.zoom}, v.width, v.height);
    // A selected region is counted as it is wherever the camera goes.
    if (!region.selected) askInView(true);
  }

  /**
   * The layers a viewport request may name: all but the filter layers, which declare
   * `computed = []` and are applied only as `member_of` clauses. Filtered here so that a host
   * calling `setLayers` directly cannot draw one.
   */
  function drawnOnly(names: readonly string[]): string[] {
    if (!meta) return [...names];
    const filterLayers = new Set(meta.layers.filter(isFilterLayer).map((l) => l.name));
    return names.filter((n) => !filterLayers.has(n));
  }

  /** The layer `colourBy` colours by, where `meta` lists it as one that can colour; else null. */
  function colourLayer(): string | null {
    const colourBy = legend.colourBy;
    if (!meta || !colourBy?.startsWith(CLUSTER_PREFIX)) return null;
    const name = colourBy.slice(CLUSTER_PREFIX.length);
    return colourLayers(meta.layers).some((l) => l.name === name) ? name : null;
  }

  function traceUnknownColourLayer(): void {
    const colourBy = legend.colourBy;
    if (meta && colourBy?.startsWith(CLUSTER_PREFIX) && colourLayer() === null) onTrace('colour-by', {refused: 1, layer: colourBy.slice(CLUSTER_PREFIX.length)});
  }

  /**
   * The layers the artifact channel asks for: the drawn layers and the colour layer, each with its
   * closure, so the colour layer's labels name the legend's rows.
   */
  function layersAsked(): string[] {
    const coloured = colourLayer();
    if (coloured === null || layersOn.includes(coloured) || !meta) return layersOn;
    return [...layersOn, ...drawnOnly(layerClosure(meta.layers, [coloured])).filter((l) => !layersOn.includes(l))];
  }

  /**
   * The layers a point request names, each putting a membership column on every band: those
   * {@link layersAsked} names, less the label layers (a layer that depends on another and declares
   * no geometry, as a clustering's topic labels), whose columns nothing reads. The first layer
   * drawn and the colour layer are always kept. A label layer's rows come from the channel.
   */
  function pointLayers(): string[] {
    const kept = new Set([layersOn[0], colourLayer()]);
    const labels = new Set(meta?.layers.filter((l) => l.depsOn.length > 0 && l.computedContent.length === 0 && l.shape === null && !kept.has(l.name)).map((l) => l.name) ?? []);
    return layersAsked().filter((l) => !labels.has(l));
  }

  /**
   * Point the channel at the layers asked for and publish the artifacts as they now stand. A layer
   * newly asked for is fetched at once, and the bands in view lacking its membership column are
   * refetched by the coverage check. A layer dropped costs nothing: its rows are filtered out.
   */
  function askLayers(): void {
    const current = views.current;
    if (!current) {
      // Before meta: published now, and asked for when the view is built.
      replaceProjection('artifacts', {...projections.artifacts, layer: layersOn[0] ?? null, layers: layersOn});
      return;
    }
    const {channel, presenter} = current;
    const before = channel.current.layers;
    const asked = layersAsked();
    // The channel publishes when its layers move; when they do not, the drawn set may still have.
    if (asked.length === before.length && asked.every((l, i) => l === before[i])) onArtifacts(channel.current);
    else channel.setLayers(asked);
    if (asked.some((l) => !before.includes(l)) && presenter.view) {
      const v = presenter.view;
      channel.refresh(v.view, v.width, v.height);
    }
  }

  /** The filter-position controls of `draft` and the `member_of` clauses, without the region leaf. */
  function filtersBesideRegion(draft: FilterDraft): FilterExpr | null {
    return withMembers(composeFilters(draft, 'filter'), projections.filters.members, 'filter');
  }

  /** One composition for the point path, the artifact channel and the region's count. */
  function requestFilters(): FilterExpr | null {
    return withSelected(filtersBesideRegion(projections.filters.draft));
  }

  /** {@link requestFilters} without `column`'s filter-position control. */
  function filtersWithout(column: string): FilterExpr | null {
    return withSelected(filtersBesideRegion(withoutClause(projections.filters.draft, column, 'filter')));
  }

  /**
   * {@link requestFilters} less the filter-position control `spec.without` names and the
   * filter-position `member_of` clauses on the layer `spec.withoutMembersOf` names, and with
   * `spec.highlighted` the highlight joined to it.
   */
  function aggregateFilters(spec: AggregateSpec): FilterExpr | null {
    const {draft, members} = projections.filters;
    const kept = spec.without === undefined ? draft : withoutClause(draft, spec.without, 'filter');
    const layer = spec.withoutMembersOf;
    const clauses = layer === undefined ? members : members.filter((m) => m.verb !== 'filter' || m.layer !== layer);
    const filters = withSelected(withMembers(composeFilters(kept, 'filter'), clauses, 'filter'));
    const highlight = spec.highlighted === true ? requestHighlight() : null;
    if (highlight === null) return filters;
    return filters === null ? highlight : {all_of: [filters, highlight]};
  }

  /**
   * The request a registered aggregate sends now, less its view: its filters by its subject, and its
   * reference. `null` for a `view` aggregate before the camera has been set.
   */
  function aggregateBody(spec: AggregateSpec): AggregateBody | null {
    const groupings = drawnGroupings(spec);
    if (groupings === null) return null;
    const subject = spec.subject ?? 'match';
    let filters: FilterExpr | null = null;
    let reference: FilterExpr | undefined = spec.reference === 'visible' ? {} : spec.reference;
    if (subject === 'match') filters = aggregateFilters(spec);
    if (subject === 'view') {
      const area = countedArea();
      if (area === null) return null;
      filters = withArea(aggregateFilters(spec), area);
      // Drawn from the same area, as the counts in view's own reference is.
      if (spec.reference !== undefined) reference = withRegion(spec.reference === 'visible' ? null : spec.reference, area.operand, area.outside)!;
    }
    return {groupings, ...(filters === null ? {} : {filters}), ...(reference === undefined ? {} : {reference})};
  }

  /** Whether `spec` ranks a layer at the cut the map draws. */
  function drawsCut(spec: AggregateSpec): boolean {
    return spec.groupings.some(isDrawn);
  }

  /** Whether `spec` asks for slots in the palette the map colours clusters from. */
  function drawsPalette(spec: AggregateSpec): boolean {
    return spec.groupings.some(isPaletteDrawn);
  }

  /**
   * `spec`'s groupings with each `cut: 'drawn'` replaced by the current view's drawn cut, and each
   * `paletteSize: 'drawn'` by the size of the store's palette; `null` before the current view has
   * noted a camera where a cut is drawn.
   */
  function drawnGroupings(spec: AggregateSpec): Grouping[] | null {
    const cut = drawsCut(spec) ? (views.current?.channel.drawnCut() ?? null) : null;
    const groupings: Grouping[] = [];
    for (const g of spec.groupings) {
      if (isDrawn(g) && cut === null) return null;
      if (!isDrawn(g) && !isPaletteDrawn(g)) {
        groupings.push(g as Grouping);
        continue;
      }
      const by = {...g.by, ...(isDrawn(g) ? {cut} : {}), ...(isPaletteDrawn(g) ? {paletteSize: paletteSize(colours.palette)} : {})};
      groupings.push({...g, by} as Grouping);
    }
    return groupings;
  }

  /** Register an aggregate. One counted in view waits for the camera to rest where it is moving. */
  function setAggregate(id: string, spec: AggregateSpec | null): void {
    aggregates.set(id, spec, spec?.subject !== 'view' || inViewTimer === null);
  }

  /**
   * The area the counts in view are taken over: the selected region, else the camera's box. `null`
   * before the camera has been set. The selected region is already in the filters; the box is not.
   */
  function countedArea(): {operand: RegionOperand; outside: boolean; selected: boolean} | null {
    const selected = region.selected;
    if (selected) return {operand: regionOperand(selected), outside: selected.outside ?? false, selected: true};
    if (!lastView) return null;
    return {operand: regionOperand({kind: 'box', bbox: lastView.input.bbox}), outside: false, selected: false};
  }

  /** `expr` joined with the area's leaf, which the selected region's filters already carry. */
  function withArea(expr: FilterExpr | null, area: {operand: RegionOperand; outside: boolean; selected: boolean}): FilterExpr | null {
    if (area.selected && expr !== null) return expr;
    return withRegion(expr, area.operand, area.outside);
  }

  /**
   * Ask for the counts in view again, now or once the camera has rested, and with them each
   * registered aggregate counted in view whose request has changed. With `changed`, as at a rest,
   * the counts in view are asked only where their request has changed too.
   */
  function askInView(rest: boolean, drop = false, changed = false): void {
    if (inViewTimer !== null) clock.cancel(inViewTimer);
    inViewTimer = null;
    if (countedArea() === null) return;
    if (!rest) {
      aggregates.refresh(false, {which: (spec) => spec.subject === 'view', changed: true});
      const highlighting = requestHighlight() !== null;
      if (highlighting !== inViewHighlighted) {
        inViewHighlighted = highlighting;
        inView.set('highlighted', highlighting ? {groupings: [{count: true}], highlighted: true} : null);
      }
      if (!inViewRegistered) {
        inViewRegistered = true;
        inView.set('counts', {groupings: [{count: true}]});
        return;
      }
      inView.refresh(drop, {changed});
      return;
    }
    inViewTimer = clock.after(IN_VIEW_REST_MS, () => {
      inViewTimer = null;
      askInView(false, false, true);
    });
  }

  function publishInView(entries: AggregatesProjection): void {
    const counts = entries.get('counts');
    const lit = entries.get('highlighted');
    const total = counts?.result?.tables[0];
    if (counts?.status === 'refused') {
      // The last counts stand, marked refused; with none, the frame's own figures.
      const v = projections.view;
      const last = v.inView ?? {visible: v.visible, matched: v.matched, highlighted: v.highlighted, shown: v.served.shown};
      replaceProjection('view', {...v, inView: {...last, status: 'refused'}});
      return;
    }
    if (!counts || !total || counts.view !== views.id) {
      if (projections.view.inView !== null && (!counts || counts.result === null)) replaceProjection('view', {...projections.view, inView: null});
      return;
    }
    countedBox = counts.result ? (boxOf.get(counts.result) ?? null) : null;
    const exact = (counts.result?.region?.exact ?? true) && !(region.selected?.outside ?? false);
    const masked = (value: number) => ({value, exact});
    const visible = masked(total.referenceTotal ?? total.total);
    const matched = masked(total.total);
    const litTotal = lit?.result?.tables[0]?.total;
    const highlighted = litTotal === undefined ? matched : masked(litTotal);
    const loading = counts.status === 'loading' || counts.status === 'retrying' || lit?.status === 'loading' || lit?.status === 'retrying';
    const status = lit?.status === 'refused' ? 'refused' : loading ? 'loading' : 'shown';
    replaceProjection('view', {...projections.view, inView: {status, visible, matched, highlighted, shown: shownInArea(projections.marks.bands)}});
    if (status === 'shown' && region.selected) region.counted({visible, matched, verdict: counts.result?.region ?? null});
  }

  /** The held marks of `bands` inside the area, by the server's predicate; the region's own count while one is selected. */
  function shownInArea(bands: Composition['exact']): number {
    const q = frameOrNull();
    if (!q) return 0;
    if (region.selected) return projections.region?.held.count ?? 0;
    const counted = countedBox ?? lastView?.input.bbox;
    if (!counted) return 0;
    const [bx0, by0, bx1, by1] = counted;
    const [wx0, wy0] = dataToWorldXY(bx0, by0, q);
    const [wx1, wy1] = dataToWorldXY(bx1, by1, q);
    const box: [number, number, number, number] = [Math.min(wx0, wx1), Math.min(wy0, wy1), Math.max(wx0, wx1), Math.max(wy0, wy1)];
    let shown = 0;
    for (const band of bands) {
      const span = WORLD_SIZE / 2 ** band.depth;
      const x0 = band.x * span;
      const y0 = band.y * span;
      if (x0 >= box[0] && y0 >= box[1] && x0 + span <= box[2] && y0 + span <= box[3]) {
        shown += band.ids.length;
        continue;
      }
      if (x0 > box[2] || y0 > box[3] || x0 + span < box[0] || y0 + span < box[1]) continue;
      for (let i = 0; i < band.ids.length; i++) if (insideBox(band.positions[i * 2]!, band.positions[i * 2 + 1]!, box)) shown += 1;
    }
    return shown;
  }

  /** `expr` with the selected region's leaf joined. */
  function withSelected(expr: FilterExpr | null): FilterExpr | null {
    const selected = region.selected;
    return withRegion(expr, selected ? regionOperand(selected) : null, selected?.outside ?? false);
  }

  /**
   * The `highlight` every request carries: the controls and `member_of` clauses in that position.
   * The region is never here, because its numbers are read off the frame's `matched`.
   */
  function requestHighlight(): FilterExpr | null {
    return withMembers(composeFilters(projections.filters.draft, 'highlight'), projections.filters.members, 'highlight');
  }

  /**
   * The filters or the selection changed, so every view's held bands answer another question. A
   * held view is reset without asking, and refills when it is next current.
   */
  function requery(): void {
    for (const held of views.all()) {
      held.presenter.cancel();
      held.replica.reset();
    }
    contentKeyAtFrame = '';
    if (lastView) setView(lastView.input);
    // Each suggestion count is taken under the filter.
    suggestions.refresh();
    aggregates.refresh(false, {changed: true});
    askInView(false);
  }

  /**
   * The token, view and filters a read on request goes out with: the caller's `view`, else the
   * current view, and the caller's `filters`, else {@link requestFilters}. `filters: null` reads
   * everything visible; `filters: undefined` counts as not given. `own` says the caller gave them.
   */
  async function readDefaults(req: {view?: string; filters?: FilterExpr | null}) {
    const asked = await viewed();
    const own = req.filters !== undefined;
    return {token: asked.token, current: asked.view, view: req.view ?? asked.view, filters: own ? req.filters! : requestFilters(), own};
  }

  async function browse(req: Omit<BrowseRequest, 'filters' | 'view'> & {filters?: FilterExpr | null; view?: string}): Promise<BrowsePage> {
    const {token, view, filters} = await readDefaults(req);
    return client.browse(token, {...req, view, filters});
  }

  /** Woken by `setView`, a region selected and `dispose`: an `inView` read waiting for its area. */
  const areaWaits = new Set<() => void>();

  function areaChanged(): void {
    const waits = [...areaWaits];
    areaWaits.clear();
    for (const wake of waits) wake();
  }

  /** Settles at the next `setView`, region selected or `dispose`; rejects where `signal` aborts first. */
  function nextArea(signal?: AbortSignal): Promise<void> {
    return new Promise((resolve, reject) => {
      const aborted = () => {
        areaWaits.delete(wake);
        reject(signal!.reason);
      };
      const wake = () => {
        signal?.removeEventListener('abort', aborted);
        resolve();
      };
      if (signal?.aborted) return reject(signal.reason);
      signal?.addEventListener('abort', aborted, {once: true});
      areaWaits.add(wake);
    });
  }

  async function items(req: StoreItemsRequest, signal?: AbortSignal): Promise<RecordsRead<ItemsHead>> {
    const {inView, filters: _filters, view: _view, ...rest} = req;
    for (;;) {
      const read = await readDefaults(req);
      let filters = read.filters;
      if (inView) {
        if (read.view !== read.current) throw new Error(`\`inView\` reads the current view '${read.current}': leave \`view\` out or set it to that.`);
        const area = countedArea();
        if (area === null) {
          if (!disposed) await nextArea(signal);
          if (disposed) throw new DOMException('the store was disposed before the camera was set', 'AbortError');
          continue;
        }
        // A caller's own filters do not carry the selected region, so its leaf is joined to them.
        filters = read.own ? withRegion(filters, area.operand, area.outside) : withArea(filters, area);
      }
      return client.items(read.token, {...rest, view: read.view, ...(filters === null ? {} : {filters})}, signal);
    }
  }

  function setMembers(clauses: readonly MemberClause[]): void {
    const members = [...clauses];
    replaceProjection('filters', {...projections.filters, members});
    region.loading(projections.marks);
    requery();
  }

  function setFilters(draft: FilterDraft): void {
    filtersSet = true;
    const expr = composeFilters(draft, 'filter');
    replaceProjection('filters', {...projections.filters, draft, expr, highlight: composeFilters(draft, 'highlight')});
    region.loading(projections.marks);
    requery();
  }

  function setLayers(names: string[]): void {
    layersOn = meta ? drawnOnly(layerClosure(meta.layers, names)) : names;
    askLayers();
  }

  /** How many times the selection has been cleared: an item or artifact asked for before a clear is not shown. */
  let selectionClears = 0;

  function clearSelection(): void {
    selectionClears += 1;
    replaceProjection('selection', {item: null, itemRefusal: null, artifact: null, artifactRefusal: null});
  }

  function setColourBy(column: string | null): void {
    const before = colourLayer();
    legend.setColourBy(column);
    traceUnknownColourLayer();
    if (colourLayer() !== before) askLayers();
    checkPointColumns();
    if (column && projections.view.composition) legend.accumulate(projections.view.composition, meta?.declaredScalars ?? []);
  }

  function setSizeBy(column: string | null, options: {rank?: boolean} = {}): void {
    legend.setSizeBy(column, options.rank ?? false);
    checkPointColumns();
    if (column && projections.view.composition) legend.accumulate(projections.view.composition, meta?.declaredScalars ?? []);
  }

  function setPointColumns(id: string, columns: readonly string[]): void {
    if (columns.length === 0) pointColumnAsks.delete(id);
    else pointColumnAsks.set(id, [...columns]);
    checkPointColumns();
  }

  function setClusterBudget(next: number | null): void {
    if (next !== null && !(Number.isInteger(next) && next > 0)) return;
    if (next === clusterBudget) return;
    clusterBudget = next;
    for (const held of views.all()) held.channel.setBudget(next ?? undefined);
    aggregates.refresh(false, {which: drawsCut, changed: true});
  }

  function setPalette(next: PaletteName): void {
    if (!Object.hasOwn(PALETTES, next)) return;
    colours.setPalette(next);
    for (const held of views.all()) held.channel.setPaletteSize(paletteSize(next));
    aggregates.refresh(false, {which: drawsPalette, changed: true});
  }

  function setBudget(next: number): void {
    if (!Number.isFinite(next) || next <= 0) return;
    budget = next;
    views.current?.presenter.setBudget(next);
    views.current?.presenter.reschedule();
  }

  async function pick(id: bigint): Promise<void> {
    const epoch = selectionClears;
    try {
      const detail = await client.item(await tokens.get(), id);
      if (disposed || epoch !== selectionClears) return;
      replaceProjection('selection', {...projections.selection, item: {id, detail}, itemRefusal: null});
    } catch (error) {
      if (disposed || epoch !== selectionClears) return;
      replaceProjection('selection', {
        ...projections.selection,
        item: null,
        itemRefusal: refusalOf(error)
      });
    }
  }

  /** The kind a served artifact's layer draws, from the meta; null where it draws none. */
  function shapeKindOf(id: bigint): ShapeKind | null {
    const layer = projections.artifacts.served.find((a) => a.mosaicaId === id)?.layer;
    if (layer === undefined) return null;
    return projections.meta?.layers.find((l) => l.name === layer)?.shape ?? null;
  }

  async function openArtifact(id: bigint): Promise<void> {
    const epoch = selectionClears;
    try {
      const asked = await viewed();
      const detail = await client.artifact(asked.token, id, {view: asked.view});
      if (disposed || epoch !== selectionClears) return;
      // A renewal may have changed the principal under the request, and a shape can be theirs.
      if (tokens.current === asked.token) shapes.hold(id, detail.shape);
      replaceProjection('selection', {...projections.selection, artifact: {id, detail}, artifactRefusal: null, item: null, itemRefusal: null});
    } catch (error) {
      if (disposed || epoch !== selectionClears) return;
      replaceProjection('selection', {
        ...projections.selection,
        artifact: null,
        artifactRefusal: refusalOf(error)
      });
    }
  }

  function extentOf(artifactId: bigint): [number, number, number, number] | null {
    const artifact = projections.artifacts.served.find((a) => a.mosaicaId === artifactId);
    const box = artifact?.box;
    if (!box || !meta) return null;
    // The box is in the wire's 32-bit grid units.
    const [x0, y0] = dataXY(gridToWorld(box[0]), gridToWorld(box[1]));
    const [x1, y1] = dataXY(gridToWorld(box[2]), gridToWorld(box[3]));
    return [x0, y0, x1, y1];
  }

  function dataXY(worldX: number, worldY: number): [number, number] {
    const q = frame();
    return [
      q.xMin + (worldX / WORLD_SIZE) * (q.xMax - q.xMin),
      q.yMin + (worldY / WORLD_SIZE) * (q.yMax - q.yMin)
    ];
  }

  /**
   * How many times the store has been cleared: a warm-up begun before a clear reads nothing.
   */
  let clears = 0;
  /** Whether the host has called `setFilters`. Until it has, the draft is seeded from `meta`. */
  let filtersSet = false;

  function clear(): void {
    tokens.forget();
    region.drop();
    forgetAnswers();
  }

  /**
   * Drop what the server answered: points, frames, counts, artifacts, shapes, records, legend
   * values, category titles, typeahead pages, the picked item and opened artifact, the `member_of`
   * clauses, a region drawn from an artifact, the region's held marks and counts, and `meta`. The
   * store reads `meta` again under the token it holds and asks again for the camera, the region's
   * counts with it. What is kept is listed on {@link Store}.
   */
  function forgetAnswers(): void {
    clears += 1;
    unconfirmed = null;
    if (identityRetry !== null) clock.cancel(identityRetry);
    identityRetry = null;
    suggestions.reset();
    for (const held of views.all()) {
      held.presenter.cancel();
      held.channel.reset();
      held.replica.reset();
    }
    // Each view is built again from the meta read next.
    views.forget();
    bandBudget = new BandBudget(bandBudget.budgetBytes);
    identities.clear();
    table.clear();
    colours.refresh();
    shapes.forget('all');
    records.forget();
    clearSelection();
    contentKeyAtFrame = '';
    corpusKey = '';
    colourAsked.clear();
    awaitingSwitchFrame = false;
    replaceProjection('view', noFrame(views.id));
    replaceProjection('marks', {...projections.marks, bands: [], standIn: [], count: NO_COUNT});
    replaceProjection('tiles', {tiles: []});
    // An artifact's id may name nothing under the next key. A drawn shape is placed against the
    // view's frame, so it is published before `meta` goes.
    if (region.selected?.kind === 'artifact') region.drop();
    else region.loading(projections.marks);
    // A clause names an artifact by an id the next key may not reach, and carries its served label.
    // A seeded draft names the previous viewer's scoped columns.
    replaceProjection('filters', {
      ...projections.filters,
      members: [],
      ...(filtersSet ? {} : {draft: emptyDraft([]), expr: null, highlight: null})
    });
    legend.clear();
    for (const column of rankingAsks.keys()) ranking.set(column, null);
    rankingAsks.clear();
    dropTitles();
    meta = null;
    warming = null;
    replaceProjection('meta', null);
    replaceProjection('status', {...NO_STATUS, unreachable});
    publishReplica(null);
    void ready().catch(() => {});
    // Each asks under the meta and token the read above brings.
    aggregates.refresh(true);
    askInView(false, true);
  }

  function refresh(): void {
    // Redraw against the current content key, refetching the artifacts with it.
    views.current?.channel.reset();
    region.loading(projections.marks);
    if (lastView) setView(lastView.input);
    aggregates.refresh(false);
    askInView(false);
  }

  /** Thrown into an answer {@link admit} refused, so nothing in it is held. */
  function identityChanged(): Error {
    return new DOMException('the identity key changed; the store asks again', 'AbortError');
  }

  /** The world outline under the current token, for {@link Store.basemapWorld}. */
  async function askWorld(): Promise<ArrayBuffer> {
    const token = await tokens.get();
    try {
      return await client.basemapWorld(token);
    } catch (error) {
      if (!(error instanceof MosaicaError && error.status === 401) || tokens.current === token) throw error;
      return client.basemapWorld(await tokens.get());
    }
  }

  /** Set by `dispose`: an answer that lands afterwards writes nothing. */
  let disposed = false;

  function dispose(): void {
    disposed = true;
    tokens.dispose();
    suggestions.dispose();
    shapes.dispose();
    records.dispose();
    legend.dispose();
    aggregates.dispose();
    ranking.dispose();
    titling.dispose();
    dropTitles();
    stopTitles();
    inView.dispose();
    areaChanged();
    if (inViewTimer !== null) clock.cancel(inViewTimer);
    if (probe !== null) clock.cancel(probe);
    if (identityRetry !== null) clock.cancel(identityRetry);
    for (const held of views.all()) {
      held.presenter.cancel();
      held.channel.cancel();
    }
    views.cancelSettle();
    if (ownsClient) client.close();
  }

  const store: Store = {
    get projections() {
      return projections;
    },
    get(name) {
      return projections[name];
    },
    subscribe(nameOrFn: ProjectionName | Listener, fn?: (value: never) => void): () => void {
      if (typeof nameOrFn === 'function') {
        all.add(nameOrFn);
        return () => all.delete(nameOrFn);
      }
      const name = nameOrFn;
      const set = perName.get(name) ?? new Set();
      const wrapped = () => (fn as (v: Projections[typeof name]) => void)(projections[name]);
      set.add(wrapped);
      perName.set(name, set);
      return () => set.delete(wrapped);
    },
    setView,
    browse,
    items,
    requestFilters,
    setAggregate,
    setFilters,
    setMembers,
    suggest: (column, q, verb) => suggestions.suggest(column, q, verb),
    forgetSuggestions: (column) => suggestions.forget(column),
    setLayers,
    setColourBy,
    setSizeBy,
    setPointColumns,
    setPalette,
    setArtifactColours: (chosen) => colours.setOverrides(chosen),
    setBudget,
    get budget() {
      return budget;
    },
    setClusterBudget,
    get clusterBudget() {
      return clusterBudget;
    },
    setCurrentView,
    frame: frameOrNull,
    pick,
    describe: (id) => records.describe(id),
    titleOf,
    heldTitle: (column, key) => titles.get(`${column}\u0000${key}`),
    openArtifact,
    needShape: (id) => shapes.need(id),
    clearSelection,
    select(shape) {
      if (region.select(shape, projections.marks)) {
        requery();
        areaChanged();
      }
    },
    extentOf,
    dataXY,
    async basemapWorld() {
      const server = options.client ?? serverOf(options.viewerUrl!);
      const shared = worldOutlines.get(server);
      if (shared) {
        try {
          return await shared;
        } catch {
          // Another store's request failed; this one asks under its own token.
        }
      }
      const asked = askWorld();
      worldOutlines.set(server, asked);
      asked.catch(() => {
        if (worldOutlines.get(server) === asked) worldOutlines.delete(server);
      });
      return asked;
    },
    clear,
    refresh,
    dispose
  };

  void ready().catch(() => {});

  return store;
}

/** Re-export for a consumer building a `ViewportResult` in a test. */
export type {ViewportResult};
