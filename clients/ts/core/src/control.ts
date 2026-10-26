/**
 * The control plane client. This file imports nothing, so the operator scripts can load it under
 * Node's type stripping.
 */

const ARROW = 'application/vnd.apache.arrow.stream';
const JSON_TYPE = 'application/json';

/**
 * The shortest wait a `429` asks for, in seconds, which is also the server's own minimum. A `429`
 * that names no wait, or one shorter than this, waits this long.
 */
export const MIN_BACKOFF = 1;

/**
 * The longest wait between attempts after a `429`, in seconds. A longer `Retry-After` is cut to
 * this.
 */
export const MAX_BACKOFF = 30;

/**
 * The most one call waits in all between attempts after a `429`, in seconds. A `429` is waited out
 * and the request sent again while the waits stay within this; the `429` whose wait would take
 * them past it is returned as the answer.
 */
export const RETRY_BUDGET_S = 300;

/**
 * The `status` of an {@link Answer} when the request reached no server. It is not an HTTP status.
 *
 * @category Control plane
 */
export const UNANSWERED = 0;

/**
 * The result of a {@link Control} call. Every response, including a refusal, is returned as an
 * `Answer`. None is thrown. If `ok` is `true`, `body` is the route's response, `B`. Otherwise it
 * is the refusal, with `error` and `detail`.
 *
 * @category Control plane
 */
export type Answer<B extends object = Record<string, unknown>> = {
  /** The HTTP status, or {@link UNANSWERED} if no server answered. */
  status: number;
  /** The response body as text. If no server answered, a sentence naming the URL and the error. */
  text: string;
  /** How many times the request was sent, including retries. */
  attempts: number;
  /** Seconds from the first attempt to the answer, including waits between attempts. */
  seconds: number;
} & (
  | {
      /** `status` is in the 2xx range. */
      ok: true;
      /** The body parsed as JSON. */
      body: B;
    }
  | {
      /** `status` is outside the 2xx range. */
      ok: false;
      /**
       * The body parsed as JSON: an object as it is, any other JSON value wrapped as `{value}`, or
       * `{}` if the body is not JSON. A refusal has `error` and `detail`.
       */
      body: Record<string, unknown>;
    }
);

/**
 * The answer to `ingest`, with the batch id the request carried.
 *
 * @category Control plane
 */
export type RowAnswer = Answer<IngestResponse> & {
  /**
   * The batch id the request was sent with. Pass it as `batch` to send the same body again, which
   * the server answers as a replay if it accepted the body the first time.
   */
  batch: string;
};

/**
 * Why a row was refused.
 *
 * - `names_two_items`: the row's values name more than one item.
 * - `unknown_mosaica_id`: its `mosaica_id` names no live or suppressed item.
 * - `names_no_item`: its values name no item, in a request that does not create items.
 * - `one_item_twice`: an earlier row of the batch names the same item.
 * - `one_value_twice`: an earlier row of the batch sets the same unique value.
 * - `value_held`: the row would give an item a unique value that an item outside the writer's
 *   terms already holds.
 * - `label_not_held`: the row, or an artifact in `publish` or `grow`, sets an access label the
 *   writer's terms do not satisfy.
 *
 * Only `ingest` gives `one_item_twice`, `one_value_twice` and `value_held`.
 *
 * @category Control plane
 */
export type RefusalReason =
  | 'names_two_items'
  | 'unknown_mosaica_id'
  | 'names_no_item'
  | 'one_item_twice'
  | 'one_value_twice'
  | 'value_held'
  | 'label_not_held';

/**
 * A row that was refused, with the reason.
 *
 * @category Control plane
 */
export type RefusedRow = {
  /** The row's position in the request, from 0. */
  row: number;
  /** Why it was refused. */
  reason: RefusalReason;
};

/**
 * A member that was refused, so the artifact was published or grown without it. Without `list`
 * and `row`, it is a whole artifact refused as `label_not_held`, for which nothing was written.
 *
 * @category Control plane
 */
export type RefusedMember = {
  /** The artifact's position in the request, from 0. */
  artifact: number;
  /** The member table the member was in. */
  list?: 'members' | 'excluding' | 'leaving';
  /** The member's row in that table, from 0. */
  row?: number;
  /** Why it was refused. */
  reason: RefusalReason;
};

/**
 * Names one item by its `mosaica_id`, the values of its unique fields, or both, keyed by column.
 * A unique field's value is a string for a keyword. For an integer it is a number or a string of
 * decimal digits, and for a timestamp it may also be a date, or a date-time with an offset, as a
 * string. `null` names nothing. Every value given must name the same item, and a row that names no
 * item, or two, is refused. A key that is neither `mosaica_id` nor a unique field is ignored and
 * listed in the answer's `ignored_columns`.
 *
 * @category Control plane
 */
export type AddressRow = {
  /** The item's `mosaica_id`, a string of decimal digits. */
  mosaica_id?: string | null;
} & {[field: string]: string | number | null};

/**
 * Several items as columns of equal length, one row per item, keyed by `mosaica_id` and unique
 * field names. Each cell is as in an {@link AddressRow}, or `null` if that column does not name
 * the row's item. `{}` holds no items. Columns of different lengths are refused with `422`. A
 * column that is neither `mosaica_id` nor a unique field is ignored and listed in
 * `ignored_columns`.
 *
 * @category Control plane
 */
export type MemberTable = {
  /** Each row's `mosaica_id`, a string of decimal digits. */
  mosaica_id?: (string | null)[];
} & {[column: string]: (string | number | null)[]};

/**
 * One change in a {@link Control.changes} request, in the route's own field names.
 *
 * @category Control plane
 */
export type ChangeItem = {
  /**
   * `delete` removes the item from every response, and a later compaction removes its rows.
   * `suppress` hides it from every response until `unsuppress` lifts the suppression. Both apply
   * from the moment the request is accepted.
   */
  op: 'delete' | 'suppress' | 'unsuppress';
  /** The item to change. Several changes in one request may name the same item. */
  match: AddressRow;
};

/**
 * The artifact another is attached to, such as the cluster a label names. The target must already
 * exist, on a layer that the attached artifact's layer names in `depends_on`.
 *
 * @category Control plane
 */
export type Attachment = {
  /** The target's layer. */
  layer: string;
  /** The target's level. Defaults to 0. */
  level?: number;
  /** The target's key. */
  key: string;
};

/**
 * One artifact in a {@link PublishRequest}, in the route's own field names. Its members come from
 * `members` or `excluding`. An artifact with `attached_to` may have neither, and then covers its
 * target's members.
 *
 * @category Control plane
 */
export type PublishedArtifact = {
  /** The publisher's key, unique within the level. You need it to name the artifact as a parent or an attachment target, or to grow it. */
  key?: string | null;
  /** The key of the view the artifact belongs to. Required on a layer scoped to a view group, and refused on any other. */
  view?: string | null;
  /** The members. `{}` is an empty membership. */
  members?: MemberTable | null;
  /** The view's items to leave out, so the membership is every other item in the view. Refused together with `members`. */
  excluding?: MemberTable | null;
  /**
   * Supplied content, ranked from most specific: one value per supplied content the layer declares,
   * and in `generated_from` the items it was made from, if the content requires them.
   */
  content?: {values: string[]; generated_from?: MemberTable | null}[];
  /** The artifact this one is attached to. */
  attached_to?: Attachment | null;
  /** Parent keys, each already held or published earlier in the request. */
  parent?: string[];
  /** `[min_x, min_y, max_x, max_y]`, on a layer whose `shape` is `bbox`. */
  bbox?: number[] | null;
  /** `[cx, cy, r]`, on a layer whose `shape` is `circle`. */
  circle?: number[] | null;
  /** `[cx, cy, a, b, angle_degrees]`, on a layer whose `shape` is `ellipse`. */
  ellipse?: number[] | null;
  /** A WKT polygon or multipolygon, on a layer whose `shape` is `polygon`. */
  wkt?: string | null;
  /** This artifact's coordinate space, overriding the request's `default_space`. */
  space?: 'view' | 'wgs84' | null;
  /** The artifact's own access labels. `null` or `[]` means none. */
  access?: string[] | null;
};

/**
 * The body of {@link Control.publish}.
 *
 * @category Control plane
 */
export type PublishRequest = {
  /** The level the artifacts go into. Defaults to 0. */
  level?: number;
  /** The coordinate space of shapes on artifacts that do not set `space`: `view` for the view's own coordinates, `wgs84` for degrees. Defaults to `view`. */
  default_space?: 'view' | 'wgs84';
  /** The artifacts, at least one. */
  artifacts: PublishedArtifact[];
};

/**
 * One artifact in a {@link GrowRequest}, named by the key it was published under. Parts the
 * artifact lacks are filled in. A row with only a key changes nothing.
 *
 * @category Control plane
 */
export type GrowingArtifact = {
  /** The key the artifact was published under. */
  key: string;
  /** The key of the view the artifact belongs to, as in {@link PublishedArtifact}. */
  view?: string | null;
  /** Without `rank`, `members` join the artifact's membership. With it, `members` and `leaving` change the generating set of the content at this rank. A content's rank is its position among the artifact's contents, and it keeps its number when an earlier content is withdrawn. */
  rank?: number | null;
  /** Members joining. */
  members?: MemberTable | null;
  /** Members leaving a generating set, applied after the joins. Requires `rank`. */
  leaving?: MemberTable | null;
  /** Parent keys. */
  parent?: string[];
  /** The artifact this one is attached to. */
  attached_to?: Attachment | null;
  /** Contents by rank. A content given at a rank that holds none, because it was withdrawn or is past the last, is added as a new content at the next free rank. */
  content?: {rank: number; values: string[]}[];
  /** As in {@link PublishedArtifact}. */
  bbox?: number[] | null;
  /** As in {@link PublishedArtifact}. */
  circle?: number[] | null;
  /** As in {@link PublishedArtifact}. */
  ellipse?: number[] | null;
  /** As in {@link PublishedArtifact}. */
  wkt?: string | null;
  /** This row's coordinate space for authored shape content, overriding the request's `default_space`. */
  space?: 'view' | 'wgs84' | null;
  /** The artifact's access labels. */
  access?: string[] | null;
};

/**
 * The JSON body of {@link Control.grow}.
 *
 * @category Control plane
 */
export type GrowRequest = {
  /** As in {@link PublishRequest}. */
  level?: number;
  /** As in {@link PublishRequest}. */
  default_space?: 'view' | 'wgs84';
  /** The artifacts, at least one. */
  artifacts: GrowingArtifact[];
};

/**
 * The publication at which a write becomes visible to viewers.
 *
 * @category Control plane
 */
export type PublicationAck = {
  /** The number of the first publication that includes the write. */
  publication: number;
  /** With `wait`, whether that publication had happened before the answer was sent. */
  visible?: boolean;
};

/**
 * The answer to `ingest`. Refused rows are not counted.
 *
 * @category Control plane
 */
export type IngestResponse = PublicationAck & {
  /** Rows the batch carried. */
  rows: number;
  /** Rows that named no item and created one. */
  created: number;
  /** Rows that edited an item they named. */
  edited: number;
  /** Rows that added an item they named to the batch's view. */
  added: number;
  /** Rows that named an item and changed nothing. */
  unchanged: number;
  /** Rows creating an item whose labels index it under more than 4,096 keys. These rows are stored. */
  over_bound: number;
  /** The positions of the first 100 of those rows. */
  over_bound_rows: number[];
  /** Rows whose coordinates lay outside the projection's domain, stored on the frame's edge. */
  clipped: number;
  /** Rows whose coordinates lay outside the view's extent, stored on its edge. */
  clamped: number;
  /** One per row, in the order sent: the `mosaica_id` of the item the row created or named, or `null` if the row was refused. */
  mosaica_ids: (string | null)[];
  /** The refused rows, with reasons. Empty with `strict`, which refuses the whole batch instead. */
  refused: RefusedRow[];
  /** Artifacts the batch's layer columns created. */
  minted: number;
  /** Memberships the batch's layer columns added. */
  joined: number;
  /** Present if this body was already accepted under this batch id, in which case nothing was written. */
  replayed?: true;
};

/**
 * The answer to `changes`.
 *
 * @category Control plane
 */
export type ChangesApplied = PublicationAck & {
  /** How many changes were applied. */
  accepted: number;
  /** The refused changes, in the order sent. Empty with `strict`. */
  refused: RefusedRow[];
  /** The `match` keys that are neither `mosaica_id` nor a unique field, which were ignored. */
  ignored_columns: string[];
};

/**
 * How one artifact's shape was fitted to each view of its layer.
 *
 * @category Control plane
 */
export type ShapeReport = {
  /** The artifact's key. */
  key: string | null;
  /** The rank of the authored shape content this reports on. Absent for the artifact's own shape. */
  content?: number;
  /** One entry per view of the layer. */
  views: {
    view: string;
    clipped: boolean;
    outside: boolean;
    rings_dropped: number;
    degrees_looking: boolean;
    vertices_in: number;
    vertices_out: number;
    parts: number;
    rings: number;
    interior_tiles: number;
    boundary_cells: number;
  }[];
};

/**
 * The answer to `publish`.
 *
 * @category Control plane
 */
export type ArtifactsPublished = PublicationAck & {
  /** One per artifact, in the order sent, with its `mosaica_id`. If the key was already held, it is the existing artifact's id. `null` for an artifact refused as `label_not_held`. */
  artifacts: {key: string | null; mosaica_id: string | null}[];
  /** Artifacts the request created. */
  created: number;
  /** How many of those have no content, on a layer that declares some. */
  without_content: number;
  /** Parts filled in on artifacts that already existed. */
  filled: number;
  /** Memberships added, to new and existing artifacts alike. */
  joined: number;
  /** The refused members, which the artifacts were published without, and the artifacts refused as `label_not_held`. Empty with `strict`. */
  refused: RefusedMember[];
  /** Member table columns that are neither `mosaica_id` nor a unique field, which were ignored. */
  ignored_columns: string[];
  /** Present if the request carried a shape or authored shape content. */
  shapes?: ShapeReport[];
};

/**
 * The answer to `grow`.
 *
 * @category Control plane
 */
export type MembershipsGrown = PublicationAck & {
  /** One per artifact, in the order sent. `withdrawn` is the rank of a content the row withdrew by emptying its generating set. For a row refused as `label_not_held`, `mosaica_id` is `null` and every count is 0. */
  artifacts: {key: string; mosaica_id: string | null; joined: number; filled: number; left: number; withdrawn?: number}[];
  /** The refused members, which the artifacts were grown without, and the rows refused as `label_not_held`. Empty with `strict`. */
  refused: RefusedMember[];
  /** Member table columns that are neither `mosaica_id` nor a unique field, which were ignored. */
  ignored_columns: string[];
  /** Present if the request filled in authored shape content. */
  shapes?: ShapeReport[];
};

/**
 * What a principal may do. Its terms, which say what it may see, are granted separately.
 *
 * - `read`: start a session for itself and make viewer requests with it.
 * - `write`: ingest, delete, suppress and annotate within its own terms, and flush and compact.
 * - `authorise-as`: start sessions for other principals.
 * - `admin`: read the server's status, and manage principals, keys, groups, grants, providers and
 *   sessions.
 * - `read-all`: with `read`, see every item and satisfy every label.
 * - `write-all`: with `write`, write across all items, declare columns, vocabularies, layers,
 *   views, view groups and shards, and drop layers and views.
 *
 * Each permission is granted on its own: `admin` does not imply `read` or `write`.
 *
 * @category Control plane
 */
export type Permission = 'read' | 'write' | 'authorise-as' | 'admin' | 'read-all' | 'write-all';

/**
 * The answer to every catalogue change. A change ends every live session whose terms or
 * permissions it could change.
 *
 * @category Control plane
 */
export type CatalogueChange = {
  /** How many live sessions the change ended. */
  sessions_ended: number;
};

/**
 * A local principal, as the catalogue holds it. Passwords and key secrets are never returned.
 *
 * @category Control plane
 */
export type PrincipalRecord = {
  /** The principal's name, which every other verb takes. */
  name: string;
  /** A person or a service. */
  kind: 'person' | 'service';
  /** Whether its credentials are refused. */
  disabled: boolean;
  /** Whether a password is set. */
  has_password: boolean;
  /** The terms granted to the principal directly. */
  terms: string[];
  /** The permissions granted to the principal directly. */
  permissions: Permission[];
  /** The groups it is a member of. */
  groups: string[];
};

/**
 * The body of `changePrincipal`.
 *
 * @category Control plane
 */
export type PrincipalChange = {
  /** `true` ends the principal's sessions and refuses each of its credentials. */
  disabled: boolean;
};

/**
 * The body of `createKey`.
 *
 * @category Control plane
 */
export type KeyCreate = {
  /** When the key and every session authorised with it end, in seconds since the Unix epoch. If omitted, the key does not expire. */
  expires_at?: number;
  /** The key's own permissions, narrower than its principal's. If omitted, it holds its principal's. */
  permissions?: Permission[];
};

/**
 * An API key as `createKey` issues it.
 *
 * @category Control plane
 */
export type IssuedKey = {
  /** The key's public identity, which listing and revoking use. */
  prefix: string;
  /** The whole key, `msk_<prefix>_<secret>`. It is returned only here, once. */
  key: string;
};

/**
 * An API key as `listKeys` lists it, without its secret.
 *
 * @category Control plane
 */
export type KeyRecord = {
  /** The key's public identity, which revoking takes. */
  prefix: string;
  /** The principal it authenticates. */
  principal: string;
  /** When it was issued, in seconds since the Unix epoch. */
  created_at: number;
  /** When it stops being accepted, in seconds since the Unix epoch, or `null` for never. */
  expires_at: number | null;
  /** Its own permissions, narrower than its principal's, or `null` if it holds its principal's. */
  permissions: Permission[] | null;
};

/**
 * A local group.
 *
 * @category Control plane
 */
export type GroupRecord = {
  /** The group's name. */
  name: string;
  /** The terms granted to the group, which each member holds. */
  terms: string[];
  /** The permissions granted to the group, which each member holds. */
  permissions: Permission[];
  /** The principals in the group. */
  members: string[];
};

/**
 * One grant or revocation. Set exactly one of `principal` and `group`, and exactly one of `term`,
 * `terms` and `permission`.
 *
 * @category Control plane
 */
export type Grant = {
  /** The local principal the grant is made to. */
  principal?: string;
  /** The group the grant is made to. */
  group?: string;
  /** One term, which says what the grantee may see. `public` and an empty term are refused. */
  term?: string;
  /** Many terms, granted or revoked as one change. One refused term refuses them all. */
  terms?: string[];
  /** A permission, which says what the grantee may do. */
  permission?: Permission;
};

/**
 * An OIDC provider as `putProvider` declares it.
 *
 * @category Control plane
 */
export type ProviderDeclaration = {
  /** The `iss` its tokens carry. */
  issuer: string;
  /** The `aud` its tokens must hold. */
  audience: string;
  /** Where its signing keys are published: `https`, or `http` to a loopback address. */
  jwks_url: string;
  /** Each turns a claim's values into terms, such as `{claim: 'groups[*]', template: '{value}'}`. */
  claim_rules?: {claim: string; template: string}[];
  /** Each gives an identity whose `claim` holds exactly `value` the terms and permissions of the local `group`. */
  role_mappings?: {claim: string; value: string; group: string}[];
};

/**
 * An OIDC provider as the catalogue lists it.
 *
 * @category Control plane
 */
export type ProviderRecord = Required<ProviderDeclaration> & {
  /** The provider's name. */
  name: string;
  /** `true` if it is declared in `mosaica.toml`, so the API cannot change or remove it. */
  read_only: boolean;
};

/**
 * A live session.
 *
 * @category Control plane
 */
export type SessionRecord = {
  /** The session's id, which `endSessions` takes. It cannot be used as a credential. */
  token_id: number;
  /** The local principal, or `null` for an OIDC identity. */
  principal: string | null;
  /** The provider an OIDC identity was accepted by, or `null`. */
  provider: string | null;
  /** The OIDC identity's `sub`, or `null`. */
  subject: string | null;
  /** The prefix of the API key it was authorised with, or `null`. */
  api_key: string | null;
  /** The principal that started it with `authorise-as`, or `null`. */
  minted_by: string | null;
  /**
   * What the session may do: `read`, and `write` if its principal holds it. A session from `login`
   * also carries `read-all` and `write-all` if the principal holds them.
   */
  permissions: Permission[];
  /** When it started, in seconds since the Unix epoch. */
  created_at: number;
  /** When it ends at the latest, in seconds since the Unix epoch. */
  expires_at: number;
};

/**
 * Which sessions `endSessions` ends: one session by id, or every session of a principal or a
 * provider.
 *
 * @category Control plane
 */
export type SessionsToEnd = {token_id: number} | {principal: string} | {provider: string};

/**
 * Options for a {@link Control}.
 *
 * @category Control plane
 */
export type ControlOptions = {
  /** The control listener's base URL. A trailing slash is ignored. */
  controlUrl: string;
  /**
   * The credential sent as the bearer token on every request: the operator credential, an API key
   * or an OIDC access token. Writes, `flush` and `compact` need `write`. Declarations and drops
   * need `write-all` as well. `status` and the catalogue methods need `admin`. A writer without
   * `write-all` writes only within its terms. If it is not set, no credential is sent, which only
   * an open server (`serve.open`) accepts.
   */
  credential?: string;
  /** A `fetch` to use in place of the global one. */
  fetch?: typeof fetch;
  /**
   * Headers to send on every request. The headers a method sets itself take precedence:
   * `authorization`, `content-type`, and on `ingest` `x-mosaica-batch-id` and `x-mosaica-view`.
   */
  headers?: Record<string, string>;
};

/**
 * Options every {@link Control} method takes.
 *
 * @category Control plane
 */
export type CallOptions = {
  /**
   * Aborts the call, during a request or a wait between attempts. The call then rejects with the
   * signal's reason.
   */
  signal?: AbortSignal;
};

/**
 * Options for a write.
 *
 * @category Control plane
 */
export type WriteOptions = CallOptions & {
  /**
   * Wait until the write is visible to viewers before answering. The server brings the next
   * publication forward and holds its answer until then, or until `serve.visible_wait_max_secs`
   * passes; the answer's `visible` says which. Sent as `wait=visible`. Defaults to `false`.
   */
  wait?: boolean;
};

/**
 * Options for a write whose rows name items: `ingest`, `changes`, `publish` and `grow`.
 *
 * @category Control plane
 */
export type StrictOptions = WriteOptions & {
  /**
   * `true` refuses the whole request at its first refused row, and nothing is written. Otherwise
   * the server applies the rows it accepts and lists each refused one in the answer's `refused`.
   */
  strict?: boolean;
};

/**
 * Options for `ingest`.
 *
 * @category Control plane
 */
export type RowOptions = StrictOptions & {
  /**
   * The batch id, sent in `x-mosaica-batch-id`. Defaults to a new random id for each call. Pass
   * the `batch` of an earlier {@link RowAnswer} to send its body again.
   */
  batch?: string;
  /**
   * The view the rows belong to, sent in `x-mosaica-view`. Required if the database has more than
   * one view. An unknown view is refused with `404`.
   */
  view?: string;
};

/** A fresh batch id: 128 random bits in hex. */
function freshBatch(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** One path segment, percent-encoded, so a name holding `/` reaches the router whole. */
const segment = encodeURIComponent;

/** A path with its query, where the query has anything in it. */
function withQuery(path: string, query: Record<string, string | undefined>): string {
  const params = new URLSearchParams();
  for (const [name, value] of Object.entries(query)) if (value !== undefined) params.set(name, value);
  const text = params.toString();
  return text ? `${path}?${text}` : path;
}

const waiting = (options: WriteOptions) => ({wait: options.wait ? 'visible' : undefined});

const naming = (options: StrictOptions) => ({...waiting(options), strict: options.strict === undefined ? undefined : String(options.strict)});

function decoded(text: string): Record<string, unknown> {
  try {
    const value: unknown = JSON.parse(text);
    if (value !== null && typeof value === 'object' && !Array.isArray(value)) return value as Record<string, unknown>;
    return {value};
  } catch {
    return {};
  }
}

/**
 * The wait a refusal asks for before the request is sent again, in seconds, or `null` where it is
 * not a `429` and names no wait. The `Retry-After` header comes first, then the body's
 * `retry_after_s`, then {@link MIN_BACKOFF}, and the wait is never less than {@link MIN_BACKOFF}.
 *
 * @internal
 */
export function retryAfterOf(status: number, header: string | null, body: unknown): number | null {
  const named = header ?? (body !== null && typeof body === 'object' ? (body as {retry_after_s?: unknown}).retry_after_s : undefined) ?? null;
  if (named === null && status !== 429) return null;
  const seconds = Number(named);
  return Number.isFinite(seconds) && seconds > MIN_BACKOFF ? seconds : MIN_BACKOFF;
}

/** Resolves after `seconds`, or rejects with the signal's reason when it aborts first. */
function pause(seconds: number, signal: AbortSignal | undefined): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) return reject(signal.reason);
    const abort = () => {
      clearTimeout(timer);
      reject(signal!.reason);
    };
    const timer = setTimeout(() => {
      signal?.removeEventListener('abort', abort);
      resolve();
    }, seconds * 1000);
    signal?.addEventListener('abort', abort, {once: true});
  });
}

/**
 * A client for the server's control plane, which changes what a database holds and who may see
 * it. It has one method per control route: declaring columns, views and layers; ingesting,
 * deleting and suppressing items; publishing annotations; flushing and compacting; and managing
 * principals, keys, groups, grants, OIDC providers and sessions. It sends one credential
 * ({@link ControlOptions.credential}) and keeps no state between calls. Request bodies are sent as
 * you give them, in the route's own field names: an Arrow IPC stream for `ingest`, either Arrow or
 * JSON for `grow`, and JSON for everything else.
 *
 * Every method resolves to an {@link Answer}, including refusals and requests that reached no
 * server. A call rejects only if its `signal` aborts, with the signal's reason.
 *
 * Writes become visible to viewers at the next publication. The server publishes every
 * `ingest.flush_max_age_secs` (90 seconds by default), or sooner once `ingest.flush_max_items`
 * rows (40,000 by default) are buffered. Deletions and suppressions apply at once.
 * {@link Control.flush} and the `wait` option bring the next publication forward.
 *
 * A busy server answers `429`. The call then waits as long as the server asks, between 1 and 30
 * seconds, and tries again, up to five minutes of waiting in all. After that it returns the `429`.
 * A request that reaches no server is returned with status {@link UNANSWERED}. `ingest` first tries
 * once more a second later.
 *
 * `ingest` sends a batch id with each request: your `batch`, or a new random id. The server
 * remembers each accepted batch id with its body. The same body sent again under that id is
 * answered as a replay, with `replayed: true` and no effect, and a different body under it is
 * refused with `409`. The same rows sent in two calls under different ids are processed twice. A
 * row that names its item by `mosaica_id` or a unique value then finds, in the second call, the
 * item the first created, and changes nothing.
 *
 * The catalogue methods need `admin`. Each catalogue change ends the live sessions it could
 * affect, and answers how many in `sessions_ended`.
 *
 * @category Control plane
 */
export class Control {
  private readonly base: string;
  private readonly credential: string | undefined;
  private readonly fetch: typeof fetch | undefined;
  private readonly headers: Record<string, string> | undefined;

  constructor(options: ControlOptions) {
    this.base = options.controlUrl.replace(/\/+$/, '');
    this.credential = options.credential;
    this.fetch = options.fetch;
    this.headers = options.headers;
  }

  private async send(
    method: string,
    path: string,
    options: CallOptions,
    body?: string | Uint8Array,
    headers: Record<string, string> = {},
    replayable = false
  ): Promise<Answer> {
    const url = this.base + path;
    // The route's own headers replace a host header of the same name in any case.
    const all = new Headers(this.headers);
    if (this.credential) all.set('authorization', `Bearer ${this.credential}`);
    for (const [name, value] of Object.entries(headers)) all.set(name, value);
    // An open server refuses a write with no content type, so a route without a body names JSON.
    if (method !== 'GET' && !all.has('content-type')) all.set('content-type', JSON_TYPE);
    const init: RequestInit = {method, headers: all};
    if (body !== undefined) init.body = body as BodyInit;
    if (options.signal) init.signal = options.signal;
    const started = performance.now();
    const seconds = () => (performance.now() - started) / 1000;
    let attempts = 0;
    let waited = 0;
    let resent = false;
    for (;;) {
      attempts += 1;
      let response: Response;
      let text: string;
      try {
        response = await (this.fetch ?? fetch)(url, init);
        text = await response.text();
      } catch (error) {
        if (options.signal?.aborted) throw options.signal.reason;
        // A request carrying a batch id is safe to send once more: the server answers a body it
        // already accepted under the id as a replay.
        if (replayable && !resent) {
          resent = true;
          await pause(retryAfterOf(429, null, null)!, options.signal);
          continue;
        }
        return {status: UNANSWERED, ok: false, body: {}, text: `${url} did not answer: ${String(error)}`, attempts, seconds: seconds()};
      }
      const answered = decoded(text);
      const wait = response.status === 429 ? Math.min(retryAfterOf(429, response.headers.get('retry-after'), answered)!, MAX_BACKOFF) : 0;
      if (response.status === 429 && waited + wait <= RETRY_BUDGET_S) {
        await pause(wait, options.signal);
        waited += wait;
        continue;
      }
      const ok = response.status >= 200 && response.status < 300;
      return {status: response.status, ok, body: answered, text, attempts, seconds: seconds()};
    }
  }

  private sendJson(method: string, path: string, body: unknown, options: CallOptions): Promise<Answer> {
    return this.send(method, path, options, JSON.stringify(body), {'content-type': JSON_TYPE});
  }

  private async rows(path: string, body: Uint8Array, options: RowOptions): Promise<RowAnswer> {
    const batch = options.batch ?? freshBatch();
    const headers: Record<string, string> = {'content-type': ARROW, 'x-mosaica-batch-id': batch};
    if (options.view !== undefined) headers['x-mosaica-view'] = options.view;
    const answer = (await this.send('POST', withQuery(path, naming(options)), options, body, headers, true)) as Answer<IngestResponse>;
    return {...answer, batch};
  }

  /**
   * `GET /control/status`: the running database's state, as JSON in `body`. It includes the
   * publication counter, the write executor, caches, admission, compaction, shards and the write
   * `limits`. `overlay` counts the item deletions and suppressions that no compaction has removed
   * yet.
   */
  status(options: CallOptions = {}): Promise<Answer> {
    return this.send('GET', '/control/status', options);
  }

  /**
   * The `limits` object from {@link Control.status}: the caps each write route puts on rows, items
   * and body bytes, one object per route. `{}` if the status request failed.
   */
  async limits(options: CallOptions = {}): Promise<Record<string, unknown>> {
    const limits = (await this.status(options)).body.limits;
    return limits !== null && typeof limits === 'object' ? (limits as Record<string, unknown>) : {};
  }

  /**
   * `POST /control/ingest`: insert or update items from one page of rows, given as an Arrow IPC
   * stream. A row names an item by its `mosaica_id` and the values of any unique columns it
   * carries. A row that names no item creates one at its position. A row whose values all equal
   * what the item already stores changes nothing. A row that names an item with no row in this
   * view, and changes nothing else, adds the item to the view. Any other row edits the item, which
   * keeps its `mosaica_id`.
   *
   * A column left out keeps the item's stored value, and a null clears it. A row without
   * coordinates changes only the columns it carries. A row that only places its item in an
   * annotation changes the annotation, and the item counts as `unchanged`.
   *
   * A row is refused if its values name two items, its `mosaica_id` names no item, or it names an
   * item or sets a unique value that an earlier row of the page does. A refused row is listed in
   * `refused` with its reason, its `mosaica_id` in the answer is `null`, and the other rows are
   * stored. With `strict` the page is refused with `409` at the first refused row, and nothing is
   * stored. {@link IngestResponse} describes the answer.
   */
  ingest(body: Uint8Array, options: RowOptions = {}): Promise<RowAnswer> {
    return this.rows('/control/ingest', body, options);
  }

  /**
   * `PUT /control/layers`: declare one annotation layer. The answer is `201` with the layer's
   * `name` and `mosaica_id`, or `200` with the existing layer's if a layer of that name already
   * has exactly this declaration. A different declaration under an existing name, or one that
   * breaks the deployment's rules, is refused with `422` saying why. A layer past the last layer
   * space, or with more levels than a space holds, is refused with `409`. A dropped layer's name
   * may be declared again.
   */
  declareLayer(declaration: object, options: WriteOptions = {}): Promise<Answer> {
    return this.sendJson('PUT', withQuery('/control/layers', waiting(options)), declaration, options);
  }

  /**
   * `PUT /control/view_groups/{name}`: declare a view group, with no views yet. The answer is
   * `201` if the group is new and `200` if the name already has this declaration. A different
   * declaration under the name is refused with `409`.
   */
  declareViewGroup(name: string, body: object, options: WriteOptions = {}): Promise<Answer> {
    return this.sendJson('PUT', withQuery(`/control/view_groups/${segment(name)}`, waiting(options)), body, options);
  }

  /**
   * `PUT /control/attributes`: declare one attribute column, named in the body. Batches sent after
   * the answer may carry it. The answer is `201` if the column is new, `200` if the name already
   * has this declaration, and `409` if it has a different one. A column declared with
   * `render: true` is available on every point drawn. A point stored before the declaration has no
   * value for it until a batch gives it one.
   *
   * `unique: true` means no two items may hold the same value. It is the one field you can change
   * on an existing column. The answer then waits while the column's index is built, and is `409`,
   * naming up to ten of the values, if more than one item holds a value. `unique: false` drops the
   * index and keeps the values.
   */
  declareAttribute(body: object, options: WriteOptions = {}): Promise<Answer> {
    return this.sendJson('PUT', withQuery('/control/attributes', waiting(options)), body, options);
  }

  /**
   * `PUT /control/shards`: set the shard size, `{size}`. When the shard taking new items reaches
   * this size it is sealed and the next one opened. A size at or below that shard's current size
   * seals it at once. The answer is `201` if the size changed, `200` if it was already set, and
   * `422` for a size of zero.
   */
  declareShards(body: object, options: WriteOptions = {}): Promise<Answer> {
    return this.sendJson('PUT', withQuery('/control/shards', waiting(options)), body, options);
  }

  /**
   * `PUT /control/vocabularies/{name}`: declare a vocabulary, the set of values of a category
   * column. A closed vocabulary's values go in the body. The answer is `201` if the vocabulary is
   * new, `200` if the name already has this declaration, in which case the values are added as a
   * page, and `409` if it has a different one.
   */
  declareVocabulary(name: string, body: object, options: WriteOptions = {}): Promise<Answer> {
    return this.sendJson('PUT', withQuery(`/control/vocabularies/${segment(name)}`, waiting(options)), body, options);
  }

  /**
   * `PATCH /control/vocabularies/{name}/values`: add one page of `{key, title?}` values to a
   * vocabulary. No value is removed, and a title given for an existing key replaces its title. An
   * unknown vocabulary is refused with `404`. For a `derived` vocabulary, the answer to a writer
   * without `write-all` leaves out `added`, `existing` and `titles`.
   */
  vocabularyValues(name: string, body: object, options: WriteOptions = {}): Promise<Answer> {
    return this.sendJson('PATCH', withQuery(`/control/vocabularies/${segment(name)}/values`, waiting(options)), body, options);
  }

  /**
   * `PUT /control/views/{name}`: declare one plain view, with no rows yet. The statuses are as for
   * {@link Control.declareViewGroup}.
   */
  declareView(name: string, body: object, options: WriteOptions = {}): Promise<Answer> {
    return this.sendJson('PUT', withQuery(`/control/views/${segment(name)}`, waiting(options)), body, options);
  }

  /**
   * `PUT /control/views/{group}/{key}`: create a view in `group` from the roster record in `body`.
   * An unknown group is refused with `404`. If a live view already holds the key with this record,
   * the answer is `200` with that view. With a different record it is `409`. A dropped view's key
   * may be used again, and the new view starts empty.
   */
  createView(group: string, key: string, body: object, options: WriteOptions = {}): Promise<Answer> {
    return this.sendJson('PUT', withQuery(`/control/views/${segment(group)}/${segment(key)}`, waiting(options)), body, options);
  }

  /**
   * `PUT /control/layers/{name}/artifacts`: publish artifacts into one level of a layer, each with
   * a first page of members, and return each artifact's `mosaica_id`. Members, excluded items and
   * a content's `generated_from` are each a {@link MemberTable}. A `mosaica_id` in a member table
   * must be an item's; an artifact's or a layer's names nothing.
   *
   * A member that names no item, or two, is left out and listed in `refused`, and the rest are
   * published. With `strict` the request is refused at the first such member, with `404` if it
   * names no item and `409` if it names two. A `generated_from` member that names no item, or two,
   * refuses the request even without `strict`. A request that creates more artifacts than the
   * level's layer space has ids left for is refused with `409`. Any other refusal applies nothing.
   */
  publish(layer: string, body: PublishRequest, options: StrictOptions = {}): Promise<Answer<ArtifactsPublished>> {
    const path = withQuery(`/control/layers/${segment(layer)}/artifacts`, naming(options));
    return this.sendJson('PUT', path, body, options) as Promise<Answer<ArtifactsPublished>>;
  }

  /**
   * `PATCH /control/layers/{name}/artifacts`: add a page of members to artifacts the layer already
   * holds, or with a `rank` change a content's generating set, and fill in parts the artifacts
   * lack. Refused members of `members` and `leaving` are listed in `refused` as for `publish`, or
   * with `strict` refuse the request. A part that differs from the one held is refused with `409`,
   * and that refusal applies nothing.
   *
   * @param body - A `Uint8Array` is sent as an Arrow IPC stream, anything else as JSON. The Arrow
   *   form has one row per artifact: `key` as `utf8`, `members` as a list of structs whose fields
   *   are the member table's columns, and optionally `view` and `access`, with `level` in the
   *   schema's metadata.
   */
  grow(layer: string, body: GrowRequest | Uint8Array, options: StrictOptions = {}): Promise<Answer<MembershipsGrown>> {
    const path = withQuery(`/control/layers/${segment(layer)}/artifacts`, naming(options));
    const sent = body instanceof Uint8Array ? this.send('PATCH', path, options, body, {'content-type': ARROW}) : this.sendJson('PATCH', path, body, options);
    return sent as Promise<Answer<MembershipsGrown>>;
  }

  /**
   * `POST /control/changes`: delete, suppress or unsuppress items, each named by its `match`, and
   * artifacts and layers, each named by its `mosaica_id`. A deletion or suppression applies to
   * every request from the moment it is accepted, without waiting for a publication.
   *
   * A change that names no item, or two, is listed in `refused` and the rest are applied. With
   * `strict` the request is refused at the first such change, with `404` if it names no item and
   * `409` if it names two. Deleting a layer here is refused with `422`; use
   * {@link Control.dropLayer}. Every change is checked before any is applied, so any other refusal
   * applies nothing.
   */
  changes(items: ChangeItem[], options: StrictOptions = {}): Promise<Answer<ChangesApplied>> {
    return this.sendJson('POST', withQuery('/control/changes', naming(options)), items, options) as Promise<Answer<ChangesApplied>>;
  }

  /**
   * `DELETE /control/layers/{name}`: drop a layer that `declareLayer` declared. The name may be
   * declared again, as a new layer that none of the old layer's `mosaica_id`s name.
   */
  dropLayer(name: string, options: WriteOptions = {}): Promise<Answer> {
    return this.send('DELETE', withQuery(`/control/layers/${segment(name)}`, waiting(options)), options);
  }

  /**
   * `DELETE /control/views/{group}/{key}`: drop a view that `createView` made, and free its key.
   * Items left with no row in any view are deleted, and the answer's `deleted` says how many.
   */
  dropView(group: string, key: string, options: WriteOptions = {}): Promise<Answer> {
    const path = withQuery(`/control/views/${segment(group)}/${segment(key)}`, waiting(options));
    return this.send('DELETE', path, options);
  }

  /**
   * `POST /control/compact`: request a compaction, which removes deleted rows from disc. The answer
   * is `202`, sent before the compaction runs, with the request's number in `body.request`. The
   * server compacts each shard that has something to reclaim, one at a time. A request made while a
   * compaction is running waits for it to finish.
   *
   * A compaction can be refused when its turn comes, for example for lack of disc space, or it can
   * fail. To follow a request, read `compaction.requests` in {@link Control.status}. `answered`
   * reaches your request's number once it has been answered. If `last_refusal.request` is then
   * your number, a shard was refused while answering it, and `detail` says why and what to do.
   * Below your number, none of its shards was refused. Above it, a later request's refusal has
   * replaced the record and may or may not concern yours. To confirm the deleted rows are gone,
   * check that `compaction.folds` rose and `overlay.retirable` fell; `compaction.fold_failures`
   * counts compactions that failed or were discarded. If no compaction can start until the server
   * restarts, the answer is `503` with that detail, and no compaction is requested.
   */
  compact(options: CallOptions = {}): Promise<Answer> {
    return this.send('POST', '/control/compact', options);
  }

  /**
   * `POST /control/flush`: bring the next publication forward. The answer is `202`, and its
   * `publication` names the first publication that includes every write sent before the flush.
   * With `wait`, the answer is held until that publication has happened.
   */
  flush(options: WriteOptions = {}): Promise<Answer> {
    return this.send('POST', withQuery('/control/flush', waiting(options)), options);
  }

  // The catalogue's verbs. Each needs `admin`. A change answers how many live sessions it ended:
  // every session of every principal, key or provider whose terms or permissions it could change.

  /** `GET /control/principals`: every local principal, in `body.principals`. */
  listPrincipals(options: CallOptions = {}): Promise<Answer<{principals: PrincipalRecord[]}>> {
    return this.send('GET', '/control/principals', options) as Promise<Answer<{principals: PrincipalRecord[]}>>;
  }

  /** `GET /control/principals/{name}`: one principal, or `404`. */
  showPrincipal(name: string, options: CallOptions = {}): Promise<Answer<PrincipalRecord>> {
    return this.send('GET', `/control/principals/${segment(name)}`, options) as Promise<Answer<PrincipalRecord>>;
  }

  /** `POST /control/principals`: create a principal with no password, key, term or permission. A name already in use is refused with `409`. */
  createPrincipal(name: string, kind: 'person' | 'service', options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.sendJson('POST', '/control/principals', {name, kind}, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `PATCH /control/principals/{name}`: disable or enable a principal. */
  changePrincipal(name: string, change: PrincipalChange, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.sendJson('PATCH', `/control/principals/${segment(name)}`, change, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `DELETE /control/principals/{name}`: delete a principal with its password, keys, grants and memberships. */
  deletePrincipal(name: string, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.send('DELETE', `/control/principals/${segment(name)}`, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `PUT /control/principals/{name}/password`: set a principal's password. A password shorter than the configured minimum is refused with `422`. */
  setPassword(name: string, password: string, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.sendJson('PUT', `/control/principals/${segment(name)}/password`, {password}, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `DELETE /control/principals/{name}/password`: remove a principal's password. */
  clearPassword(name: string, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.send('DELETE', `/control/principals/${segment(name)}/password`, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `GET /control/principals/{name}/keys`: a principal's API keys, without their secrets, in `body.keys`. */
  listKeys(principal: string, options: CallOptions = {}): Promise<Answer<{keys: KeyRecord[]}>> {
    return this.send('GET', `/control/principals/${segment(principal)}/keys`, options) as Promise<Answer<{keys: KeyRecord[]}>>;
  }

  /** `POST /control/principals/{name}/keys`: issue an API key. The whole key is returned only in this answer. */
  createKey(principal: string, body: KeyCreate = {}, options: CallOptions = {}): Promise<Answer<IssuedKey>> {
    return this.sendJson('POST', `/control/principals/${segment(principal)}/keys`, body, options) as Promise<Answer<IssuedKey>>;
  }

  /** `DELETE /control/keys/{prefix}`: revoke an API key. Every session started with it ends. */
  revokeKey(prefix: string, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.send('DELETE', `/control/keys/${segment(prefix)}`, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `GET /control/groups`: every local group, in `body.groups`. */
  listGroups(options: CallOptions = {}): Promise<Answer<{groups: GroupRecord[]}>> {
    return this.send('GET', '/control/groups', options) as Promise<Answer<{groups: GroupRecord[]}>>;
  }

  /** `GET /control/groups/{name}`: one group, or `404`. */
  showGroup(name: string, options: CallOptions = {}): Promise<Answer<GroupRecord>> {
    return this.send('GET', `/control/groups/${segment(name)}`, options) as Promise<Answer<GroupRecord>>;
  }

  /** `POST /control/groups`: create a group. A name already in use is refused with `409`. */
  createGroup(name: string, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.sendJson('POST', '/control/groups', {name}, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `DELETE /control/groups/{name}`: delete a group with its grants and memberships. */
  deleteGroup(name: string, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.send('DELETE', `/control/groups/${segment(name)}`, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `PUT /control/groups/{name}/members/{principal}`: add a principal to a group. */
  addMember(group: string, principal: string, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.send('PUT', `/control/groups/${segment(group)}/members/${segment(principal)}`, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `DELETE /control/groups/{name}/members/{principal}`: remove a principal from a group. */
  removeMember(group: string, principal: string, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.send('DELETE', `/control/groups/${segment(group)}/members/${segment(principal)}`, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `POST /control/grants`: grant terms or a permission to a principal or a group. A grant already held changes nothing. */
  grant(grant: Grant, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.sendJson('POST', '/control/grants', grant, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `POST /control/grants/revoke`: revoke terms or a permission. Revoking a grant that is not held changes nothing. */
  revokeGrant(grant: Grant, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.sendJson('POST', '/control/grants/revoke', grant, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `GET /control/providers`: every OIDC provider, declared through the API or in `mosaica.toml`, in `body.providers`. */
  listProviders(options: CallOptions = {}): Promise<Answer<{providers: ProviderRecord[]}>> {
    return this.send('GET', '/control/providers', options) as Promise<Answer<{providers: ProviderRecord[]}>>;
  }

  /** `GET /control/providers/{name}`: one provider, or `404`. */
  showProvider(name: string, options: CallOptions = {}): Promise<Answer<ProviderRecord>> {
    return this.send('GET', `/control/providers/${segment(name)}`, options) as Promise<Answer<ProviderRecord>>;
  }

  /**
   * `PUT /control/providers/{name}`: declare a provider, or replace one whole. Replacing a provider
   * ends every session authorised through it. A provider declared in `mosaica.toml` is refused with
   * `409`.
   */
  putProvider(name: string, declaration: ProviderDeclaration, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.sendJson('PUT', `/control/providers/${segment(name)}`, declaration, options) as Promise<Answer<CatalogueChange>>;
  }

  /** `DELETE /control/providers/{name}`: remove a provider. Every session authorised through it ends. */
  dropProvider(name: string, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.send('DELETE', `/control/providers/${segment(name)}`, options) as Promise<Answer<CatalogueChange>>;
  }

  /**
   * `GET /control/sessions`: live sessions, in `body.sessions`. Lists all of them, a local
   * principal's (including those started on its behalf), or those authorised through a provider.
   */
  listSessions(which: {principal?: string; provider?: string} = {}, options: CallOptions = {}): Promise<Answer<{sessions: SessionRecord[]}>> {
    return this.send('GET', withQuery('/control/sessions', which), options) as Promise<Answer<{sessions: SessionRecord[]}>>;
  }

  /** `POST /control/sessions/end`: end one session by `token_id`, or every session of a principal or a provider. */
  endSessions(which: SessionsToEnd, options: CallOptions = {}): Promise<Answer<CatalogueChange>> {
    return this.sendJson('POST', '/control/sessions/end', which, options) as Promise<Answer<CatalogueChange>>;
  }
}
