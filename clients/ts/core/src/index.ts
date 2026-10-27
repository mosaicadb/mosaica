/**
 * `@mosaicajs/client` is the headless part of the TypeScript client. It talks to a Mosaica server
 * and holds the state of a map, and leaves the drawing to other code. It has two layers.
 * {@link MosaicaClient} is a thin HTTP client with one method per viewer and session route, and it
 * keeps no state between calls. The store, which {@link createStore} makes, is built on it: you
 * tell the store where the camera is, what to filter and which layers to draw, and it fetches,
 * caches and composes the data. It publishes the result as projections: read-only snapshots of
 * one part of the map's state each, such as `marks`, the points to draw, or `view`, the counts for
 * the current view. `MosaicaLayer` in
 * `@mosaicajs/deck`, the elements in `@mosaicajs/components` and the hooks in `@mosaicajs/react`
 * all draw from a store, and your own code can read its projections too.
 *
 * ```mermaid
 * flowchart LR
 *   server["Mosaica server"] <--> client["MosaicaClient"]
 *   client <--> store["Store"]
 *   app["Your app"] -- "camera, filters, layers" --> store
 *   store -- "projections" --> draw["MosaicaLayer, components, your code"]
 * ```
 *
 * {@link Control} is a separate client for the server's control plane, which declares, ingests,
 * deletes and publishes data. It sends an operator credential or an API key, so it belongs in a
 * back end or a script. {@link RecordsRead} is the paged read of items or artifacts that both
 * `MosaicaClient` and the store return.
 *
 * The rest of the package is helpers for the store: building the filter drafts and `member_of`
 * clauses it takes, listing which layers it can draw and colour by, converting coordinates,
 * choosing palettes and formatting counts.
 *
 * @module @mosaicajs/client
 */
export {createStore, CLUSTER_PREFIX, DEFAULT_BUDGET, REGION_HELD_LIMIT, type Store} from './store.js';
export type {
  AggregateEntry,
  AggregateSpec,
  AggregateSpecGrouping,
  DrawnLayerBy,
  AggregatesProjection,
  FieldSummary,
  ArtifactsProjection,
  FiltersProjection,
  InViewCounts,
  LegendProjection,
  MarksProjection,
  ProjectionName,
  Projections,
  RegionProjection,
  ReplicaProjection,
  SelectionProjection,
  SelectionShape,
  StatusProjection,
  StoreItemsRequest,
  StoreOptions,
  TilesProjection,
  TokenSupplier,
  ViewInput,
  ViewProjection
} from './store.js';
export type {SuggestionPage, SuggestState} from './suggestions.js';
export type {Band, BandMembership} from './bands.js';
export type {ComposedTile, Composition, StandInPiece} from './compose.js';
export type {PresentedStatus, Refusal} from './presented.js';
export {subtreeOf, type ServedLineage} from './artifactChannel.js';
export {NO_ORDINAL, type ArtifactEntry, type ArtifactTable, type ArtifactTableChange} from './artifactTable.js';
export type {Domain, Ranks, ValueSample} from './encoding.js';
export {DEFAULT_PALETTE, NEUTRAL, PALETTES, artifactColour, paletteSize, type ChosenColours, type Palette, type PaletteName, type Rgba} from './palette.js';

export {formatCount, formatMasked, NO_COUNT, NO_MASKED, type Count, type FormatOptions, type Masked} from './counts.js';

export {
  activeCount,
  composeFilters,
  emptyDraft,
  isPopulated,
  textQueryOf,
  textTerms,
  withoutClause,
  type ClauseVerb,
  type ColumnDraft,
  type FilterDraft,
  type TextTerms
} from './filters.js';
export {browsableLayers, colourLayers, drawableLayers, isFilterLayer, layerClosure, layerEntries, type LayerEntry} from './layers.js';
export {artifactName} from './names.js';
export {memberKey, memberLeaf, memberOf, withMember, withMembers, withoutMember, type MemberClause} from './members.js';

export {GRID32, MAX_DEPTH, WORLD_SIZE, dataToWorldXY, gridToWorld, gridToWorldXY} from './coords.js';
export {lonLatOfCell} from './projection.js';

export {MosaicaClient, MosaicaError, type PartSink, type MosaicaClientOptions, type TileSink} from './client.js';
export {inlineDecoder, setWorkerFactory, type Decoder} from './decoder.js';
export {RecordsRead} from './records.js';
export {PartialAggregate} from './aggregate.js';
export {
  Control,
  UNANSWERED,
  type AddressRow,
  type Answer,
  type ArtifactsPublished,
  type Attachment,
  type CallOptions,
  type CatalogueChange,
  type ChangeItem,
  type ChangesApplied,
  type ControlOptions,
  type Grant,
  type GroupRecord,
  type GrowingArtifact,
  type GrowRequest,
  type IngestResponse,
  type IssuedKey,
  type KeyCreate,
  type KeyRecord,
  type MemberTable,
  type MembershipsGrown,
  type Permission,
  type PrincipalChange,
  type PrincipalRecord,
  type ProviderDeclaration,
  type ProviderRecord,
  type PublicationAck,
  type PublishedArtifact,
  type PublishRequest,
  type RefusalReason,
  type RefusedMember,
  type RefusedRow,
  type RowAnswer,
  type RowOptions,
  type SessionRecord,
  type SessionsToEnd,
  type ShapeReport,
  type StrictOptions,
  type WriteOptions
} from './control.js';
export * from './types.js';
