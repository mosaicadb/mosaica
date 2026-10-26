import {
  checkArtifactsTrailer,
  checkTrailerCounts,
  decodeSubCells,
  decodeTiles,
  decodeViewport,
  parseTrailer,
  stageNsOf,
  type ArtifactsFramePart,
  type PointsPart
} from './decode.js';
import {retryAfterOf} from './control.js';
import {createDecoder, type Decoder} from './decoder.js';
import {parseRegionVerdict} from './region.js';
import {FRAME_ARTIFACTS, FRAME_POINTS, FRAME_SUB_CELLS, FRAME_TILES, FRAME_TRAILER, FrameReader, type Frame} from './frame.js';
import {readAggregate, type PartialAggregate} from './aggregate.js';
import {openRecords, type RecordsRead} from './records.js';
import type {AggregateRequest, AggregateResult, ArrowType, ArtifactDetail, ArtifactsHead, ArtifactsRequest, BrowsePage, BrowseRequest, BrowseRow, CategoryValue, CountsSink, FilterExpr, FilterOperandSet, Grouping, ItemDetail, ItemsHead, ItemsRequest, Layer, Login, LoginCredential, AuthoriseTarget, MapProjection, Meta, RegionVerdict, Session, Shape, ShapeKind, SuggestResult, TileCounts, TileScheme, ViewMetadataValue, ViewportArtifactsFrame, ViewportArtifactsRequest, ViewportArtifactsResponse, ViewportCounts, ViewportPart, ViewportRequest, ViewportResponse, ViewportResult} from './types.js';

/**
 * A callback that receives a `/v1/viewport` response's points as they arrive, so a wide view can
 * be drawn before the whole body has loaded. Pass it as `onPart` to {@link MosaicaClient.viewport}.
 * Parts arrive in the order the server sent them. Each holds whole tiles: their counts and their
 * points, with `subCells` set to `null`. If the callback returns a promise, the next part waits
 * until it settles.
 *
 * @category HTTP client
 */
export type PartSink = (part: ViewportPart) => void | Promise<void>;

/** A viewport body's counts, decoded. */
type Counts = Pick<ViewportCounts, 'tiles' | 'subCells'>;

/**
 * A callback that receives a `/v1/artifacts/viewport` response's frames as they arrive, with the
 * response's identity and content keys. Pass it as `onTile` to
 * {@link MosaicaClient.viewportArtifacts}. Frames arrive in the order the server sent them. If the
 * callback returns a promise, the next frame waits until it settles.
 *
 * @category HTTP client
 */
export type TileSink = (frame: ViewportArtifactsFrame, keys: {identityKey: string; contentKey: string}) => void | Promise<void>;

/**
 * Decodes a viewport body's counts once they are whole: at the tiles frame, or at the sub-cells
 * frame the server sends straight after it where the request asked for an underlay. Returns a
 * function to pass each frame in wire order until it answers; it answers `null` before the counts
 * are whole and the counts at the frame that completes them. Any other frame after the tiles
 * completes them without sub-cells.
 */
function countsReader(underlay: boolean): (frame: Frame) => Counts | null {
  let tiles: Uint8Array | null = null;
  return (frame) => {
    if (frame.kind === FRAME_TILES) {
      tiles = frame.payload;
      return underlay ? null : {tiles: decodeTiles(tiles), subCells: null};
    }
    if (tiles === null) return null;
    return {tiles: decodeTiles(tiles), subCells: frame.kind === FRAME_SUB_CELLS ? decodeSubCells(frame.payload) : null};
  };
}

/** A watch for {@link readWatching} that hands the counts to `sink` with the response's keys. */
function countsWatch(sink: CountsSink, underlay: boolean, keys: {identityKey: string; contentKey: string}): (frame: Frame) => boolean {
  const read = countsReader(underlay);
  return (frame) => {
    const counts = read(frame);
    if (counts) sink({...counts, ...keys});
    return counts !== null;
  };
}

/**
 * Reads a whole body, passing each frame to `watch` as it completes until `watch` returns `true`.
 * The frames are checked against the viewport grammar as they pass.
 */
async function readWatching(response: Response, watch: (frame: Frame) => boolean): Promise<Uint8Array> {
  const frames = new FrameReader();
  if (!response.body) {
    const bytes = new Uint8Array(await response.arrayBuffer());
    for (const frame of frames.push(bytes)) if (watch(frame)) break;
    return bytes;
  }
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  let watching = true;
  try {
    for (;;) {
      const {done, value} = await reader.read();
      if (done) break;
      chunks.push(value);
      size += value.byteLength;
      if (!watching) continue;
      for (const frame of frames.push(value)) {
        if (watch(frame)) {
          watching = false;
          break;
        }
      }
    }
  } catch (error) {
    void reader.cancel().catch(() => {});
    throw error;
  }
  const bytes = new Uint8Array(size);
  let at = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, at);
    at += chunk.byteLength;
  }
  return bytes;
}

/** What either decode path hands back, before the headers are folded in around it. */
type Decoded = {
  result: ViewportResult;
  bytes: number;
  points: number;
  ms: number;
  workerMs: number | null;
  /** The trailer's `stage_ns`. */
  stageNs: number[] | null;
};

/**
 * Empty point columns, for a result whose points went to a part sink. Each call makes fresh
 * buffers, so no two results share a `scalars` object.
 */
function emptyPoints() {
  return {
    ids: new BigUint64Array(0),
    codes: new BigUint64Array(0),
    positions: new Float64Array(0),
    world: new Float32Array(0),
    scalars: {} as Record<string, never>,
    membership: {} as Record<string, never>,
    // No points, so no highlight column. `null` says that; an empty array would say nothing is
    // highlighted.
    highlighted: null,
    pointsProjection: 'full' as const
  };
}

/**
 * The trailer's `stage_ns` in a whole body, found by walking the frame headers to the trailer, or
 * `null` where the walk finds none. The decoder checks the body's grammar.
 */
function stageNsOfBody(bytes: Uint8Array): number[] | null {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  for (let at = 0; at + 5 <= bytes.byteLength; ) {
    const end = at + 5 + view.getUint32(at + 1, true);
    if (end > bytes.byteLength) return null;
    if (bytes[at] === FRAME_TRAILER) return stageNsOf(parseTrailer(bytes.subarray(at + 5, end)));
    at = end;
  }
  return null;
}

/** The same result with its points removed, since they went to the part sink. */
function headOnly(result: ViewportResult): ViewportResult {
  return {...result, ...emptyPoints()};
}

/**
 * The run of tiles whose served counts add up to one points frame's rows.
 *
 * The server does not split a tile across frames, so a frame's rows are some run of the tiles
 * batch, found by adding up the counts the server already sent. A frame whose rows end inside a
 * tile cannot be attributed, and is refused rather than assigned to a tile it may not belong to.
 */
function tilesFor(tiles: readonly TileCounts[], from: number, rows: number): {tiles: TileCounts[]; next: number} {
  let at = from;
  let sum = 0;
  while (at < tiles.length && sum < rows) sum += Number(tiles[at++]!.served);
  if (sum !== rows) {
    throw new Error(
      `a points frame of ${rows} rows does not end on a tile boundary (tiles ${from}..${at} serve ${sum})`
    );
  }
  return {tiles: tiles.slice(from, at), next: at};
}

/**
 * The error a request rejects with when the server refuses it. It carries the HTTP status and the
 * `error` code and `detail` from the server's JSON error body, and its `message` is
 * `"<status> <code>: <detail>"`. A request that reaches no server rejects with the `fetch` error
 * instead.
 *
 * @category HTTP client
 */
export class MosaicaError extends Error {
  constructor(
    /** The HTTP status, such as `404` or `422`. */
    readonly status: number,
    /**
     * The error code from the body: `bad-credential`, `expired-token`, `unknown`, `conflict`,
     * `contract`, `backpressure`, `fail-closed` or `not-ready`. It is `http-error` if the body is
     * not JSON or has no code, as with an error page from a proxy, and then only `status` says what
     * went wrong.
     */
    readonly code: string,
    /** What was wrong, from the body's `detail`, or the HTTP status text if the body has none. */
    readonly detail: string,
    /**
     * How many seconds to wait before sending the request again. It is the `Retry-After` header,
     * else the body's `retry_after_s`, else 1, and never less than 1. `null` if the refusal is not a
     * `429` and names no wait.
     */
    readonly retryAfterS: number | null = null
  ) {
    super(`${status} ${code}: ${detail}`);
    this.name = 'MosaicaError';
  }
}

async function fail(response: Response): Promise<never> {
  let code = 'http-error';
  let detail = response.statusText;
  let body: {error?: string; detail?: string} | null = null;
  try {
    body = (await response.json()) as {error?: string; detail?: string};
    code = body.error ?? code;
    detail = body.detail ?? detail;
  } catch {
    // A body that is not JSON, such as a proxy's, still gives a MosaicaError.
  }
  throw new MosaicaError(response.status, code, detail, retryAfterOf(response.status, response.headers.get('retry-after'), body));
}

/**
 * Options for a {@link MosaicaClient}.
 *
 * @category HTTP client
 */
export type MosaicaClientOptions = {
  /**
   * The viewer server's base URL, such as `https://maps.example.org` or `/mosaica`. A trailing
   * slash is ignored. An empty string sends requests to the page's own origin.
   */
  viewerUrl: string;
  /**
   * The session listener's base URL. Only `authorise` and `revoke` use it, and they throw if it is
   * not set.
   */
  sessionUrl?: string;
  /**
   * The credential `authorise` and `revoke` send: an API key whose principal holds `authorise-as`,
   * or the operator credential. It can start a session for any principal, so it is meant for a
   * back end or a script. A browser page can reach the session listener only from an origin listed
   * in `serve.dev_cors_origins`, a development setting. If it is not set, no credential is sent,
   * which only an open server (`serve.open`) accepts.
   */
  sessionCredential?: string;
  /**
   * Called once per `viewport` and `viewportArtifacts` response with decode timings, for
   * instrumentation. `ms` is the time from the response's headers to the decoded result, including
   * reading the body. `bytes` is the body's size and `points` the number of points decoded.
   * `workerMs` is the time a worker spent decoding, summed over the frames of a streamed response,
   * or `null` if the response was decoded on this thread.
   */
  onDecode?: (ms: number, bytes: number, points: number, workerMs: number | null) => void;
  /**
   * Where responses are decoded. By default the client creates a decoder that uses web workers if
   * it can, and this thread otherwise. You close a decoder you pass in; {@link MosaicaClient.close}
   * closes only one the client created.
   */
  decoder?: Decoder;
  /** A `fetch` to use in place of the global one. */
  fetch?: typeof fetch;
  /**
   * Headers to send on every request. A method's own `authorization` and `content-type` headers
   * take precedence over these.
   */
  headers?: Record<string, string>;
};

/**
 * A thin HTTP client for the viewer and session routes, with one method per route. It keeps no
 * state between calls. The store that {@link createStore} makes is built on it and adds caching
 * and composition. The viewer methods take a session token as their first argument, and every
 * count and record they return covers only what that session may see.
 *
 * A method rejects with a {@link MosaicaError} if the server refuses the request, and with the
 * `fetch` error if no server answers. On a viewer route, `403 expired-token` means the session has
 * ended, and so does `401 bad-credential` for a token that worked before.
 *
 * No method retries on its own. A server too busy to take a request answers `429 backpressure`,
 * and the error's `retryAfterS` says how many seconds to wait before calling again with the same
 * arguments. {@link MosaicaClient.suggest} returns this as `{status: 'shed', retryAfterS}` instead
 * of rejecting. The store and {@link Control} retry for you. Here is a helper that makes five
 * attempts:
 *
 * ```ts
 * async function retrying<T>(call: () => Promise<T>, attempts = 5): Promise<T> {
 *   for (let attempt = 1; ; attempt++) {
 *     try {
 *       return await call();
 *     } catch (error) {
 *       if (!(error instanceof MosaicaError) || error.status !== 429 || attempt === attempts) throw error;
 *       await new Promise((resolve) => setTimeout(resolve, (error.retryAfterS ?? 1) * 1000));
 *     }
 *   }
 * }
 *
 * const read = await retrying(() => client.items(token, {view: 'papers', fields: ['title', 'year']}));
 * ```
 *
 * @category HTTP client
 */
export class MosaicaClient {
  /**
   * Where responses are turned into typed arrays.
   *
   * Created lazily and shared across requests. In a browser this is a worker, so decode does not
   * compete with drawing; elsewhere it decodes on this thread.
   */
  private decoder: Decoder | null = null;

  private readonly opts: MosaicaClientOptions;

  constructor(opts: MosaicaClientOptions) {
    this.opts = {...opts, viewerUrl: opts.viewerUrl.replace(/\/+$/, ''), sessionUrl: opts.sessionUrl?.replace(/\/+$/, '')};
  }

  /** The session listener's base URL, or a refusal naming the option where it is unset. */
  private sessionBase(method: string): string {
    const url = this.opts.sessionUrl;
    if (url === undefined) throw new Error(`${method} needs the session listener's base URL: pass \`sessionUrl\` to the MosaicaClient.`);
    return url;
  }

  /**
   * One request, through the host's `fetch`, with the host's headers and then the route's own,
   * which replace a host header of the same name in any case.
   */
  private send(url: string, init: Omit<RequestInit, 'headers'> & {headers?: Record<string, string>} = {}): Promise<Response> {
    const headers = new Headers(this.opts.headers);
    for (const [name, value] of Object.entries(init.headers ?? {})) headers.set(name, value);
    return (this.opts.fetch ?? fetch)(url, {...init, headers});
  }

  /**
   * Close the decoder the client created and stop its workers. A decoder passed in as `decoder` is
   * left open.
   */
  close(): void {
    if (!this.opts.decoder) this.decoder?.close();
    this.decoder = null;
  }

  /**
   * `POST /v1/login`: log in and start a session. The principal the credential identifies must hold
   * `read`. The credential is sent in the body, with no bearer token.
   *
   * @param credential - One credential: a local principal's name and password, an API key, or an
   *   OIDC access token.
   * @returns The session's `token`, for the viewer methods and `logout`, and `expiresAt`, in seconds
   *   since the Unix epoch.
   * @throws {@link MosaicaError} if the server refuses: `401` for any credential it does not accept,
   *   and `403` if the principal does not hold `read`.
   */
  async login(credential: LoginCredential, signal?: AbortSignal): Promise<Login> {
    const body =
      'password' in credential
        ? {password: {principal: credential.principal, password: credential.password}}
        : 'apiKey' in credential
          ? {api_key: credential.apiKey}
          : {access_token: credential.accessToken};
    const response = await this.send(`${this.opts.viewerUrl}/v1/login`, {
      method: 'POST',
      headers: {'content-type': 'application/json'},
      body: JSON.stringify(body),
      signal
    });
    if (!response.ok) await fail(response);
    const answer = (await response.json()) as {token: string; expires_at: number};
    return {token: answer.token, expiresAt: answer.expires_at};
  }

  /**
   * `POST /v1/logout`: end the session of `token`.
   *
   * @throws {@link MosaicaError} with `401` or `403` if the session has already ended.
   */
  async logout(token: string, signal?: AbortSignal): Promise<void> {
    const response = await this.send(`${this.opts.viewerUrl}/v1/logout`, {
      method: 'POST',
      // A write with no content type is refused by an open server, so the empty body is named.
      headers: {...bearer(token), 'content-type': 'application/json'},
      signal
    });
    if (!response.ok) await fail(response);
  }

  /**
   * `POST /session/authorise`: start a session on someone's behalf, sending the `sessionCredential`
   * option. A session for a principal or an OIDC identity carries its terms and its `read` and
   * `write` permissions, and the target must hold `read`. A session for `terms` holds those terms
   * and `read`. A term is trimmed, and a term that is empty, holds a control character or is
   * `public` is dropped. Only the operator credential may ask for `terms` or `readAll`.
   *
   * @param target - The local principal to act as, the access token of the OIDC identity to act as,
   *   the terms the session holds, or `readAll` for a session that sees every item.
   * @returns The session's `token`, for the viewer methods, its `tokenId`, for `revoke`, and
   *   `expiresAt`, in seconds since the Unix epoch.
   * @throws {@link MosaicaError} if the server refuses: `401` for a credential it does not accept;
   *   `403` if the key's principal does not hold `authorise-as`, the target does not hold `read`, or
   *   a key asks for `terms`; `404` if `principal` names no enabled principal; `422` for an access
   *   token it does not accept.
   * @throws `Error` if the client has no `sessionUrl`.
   */
  async authorise(target: AuthoriseTarget, signal?: AbortSignal): Promise<Session> {
    const credential = this.opts.sessionCredential;
    const body =
      'principal' in target
        ? {principal: target.principal}
        : 'terms' in target
          ? {terms: target.terms}
          : 'readAll' in target
            ? {read_all: true}
            : {access_token: target.accessToken};
    const response = await this.send(`${this.sessionBase('authorise')}/session/authorise`, {
      method: 'POST',
      headers: {...bearer(credential), 'content-type': 'application/json'},
      body: JSON.stringify(body),
      signal
    });
    if (!response.ok) await fail(response);
    const answer = (await response.json()) as {token: string; token_id: number; expires_at: number};
    return {token: answer.token, tokenId: answer.token_id, expiresAt: answer.expires_at};
  }

  /**
   * `POST /session/revoke`: end the session `tokenId` names. An API key can end a session that a
   * key of its own principal started, and the operator credential can end any session. An id that
   * names no live session is not an error.
   *
   * @throws {@link MosaicaError} if the server refuses: `401` for a credential it does not accept,
   *   `403` if the key's principal does not hold `authorise-as`, and `422` for a malformed request.
   * @throws `Error` if the client has no `sessionUrl`.
   */
  async revoke(tokenId: number, signal?: AbortSignal): Promise<void> {
    const credential = this.opts.sessionCredential;
    const response = await this.send(`${this.sessionBase('revoke')}/session/revoke`, {
      method: 'POST',
      headers: {...bearer(credential), 'content-type': 'application/json'},
      body: JSON.stringify({token_id: tokenId}),
      signal
    });
    if (!response.ok) await fail(response);
  }


  /**
   * `GET /v1/basemaps/world`: the world outline drawn under a geographic view, with land,
   * coastlines and country borders, in the server's `MBW1` encoding.
   *
   * @throws {@link MosaicaError} if the server refuses.
   */
  async basemapWorld(token: string, signal?: AbortSignal): Promise<ArrayBuffer> {
    const response = await this.send(`${this.opts.viewerUrl}/v1/basemaps/world`, {headers: bearer(token), signal});
    if (!response.ok) await fail(response);
    return response.arrayBuffer();
  }

  /**
   * `GET /v1/meta`: the deployment's metadata. It lists the views, view groups, columns, filter
   * operands and request limits, and the layers this session may know of. The layers depend on
   * the viewer, so a result applies only to the token it was fetched with.
   *
   * @throws {@link MosaicaError} if the server refuses.
   * @throws `Error` if the response lacks a field this client requires, which means the server and
   *   the client are from different versions.
   */
  async meta(token: string, signal?: AbortSignal): Promise<Meta> {
    const response = await this.send(`${this.opts.viewerUrl}/v1/meta`, {
      headers: bearer(token),
      signal
    });
    if (!response.ok) await fail(response);
    const m = (await response.json()) as RawMeta;
    requireFields(m, META_FIELDS, '/v1/meta');
    requireFields(m.selection, SELECTION_FIELDS, "/v1/meta's selection");
    for (const v of m.views) requireFields(v, VIEW_FIELDS, 'a view in /v1/meta');
    for (const g of m.groups) requireFields(g, GROUP_FIELDS, 'a group in /v1/meta');
    for (const c of m.declared_scalars) requireFields(c, DECLARED_FIELDS, 'a declared scalar in /v1/meta');
    for (const c of m.scoped_scalars) requireFields(c, SCOPED_FIELDS, 'a scoped scalar in /v1/meta');
    for (const f of m.filter_operands) requireFields(f, OPERAND_FIELDS, 'a filter operand in /v1/meta');
    for (const l of m.layers) {
      requireFields(l, LAYER_FIELDS, 'a layer in /v1/meta');
      for (const v of l.levels) requireFields(v, LEVEL_FIELDS, `a level of layer ${l.name} in /v1/meta`);
    }
    const category = (c: RawCategory | null) => (c ? {vocabulary: c.vocabulary, kind: c.kind, visibility: c.visibility} : null);
    return {
      apiVersion: m.api_version,
      bundleFormat: m.bundle_format,
      views: m.views.map((s) => ({
        id: s.id,
        displayName: s.display_name,
        quantisation: {
          xMin: s.quantisation.x_min,
          xMax: s.quantisation.x_max,
          yMin: s.quantisation.y_min,
          yMax: s.quantisation.y_max
        },
        projection: s.projection,
        worldAspect: s.world_aspect,
        tileScheme: s.tile_scheme,
        tile: s.tile,
        // The three roster fields are null together on a plain view, so `group` decides the case.
        roster: s.group === null ? null : {group: s.group, key: s.key!, metadata: s.metadata!}
      })),
      groups: m.groups.map((g) => ({
        name: g.name,
        title: g.title,
        membersOf: g.members_of,
        views: g.views
      })),
      declaredScalars: m.declared_scalars.map((s) => ({
        name: s.name,
        arrowType: s.arrow_type,
        category: category(s.category),
        render: s.render,
        index: s.index,
        unique: s.unique,
        analyser: s.analyser,
        homes: s.homes
      })),
      scopedScalars: m.scoped_scalars.map((s) => ({
        name: s.name,
        arrowType: s.arrow_type,
        scope: {group: s.scope.group},
        category: category(s.category),
        analyser: s.analyser,
        render: s.render,
        index: s.index,
        views: s.views
      })),
      selection: {
        kMin: m.selection.k_min,
        kMaxMarks: m.selection.k_max_marks,
        maxK: m.selection.max_k,
        thetaTargetMarks: m.selection.theta_target_marks,
        maxUnderlayOffset: m.selection.max_underlay_offset,
        maxArtifactsPerTile: m.selection.max_artifacts_per_tile,
        maxCategoryValues: m.selection.max_category_values,
        maxRegionVertices: m.selection.max_region_vertices,
        maxRegionCells: m.selection.max_region_cells,
        maxBrowseRows: m.selection.max_browse_rows,
        maxShapeVertices: m.selection.max_shape_vertices,
        maxSuggestions: m.selection.max_suggestions,
        maxSuggestionWalk: m.selection.max_suggestion_walk,
        maxSuggestSetEntities: m.selection.max_suggest_set_entities,
        maxPageRows: m.selection.max_page_rows,
        maxPageBytes: m.selection.max_page_bytes,
        maxAggregateGroupings: m.selection.max_aggregate_groupings,
        maxAggregateTop: m.selection.max_aggregate_top,
        maxAggregateNamed: m.selection.max_aggregate_named,
        maxAggregateBins: m.selection.max_aggregate_bins,
        maxAggregateCells: m.selection.max_aggregate_cells
      },
      maxTilesPerRequest: m.selection.max_tiles_per_request,
      filterOperands: m.filter_operands.map((f) => ({
        column: f.column,
        family: f.family,
        operands: f.operands,
        // Present only on a group-scoped column.
        ...(f.scope ? {scope: {group: f.scope.group}} : {})
      })),
      // Filtered per principal by the server: the layers this principal may know exist.
      layers: m.layers.map((l) => ({
        name: l.name,
        title: l.title,
        views: l.views,
        membership: l.membership,
        hierarchy: {kind: l.hierarchy.kind, pruneChildren: l.hierarchy.prune_children},
        levels: l.levels.map((v) => ({level: v.level, title: v.title, zoom: v.zoom})),
        computedContent: l.computed_content,
        shape: l.shape,
        suppliedContent: l.supplied_content,
        depsOn: l.depends_on,
        version: l.version
      }))
    };
  }

  /**
   * `POST /v1/viewport`: the points and counts for one area of a view at one zoom. It returns each
   * tile's counts and a sample of up to `k` points per tile, each point tagged with the artifacts of
   * the layers `layers` names. Fields of `req` you leave unset take the server's defaults: `k`
   * defaults to `meta.selection.kMaxMarks`, and the server caps it at `selection.maxK`. Set `k` to
   * `0` to fetch only the counts.
   *
   * @param opts.signal - Aborts the request and the reading of its body.
   * @param opts.background - Decode on a separate worker, so responses the user is waiting for do
   *   not queue behind this one. Defaults to `false`.
   * @param opts.onPart - Receives the points as they arrive, before the body has finished (see
   *   {@link PartSink}). The returned `result` then carries the counts and no points. Not called
   *   when `k` is `0`.
   * @param opts.onCounts - Receives the tiles' counts, and the sub-cells if `underlayOffset` asked
   *   for them, as soon as they arrive and before any points. Called at most once. The returned
   *   `result` carries the same counts.
   * @returns The decoded result, the server's timings, the response's keys (`identityKey`,
   *   `contentKey`, `pin`, `stale`, `region`) and the body's size in `bytes`.
   * @throws {@link MosaicaError} if the server refuses: `404` for an unknown view, `422` for a
   *   malformed request.
   * @throws `Error` if the body is malformed or ends before its trailer. Parts already passed to
   *   `onPart` hold whole tiles and remain valid.
   */
  async viewport(
    token: string,
    req: ViewportRequest,
    opts: {signal?: AbortSignal; background?: boolean; onPart?: PartSink; onCounts?: CountsSink} = {}
  ): Promise<ViewportResponse> {
    const {signal, background = false, onPart, onCounts} = opts;
    // Each optional field is sent only when the caller set it, so an unset one takes the server's
    // default. For `layers` and `levels` an empty array is a request for none, which differs from
    // leaving the field out.
    const body: Record<string, unknown> = {view: req.view, zoom: req.zoom};
    if (req.bbox) body.bbox = req.bbox;
    // A tile prefix at depth 16 needs 32 bits, which a JSON number holds exactly.
    if (req.tiles) body.tiles = req.tiles.map(Number);
    if (req.k !== undefined) body.k = req.k;
    if (req.underlayOffset) body.underlay_offset = req.underlayOffset;
    if (req.filters) body.filters = req.filters;
    if (req.highlight) body.highlight = req.highlight;
    if (req.pointRows !== undefined) body.point_rows = req.pointRows;
    if (req.layers !== undefined) body.layers = req.layers;
    if (req.artifactBudget !== undefined) body.artifact_budget = req.artifactBudget;
    if (req.levels !== undefined) body.levels = req.levels;
    // The stamp is held as the string the server sent and travels as the object it parses to.
    if (req.stamp) body.pin = JSON.parse(req.stamp);

    const response = await this.send(`${this.opts.viewerUrl}/v1/viewport`, {
      method: 'POST',
      headers: {...bearer(token), 'content-type': 'application/json'},
      body: jsonBody(body),
      signal
    });
    if (!response.ok) await fail(response);
    const coordinates = coordinatesOf(response);
    const decoder = (this.decoder ??= this.opts.decoder ?? createDecoder());
    // A counts-only response (`k = 0`) has no points and decodes in milliseconds, so it decodes on
    // this thread rather than queueing behind a point decode in a worker lane. It has no points
    // frames to hand a part sink.
    const counts = req.k === 0;
    const keys = {identityKey: coordinates.identityKey, contentKey: coordinates.contentKey};
    const columnsAsked = Array.isArray(req.pointRows) ? [...req.pointRows] : null;
    const layersAsked: readonly string[] | 'all' = req.layers === 'all' ? 'all' : [...(req.layers ?? [])];
    const underlay = Boolean(req.underlayOffset);
    const decoded =
      onPart && !counts
        ? await this.streamed(decoder, response, {...keys, columnsAsked, layersAsked}, onPart, background, underlay, onCounts)
        : await this.whole(decoder, response, counts, background, onCounts && countsWatch(onCounts, underlay, keys));
    this.opts.onDecode?.(decoded.ms, decoded.bytes, decoded.points, decoded.workerMs);
    return {
      result: decoded.result,
      timings: {...timingsOf(response), stageNs: decoded.stageNs},
      ...coordinates,
      columnsAsked,
      layersAsked,
      bytes: decoded.bytes
    };
  }

  /**
   * Reads the whole body, then decodes it: the path for a caller with no part sink. `watch`, where
   * given, sees the frames as they arrive.
   */
  private async whole(
    decoder: Decoder,
    response: Response,
    counts: boolean,
    background: boolean,
    watch?: (frame: Frame) => boolean
  ): Promise<Decoded> {
    const started = performance.now();
    const bytes = watch ? await readWatching(response, watch) : new Uint8Array(await response.arrayBuffer());
    // Read before decoding: the worker path transfers the buffer, which detaches it and leaves
    // `byteLength` at 0.
    const size = bytes.byteLength;
    const stageNs = stageNsOfBody(bytes);
    const result = counts ? decodeViewport(bytes) : await decoder.decode(bytes, background);
    return {
      result,
      bytes: size,
      points: result.ids.length,
      ms: performance.now() - started,
      workerMs: counts ? null : decoder.lastWorkerMs,
      stageNs
    };
  }

  /**
   * Reads the body as it arrives and hands each points frame to the sink once it is whole, so a
   * wide view draws before its last byte arrives. The server sends whole tiles per frame, in tile
   * order, and each frame is an Arrow stream that decodes alone.
   *
   * The tiles frame comes first, so each part carries the counts of the tiles its points belong
   * to. A part is a set of whole bands, and the replica stores it as it stores a whole response.
   *
   * A body that stops without its trailer throws here. The parts already delivered stay correct:
   * they come from one generation, and each tile's points are an id-order prefix of its served
   * set. The request does not resolve, so nothing marks the region covered.
   */
  private async streamed(
    decoder: Decoder,
    response: Response,
    coordinates: {identityKey: string; contentKey: string; columnsAsked: readonly string[] | null; layersAsked: readonly string[] | 'all'},
    onPart: PartSink,
    background: boolean,
    underlay: boolean,
    onCounts?: CountsSink
  ): Promise<Decoded> {
    const started = performance.now();
    if (!response.body) {
      // A `fetch` that gives no stream, such as a polyfill or a mock. The sink gets the whole
      // result as one part.
      const whole = await this.whole(decoder, response, false, background, onCounts && countsWatch(onCounts, underlay, coordinates));
      await onPart({result: whole.result, ...coordinates});
      return {...whole, result: headOnly(whole.result)};
    }
    const reader = response.body.getReader();
    const frames = new FrameReader();
    // The counts are decoded here, once, as they land.
    const readCounts = countsReader(underlay);
    let counts: Counts | null = null;
    let trailerBytes: Uint8Array | null = null;
    let bytes = 0;
    let flushes = 0;
    let points = 0;
    // Summed over the frames, and null where the decoder measures nothing, as the inline one does.
    let workerMs: number | null = null;
    // The tiles frame's rows, consumed in step with the point frames that satisfy them.
    let tileAt = 0;
    // Parts are delivered in wire order by awaiting each frame's decode in turn, whichever worker
    // decodes it. Reading continues while the sink takes a part, but the next part waits until the
    // sink's promise settles.
    let delivering: Promise<void> = Promise.resolve();
    // The chain is awaited only after the body has been read. Until then a rejection needs a
    // handler, or it is reported as unhandled.
    delivering.catch(() => {});

    // Set when the read fails: an abort, a transport fault, or a frame the grammar refuses. No part
    // is delivered after it, so an abandoned request does not keep filling the store.
    let abandoned = false;

    const deliver = (decoding: Promise<PointsPart>) => {
      // Awaited only when the chain reaches it, which is never once an earlier frame has failed.
      decoding.catch(() => {});
      delivering = delivering.then(async () => {
        const part = await decoding;
        // Read here: the decoder reports the last reply's time.
        const frameMs = decoder.lastWorkerMs;
        if (abandoned) return;
        if (frameMs !== null) workerMs = (workerMs ?? 0) + frameMs;
        const rows = part.ids.length;
        points += rows;
        // The grammar puts the tiles frame before every points frame, so the counts are whole.
        const run = tilesFor(counts!.tiles, tileAt, rows);
        tileAt = run.next;
        await onPart({
          result: {
            tiles: run.tiles,
            // The part's `projection` is the result's `pointsProjection`.
            ...part,
            pointsProjection: part.projection,
            subCells: null
          },
          ...coordinates
        });
      });
      delivering.catch(() => {});
    };

    try {
      for (;;) {
        const {done, value} = await reader.read();
        if (done) break;
        bytes += value.byteLength;
        for (const frame of frames.push(value)) {
          if (counts === null) {
            counts = readCounts(frame);
            if (counts) onCounts?.({...counts, ...coordinates});
          }
          switch (frame.kind) {
            case FRAME_POINTS:
              flushes += 1;
              deliver(decoder.decodePoints(frame.payload, background));
              break;
            case FRAME_TRAILER:
              trailerBytes = frame.payload;
              break;
          }
        }
      }
    } catch (error) {
      abandoned = true;
      // Releases the body for a fault the transport did not itself raise; an aborted stream is
      // already errored and this is a no-op on it.
      void reader.cancel().catch(() => {});
      throw error;
    }
    // Every whole frame is delivered before the body is checked. A body that stopped without its
    // trailer is refused below, but the whole tiles it delivered are correct and the caller keeps
    // them.
    await delivering;
    // Throws on a body that stopped inside a frame or without its trailer.
    frames.end();
    const whole = counts!;
    const trailer = parseTrailer(trailerBytes!);
    checkTrailerCounts(trailer, flushes, points);
    // Every tile the server served points for has had them.
    for (let i = tileAt; i < whole.tiles.length; i++) {
      if (whole.tiles[i]!.served !== 0n) {
        throw new Error(`tile ${whole.tiles[i]!.tile} was served ${whole.tiles[i]!.served} points that no frame carried`);
      }
    }
    return {
      result: {tiles: whole.tiles, ...emptyPoints(), subCells: whole.subCells},
      bytes,
      points,
      ms: performance.now() - started,
      workerMs,
      stageNs: stageNsOf(trailer)
    };
  }

  /**
   * `POST /v1/artifacts/viewport`: the artifacts of the named layers in each tile of one area of a
   * view at one zoom. A tile lists the artifacts that have a member the viewer can see inside it, at
   * most `perTile` per level, largest first. An artifact that spans several tiles appears in each,
   * with the same figures. A `nested` or `dag` layer is answered for all the tiles together, in one
   * frame that comes first. Fields of `req` you leave unset take the server's defaults.
   *
   * @param opts.signal - Aborts the request and the reading of its body.
   * @param opts.background - Decode on a separate worker, as for {@link MosaicaClient.viewport}.
   * @param opts.onTile - Receives each frame as it arrives, before the body has finished (see
   *   {@link TileSink}). A frame already passed on remains valid if the body later fails.
   * @returns Every frame, the response's keys (`identityKey`, `contentKey`, `pin`, `stale`,
   *   `region`), the server's timings and the body's size in `bytes`.
   * @throws {@link MosaicaError} if the server refuses: `404` for an unknown view, `422` for a
   *   malformed request or a `perTile` over `selection.maxArtifactsPerTile`.
   * @throws `Error` if the body is malformed or ends before its trailer.
   */
  async viewportArtifacts(
    token: string,
    req: ViewportArtifactsRequest,
    opts: {signal?: AbortSignal; background?: boolean; onTile?: TileSink} = {}
  ): Promise<ViewportArtifactsResponse> {
    const {signal, background = false, onTile} = opts;
    const body: Record<string, unknown> = {view: req.view, zoom: req.zoom, per_tile: req.perTile};
    if (req.bbox) body.bbox = req.bbox;
    if (req.tiles) body.tiles = req.tiles.map(Number);
    if (req.layers !== undefined) body.layers = req.layers;
    if (req.levels !== undefined) body.levels = req.levels;
    if (req.computed !== undefined) body.computed = req.computed;
    if (req.filters) body.filters = req.filters;
    if (req.highlight) body.highlight = req.highlight;
    if (req.budget !== undefined) body.budget = req.budget;
    if (req.paletteSize !== undefined) body.palette_size = req.paletteSize;
    if (req.stamp) body.pin = JSON.parse(req.stamp);

    const started = performance.now();
    const response = await this.send(`${this.opts.viewerUrl}/v1/artifacts/viewport`, {
      method: 'POST',
      headers: {...bearer(token), 'content-type': 'application/json'},
      body: jsonBody(body),
      signal
    });
    if (!response.ok) await fail(response);
    const decoder = (this.decoder ??= this.opts.decoder ?? createDecoder());
    const coordinates = coordinatesOf(response);
    const keys = {identityKey: coordinates.identityKey, contentKey: coordinates.contentKey};
    // A frame of no rows names no tile, so a tile frame is matched to the request's tiles by
    // position, as the server answers them: in order, with repeats removed.
    const asked = req.tiles ? [...new Set(req.tiles)] : null;
    const reader = new FrameReader('artifacts');
    const out: ViewportArtifactsFrame[] = [];
    let trailer: Uint8Array | null = null;
    let bytes = 0;
    let rows = 0;
    let kind5 = 0;
    let tileFrames = 0;
    let workerMs: number | null = null;
    let abandoned = false;
    // Frames are decoded in the lanes in turn and handed over in wire order by awaiting each in turn.
    let delivering: Promise<void> = Promise.resolve();
    const deliver = (decoding: Promise<ArtifactsFramePart>, first: boolean) => {
      // Awaited only when the chain reaches it, which is never once an earlier frame has failed.
      decoding.catch(() => {});
      delivering = delivering.then(async () => {
        const part = await decoding;
        const frameMs = decoder.lastWorkerMs;
        if (abandoned) return;
        if (frameMs !== null) workerMs = (workerMs ?? 0) + frameMs;
        // The treed frame is first and holds a row; every row of a tile frame names its tile.
        const treed = first && part.artifacts.length > 0 && part.tile === null;
        const frame: ViewportArtifactsFrame = {treed, tile: treed ? null : (part.tile ?? asked?.[tileFrames] ?? null), artifacts: part.artifacts};
        if (!treed) tileFrames += 1;
        rows += part.artifacts.length;
        out.push(frame);
        await onTile?.(frame, keys);
      });
      delivering.catch(() => {});
    };
    try {
      const consume = (chunk: Uint8Array) => {
        bytes += chunk.byteLength;
        for (const frame of reader.push(chunk)) {
          if (frame.kind === FRAME_ARTIFACTS) {
            kind5 += 1;
            deliver(decoder.decodeArtifacts(frame.payload, background), kind5 === 1);
          } else if (frame.kind === FRAME_TRAILER) {
            trailer = frame.payload;
          }
        }
      };
      if (!response.body) consume(new Uint8Array(await response.arrayBuffer()));
      else {
        const body = response.body.getReader();
        try {
          for (;;) {
            const {done, value} = await body.read();
            if (done) break;
            consume(value);
          }
        } catch (error) {
          void body.cancel().catch(() => {});
          throw error;
        }
      }
    } catch (error) {
      abandoned = true;
      throw error;
    }
    await delivering;
    reader.end();
    checkArtifactsTrailer(trailer!, kind5, rows);
    this.opts.onDecode?.(performance.now() - started, bytes, 0, workerMs);
    return {frames: out, timings: {...timingsOf(response), stageNs: null}, ...coordinates, bytes};
  }

  /**
   * `GET /v1/categories/{column}`: what a category column's codes stand for, each as a `code`,
   * `key` and `title`. With `codes`, it looks up those codes, in the order given. Without, it lists
   * every value in key order, fetching page after page until the last.
   *
   * A code missing from the answer is either unknown or a value the viewer may not see, and the
   * server does not say which. Every code on a point this session was served resolves.
   *
   * @param column - The column's name, or `<column>@<key>` for a group-scoped category pinned to
   *   one view.
   * @param opts.codes - The codes to look up. If omitted, every value is listed. An empty list
   *   returns `[]` without a request.
   * @param opts.limit - The page size when listing. Defaults to, and is capped at,
   *   `selection.maxCategoryValues`.
   * @param opts.view - The view whose values a group-scoped column lists.
   * @throws {@link MosaicaError} if the server refuses: `404` for a column that is not a category,
   *   `422` for a malformed request, such as `limit: 0` or a group-scoped column with no view.
   */
  async categories(
    token: string,
    column: string,
    opts: {codes?: readonly number[]; limit?: number; view?: string; signal?: AbortSignal} = {}
  ): Promise<CategoryValue[]> {
    const base = `${this.opts.viewerUrl}/v1/categories/${encodeURIComponent(column)}`;
    const out: CategoryValue[] = [];
    // A group-scoped category's codes are per view, so the view decides which column answers.
    const view = opts.view ? `view=${encodeURIComponent(opts.view)}` : '';

    if (opts.codes) {
      // An empty `codes=` would be read as the enumeration form.
      if (opts.codes.length === 0) return out;
      const url = `${base}?codes=${[...opts.codes].join(',')}${view ? `&${view}` : ''}`;
      const page = await this.categoryPage(token, url, opts.signal);
      return page.values;
    }

    let cursor: string | null = null;
    do {
      const params = new URLSearchParams();
      if (opts.limit !== undefined) params.set('limit', String(opts.limit));
      if (cursor !== null) params.set('after', cursor);
      if (opts.view !== undefined) params.set('view', opts.view);
      const query = params.toString();
      const page = await this.categoryPage(token, query ? `${base}?${query}` : base, opts.signal);
      out.push(...page.values);
      cursor = page.next;
    } while (cursor !== null);
    return out;
  }

  private async categoryPage(
    token: string,
    url: string,
    signal: AbortSignal | undefined
  ): Promise<{values: CategoryValue[]; next: string | null}> {
    const response = await this.send(url, {headers: bearer(token), signal});
    if (!response.ok) await fail(response);
    const body = (await response.json()) as RawCategories;
    return {
      values: body.values.map((v) => ({code: v.code, key: v.key, title: v.title ?? null})),
      next: body.next
    };
  }

  /**
   * `GET /v1/categories/{column}/suggest`: typeahead for a category column. It returns up to
   * `limit` values whose key or title, or a word in either, starts with `q`, ignoring case. Values
   * are ordered by the matched text, and only values the viewer may see are offered. The page
   * repeats `q`, so you can match a page to its request. With `filters` the request is a `POST`.
   *
   * A `429` is returned as `{status: 'shed', retryAfterS, detail}` instead of thrown;
   * {@link SuggestResult} lists the causes.
   *
   * @param column - As for {@link MosaicaClient.categories}.
   * @param q - The text typed. An empty string matches every value.
   * @param opts.limit - The page size. Defaults to, and is capped at, `selection.maxSuggestions`.
   * @param opts.counts - `true` adds each value's count of the visible items that carry it, and
   *   the page's `total`, the number of items the counts cover.
   * @param opts.view - As for `categories`. It is also the view `filters` applies in.
   * @param opts.filters - A filter expression, as the viewport takes. The counts then cover only
   *   the items in `view` that pass it. It changes nothing else, so a value it excludes is still
   *   offered, with count `0`. Requires `view`.
   * @throws {@link MosaicaError} for any other refusal: `404` for a column that is not a category,
   *   `422` for a `q` over 256 bytes, `filters` without `view`, or a malformed request.
   */
  async suggest(
    token: string,
    column: string,
    q: string,
    opts: {limit?: number; counts?: boolean; view?: string; filters?: FilterExpr; signal?: AbortSignal} = {}
  ): Promise<SuggestResult> {
    const route = `${this.opts.viewerUrl}/v1/categories/${encodeURIComponent(column)}/suggest`;
    let response: Response;
    if (opts.filters !== undefined) {
      const body: Record<string, unknown> = {q, filters: opts.filters};
      if (opts.limit !== undefined) body.limit = opts.limit;
      if (opts.counts) body.counts = true;
      if (opts.view !== undefined) body.view = opts.view;
      response = await this.send(route, {
        method: 'POST',
        headers: {...bearer(token), 'content-type': 'application/json'},
        body: JSON.stringify(body),
        signal: opts.signal
      });
    } else {
      const params = new URLSearchParams({q});
      if (opts.limit !== undefined) params.set('limit', String(opts.limit));
      if (opts.counts) params.set('counts', 'true');
      if (opts.view !== undefined) params.set('view', opts.view);
      response = await this.send(`${route}?${params.toString()}`, {
        headers: bearer(token),
        signal: opts.signal
      });
    }
    if (response.status === 429) {
      let body: {detail?: unknown} | null = null;
      try {
        body = (await response.json()) as {detail?: unknown};
      } catch {
        // A 429 whose body is not JSON, such as a proxy's, is shed all the same.
      }
      const detail = typeof body?.detail === 'string' ? body.detail : null;
      return {status: 'shed', retryAfterS: retryAfterOf(429, response.headers.get('retry-after'), body)!, detail};
    }
    if (!response.ok) await fail(response);
    const body = (await response.json()) as RawSuggest;
    return {
      status: 'ok',
      column: body.column,
      q: body.q,
      values: body.values.map((v) => ({
        code: v.code,
        key: v.key,
        title: v.title ?? null,
        match: {field: v.match.field, start: v.match.start, len: v.match.len},
        ...(v.count !== undefined ? {count: v.count} : {})
      })),
      more: body.more,
      ...(body.total !== undefined ? {total: body.total} : {}),
      ...regionOf(response)
    };
  }

  /**
   * `POST /v1/items/{mosaica_id}`: one item's full record, its position in each view the viewer
   * may reach, and the labels that let this session see it. {@link ItemDetail} describes each
   * field.
   *
   * @param mosaicaId - The item's `mosaica_id`, as in a viewport result's `ids`.
   * @throws {@link MosaicaError} with `404` for an id that names no item, and the same `404` for an
   *   item the viewer may not see.
   */
  async item(token: string, mosaicaId: bigint, signal?: AbortSignal): Promise<ItemDetail> {
    const response = await this.send(`${this.opts.viewerUrl}/v1/items/${mosaicaId.toString()}`, {
      method: 'POST',
      headers: {...bearer(token), 'content-type': 'application/json'},
      body: '{}',
      signal
    });
    if (!response.ok) await fail(response);
    const body = (await response.json()) as {
      fields: Record<string, unknown>;
      labels: string[];
      views: {id: string; x: number; y: number}[];
      scoped: Record<string, Record<string, unknown>>;
    };
    return {
      fields: body.fields ?? {},
      // Required by the response schema and read as given: an empty list is an answer.
      views: body.views,
      scoped: body.scoped,
      labels: body.labels
    };
  }

  /**
   * `POST /v1/artifacts/{mosaica_id}`: one artifact's layer, key, count and geometry in a view.
   * The count and the geometry cover only the members the viewer may see. Geometry is in 32-bit
   * grid units ({@link GRID32}), and a geometry field is `null` if the layer does not declare it.
   *
   * @param opts.view - The view to count and place the artifact in.
   * @param opts.zoom - The zoom to simplify the shape for, rounded down and clamped to 0 to 16. If
   *   omitted, the full stored shape is returned.
   * @throws {@link MosaicaError} with the same `404` for an id that names nothing, a point, a
   *   suppressed artifact, an artifact below its layer's existence criterion, and an artifact on a
   *   layer the viewer cannot reach.
   */
  async artifact(
    token: string,
    mosaicaId: bigint,
    opts: {view: string; zoom?: number; signal?: AbortSignal}
  ): Promise<ArtifactDetail> {
    const body: Record<string, unknown> = {view: opts.view};
    // The depth the shape is drawn at, which the server simplifies it to. Left out, the whole
    // stored shape is served.
    if (opts.zoom !== undefined) body.zoom = Math.max(0, Math.min(16, Math.floor(opts.zoom)));
    const response = await this.send(`${this.opts.viewerUrl}/v1/artifacts/${mosaicaId.toString()}`, {
      method: 'POST',
      headers: {...bearer(token), 'content-type': 'application/json'},
      body: jsonBody(body),
      signal: opts.signal
    });
    if (!response.ok) await fail(response);
    const served = (await response.json()) as {
      layer: string;
      key?: string;
      masked_count: number;
      centroid?: [number, number];
      box?: [number, number, number, number];
      shape?: Shape;
    };
    return {
      layer: served.layer,
      // Absent rather than null when the publisher supplied none.
      key: served.key ?? null,
      // Absent where the layer declares no such property. A served artifact always has the
      // geometry its layer declares.
      centroid: served.centroid ?? null,
      box: served.box ?? null,
      shape: served.shape ?? null,
      // A `bigint`, as the same count is on the Arrow wire, so a panel prints both one way.
      maskedCount: BigInt(served.masked_count)
    };
  }

  /**
   * `POST /v1/artifacts/browse`: one page of a layer's hierarchy, following parent and child links
   * regardless of what the map shows. It reads the layer's roots, one artifact's children together
   * with that artifact's parents, or the results of a name search. Each row carries its count of
   * visible members and, if `filters` is set, its count of matching ones. Rows are ordered by
   * count, highest first, then by `mosaica_id`, using the matching count if `filters` is set. To
   * read the next page, pass `next` back as `cursor`. It is `null` on the last page.
   *
   * A parent is listed only if the viewer is served both ends of the link. A `parent` the viewer is
   * not served returns an empty page.
   *
   * @throws {@link MosaicaError} if the server refuses: `404` for an unknown view, `422` for an
   *   unknown layer, both `parent` and `q`, `limit: 0` or a malformed filter.
   */
  async browse(token: string, req: BrowseRequest, signal?: AbortSignal): Promise<BrowsePage> {
    const body: Record<string, unknown> = {view: req.view, layer: req.layer};
    if (req.level !== undefined) body.level = req.level;
    if (req.parent !== undefined) body.parent = req.parent.toString();
    if (req.q !== undefined) body.q = req.q;
    if (req.filters) body.filters = req.filters;
    if (req.limit !== undefined) body.limit = req.limit;
    if (req.cursor !== undefined) body.cursor = req.cursor;
    if (req.paletteSize !== undefined) body.palette_size = req.paletteSize;
    const response = await this.send(`${this.opts.viewerUrl}/v1/artifacts/browse`, {
      method: 'POST',
      headers: {...bearer(token), 'content-type': 'application/json'},
      body: jsonBody(body),
      signal
    });
    if (!response.ok) await fail(response);
    const page = (await response.json()) as RawBrowsePage;
    return {
      artifacts: (page.artifacts ?? []).map(browseRow),
      // Filled on the children form only.
      parents: (page.parents ?? []).map(browseRow),
      next: page.next ?? null
    };
  }

  /**
   * `POST /v1/items`: read the items the viewer may see in a view, one page at a time. It resolves
   * when the first response's head ({@link ItemsHead}) arrives. Iterate the result for each page as
   * an Arrow table; further pages are fetched as you iterate. {@link RecordsRead} says how a read
   * ends.
   *
   * @throws {@link MosaicaError} if the server refuses the first request: `404` for an unknown
   *   view, `422` for a malformed filter or a request {@link ItemsRequest} says is refused, and
   *   `429` if too many bulk reads are running.
   */
  items(token: string, req: ItemsRequest, signal?: AbortSignal): Promise<RecordsRead<ItemsHead>> {
    return this.bulkRead('items', token, req, signal, (raw) => {
      requireFields(raw, ['order', 'page_rows'], 'the head of a /v1/items response');
      const head = raw as {order: ItemsHead['order']; page_rows: number; visible?: number; matched?: number};
      return {order: head.order, pageRows: head.page_rows, visible: head.visible ?? null, matched: head.matched ?? null};
    });
  }

  /**
   * `POST /v1/artifacts`: read the artifacts of one layer that the viewer is served, one page at a
   * time, in the same way as {@link MosaicaClient.items}.
   *
   * @throws {@link MosaicaError} if the server refuses the first request: `404` for an unknown
   *   view, `422` for a malformed filter or a request {@link ArtifactsRequest} says is refused, and
   *   `429` if too many bulk reads are running.
   */
  artifacts(token: string, req: ArtifactsRequest, signal?: AbortSignal): Promise<RecordsRead<ArtifactsHead>> {
    return this.bulkRead('artifacts', token, req, signal, (raw) => {
      requireFields(raw, ['page_rows'], 'the head of a /v1/artifacts response');
      const head = raw as {page_rows: number; served?: number; matched?: number};
      return {pageRows: head.page_rows, served: head.served ?? null, matched: head.matched ?? null};
    });
  }

  private bulkRead<Head>(
    route: 'items' | 'artifacts',
    token: string,
    req: ItemsRequest | ArtifactsRequest,
    signal: AbortSignal | undefined,
    parseHead: (raw: unknown) => Head
  ): Promise<RecordsRead<Head>> {
    const request = (cursor?: string) => this.post(route, token, cursor === undefined ? req : {...req, cursor, count: undefined}, signal);
    return openRecords(req, request, signal, parseHead);
  }

  /**
   * `POST /v1/aggregate`: exact counts of how the items the viewer may see in a view are
   * distributed, as one table per grouping. It reads every page of every response and joins each
   * table's pages into one Arrow table. {@link AggregateTable} lists the columns.
   *
   * The server works out the viewer's visible set afresh for each response, so a deletion or
   * suppression accepted during a long read applies from the next response.
   * {@link AggregateResult.recomposed} says whether that happened.
   *
   * ```ts
   * const {tables} = await client.aggregate(token, {
   *   view: 'papers',
   *   filters: {year: {range: {gte: 2020}}},
   *   groupings: [{count: true}, {count: true, by: {field: 'venue', top: 10}}]
   * });
   * console.log(tables[0]!.total, tables[1]!.rows.toArray());
   * ```
   *
   * @param options - `follow`: whether to read the responses after the first. Defaults to `true`.
   *   With `false` the result holds only the first response's pages, and
   *   {@link AggregateResult.next} is the cursor to pass back as `cursor` to continue.
   * @throws {@link MosaicaError} if the server refuses the first request: `404` for an unknown
   *   view, `422` for an invalid request, naming the limit if one is exceeded.
   * @throws {@link PartialAggregate} if a later request is refused or a body is cut off. It holds
   *   the pages read so far and the cursor to continue from.
   * @throws `Error` if a response's trailer does not match its body, and the signal's reason if it
   *   aborts.
   */
  aggregate(token: string, req: AggregateRequest, signal?: AbortSignal, options: {follow?: boolean} = {}): Promise<AggregateResult> {
    const sent = {...req, groupings: req.groupings.map(wireGrouping)};
    const request = (cursor?: string) => this.post('aggregate', token, cursor === undefined ? sent : {...sent, cursor}, signal);
    return readAggregate(req, request, signal, options.follow ?? true);
  }

  /** One bulk read's request: each field of `given` that is set, under its wire name. */
  private async post(route: string, token: string, given: object, signal: AbortSignal | undefined): Promise<Response> {
    const body: Record<string, unknown> = {};
    for (const [name, value] of Object.entries(given)) {
      if (value !== undefined) body[name.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`)] = value;
    }
    const response = await this.send(`${this.opts.viewerUrl}/v1/${route}`, {
      method: 'POST',
      headers: {...bearer(token), 'content-type': 'application/json'},
      // A `mosaica_id` travels as a decimal string.
      body: jsonBody(body),
      signal
    });
    if (!response.ok) await fail(response);
    return response;
  }
}

/** A grouping as the wire names it: a layer's `paletteSize` is `palette_size`. */
function wireGrouping(grouping: Grouping): object {
  const by = grouping.by;
  if (!by || !('layer' in by) || by.paletteSize === undefined) return grouping;
  const {paletteSize, ...rest} = by;
  return {...grouping, by: {...rest, palette_size: paletteSize}};
}

/** One row of `POST /v1/artifacts/browse`, as the JSON carries it. */
type RawBrowseRow = {
  mosaica_id: string;
  key?: string | null;
  name?: string | null;
  masked_count: number | string;
  matched_count?: number | string | null;
  rung: number;
  parent_ids?: string[];
  child_count: number;
  slot?: number | null;
};

type RawBrowsePage = {artifacts?: RawBrowseRow[]; parents?: RawBrowseRow[]; next?: string | null};

function browseRow(r: RawBrowseRow): BrowseRow {
  return {
    mosaicaId: BigInt(r.mosaica_id),
    // Absent where the publisher supplied none and where this principal may not read the text.
    // The two look the same, as they do on the artifacts frame.
    key: r.key ?? null,
    name: r.name ?? null,
    maskedCount: BigInt(r.masked_count),
    matchedCount: r.matched_count === undefined || r.matched_count === null ? null : BigInt(r.matched_count),
    rung: r.rung,
    // A null cell is the empty list.
    parentIds: (r.parent_ids ?? []).map((v) => BigInt(v)),
    childCount: r.child_count,
    slot: r.slot ?? null
  };
}

/** The fields `/v1/meta` must carry, and those each of its blocks and list elements must. */
const VIEW_FIELDS = ['id', 'display_name', 'quantisation', 'projection', 'world_aspect', 'tile_scheme', 'tile', 'group', 'key', 'metadata'];
const GROUP_FIELDS = ['name', 'title', 'members_of', 'views'];
/** The `authorization` header for `token`, or none where there is no token, as an open server takes. */
function bearer(token: string | undefined): Record<string, string> {
  return token ? {authorization: `Bearer ${token}`} : {};
}

/**
 * `value` as JSON, a `bigint` written as its decimal digits in a string: a filter comparand past
 * 2^53, which a JSON number read as a double would round, is sent that way.
 *
 * @internal
 */
export function jsonBody(value: unknown): string {
  return JSON.stringify(value, (_, v: unknown) => (typeof v === 'bigint' ? v.toString() : v));
}

const DECLARED_FIELDS = ['name', 'arrow_type', 'category', 'analyser', 'render', 'index', 'unique', 'homes'];
const SCOPED_FIELDS = ['name', 'arrow_type', 'scope', 'category', 'analyser', 'render', 'index', 'views'];
const OPERAND_FIELDS = ['column', 'family', 'operands'];
const LAYER_FIELDS = ['name', 'title', 'views', 'membership', 'hierarchy', 'levels', 'computed_content', 'shape', 'supplied_content', 'depends_on', 'version'];
const LEVEL_FIELDS = ['level', 'title', 'zoom'];
const META_FIELDS = ['api_version', 'bundle_format', 'views', 'groups', 'declared_scalars', 'scoped_scalars', 'filter_operands', 'selection', 'layers'] as const;
const SELECTION_FIELDS = [
  'k_min',
  'k_max_marks',
  'max_k',
  'theta_target_marks',
  'max_underlay_offset',
  'max_tiles_per_request',
  'max_artifacts_per_tile',
  'max_category_values',
  'max_shape_vertices',
  'max_region_vertices',
  'max_region_cells',
  'max_suggestions',
  'max_suggestion_walk',
  'max_suggest_set_entities',
  'max_browse_rows',
  'max_page_rows',
  'max_page_bytes',
  'max_aggregate_groupings',
  'max_aggregate_top',
  'max_aggregate_named',
  'max_aggregate_bins',
  'max_aggregate_cells'
] as const;

/** Throws where `body` is not an object or lacks one of `fields`. */
function requireFields(body: unknown, fields: readonly string[], where: string): void {
  if (typeof body !== 'object' || body === null) throw new Error(`${where} is not an object; the server and this client are from different versions`);
  for (const field of fields) {
    if (!(field in body)) throw new Error(`${where} has no \`${field}\`, which the contract requires; the server and this client are from different versions`);
  }
}

type RawCategory = {vocabulary: string; kind: 'declared' | 'discovered'; visibility: 'derived' | 'public'};

/** `GET /v1/meta`'s snake_case wire shape, mapped to {@link Meta} above. */
type RawMeta = {
  api_version: number;
  bundle_format: number;
  views: {
    id: string;
    display_name: string;
    quantisation: {x_min: number; x_max: number; y_min: number; y_max: number};
    projection: MapProjection;
    world_aspect: number | null;
    tile_scheme: TileScheme | null;
    tile: {z: number; x: number; y: number} | null;
    /** The roster record; all three null together on a plain view. */
    group: string | null;
    key: string | null;
    metadata: Record<string, ViewMetadataValue> | null;
  }[];
  groups: {name: string; title: string | null; members_of: string | null; views: string[]}[];
  declared_scalars: {
    name: string;
    arrow_type: ArrowType;
    category: RawCategory | null;
    analyser: string | null;
    render: boolean;
    index: boolean;
    unique: boolean;
    homes: ('rendered' | 'value_column' | 'record')[];
  }[];
  scoped_scalars: {
    name: string;
    arrow_type: ArrowType;
    scope: {group: string};
    category: RawCategory | null;
    analyser: string | null;
    render: boolean;
    index: boolean;
    views: string[];
  }[];
  filter_operands: {
    column: string;
    family: FilterOperandSet['family'];
    operands: string[];
    scope?: {group: string};
  }[];
  layers: {
    name: string;
    title: string | null;
    views: string[];
    membership: Layer['membership'];
    hierarchy: {kind: Layer['hierarchy']['kind']; prune_children: boolean};
    levels: {level: number; title: string | null; zoom: [number, number] | null}[];
    computed_content: string[];
    shape: ShapeKind | null;
    supplied_content: string[];
    depends_on: string[];
    version: number;
  }[];
  selection: {
    k_min: number;
    k_max_marks: number;
    max_k: number;
    theta_target_marks: number;
    max_underlay_offset: number;
    max_tiles_per_request: number;
    max_artifacts_per_tile: number;
    max_category_values: number;
    max_shape_vertices: number;
    max_region_vertices: number;
    max_region_cells: number;
    max_suggestions: number;
    max_suggestion_walk: number;
    max_suggest_set_entities: number;
    max_browse_rows: number;
    max_page_rows: number;
    max_page_bytes: number;
    max_aggregate_groupings: number;
    max_aggregate_top: number;
    max_aggregate_named: number;
    max_aggregate_bins: number;
    max_aggregate_cells: number;
  };
};

/** `GET /v1/categories/{column}`'s wire shape. */
type RawCategories = {
  column: string;
  values: {code: number; key: string; title?: string | null}[];
  next: string | null;
};

/** `GET /v1/categories/{column}/suggest`'s wire shape. */
type RawSuggest = {
  column: string;
  q: string;
  values: {
    code: number;
    key: string;
    title?: string | null;
    match: {field: 'key' | 'title'; start: number; len: number};
    count?: number;
  }[];
  more: boolean;
  total?: number;
};

/** A viewport-shaped response's keys, from its headers. */
function coordinatesOf(response: Response): Pick<ViewportResponse, 'identityKey' | 'contentKey' | 'pin' | 'stale' | 'region'> {
  return {
    identityKey: response.headers.get('x-mosaica-identity-key') ?? '',
    // The quotes are the entity-tag syntax and not part of the key.
    contentKey: (response.headers.get('etag') ?? '').replace(/^"|"$/g, ''),
    pin: response.headers.get('x-mosaica-pin'),
    stale: response.headers.get('x-mosaica-stale') === '1',
    // Absent unless the request carried a region leaf.
    region: parseRegionVerdict(response.headers.get('x-mosaica-region'))
  };
}

/** The server's times from a viewport-shaped response's headers: to its first flush, not to its end. */
function timingsOf(response: Response): Omit<ViewportResponse['timings'], 'stageNs'> {
  return {
    serverUs: Number(response.headers.get('x-mosaica-server-us') ?? 0),
    admissionUs: Number(response.headers.get('x-mosaica-admission-us') ?? 0)
  };
}

/** `{region}` where the response carries `x-mosaica-region`, and nothing otherwise. */
function regionOf(response: Response): {region?: RegionVerdict} {
  const region = parseRegionVerdict(response.headers.get('x-mosaica-region'));
  return region === null ? {} : {region};
}
