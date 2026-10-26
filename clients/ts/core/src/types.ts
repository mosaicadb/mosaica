import type {Table} from 'apache-arrow';

/**
 * A session created by {@link MosaicaClient.authorise}.
 *
 * @category Requests and responses
 */
export type Session = Login & {
  /** The session's id, which {@link MosaicaClient.revoke} takes. It is not a credential. */
  tokenId: number;
};

/**
 * A session, as {@link MosaicaClient.login} returns it.
 *
 * @category Requests and responses
 */
export type Login = {
  /** The bearer token every viewer route takes. It is a credential. */
  token: string;
  /**
   * When the session expires, in seconds since the Unix epoch. It is no later than the server's
   * `token_max_lifetime` after the request, nor than the expiry of the API key or access token
   * used to create it. A change to the catalogue can end it sooner.
   */
  expiresAt: number;
};

/**
 * The credential {@link MosaicaClient.login} sends: a password, an API key or an OIDC access token.
 *
 * @category Requests and responses
 */
export type LoginCredential =
  | {
      /** A local principal's name. */
      principal: string;
      /** Its password. */
      password: string;
    }
  | {
      /** An API key, `msk_<prefix>_<secret>`. */
      apiKey: string;
    }
  | {
      /** An OIDC access token issued by a declared provider. */
      accessToken: string;
    };

/**
 * Who {@link MosaicaClient.authorise} creates a session for.
 *
 * @category Requests and responses
 */
export type AuthoriseTarget =
  | {
      /** A local principal, by name. */
      principal: string;
    }
  | {
      /** An OIDC identity, by the access token its provider issued. */
      accessToken: string;
    }
  | {
      /**
       * A set of terms, for a session that belongs to no principal and has `read` permission only.
       * Only the operator credential may ask for one. Each term is trimmed, and a term that is
       * empty or holds a control character is dropped. Every session holds `public`, so naming it
       * changes nothing.
       */
      terms: string[];
    }
  | {
      /**
       * A superuser session with `read` and `read-all`, which sees every item and satisfies every
       * label, including items added after it was created. Deletions and suppressions still apply.
       * Only the operator credential may ask for one.
       */
      readAll: true;
    };

/**
 * The extent of a view's data coordinates. Positions are stored as 32-bit grid units on each axis:
 * `0` at the minimum and the top of the grid at the maximum, which is inclusive.
 *
 * @category Meta
 */
export type Quantisation = {
  /** The smallest x, in data coordinates. */
  xMin: number;
  /** The largest x, in data coordinates. */
  xMax: number;
  /** The smallest y, in data coordinates. */
  yMin: number;
  /** The largest y, in data coordinates. */
  yMax: number;
};

/**
 * A declared column's storage type. `timestamp_us` is microseconds since the Unix epoch.
 * `keyword` is a short string matched exactly and `text` is prose searched by word. Neither string
 * type is sent with points; read them with {@link MosaicaClient.item}. `utf8` cannot be declared.
 *
 * @category Meta
 */
export type ArrowType =
  | 'bool'
  | 'u8'
  | 'u16'
  | 'u32'
  | 'u64'
  | 'i8'
  | 'i16'
  | 'i32'
  | 'i64'
  | 'f32'
  | 'f64'
  | 'timestamp_us'
  | 'utf8'
  | 'keyword'
  | 'text';

/**
 * Marks an integer column as a category. Points carry a category as an integer code, and this
 * object is the only sign that a column is one. The codes are random and their order means
 * nothing, so do not colour them on a continuous scale. {@link MosaicaClient.categories} turns
 * codes into values.
 *
 * @category Meta
 */
export type CategoryDescriptor = {
  /**
   * The name of the value set the codes refer to. Columns that share a vocabulary share its keys,
   * codes and titles, so one palette serves them all. Which values are visible is decided per
   * column.
   */
  vocabulary: string;
  /**
   * `declared`: the value set is fixed, and a write with an unknown key is refused. `discovered`: a
   * new key gets a code when it is first written.
   */
  kind: 'declared' | 'discovered';
  /**
   * `public`: every viewer is offered every value. `derived`: a viewer is offered only the values
   * held by at least one item they can see.
   */
  visibility: 'derived' | 'public';
};

/**
 * A declared column with one value per item.
 *
 * @category Meta
 */
export type DeclaredScalar = {
  /** The column's name, which filters, `/v1/categories/{column}` and `ViewportResult.scalars` use. */
  name: string;
  /** The column's storage type. */
  arrowType: ArrowType;
  /** How the column works as a category, or `null` if it is not one. */
  category: CategoryDescriptor | null;
  /**
   * Whether points can carry the column in a viewport response, which is what lets it colour or
   * size them. A missing value arrives as null, or as code `0` in a category. A column without
   * `render` can still be filterable (see {@link Meta.filterOperands}) and is returned by
   * {@link MosaicaClient.item}.
   */
  render: boolean;
  /**
   * Whether the column has a filter index. A column with none of `render`, `index` and `unique`
   * can be read on an item but not filtered on.
   */
  index: boolean;
  /**
   * Whether every item holds a different value. A unique column takes `eq` and `in` filters, which
   * find items by value, even without `render` or `index`.
   */
  unique: boolean;
  /** For a `text` column, its analyser as `<name>/<version>`; otherwise `null`. */
  analyser: string | null;
  /**
   * Where {@link MosaicaClient.items} reads the value from: `rendered` (the view's point columns),
   * `value_column` (a per-item column) or `record` (the record store). A column held only in
   * `record` reads fastest in `stored` order.
   */
  homes: ('rendered' | 'value_column' | 'record')[];
};

/**
 * A group-scoped column: a family with one column per view of a view group, so an item can hold a
 * different value in each view. Absent from `/v1/meta` if the viewer cannot reach the group.
 *
 * @category Meta
 */
export type ScopedScalar = {
  /** The family's name. A filter uses it, pinned to a view if needed (see {@link FilterOperandSet.scope}). */
  name: string;
  /** The storage type, as on a {@link DeclaredScalar}. */
  arrowType: ArrowType;
  /** The view group whose views each have a column of this family. */
  scope: {group: string};
  /** How the family works as a category, or `null` if it is not one. */
  category: CategoryDescriptor | null;
  /** For a `text` family, its analyser as `<name>/<version>`; otherwise `null`. */
  analyser: string | null;
  /** Whether points carry the column in a viewport response. Only views listed in {@link views} carry it. */
  render: boolean;
  /** Whether the family has a filter index. {@link Meta.filterOperands} lists the families you can filter on. */
  index: boolean;
  /**
   * The views the viewer can reach whose points carry this column: the group's own views, and the
   * same keys in any group that shares them. A view created on a running service appears once a
   * flush has written its values.
   */
  views: string[];
};

/**
 * A column you can filter on, and how. Build filter controls from these. `family` says which
 * control fits: a `category` has a list of values ({@link MosaicaClient.categories}), a `keyword`
 * takes typed text, a `text` column is searched by words, and a `numeric` one by range.
 *
 * @category Meta
 */
export type FilterOperandSet = {
  /** The column or family name a filter uses. */
  column: string;
  /** The column's filter family, which decides its operators. */
  family: 'category' | 'keyword' | 'text' | 'numeric';
  /**
   * The operators the column accepts: `eq` and `in` for a category; `eq`, `in`, `prefix` and
   * `contains` for a keyword; `eq`, `in` and `range` for a number; `match` and `phrase` for text.
   * A unique column with neither `render` nor `index` takes only `eq` and `in`.
   */
  operands: string[];
  /**
   * The view group, if the column is group-scoped. In a view of that group, or of a group that
   * shares its views, a filter names the column plainly and reads the value for the request's view.
   * In any other view it must name a view as `column@key`, or the request is refused with `422`.
   */
  scope?: {group: string};
};

/**
 * A filter expression, as the viewer routes take it.
 *
 * Each node has exactly one key: a column name mapped to a {@link FilterOperator}, `region`,
 * `member_of`, or one of the combinators `all_of`, `any_of` and `none_of`. `all_of: []` matches
 * every item and `any_of: []` matches none. `none_of` matches every visible item that none of its
 * branches matches, including items with no value in the column. The server refuses with `422` a
 * node with two keys, an unknown column, an operator the column does not take, or nesting deeper
 * than the deployment allows.
 *
 * A value that does not exist and a value the viewer cannot see both match nothing, with the same
 * response.
 *
 * @category Filters
 */
export type FilterExpr =
  | {all_of: FilterExpr[]}
  | {any_of: FilterExpr[]}
  | {none_of: FilterExpr[]}
  | {region: RegionOperand}
  | {member_of: MemberOfOperand}
  | {[column: string]: FilterOperator};

/**
 * The `region` filter: the items whose position lies inside a shape. Give exactly one of:
 *
 * - `polygon`: at least three `[x, y]` vertices, closed automatically. More than
 *   `selection.maxRegionVertices` is refused with `422`.
 * - `bbox`: `[x0, y0, x1, y1]`, edges included.
 * - `circle`: `[cx, cy, r]`.
 * - `ellipse`: `[cx, cy, a, b, angle_degrees]`.
 * - `artifact`: an artifact's `mosaica_id` as a decimal string, matching its members. An artifact
 *   that does not exist, is not served to the viewer, is on another view, or has an authored shape
 *   matches nothing.
 *
 * Coordinates are in the view's data coordinates. A shape with a very long boundary may be
 * answered for a slightly larger area; the response's {@link RegionVerdict} says so.
 *
 * @category Filters
 */
export type RegionOperand =
  | {polygon: [number, number][]; space?: 'view'}
  | {bbox: [number, number, number, number]; space?: 'view'}
  | {circle: [number, number, number]; space?: 'view'}
  | {ellipse: [number, number, number, number, number]; space?: 'view'}
  | {artifact: string};

/**
 * The `member_of` filter: the visible members of one artifact, such as a cluster. It works in
 * `filters` to narrow the map and in `highlight` to light it. {@link memberOf} builds one.
 *
 * A layer the viewer's `/v1/meta` does not list is refused with `422`. An artifact that does not
 * exist, belongs to another layer, is suppressed or is not served to the viewer matches nothing.
 *
 * @category Filters
 */
export type MemberOfOperand = {
  /** The layer's name, as `/v1/meta` lists it. */
  layer: string;
  /** The artifact's `mosaica_id` as a decimal string, since a JSON number cannot hold every 64-bit id. */
  artifact: string;
};

/**
 * Whether a response with a `region` filter is exact (the `x-mosaica-region` header). With
 * `exact: false`, a shape's boundary needed more than `selection.maxRegionCells` cells, so the
 * answer covers a slightly larger area made of cells at depth `depth`. It depends only on the
 * shapes, never on the items. With several regions it reports the coarsest.
 *
 * @category Filters
 */
export type RegionVerdict = {exact: true; depth: null} | {exact: false; depth: number};

/**
 * One column's condition, with exactly one key.
 *
 * A category takes its key (a string) or its code (a number), and a keyword takes exact strings.
 * A numeric column takes numbers, and `range` takes at least one bound and at most one per side.
 * An integer or timestamp column also takes a `bigint`, which stays exact above 2^53. A timestamp
 * takes microseconds since the Unix epoch, a date (`'2020-01-01'`, midnight UTC) or a date-time
 * with an offset (`'2020-01-01T09:30:00Z'`). A `text` column takes `match` or `phrase`.
 *
 * @category Filters
 */
export type FilterOperator =
  | {eq: string | number | boolean | bigint}
  | {in: (string | number | bigint)[]}
  | {prefix: string}
  | {contains: string}
  /** Every word must appear, unless `minimum_should_match` says how many must. */
  | {match: string | {query: string; minimum_should_match?: number}}
  /** The words must appear next to each other, in order. */
  | {phrase: string}
  | {range: {gte?: number | bigint | string; gt?: number | bigint | string; lte?: number | bigint | string; lt?: number | bigint | string}};

/**
 * An annotation layer, such as a clustering or a set of boundaries, as `/v1/meta` lists it. A
 * layer the viewer cannot reach is not listed. The entry does not say how many artifacts the layer
 * holds.
 *
 * @category Meta
 */
export type Layer = {
  /** The layer's name, which requests use. */
  name: string;
  /** The layer's display title, or `null` if it has none. */
  title: string | null;
  /** The views the layer appears in that the viewer can reach. */
  views: string[];
  /**
   * How an artifact's members are decided: `enumerated`, a stored list; `spatial`, the items inside
   * its shape; `attribute`, the items holding one value of a column. An attribute layer arrives as
   * `{attribute: <column>}`, which this type does not declare.
   */
  membership: 'enumerated' | 'spatial' | 'attribute';
  /**
   * How artifacts relate. `flat`: no hierarchy. `nested`: a tree of parent links, with no levels.
   * `dag`: like `nested`, but a child may have several parents. `stacked`: independent levels.
   * `tiered`: levels whose artifacts sit inside those of the level above. With `pruneChildren`, a
   * parent and its child that both qualify serve only the child, unless a request's
   * `artifactBudget` asks for more detail.
   */
  hierarchy: {kind: 'flat' | 'nested' | 'dag' | 'stacked' | 'tiered'; pruneChildren: boolean};
  /**
   * The layer's levels, each with its number, title and `zoom`: the `[min, max]` tile depths at
   * which it is shown by default, or `null` for every depth. Empty for a `nested` or `dag` layer.
   */
  levels: {level: number; title: string | null; zoom: [number, number] | null}[];
  /**
   * The geometry computed per request from the members the viewer can see: any of `centroid`,
   * `box` and `hull`.
   */
  computedContent: string[];
  /**
   * The kind of outline the layer draws, or `null` if it draws none. A `derived` outline is the
   * hull of the members the viewer can see, so it differs between viewers; do not reuse it for
   * another viewer. A `predicate` or `authored` outline is the same for everyone.
   */
  shape: ShapeKind | null;
  /** The types of publisher-supplied content each artifact carries, in the order of {@link Artifact.content}. */
  suppliedContent: string[];
  /** The layers whose artifacts this layer's artifacts attach to, as topic labels attach to clusters. Usually empty. */
  depsOn: string[];
  /** A number that changes when the layer's access rule is edited. */
  version: number;
};

/**
 * The map projection that placed a view's positions. See {@link ViewInfo.projection}.
 *
 * @category Meta
 */
export type MapProjection = 'web_mercator' | 'equirectangular' | 'gall_isographic' | 'none';

/**
 * A basemap tile scheme. The only one is `xyz`, the `z/x/y` scheme slippy-map tile servers use.
 * See {@link ViewInfo.tileScheme}.
 *
 * @category Meta
 */
export type TileScheme = 'xyz';

/**
 * A view, one 2D layout of the items, as `/v1/meta` lists it. Use its projection fields to choose
 * a basemap, and {@link lonLatOfCell} to turn a position into longitude and latitude.
 *
 * @category Meta
 */
export type ViewInfo = {
  /** The view's id, which requests use: the view's name, or `<group>:<key>` for a view in a group. */
  id: string;
  /** The view's display name. For a view in a group it is the id. */
  displayName: string;
  /** The extent of this view's data coordinates. Each view has its own. */
  quantisation: Quantisation;
  /** The map projection that placed the positions, or `none` if the view is not a map. */
  projection: MapProjection;
  /**
   * The width-to-height ratio to draw the world at: `1` for `web_mercator`, `2cos φ₁` for
   * `equirectangular` and `gall_isographic` (φ₁ being the standard parallel), and `null` for
   * `none`.
   */
  worldAspect: number | null;
  /**
   * `xyz` if the view lines up with slippy-map tiles, so raster tiles can be drawn under the
   * points; otherwise `null`. Only a `web_mercator` view can line up.
   */
  tileScheme: TileScheme | null;
  /** The tile the view covers, or `null` if `tileScheme` is `null`. */
  tile: {z: number; x: number; y: number} | null;
  /** The view's place in its group, or `null` if it is in no group. */
  roster: ViewRoster | null;
};

/**
 * A view's place in its group. The view's id is `<group>:<key>`. The group's views are ordered as
 * {@link ViewGroup.views} lists them, in creation order. To move between views, step through that
 * list; keys are your own strings and need not sort in order.
 *
 * @category Meta
 */
export type ViewRoster = {
  /** The group's name. */
  group: string;
  /** The view's key within its group. */
  key: string;
  /**
   * The view's metadata, one entry per name the group declares. Empty in a group that borrows
   * another group's keys; read the metadata there.
   */
  metadata: Record<string, ViewMetadataValue>;
};

/**
 * One metadata value of a view, such as the date a snapshot was taken. `timestamp_us` is
 * microseconds since the Unix epoch. Metadata describes the view as a whole and cannot be filtered
 * on.
 *
 * @category Meta
 */
export type ViewMetadataValue =
  | {type: 'bool'; value: boolean}
  | {type: 'int'; value: number}
  | {type: 'float'; value: number}
  | {type: 'text'; value: string}
  | {type: 'timestamp_us'; value: number};

/**
 * A view group: many views of the same kind, such as one layout per year. A request names one of
 * its views, never the group.
 *
 * @category Meta
 */
export type ViewGroup = {
  /** The group's name. */
  name: string;
  /** The group's display title, or `null` if it has none. */
  title: string | null;
  /**
   * The group whose keys this group's views use, if this group is a second layout over another
   * group's views; otherwise `null`.
   */
  membersOf: string | null;
  /** The ids of the group's views the viewer can reach, as `<group>:<key>`, in creation order. */
  views: string[];
};

/**
 * The server's description of what the viewer can use, from `GET /v1/meta`: views, columns,
 * filterable columns, annotation layers and request limits. It lists only what the viewer can
 * reach, so do not show one viewer's `Meta` to another.
 *
 * @category Meta
 */
export type Meta = {
  /** The viewer API's version, `1`. */
  apiVersion: number;
  /** The bundle format version the server opened. */
  bundleFormat: number;
  /**
   * The views the viewer can reach: views in no group first, then each group's in creation order.
   * Asking for any other view is a `404`.
   */
  views: ViewInfo[];
  /** The view groups the viewer can reach. */
  groups: ViewGroup[];
  /** The columns with one value per item, in declaration order. */
  declaredScalars: DeclaredScalar[];
  /** The group-scoped columns the viewer can reach. */
  scopedScalars: ScopedScalar[];
  /** The annotation layers the viewer can reach. */
  layers: Layer[];
  /** How many points the server selects per tile, and the deployment's request limits. */
  selection: {
    /** The fewest points a tile serves if it has that many. */
    kMin: number;
    /** The most points one tile serves, and the default `k`. */
    kMaxMarks: number;
    /** The largest `k` a request can ask for. A tile serves at most `min(k, maxK, kMaxMarks)` points. */
    maxK: number;
    /**
     * The number of points an unfiltered request aims to serve in an average occupied tile. A
     * filtered request serves every match up to the per-tile limit.
     */
    thetaTargetMarks: number;
    /** The largest `underlayOffset` a viewport request may ask for. */
    maxUnderlayOffset: number;
    /** The largest `perTile` a {@link MosaicaClient.viewportArtifacts} request may ask for. */
    maxArtifactsPerTile: number;
    /** The page size of {@link MosaicaClient.categories}, and its largest. A shorter page is the last. */
    maxCategoryValues: number;
    /** The most vertices a `region` polygon may have; more is a `422`. */
    maxRegionVertices: number;
    /**
     * The most boundary cells a `region` may use. A larger shape is still answered, for a slightly
     * larger area, and {@link RegionVerdict} says so.
     */
    maxRegionCells: number;
    /** The page size of {@link MosaicaClient.browse}, and its largest. */
    maxBrowseRows: number;
    /** The most vertices a published shape may have. */
    maxShapeVertices: number;
    /** The default and largest `limit` of {@link MosaicaClient.suggest}. */
    maxSuggestions: number;
    /** The most values one suggestion request examines before it stops and answers `more: true`. */
    maxSuggestionWalk: number;
    /**
     * Up to this many visible items, suggestions come only from values the viewer can see. Above
     * it, `more` may mean the search reached `maxSuggestionWalk`.
     */
    maxSuggestSetEntities: number;
    /** The most rows in one page of {@link MosaicaClient.items} or {@link MosaicaClient.artifacts}. */
    maxPageRows: number;
    /**
     * The most bytes in one page of {@link MosaicaClient.items} or {@link MosaicaClient.artifacts},
     * before compression. A single larger row is sent on its own.
     */
    maxPageBytes: number;
    /** The most groupings one {@link MosaicaClient.aggregate} request may have. */
    maxAggregateGroupings: number;
    /** The largest `top` of an aggregate grouping. */
    maxAggregateTop: number;
    /** The most `values` or `artifacts` an aggregate grouping may name. */
    maxAggregateNamed: number;
    /** The largest `bins` of an aggregate grouping. */
    maxAggregateBins: number;
    /** The most cells an aggregate grouping's `cells` may cover. */
    maxAggregateCells: number;
  };
  /** The most tiles one viewport request may cover; more is a `422`. */
  maxTilesPerRequest: number;
  /**
   * The columns you can filter on, with their operators. Build filter controls from this list. It
   * differs from {@link Meta.declaredScalars}: a filterable column need not arrive with the points,
   * and a column that arrives with them need not be filterable.
   */
  filterOperands: FilterOperandSet[];
};

/**
 * One category value: what a code stands for, and how to show it.
 *
 * @category Requests and responses
 */
export type CategoryValue = {
  /** The integer code that points carry for this value. Never `0`, which means no value. */
  code: number;
  /** The value's key, which filters use. Show it if there is no title. */
  key: string;
  /** The value's display title, or `null` if none was written. Values of a discovered vocabulary have none. */
  title: string | null;
};

/**
 * Where a suggestion matched the query, so you can highlight the match. `start` and `len` count
 * Unicode code points, which differ from JavaScript string indices for characters such as emoji;
 * index `Array.from(text)` to be exact.
 *
 * @category Requests and responses
 */
export type MatchSpan = {
  /** The field that matched. For a value with no title it is `key`. */
  field: 'key' | 'title';
  /** The code point the match starts at. */
  start: number;
  /** The match's length in code points. */
  len: number;
};

/**
 * One suggested category value, with where it matched and, if asked for, a count.
 *
 * @category Requests and responses
 */
export type SuggestValue = {
  /** The value's code. Never `0`. */
  code: number;
  /** The value's key. */
  key: string;
  /** The value's display title, or `null` if none was written. */
  title: string | null;
  /** Where the query matched. */
  match: MatchSpan;
  /**
   * How many visible items hold this value, in the request's view and matching its `filters` if
   * it gave them. Present only if the request set `counts: true`.
   */
  count?: number;
};

/**
 * One page of suggestions from {@link MosaicaClient.suggest}.
 *
 * @category Requests and responses
 */
export type SuggestPage = {
  /** The column, as the request spelt it. */
  column: string;
  /** The query as the request sent it, so you can match a page to its request. */
  q: string;
  /** The matching values the viewer can see, ordered by the matched text. */
  values: SuggestValue[];
  /**
   * `true` if there may be more matches: the page filled, or the server examined
   * `selection.maxSuggestionWalk` values. There is no next page; a longer query narrows the list.
   */
  more: boolean;
  /**
   * How many items the counts are taken over, so a value's share is `count / total`. Present only
   * if the request set `counts: true`.
   */
  total?: number;
  /** Whether the counts are exact, if the request's `filters` had a `region` and it asked for counts. */
  region?: RegionVerdict;
};

/**
 * What {@link MosaicaClient.suggest} returns: `status: 'ok'` with the page, or `status: 'shed'` if
 * the server was too busy (`429`). A shed request may be sent again after `retryAfterS` seconds,
 * and `detail` gives the server's reason. A viewer can have one suggestion request running at a
 * time, so the usual cause is that the previous one has not finished.
 *
 * @category Requests and responses
 */
export type SuggestResult =
  | ({status: 'ok'} & SuggestPage)
  | {status: 'shed'; retryAfterS: number; detail: string | null};

/**
 * The request {@link MosaicaClient.viewport} sends: the points and counts for an area of the map.
 * Give exactly one of `bbox` and `tiles`. A field you leave unset takes the server's default.
 *
 * @category Requests and responses
 */
export type ViewportRequest = {
  /** A view id from {@link Meta.views}. Any other view is a `404`. */
  view: string;
  /** The tile depth to answer at, `0` to `16`. */
  zoom: number;
  /**
   * The area, as `[x0, y0, x1, y1]` in the view's data coordinates with `x0 <= x1` and
   * `y0 <= y1`. The response covers every tile at depth `zoom` that the box touches.
   */
  bbox?: [number, number, number, number];
  /**
   * The tiles at depth `zoom` to answer, in place of `bbox`, as Morton prefixes (the tile's
   * position code). They are answered in the order given, without duplicates. Leave out the tiles
   * you already hold.
   */
  tiles?: bigint[];
  /**
   * The most points each tile serves. Defaults to `selection.kMaxMarks`, and is capped at
   * `selection.maxK` and `selection.kMaxMarks`. `0` asks for counts only. Keep it the same as the
   * map zooms in, or points already drawn drop out.
   */
  k?: number;
  /**
   * Also return exact visible counts for the cells `underlayOffset` levels below `zoom`, as
   * {@link ViewportResult.subCells}, to draw a density underlay. `0` or unset returns none. Above
   * `selection.maxUnderlayOffset`, past depth 16, or over the deployment's cell limit, the request
   * is a `422`.
   */
  underlayOffset?: number;
  /**
   * The {@link ViewportResponse.pin} of the response on screen, so the server can report whether
   * the data has changed since ({@link ViewportResponse.stale}).
   */
  stamp?: string | null;
  /**
   * The filter, or `null` for none. It narrows which points are served and each tile's `matched`
   * count. It does not change `visible` or the response's `identityKey`, so drop the points you
   * hold yourself when the filter changes.
   */
  filters?: FilterExpr | null;
  /**
   * An expression to highlight, or `null` for none. It does not change which points are served. It
   * adds a `highlighted` count to each tile and a flag to each point, for the items that match both
   * `filters` and `highlight`. Combine several highlights with `any_of` or `all_of`.
   */
  highlight?: FilterExpr | null;
  /**
   * Which columns each point carries. `'full'`, the default, is every `render` column. A list
   * names the `render` columns to carry, and `[]` gives the position alone; any other name is a
   * `422`. `'highlight'` gives only each point's `mosaica_id` and highlight flag, for when only the
   * highlight has changed; once {@link ViewportResponse.stale} is `true`, ask for the columns again.
   * Without a `highlight`, `'highlight'` is answered as `'full'`. The same points are served
   * whichever you choose.
   */
  pointRows?: 'full' | 'highlight' | readonly string[];
  /**
   * The annotation layers to tag points with: each adds a column saying which of its artifacts
   * holds each point ({@link ViewportResult.membership}). Unset or `[]` is none and `'all'` is
   * every layer the viewer can reach. Unknown names are ignored. The artifacts themselves come from
   * {@link MosaicaClient.viewportArtifacts}.
   */
  layers?: string[] | 'all';
  /**
   * Which levels of each named layer tag the points. Unset uses the levels whose `zoom` range
   * covers the request's `zoom`, or every level if none declares a range. `'all'` is every level,
   * `[]` is none, and a level a layer does not have is ignored.
   */
  levels?: number[] | 'all';
  /**
   * The most artifacts a `nested` or `dag` layer tags points with. The server meets it by using
   * ancestors in place of their descendants. Unset means no limit; other layers ignore it.
   */
  artifactBudget?: number;
};

/**
 * The request {@link MosaicaClient.viewportArtifacts} sends: the artifacts of the named layers in
 * each tile of an area. Give exactly one of `bbox` and `tiles`. A field you leave unset takes the
 * server's default.
 *
 * @category Requests and responses
 */
export type ViewportArtifactsRequest = {
  /** A view id from {@link Meta.views}. Any other view is a `404`. */
  view: string;
  /** The tile depth to answer at, `0` to `16`. */
  zoom: number;
  /** The area, as for {@link ViewportRequest.bbox}. */
  bbox?: [number, number, number, number];
  /** The tiles to answer, in place of `bbox`, as for {@link ViewportRequest.tiles}. Each gets one frame. */
  tiles?: bigint[];
  /** The layers to answer for, as for {@link ViewportRequest.layers}. Unset or `[]` is none. */
  layers?: string[] | 'all';
  /** Which levels of each layer to answer for, as for {@link ViewportRequest.levels}. */
  levels?: number[] | 'all';
  /**
   * The most artifacts one level serves in one tile, largest first. Required, and at most
   * `selection.maxArtifactsPerTile`; more is a `422`.
   */
  perTile: number;
  /**
   * Which geometry each artifact carries: any of `'centroid'` and `'box'`. Unset gives what the
   * layer declares, and `[]` gives none. A geometry the layer does not declare is left out. Read
   * an artifact's outline with
   * {@link MosaicaClient.artifact}.
   */
  computed?: ComputedProperty[];
  /** A filter, answered as each artifact's {@link Artifact.matched}. It changes nothing else. */
  filters?: FilterExpr | null;
  /** An expression to highlight, answered as each artifact's {@link Artifact.highlighted}. */
  highlight?: FilterExpr | null;
  /** As for {@link ViewportRequest.stamp}. */
  stamp?: string | null;
  /**
   * The most artifacts to serve from the `nested` and `dag` layers named, using ancestors in place
   * of their descendants. Unset means no limit.
   */
  budget?: number;
  /**
   * The number of colours in your palette, from 2 to 32. Each artifact then carries a
   * {@link Artifact.slot}. Outside that range the request is a `422`.
   */
  paletteSize?: number;
};

/**
 * A geometry an artifacts request can ask for. Read an artifact's outline with
 * {@link MosaicaClient.artifact}.
 *
 * @category Meta
 */
export type ComputedProperty = 'centroid' | 'box';

/**
 * The kind of outline a layer draws: `derived`, the hull of the members the viewer can see;
 * `predicate`, the shape that decides a spatial layer's membership; `authored`, a shape the
 * publisher supplied. See {@link Layer.shape}.
 *
 * @category Meta
 */
export type ShapeKind = 'derived' | 'predicate' | 'authored';

/**
 * An artifact's outline: a list of parts, each a list of rings, each a list of `[x, y]` vertices
 * in 32-bit grid units. A part's first ring is its outer boundary and the others are holes. A
 * derived hull has one part per separate group of visible members, and no holes.
 *
 * The outline is simplified for the zoom it was requested at and may be off by up to a cell. To
 * know whether a point is a member, use {@link ViewportResult.membership}.
 *
 * @category Requests and responses
 */
export type Shape = [number, number][][][];

/**
 * One tile's exact counts of the items the viewer can see. The `served` points are a sample of the
 * `matched` ones, so show `served` alongside `matched` or `visible`.
 *
 * @category Requests and responses
 */
export type TileCounts = {
  /** The tile's Morton prefix at the request's `zoom`. */
  tile: bigint;
  /** How many items in the tile the viewer can see. Filters do not change it. */
  visible: bigint;
  /** How many of `visible` match the request's filter; equal to `visible` with no filter. */
  matched: bigint;
  /** How many points the response carries for the tile. */
  served: bigint;
  /**
   * How many of `matched` also match the request's `highlight`, whether served or not. Equal to
   * `matched` with no highlight.
   */
  highlighted: bigint;
};

/**
 * One cell of the density underlay ({@link ViewportRequest.underlayOffset}).
 *
 * @category Requests and responses
 */
export type SubCell = {
  /** The cell's Morton prefix at depth `zoom + underlayOffset`. */
  cell: bigint;
  /** How many items in the cell the viewer can see. Never `0`: empty cells are left out. */
  count: bigint;
};

/**
 * One annotation artifact, such as a cluster, a boundary or a topic label, as
 * {@link MosaicaClient.viewportArtifacts} serves it.
 *
 * Counts and geometry cover only the members the viewer can see, so two viewers can get different
 * figures for the same `mosaicaId`. They cover members outside the current view too, so they do
 * not change as the map moves. An artifact the viewer may not see is left out of the response.
 *
 * @category Requests and responses
 */
export type Artifact = {
  /** The artifact's layer. */
  layer: string;
  /** The artifact's `mosaica_id`, the same for every viewer. */
  mosaicaId: bigint;
  /** The publisher's key for the artifact, or `null` if it has none. */
  key: string | null;
  /** How many of the artifact's members the viewer can see. */
  maskedCount: bigint;
  /**
   * The centre of the visible members, in 32-bit grid units ({@link gridToWorldXY} converts it).
   * `null` if the layer does not declare it or the request's `computed` left it out.
   */
  centroid: [number, number] | null;
  /** The bounding box of the visible members, `[minX, minY, maxX, maxY]` in 32-bit grid units; `null` as for `centroid`. */
  box: [number, number, number, number] | null;
  /**
   * The publisher's content, such as label text, one entry per type in the layer's
   * {@link Layer.suppliedContent}. If the artifact has several descriptions for different access
   * levels, this is the one the viewer qualifies for.
   */
  content: string[];
  /**
   * The artifact's parents in the same frame, in ascending order. A `dag` layer may give several,
   * and the first is always the same one. Empty for a root, for a flat layer, and when the parent is not served
   * to the viewer; treat all three as a root.
   */
  parentIds: bigint[];
  /**
   * The level to draw the artifact at: its declared level on a `stacked` or `tiered` layer, its
   * depth in the tree on a `nested` or `dag` layer (a root is `0`), and `0` on a flat layer. Use
   * this rather than counting `parentIds`.
   */
  rung: number;
  /**
   * Whether any visible member in the frame's tiles matches the request's filter, or `null` with
   * no filter. The filter changes nothing else about the artifact. The points you hold are a
   * sample, so do not work this out from them.
   */
  matched: boolean | null;
  /**
   * Whether any visible member in the frame's tiles matches both the filter and the highlight, or
   * `null` with no highlight.
   */
  highlighted: boolean | null;
  /**
   * The `mosaicaId` of the artifact this one attaches to, as a topic label attaches to its
   * cluster, or `null`. An attached artifact is served only with its target. It takes `matched`
   * and `highlighted` from its target. Its `maskedCount` counts its own members, or its target's
   * if it has none, so it is not the number to show beside a label.
   */
  target: bigint | null;
  /**
   * The artifact's colour index, below the request's `paletteSize`, or `null` without one. Slots
   * are chosen so that neighbouring artifacts differ, and stay the same at any zoom, area, budget
   * or filter.
   */
  slot: number | null;
};

/**
 * A decoded viewport response: per-tile counts and the served points. The per-point arrays are
 * all in the order of `ids`.
 *
 * @category Requests and responses
 */
export type ViewportResult = {
  /** One entry per tile with anything visible to the viewer, in the response's tile order. */
  tiles: TileCounts[];
  /** Each point's `mosaica_id`. */
  ids: BigUint64Array;
  /**
   * Each point's 64-bit Morton code, its two 32-bit grid coordinates interleaved. Shifting right by
   * `64 - 2z` gives the tile at depth `z` that holds it.
   */
  codes: BigUint64Array;
  /** Interleaved x, y per point in cell space, `[0, 65536)` per axis, with a fraction for the position inside the cell. */
  positions: Float64Array;
  /**
   * The same positions in world space, `[0, 512)` per axis ({@link WORLD_SIZE}): cell space divided
   * by 128. This is the space the renderer draws in.
   */
  world: Float32Array;
  /** The `render` columns the request asked for in `pointRows`, by column name. */
  scalars: Record<string, ScalarColumn>;
  /**
   * One column per layer the request named in `layers` that tags any point, by layer name. Each
   * gives, per point, the deepest artifact of the layer that holds it; see {@link MembershipColumn}.
   * {@link MosaicaClient.artifacts} with `ids` reads those artifacts.
   */
  membership: Record<string, MembershipColumn>;
  /** One byte per point: `1` if it matches the request's `highlight`, otherwise `0`. `null` with no highlight. */
  highlighted: Uint8Array | null;
  /**
   * `'highlight'` if the points came with only `ids` and `highlighted` (asked for with
   * `pointRows: 'highlight'` and a `highlight`); you then match the flags to the points you hold by
   * `mosaica_id`. Otherwise `'full'`.
   */
  pointsProjection: 'full' | 'highlight';
  /** The density underlay's cells if the request set `underlayOffset`; otherwise `null`. */
  subCells: SubCell[] | null;
};

/**
 * One layer's membership column. See {@link ViewportResult.membership}.
 *
 * @category Requests and responses
 */
export type MembershipColumn = {
  /**
   * Per point, `0` if no artifact holds it, otherwise `1 + d`, where `ids[d]` is the artifact.
   * A `Uint16Array` for at most 65,535 points.
   */
  index: Uint16Array | Uint32Array;
  /** The artifacts' `mosaica_id`s, in the order first seen. */
  ids: BigUint64Array;
};

/**
 * One `render` column's values for a response's points, in point order. The values are a typed
 * array you can pass to a GPU attribute without copying. Whether the column is a category is on
 * {@link DeclaredScalar.category}.
 *
 * @category Requests and responses
 */
export type ScalarColumn = ScalarValues & {
  /**
   * Per point, `1` if it has a value and `0` if not, in which case `values[i]` is a meaningless
   * zero. Missing or `null` if every point has a value. A category uses code `0` for no value.
   */
  present?: Uint8Array | null;
};

/**
 * A column's storage type and its values. 64-bit integers and `timestamp_us` (microseconds since
 * the Unix epoch) are `bigint` arrays.
 *
 * @category Requests and responses
 */
export type ScalarValues =
  | {arrowType: 'bool'; values: boolean[]}
  | {arrowType: 'utf8'; values: string[]}
  | {arrowType: 'u8'; values: Uint8Array}
  | {arrowType: 'u16'; values: Uint16Array}
  | {arrowType: 'u32'; values: Uint32Array}
  | {arrowType: 'u64'; values: BigUint64Array}
  | {arrowType: 'i8'; values: Int8Array}
  | {arrowType: 'i16'; values: Int16Array}
  | {arrowType: 'i32'; values: Int32Array}
  | {arrowType: 'i64'; values: BigInt64Array}
  | {arrowType: 'f32'; values: Float32Array}
  | {arrowType: 'f64'; values: Float64Array}
  | {arrowType: 'timestamp_us'; values: BigInt64Array};

/**
 * The server's timings for one response.
 *
 * @category Requests and responses
 */
export type Timings = {
  /**
   * Microseconds from admission to the server's first write (`x-mosaica-server-us`), or `0` if the
   * header is missing. It does not cover the rest of the stream.
   */
  serverUs: number;
  /** Microseconds the request waited to be admitted (`x-mosaica-admission-us`), or `0` if the header is missing. */
  admissionUs: number;
  /**
   * Per-stage timings in nanoseconds, or `null`. The server sends them only if it was built with
   * the `bench-timing` feature and `serve.stage_timing` is on.
   */
  stageNs: number[] | null;
};

/**
 * Part of a viewport response, passed to a {@link PartSink} as it arrives so you can draw points
 * before the whole response has loaded. A part holds whole tiles, listed in `result.tiles`; its
 * `subCells` is `null`.
 *
 * @category Requests and responses
 */
export type ViewportPart = {
  /** The part's points, with the tiles they cover. */
  result: ViewportResult;
  /** As for {@link ViewportResponse.identityKey}. */
  identityKey: string;
  /** As for {@link ViewportResponse.contentKey}. */
  contentKey: string;
  /** As for {@link ViewportResponse.columnsAsked}. */
  columnsAsked?: readonly string[] | null;
  /** As for {@link ViewportResponse.layersAsked}. */
  layersAsked?: readonly string[] | 'all';
};

/**
 * A viewport response's counts, passed to a {@link CountsSink}. The server sends the counts before
 * any point, so you can show them first.
 *
 * @category Requests and responses
 */
export type ViewportCounts = {
  /** Every tile's counts, in the response's tile order. */
  tiles: TileCounts[];
  /** The density underlay, or `null` if the request asked for none. */
  subCells: SubCell[] | null;
  /** As for {@link ViewportResponse.identityKey}. */
  identityKey: string;
  /** As for {@link ViewportResponse.contentKey}. */
  contentKey: string;
};

/**
 * Receives a viewport response's counts as soon as they arrive; see {@link ViewportCounts}.
 *
 * @category Requests and responses
 */
export type CountsSink = (counts: ViewportCounts) => void;

/**
 * What {@link MosaicaClient.viewport} returns.
 *
 * @category Requests and responses
 */
export type ViewportResponse = {
  /** The decoded response. If the call was given a part sink, the points went there and this result's per-point arrays are empty. */
  result: ViewportResult;
  /** The server's timings. */
  timings: Timings;
  /**
   * Identifies the set of items the viewer may see in this view (`x-mosaica-identity-key`). It is
   * derived from the viewer's access terms, the view and the dataset, so it changes when the
   * session belongs to a viewer with different access and when the dataset is rebuilt or
   * compacted. When it changes, discard what you cached under the old key.
   */
  identityKey: string;
  /**
   * Identifies the version of the data the response was answered from: its entity tag (`etag`),
   * without quotes. It changes when items may have been added, when a deletion, suppression or
   * lift applies, and when the server restarts. You may still draw a tile cached under an older
   * content key; to refresh it, list it in {@link ViewportRequest.tiles}.
   */
  contentKey: string;
  /**
   * The `render` columns the request asked for in {@link ViewportRequest.pointRows}, or `null` if it
   * did not name any.
   */
  columnsAsked?: readonly string[] | null;
  /**
   * The layers the request named in {@link ViewportRequest.layers}. A named layer with no column in
   * {@link ViewportResult.membership} tags no point.
   */
  layersAsked?: readonly string[] | 'all';
  /**
   * The version of the data this response was computed from (`x-mosaica-pin`), or `null` if the
   * header is missing. Send it back as {@link ViewportRequest.stamp} to learn whether the data has
   * changed. Shortly after a write, it can still name the previous version, and the new items are
   * not yet visible. Deletions and suppressions apply at once.
   */
  pin: string | null;
  /**
   * Whether the data has changed since the request's `stamp`, or `false` if no stamp was sent. It
   * changes whenever the server flushes, merges or compacts data, even if nothing the viewer can
   * see changed. It is
   * advisory: nothing expires while it is `true`.
   */
  stale: boolean;
  /** Whether the request's `region` filters were answered exactly, or `null` if it had none. */
  region: RegionVerdict | null;
  /** The size of the response body, in bytes. */
  bytes: number;
};

/**
 * One frame of a {@link MosaicaClient.viewportArtifacts} response, passed to its `onTile` callback
 * as it arrives.
 *
 * @category Requests and responses
 */
export type ViewportArtifactsFrame = {
  /**
   * Whether this is the treed frame, which holds the `nested` and `dag` layers' artifacts for the
   * whole area, cut to the request's `budget`. It comes first, and is left out if it would be empty.
   */
  treed: boolean;
  /**
   * The tile this frame answers, as its Morton prefix at the request's `zoom`. `null` for the
   * treed frame, and for an empty frame of a `bbox` request.
   */
  tile: bigint | null;
  /**
   * The artifacts with a visible member in the tile, at most `perTile` per layer and level, largest
   * `maskedCount` first.
   */
  artifacts: Artifact[];
};

/**
 * What {@link MosaicaClient.viewportArtifacts} returns.
 *
 * @category Requests and responses
 */
export type ViewportArtifactsResponse = {
  /** Every frame in order: the treed frame if there is one, then one per tile. */
  frames: ViewportArtifactsFrame[];
  /** The server's timings. `stageNs` is always `null`. */
  timings: Timings;
  /** As for {@link ViewportResponse.identityKey}. */
  identityKey: string;
  /** As for {@link ViewportResponse.contentKey}. */
  contentKey: string;
  /** As for {@link ViewportResponse.pin}. */
  pin: string | null;
  /** As for {@link ViewportResponse.stale}. */
  stale: boolean;
  /** As for {@link ViewportResponse.region}. */
  region: RegionVerdict | null;
  /** The size of the response body, in bytes. */
  bytes: number;
};

/**
 * One item's record, as {@link MosaicaClient.item} returns it.
 *
 * @category Requests and responses
 */
export type ItemDetail = {
  /**
   * The item's values, by column name. Numbers are JavaScript numbers, so a 64-bit integer above
   * 2^53 loses precision. A category value is its key. A column with no value is left out.
   */
  fields: Record<string, unknown>;
  /**
   * The item's position in each view it is in that the viewer can reach, sorted by id. An item in
   * no view at all is a `404`.
   */
  views: ItemViewPosition[];
  /**
   * The item's group-scoped values, by family name and then by view key, such as
   * `{mood: {'2026-Q1': 'calm'}}`. {@link Meta.scopedScalars} gives each family's group. Views the
   * viewer cannot reach are left out, and so is a `text` family, which has no per-item value.
   */
  scoped: Record<string, Record<string, unknown>>;
  /**
   * Why the viewer can see the item: the parts of its access labels the viewer satisfies, sorted.
   * Each entry is a term the viewer holds, `public`, or a conjunction the viewer satisfies, written
   * as terms joined by `&` (such as `staff&uk`). It does not give the item's full labels. It can be
   * empty.
   */
  labels: string[];
};

/**
 * Where one view places an item. See {@link ItemDetail.views}.
 *
 * @category Requests and responses
 */
export type ItemViewPosition = {
  /** The view's id. */
  id: string;
  /**
   * The item's x in that view's 32-bit grid units ({@link ViewInfo.quantisation}). Each view has
   * its own extent, so the same item has a different position in each.
   */
  x: number;
  /** The item's y, in the same units as `x`. */
  y: number;
};

/**
 * One artifact, as {@link MosaicaClient.artifact} returns it. Its count and geometry match what
 * the viewport serves for it in the same view. It does not list the members.
 *
 * @category Requests and responses
 */
export type ArtifactDetail = {
  /** The artifact's layer. */
  layer: string;
  /** The publisher's key for the artifact, or `null` if it has none. */
  key: string | null;
  /** How many of the artifact's members the viewer can see in the requested view. */
  maskedCount: bigint;
  /** The centre of the visible members, in 32-bit grid units; `null` if the layer does not declare it. */
  centroid: [number, number] | null;
  /** The bounding box of the visible members, `[minX, minY, maxX, maxY]` in 32-bit grid units; `null` if the layer does not declare it. */
  box: [number, number, number, number] | null;
  /**
   * The artifact's outline, or `null` if its layer draws none. A predicate or authored outline is
   * simplified for the call's `zoom`, or sent whole without one.
   */
  shape: Shape | null;
};

/**
 * The request {@link MosaicaClient.browse} sends: one page of a layer's artifacts by hierarchy,
 * whatever the map shows. It has three forms:
 *
 * - Roots, with neither `parent` nor `q`: the artifacts with no parent served to the viewer.
 * - Children, with `parent`: its children, and its own parents in {@link BrowsePage.parents}. On a
 *   `dag` layer a child appears under each of its parents.
 * - Search, with `q`: the artifacts whose key or {@link BrowseRow.name} contains `q`, ignoring
 *   case.
 *
 * Giving both `parent` and `q` is a `422`. Rows are ordered by `matchedCount` if `filters` is set,
 * otherwise by `maskedCount`, then by `mosaicaId`.
 *
 * @category Requests and responses
 */
export type BrowseRequest = {
  /** The view the counts are taken in. */
  view: string;
  /**
   * The layer to browse. A layer `/v1/meta` does not list, or one whose artifacts attach to
   * another layer's, is a `422`.
   */
  layer: string;
  /**
   * The level to browse on a `stacked` or `tiered` layer. On any other layer, or for a level the
   * layer does not have, it is a `422`.
   */
  level?: number;
  /**
   * The parent whose children to list, by `mosaica_id`. An artifact that does not exist, belongs to
   * another layer, is suppressed or is not served to the viewer gives an empty page.
   */
  parent?: bigint;
  /** Text to search for in each artifact's key or {@link BrowseRow.name}, ignoring case. */
  q?: string;
  /**
   * A filter. Each row then carries `matchedCount` and the page is ordered by it. The same rows are
   * served, including those with a `matchedCount` of zero.
   */
  filters?: FilterExpr | null;
  /** The page size. Defaults to `selection.maxBrowseRows` and is capped at it; `0` is a `422`. */
  limit?: number;
  /** The previous page's {@link BrowsePage.next}. */
  cursor?: string;
  /** As for {@link ViewportArtifactsRequest.paletteSize}: each row then carries a {@link BrowseRow.slot}. */
  paletteSize?: number;
};

/**
 * One row of a browse page.
 *
 * @category Requests and responses
 */
export type BrowseRow = {
  /** The artifact's `mosaica_id`. */
  mosaicaId: bigint;
  /** The publisher's key for the artifact, or `null` if it has none. */
  key: string | null;
  /**
   * A name to show: the artifact's first supplied text, or if it has none, the text of an artifact
   * attached to it, such as a cluster's topic label. `null` if neither has text.
   */
  name: string | null;
  /** How many of the artifact's members the viewer can see. Filters do not change it. */
  maskedCount: bigint;
  /** How many of those members match the request's filter, or `null` with no filter. */
  matchedCount: bigint | null;
  /**
   * The artifact's level: its declared level on a `stacked` or `tiered` layer, its depth in the
   * tree on a `nested` or `dag` layer, and `0` on a flat layer.
   */
  rung: number;
  /** The artifact's parents that are served to the viewer, in ascending order. */
  parentIds: bigint[];
  /** How many children of this artifact are served to the viewer. `0` for a leaf, and on a `flat` or `stacked` layer. */
  childCount: number;
  /** As for {@link Artifact.slot}: `null` if the request had no `paletteSize`. */
  slot: number | null;
};

/**
 * One page of {@link MosaicaClient.browse}.
 *
 * @category Requests and responses
 */
export type BrowsePage = {
  /** The page's rows. */
  artifacts: BrowseRow[];
  /** In the children form, the parent's own parents; otherwise `[]`. */
  parents: BrowseRow[];
  /** The cursor for the next page, or `null` if this is the last. */
  next: string | null;
};

/**
 * The request {@link MosaicaClient.items} sends: every visible item in `view` that matches
 * `filters`, as pages of Arrow rows. A field you leave unset takes the server's default.
 *
 * Each page's columns are `mosaica_id` (`uint64`), then the `fields` in the order named, then
 * `mosaica:x` and `mosaica:y` with `position`, `mosaica:labels` with `labels`, and
 * `mosaica:matched` with `keepUnmatched`. A missing value is null. A category is a dictionary
 * column of its keys, with a dictionary per page, so compare keys rather than dictionary indices
 * across pages.
 *
 * @category Requests and responses
 */
export type ItemsRequest = {
  /** The view to read. Items with no position in it are not returned. */
  view: string;
  /**
   * The columns to return, by name, each once. Pin a group-scoped column outside its group as
   * `<field>@<key>`. `[]` returns `mosaica_id` alone. An unknown or repeated name is a `422`.
   */
  fields: string[];
  /** Return each item's position as `mosaica:x` and `mosaica:y` (`float64`) in the view's coordinates, such as degrees on a geographic view. */
  position?: boolean;
  /** Return `mosaica:labels` (`list<utf8>`): the parts of each item's labels the viewer satisfies, as {@link ItemDetail.labels} gives them. */
  labels?: boolean;
  /** A filter. Only matching items are returned, unless `keepUnmatched` is set. */
  filters?: FilterExpr;
  /** Return every visible item, with a `mosaica:matched` column saying which match `filters`. */
  keepUnmatched?: boolean;
  /**
   * Put the {@link ItemsHead.visible} and {@link ItemsHead.matched} counts in the head. A `422`
   * with `cursor`. {@link MosaicaClient.items} sends it on the first request only.
   */
  count?: boolean;
  /**
   * `map` orders rows by their place on the map. `stored` orders them as the record store holds
   * them, which is faster for columns held only there. Unset keeps the cursor's order, or lets the
   * server choose. An order different from the cursor's is a `422`.
   */
  order?: 'map' | 'stored';
  /** Rows per page, at most `meta.selection.maxPageRows`, which is the default. `0` is a `422`. */
  pageRows?: number;
  /** The most pages one response may carry. Unset carries as many as the server's limits allow; `0` is a `422`. */
  pages?: number;
  /** A previous response's cursor, to continue the read. */
  cursor?: string;
  /** Compress each page with zstd. The client decodes either form. */
  compression?: 'zstd';
};

/**
 * The request {@link MosaicaClient.artifacts} sends: every artifact of `layer` served to the
 * viewer, as pages of Arrow rows ordered by level and then by publication order. A field you leave
 * unset takes the server's default.
 *
 * Each page's columns are `mosaica_id` (`uint64`), then the `fields` in the order named, then
 * `matched_count` (`uint64`) if the request has `filters`.
 *
 * @category Requests and responses
 */
export type ArtifactsRequest = {
  /** The view the counts and geometry are taken in. */
  view: string;
  /** A layer `/v1/meta` lists. Any other is a `422`. */
  layer: string;
  /**
   * The properties to return, each once. `centroid` gives the columns `centroid_x` and
   * `centroid_y`, and `box` gives `box_x_min`, `box_y_min`, `box_x_max` and `box_y_max`, in the
   * view's coordinates. `shape` is a WKB `MultiPolygon`. `slot` needs
   * {@link ArtifactsRequest.paletteSize}. A repeated property is a `422`.
   */
  fields: ('key' | 'level' | 'parents' | 'target' | 'masked_count' | 'content' | 'centroid' | 'box' | 'shape' | 'slot')[];
  /** Only the artifacts at this level. A `422` on a layer with one level, or past its last level. */
  level?: number;
  /** Only the children of this artifact. A `422` with `q`. */
  parent?: bigint;
  /** Only the artifacts whose key or first text contains this, ignoring case. A `422` with `parent`. */
  q?: string;
  /** Only these artifacts, such as the ones that tag a point. An id not served to the viewer gives no row. */
  ids?: bigint[];
  /** Only the artifacts with a visible member that matches, and a `matched_count` column. */
  filters?: FilterExpr;
  /** With `filters`, return every artifact, with a `matched_count` of zero for those with no match. */
  keepUnmatched?: boolean;
  /**
   * Put the {@link ArtifactsHead.served} and {@link ArtifactsHead.matched} counts in the head. A
   * `422` with `cursor`. {@link MosaicaClient.artifacts} sends it on the first request only.
   */
  count?: boolean;
  /** As for {@link ItemsRequest.pageRows}. */
  pageRows?: number;
  /** As for {@link ItemsRequest.pages}. */
  pages?: number;
  /** As for {@link ItemsRequest.cursor}. */
  cursor?: string;
  /** As for {@link ItemsRequest.compression}. */
  compression?: 'zstd';
  /** As for {@link ViewportArtifactsRequest.paletteSize}. Without it the `slot` column is null. */
  paletteSize?: number;
};

/**
 * The head of a {@link MosaicaClient.items} response, as {@link RecordsRead.head}.
 *
 * @category Requests and responses
 */
export type ItemsHead = {
  /** The order of the rows. */
  order: 'map' | 'stored';
  /** The page size used. */
  pageRows: number;
  /** How many items in the view the viewer can see, or `null` unless the request set `count`. */
  visible: number | null;
  /** How many of those match the filter, or `null` unless the request set `count`. */
  matched: number | null;
};

/**
 * The head of a {@link MosaicaClient.artifacts} response, as {@link RecordsRead.head}.
 *
 * @category Requests and responses
 */
export type ArtifactsHead = {
  /** The page size used. */
  pageRows: number;
  /** How many artifacts the request selects, or `null` unless the request set `count`. */
  served: number | null;
  /** How many of those have a visible member that matches the filter, or `null` unless the request set `count`. */
  matched: number | null;
};

/**
 * Sent after each page of an items or artifacts read.
 *
 * @category Requests and responses
 */
export type PageEnd = {
  /** The cursor to continue after this page, or `null` if no rows remain. */
  next: string | null;
  /**
   * Why the page ended. `rows`: it holds `pageRows` rows. `bytes`: the next row would have taken
   * it past `meta.selection.maxPageBytes`. `time`: the response ran out of time and ends after this
   * page. `end`: no rows remain.
   */
  endedBy: 'rows' | 'bytes' | 'time' | 'end';
};

/**
 * Sent at the end of an items or artifacts response.
 *
 * @category Requests and responses
 */
export type RecordsTrailer = {
  /** How many pages the response carried, including an empty one. */
  pages: number;
  /** How many rows the response carried. */
  rows: number;
  /** The cursor to continue from, or `null` if no rows remain. */
  next: string | null;
  /**
   * Why the response ended. `end`: no rows remain. `pages`: it carried the request's `pages`.
   * `budget_bytes` and `budget_time`: it reached the server's size or time limit. `deadline`: the
   * stream timed out.
   */
  endedBy: 'end' | 'pages' | 'budget_bytes' | 'budget_time' | 'deadline';
  /** How long the response took on the server, in microseconds, including time spent waiting for the client. */
  streamUs: number;
};

/**
 * The request {@link MosaicaClient.aggregate} sends: summary tables over the visible items in
 * `view`, such as counts per category, a histogram, or counts per map cell. Each
 * {@link Grouping} gives one table. A field you leave unset takes the server's default.
 *
 * @category Requests and responses
 */
export type AggregateRequest = {
  /** The view to count in. Items with no position in it are not counted. */
  view: string;
  /** The items to count. Unset counts every visible item in `view`. */
  filters?: FilterExpr;
  /**
   * A second set to compare each count with, from the same visible items. `{}` is every visible
   * item. With it, each table with `count` also gains `reference_count` and `lift` columns.
   */
  reference?: FilterExpr;
  /** The tables to compute, at most `meta.selection.maxAggregateGroupings`. */
  groupings: Grouping[];
  /** As for {@link ItemsRequest.pageRows}. */
  pageRows?: number;
  /** As for {@link ItemsRequest.pages}. */
  pages?: number;
  /** A previous result's {@link AggregateResult.next}, to continue the same request. */
  cursor?: string;
  /** As for {@link ItemsRequest.compression}. */
  compression?: 'zstd';
};

/**
 * One table of an aggregate: how to divide the items into rows, and which figures each row holds.
 * `by` divides them into groups and `cells` into map cells; with both, each group is divided into
 * cells. Name at least one figure; `{count: true}` alone counts the whole set.
 *
 * The field figures (`values`, `missing`, `min`, `max`, `mean` and `summary`) each take a list of
 * number or timestamp columns declared with `index` or `render`, and add one column per field
 * named `<figure>_<field>`. They are computed over every visible item in the view, so they cannot
 * be combined with `by`, `cells`, `filters` or `reference`, and their table has one row. All
 * figures are exact; `min`, `max` and `mean` are null if the field has no values. `values_` and
 * `missing_` columns are `bigint`; `min_` and `max_` are typed as a histogram's edges, and `mean_`
 * is a timestamp on a timestamp field and a number otherwise.
 *
 * @category Requests and responses
 */
export type Grouping = {
  /** Divide the items into groups. Unset treats them as one group. */
  by?: AggregateBy;
  /** Divide each group into map cells. Unset keeps each group whole. */
  cells?: AggregateCells;
  /** Add a `count` column: the items in each row. */
  count?: true;
  /** Add `values_<field>`: how many items hold a finite value. */
  values?: string[];
  /** Add `missing_<field>`: how many items hold no value. */
  missing?: string[];
  /** Add `min_<field>`: the smallest finite value. */
  min?: string[];
  /** Add `max_<field>`: the largest finite value. */
  max?: string[];
  /** Add `mean_<field>`: the mean of the finite values, rounded to the microsecond on a timestamp. */
  mean?: string[];
  /** Add all five figures for each field. */
  summary?: string[];
};

/**
 * How a grouping divides items into groups. There are three kinds:
 *
 * - A category column, as its `top` values by count or the `values` you name. The column needs
 *   `index` or `render`, or a `derived` vocabulary.
 * - An annotation layer, as its `top` artifacts by count or the `artifacts` you name. `level` is
 *   required on a layer with several levels and refused on one with one level. `paletteSize` (2 to
 *   32) gives each artifact its {@link Artifact.slot}. On a `nested` or `dag` layer, `top` requires
 *   a `cut`, and lists the artifacts {@link MosaicaClient.viewportArtifacts} would draw for it. A
 *   `cut` on any other layer is a `422`.
 * - A number or timestamp column in `bins`, as a histogram. The column needs `index` or `render`.
 *
 * Pin a group-scoped column outside its group as `<field>@<key>`. A named value or artifact the
 * viewer cannot see gets no row.
 *
 * For a histogram, `range` cuts `[lower, upper]` into `bins` equal bins; bounds may be numbers,
 * `bigint`s, or for a timestamp, dates or date-times as in a filter. Without a `range`, the server
 * picks round edges that cover every visible item in the view, in at most `bins` bins, so they stay
 * the same as the filters and the map change. With `sample`, a set more than about 64 times larger
 * than `sample` is counted from about `sample` of its items and the counts are scaled up; a smaller
 * set is counted exactly. {@link AggregateTable.sample} says which happened. A histogram cannot have `cells`, and its table is always one page.
 *
 * @category Requests and responses
 */
export type AggregateBy =
  | {field: string; top: number}
  | {field: string; values: string[]}
  | {field: string; bins: number; range?: [number | bigint | string, number | bigint | string]; sample?: number}
  | {layer: string; level?: number; top: number; cut?: AggregateCut; paletteSize?: number}
  | {layer: string; level?: number; artifacts: bigint[]; paletteSize?: number};

/**
 * The map position a `nested` or `dag` layer is drawn at, as in
 * {@link ViewportArtifactsRequest}. It covers at most `meta.maxTilesPerRequest` tiles.
 *
 * @category Requests and responses
 */
export type AggregateCut = {
  /** The tile depth, `0` to `16`. */
  zoom: number;
  /** The area, `[x0, y0, x1, y1]` in the view's data coordinates. */
  bbox: [number, number, number, number];
  /** The most artifacts to draw. Unset means no limit. */
  budget?: number;
};

/**
 * Divides each group into map cells at `depth`. Depths 0 to 16 are the map's tiles at that zoom,
 * and 17 to 32 are finer.
 *
 * @category Requests and responses
 */
export type AggregateCells = {
  /** The cell depth, `0` to `32`. */
  depth: number;
  /**
   * The area, `[x0, y0, x1, y1]` in the view's data coordinates. Only the cells it touches are
   * listed, each counted whole. Unset covers the whole view. At most
   * `meta.selection.maxAggregateCells` cells.
   */
  area?: [number, number, number, number];
};

/**
 * One table of {@link AggregateResult.tables}.
 *
 * The rows are an Arrow table whose columns depend on the grouping, in this order:
 *
 * - `group`, with `by`: `listed` for a named or top group, `rest` for the items in no listed group,
 *   and `none` for the items with no value.
 * - `key` and `title`, with `by`: the value's key and title, or the artifact's `mosaica_id` and
 *   name. `title` is present for a layer, or for a column whose vocabulary has titles. Both are
 *   null for `rest` and `none`.
 * - `slot`, with `by` on a layer: the artifact's colour index, or null without `paletteSize` and
 *   on `rest` and `none`.
 * - `lower` and `upper`, with `bins`, in place of `key` and `title`: the bin's edges. A bin holds
 *   values from `lower` up to `upper`, and the last bin includes its `upper`. An edge is a `bigint` on
 *   an integer field, a number on a float field or if a `range` bound is fractional, and a
 *   timestamp in milliseconds, as Arrow reads it, on a timestamp field.
 * - `cell`, with `cells`: the cell's Morton prefix, as a `bigint`.
 * - `count`, with `count`. With a `reference` too, `reference_count` and `lift`, which is null if
 *   either count it divides by is 0.
 * - The field figures' columns, as {@link Grouping} describes.
 *
 * @category Requests and responses
 */
export type AggregateTable = {
  /** The table's index in the request's `groupings`. */
  grouping: number;
  /** How many items are in the set. */
  total: number;
  /** How many items are in the reference set, or `null` without a `reference`. */
  referenceTotal: number | null;
  /** How many groups have an item in the set, before the cut to `top` or the named list; `null` without `by`. */
  groups: number | null;
  /** How a histogram with a `sample` was counted; otherwise `null`. */
  sample: AggregateSample | null;
  /** The rows, from every page read. */
  rows: Table;
};

/**
 * How a histogram with a `sample` was counted.
 *
 * @category Requests and responses
 */
export type AggregateSample = {
  /** Whether the counts are scaled up from a sample. `false` if every item was counted. */
  sampled: boolean;
  /** How many items of the set were counted. */
  items: number;
  /** How many items of the reference set were counted, or `null` without a `reference`. */
  referenceItems: number | null;
};

/**
 * What {@link MosaicaClient.aggregate} returns.
 *
 * @category Requests and responses
 */
export type AggregateResult = {
  /** One table per grouping, in the order of `groupings`. */
  tables: AggregateTable[];
  /** Whether the `region` filters in `filters` or `reference` were answered exactly; `null` if there were none. */
  region: RegionVerdict | null;
  /**
   * Whether the data changed between pages, so the tables mix counts from before and after the
   * change.
   */
  recomposed: boolean;
  /** The identity key for the viewer and view (see {@link ViewportResponse.identityKey}), or empty if the response had none. */
  identityKey: string;
  /** Where to continue the read, or `null` once every table is complete. */
  next: string | null;
};
