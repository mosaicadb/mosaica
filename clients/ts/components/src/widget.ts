import {
  CLUSTER_PREFIX,
  colourLayers,
  createStore,
  emptyDraft,
  memberKey,
  textQueryOf,
  withMembers,
  type ColumnDraft,
  type FilterDraft,
  type FilterExpr,
  type FilterOperandSet,
  type MemberClause,
  type PaletteName,
  type Store,
  type TokenSupplier
} from '@mosaicajs/client';
import type {Sizing} from '@mosaicajs/deck';
import {hexOf, isSingle} from '@mosaicajs/deck/internal';
import {artifactGroupings, listedGroups} from './aggregate.js';
import {idString} from './base.js';
import {colouringOf, setSizing, sizingOf} from './colouring.js';
import type {MosaicaExplorer} from './explorer.js';
import './explorer.js';

/**
 * The notebook widget's JavaScript half: what anywidget evaluates as `_esm`, exported from the
 * single-file bundle. The kernel half is `clients/py/mosaica/widget.py`.
 *
 * The token is not model state: no traitlet carries it, so saving widget state, `nbconvert
 * --execute` or papermill cannot write it into a notebook. `initialize` sends `ready` once per
 * model and the kernel answers with the token as a custom message. Later calls to the token
 * supplier (renewal before expiry, or after a refusal that ends the session) send `reauthorise`.
 * A page reload makes a new model, which sends `ready` again.
 *
 * `height` is the map's height in the notebook's flow. While the output holding the map is shown full
 * screen, the map fills the room below its top instead, and takes `height` again on the way out.
 *
 * Only controls and selections cross the kernel boundary. `url`, `explorer_layout`, `height`,
 * `title_field`, `artifacts_per_tile`, `budget_min`, `budget_max`, `cluster_budget_min` and `cluster_budget_max` come down; `view`, `bbox`, `layers`, `colour_by`, `palette`, `size_by`, `size_min`,
 * `size_max`, `size_scale` and `filters` go both ways, and so do `budget`, sent up when Most points is let go,
 * `cluster_budget`, sent up when Most clusters is let go, `palette`, sent up when one is chosen, and
 * `value_colours` and `cluster_colours`, sent up as each colour is chosen on a card or in Edit
 * colours; `selected`, `selected_artifact` and `region` go up. Up-syncs happen at the settle (a new
 * composition shown, or a region's counts), not per frame. Ids cross as decimal strings, since a
 * `mosaica_id` is a `u64`.
 *
 * The token is per model; the store and camera are per view, since a store has one view input.
 * Each view builds its own store over the shared supplier, which returns a held token without a
 * round trip. Up-syncs come from the active view, the one whose camera last moved; down-syncs go
 * to every view.
 *
 * The basemap comes down as `_basemap`: the value, in which an image named by a local path is
 * `{local: path}`, and the images it names that way, each with the number of the kernel's read of
 * the file. The two arrive in one update, so the page never
 * hands the explorer a path. Whenever the listing names reads the page does not hold, the page asks
 * for them with `basemap-images`, and the kernel answers with a `basemap-image` message per read,
 * carrying its bytes; one for a read not listed is dropped. The page makes an object URL of each
 * and gives the explorer that URL in the path's place. A path still listed keeps its URL until its
 * new read arrives, and the explorer is given `none` while a listed path has none at all. An
 * object URL is revoked when its path's new read arrives or the path is no longer listed, and
 * when the model is torn down. A basemap the map refuses goes to the kernel as an `error` message.
 *
 * Backbone fires `change:<key>` for a `model.set` made here as well as one from the kernel, so
 * each up-sync sets a flag the down-sync handlers check. Without it a `bbox` from Python would fit
 * the camera, the settle would report a box of a different aspect, and that would fit the camera
 * again, without end.
 */

/** The part of anywidget's `AnyModel` this module uses, typed here so a test can fake it. */
export type WidgetModel = {
  get(key: string): unknown;
  set(key: string, value: unknown): void;
  save_changes(): void;
  on(event: string, cb: (...args: unknown[]) => void): void;
  off(event: string, cb?: (...args: unknown[]) => void): void;
  send(content: unknown, callbacks?: unknown, buffers?: ArrayBuffer[]): void;
};

/** What the kernel sends the page. `expires_at` is seconds since the epoch, as `/v1/login` and `/session/authorise` report it, or `null` when unknown. */
export type KernelMessage =
  | {type: 'token'; token: string; expires_at: number | null}
  | {type: 'refused'; detail: string}
  | {type: 'basemap-image'; path: string; read: number; mime: string};

/** What the page sends the kernel. */
export type PageMessage = {type: 'ready'} | {type: 'reauthorise'} | {type: 'basemap-images'; reads: Record<string, number>} | {type: 'error'; what: string; detail: string};

type ViewState = {
  explorer: MosaicaExplorer;
  store: Store | null;
  unsubscribe: (() => void) | null;
  /** The box this view's camera last reported, in data coordinates, which its settle syncs up. */
  lastBbox: [number, number, number, number] | null;
};

type ModelState = {
  supplier: TokenSupplier;
  storeFactory: StoreFactory;
  views: Map<MosaicaExplorer, ViewState>;
  /** The view whose settles sync up: the last one mounted or moved. */
  active: ViewState | null;
  /**
   * A `bbox` the kernel set before any map could fit it (no view, or no meta yet); fitted at the
   * first chance. A kernel's box is fitted to fill the map along its limiting side, with no margin,
   * so a box the widget wrote back reopens on the same camera after a reload.
   */
  pendingFit: [number, number, number, number] | null;
  syncingUp: boolean;
  /** The object URL of each local image's bytes, by the path `_basemap` names it by, with the read they came from. */
  images: Map<string, {read: number; url: string}>;
  dispose(): void;
};

const states = new WeakMap<object, ModelState>();

/** For a test: the state behind a model, once `initialize` has run. */
export function stateOf(model: object): {stores: (Store | null)[]; views: number} | null {
  const s = states.get(model);
  return s ? {stores: [...s.views.values()].map((v) => v.store), views: s.views.size} : null;
}

/**
 * The token supplier over the comm, shared by every view of the model. A held token not about to
 * expire is returned without a round trip. Otherwise one request is outstanding at a time
 * (`ready` first, then `reauthorise`), settled by the next `token` or `refused` message, and
 * concurrent calls share it. A token with `expires_at: null` is not renewed early; when the
 * server refuses it the store reports `expired`.
 */
function tokenSupplier(model: WidgetModel, onMessage: (cb: (msg: KernelMessage) => void) => void): TokenSupplier {
  type Got = {token: string; expiresAt: number};
  let outstanding: {resolve(v: Got): void; reject(e: Error): void} | null = null;
  let asked = false;
  let held: Got | null = null;
  onMessage((msg) => {
    if (!outstanding || (msg.type !== 'token' && msg.type !== 'refused')) return;
    const o = outstanding;
    outstanding = null;
    if (msg.type === 'token') {
      held = {token: msg.token, expiresAt: msg.expires_at === null ? Number.POSITIVE_INFINITY : msg.expires_at};
      o.resolve(held);
    } else if (msg.type === 'refused') {
      o.reject(new Error(msg.detail));
    }
  });
  let inflight: Promise<Got> | null = null;
  return () => {
    if (held && (held.expiresAt === Number.POSITIVE_INFINITY || Date.now() / 1000 < held.expiresAt - 60)) return Promise.resolve(held);
    if (inflight) return inflight;
    inflight = new Promise<Got>((resolve, reject) => {
      outstanding = {resolve, reject};
      model.send(asked ? {type: 'reauthorise'} : {type: 'ready'});
      asked = true;
    }).finally(() => {
      inflight = null;
    });
    return inflight;
  };
}

/** The one column every leaf under `expr` names, or `null` where they name several or none. */
function columnOf(expr: FilterExpr): string | null {
  const keys = Object.keys(expr);
  if (keys.length !== 1) return null;
  const key = keys[0]!;
  if (key === 'none_of') return null;
  if (key !== 'all_of' && key !== 'any_of') return key;
  const inner = (expr as Record<string, FilterExpr[]>)[key]!;
  const columns = new Set(inner.map(columnOf));
  return columns.size === 1 && !columns.has(null) ? [...columns][0]! : null;
}

/** Whether a `member_of` leaf is anywhere in `expr`. */
function holdsMemberOf(expr: FilterExpr): boolean {
  return Object.entries(expr).some(([key, value]) => key === 'member_of' || ((key === 'all_of' || key === 'any_of' || key === 'none_of') && Array.isArray(value) && value.some(holdsMemberOf)));
}

/** One `member_of` leaf, or one wrapped alone in `none_of`, as the clause the store holds of it. */
function memberClauseOf(part: FilterExpr): MemberClause | null {
  const outside = 'none_of' in part && Array.isArray(part.none_of) && part.none_of.length === 1;
  const leaf = (outside ? (part.none_of as FilterExpr[])[0]! : part) as Record<string, unknown>;
  if (Object.keys(leaf).length !== 1 || !('member_of' in leaf)) return null;
  const operand = leaf.member_of as Record<string, unknown>;
  const {layer, artifact} = operand;
  if (artifact !== undefined && Object.keys(operand).some((k) => k !== 'layer' && k !== 'artifact')) {
    throw new Error(`member_of names its artifact by its mosaica_id alone, with layer and artifact and nothing else; got ${JSON.stringify(operand)}`);
  }
  if (artifact === undefined) {
    throw new Error(
      `member_of on ${JSON.stringify(layer)} names no artifact by its mosaica_id, and the map holds a member_of filter by mosaica_id alone; give the artifact's mosaica_id, which browse_artifacts lists`
    );
  }
  const id = typeof artifact === 'number' && Number.isSafeInteger(artifact) ? String(artifact) : artifact;
  if (typeof layer !== 'string' || typeof id !== 'string' || !/^[0-9]+$/.test(id)) {
    throw new Error(`member_of takes a layer's name and an artifact's mosaica_id as a decimal string; got ${JSON.stringify(operand)}`);
  }
  return {layer, artifact: BigInt(id), outside, verb: 'filter'};
}

/**
 * The `filters` traitlet's expression split into the `member_of` clauses the store holds apart from
 * its controls and the rest, for {@link draftOf}. A clause is a `member_of` leaf, or one wrapped
 * alone in `none_of`, at the top of the expression or in its top `all_of`, which is where
 * `withMembers` puts the store's clauses when the up-sync composes them back beside the controls'
 * expression.
 */
export function membersOf(expr: FilterExpr | null): {rest: FilterExpr | null; members: MemberClause[]} {
  if (expr === null) return {rest: null, members: []};
  const parts = 'all_of' in expr && Array.isArray(expr.all_of) ? (expr.all_of as FilterExpr[]) : [expr];
  const members: MemberClause[] = [];
  const rest: FilterExpr[] = [];
  for (const part of parts) {
    const clause = memberClauseOf(part);
    if (clause) members.push(clause);
    else rest.push(part);
  }
  if (members.length === 0) return {rest: expr, members};
  return {rest: rest.length === 0 ? null : rest.length === 1 ? rest[0]! : {all_of: rest}, members};
}

/** The prefix of the ids under which the widget asks a layer for the names of its clauses' artifacts. */
const NAMES = 'widget-member-names:';

/**
 * Ask for the names of the artifacts `clauses` name without a label, one
 * aggregate per layer, as a cluster card asks for its rows' names: each artifact is named at every
 * level of its layer, and the level it is on answers with its name. A clause a card made carries
 * its name already; one from the kernel names its artifact by `mosaica_id` alone.
 */
export function askMemberNames(store: Store, clauses: readonly MemberClause[]): void {
  const meta = store.get('meta');
  if (!meta) return;
  const wanted = new Map<string, bigint[]>();
  for (const c of clauses) if (c.label === undefined) wanted.set(c.layer, [...(wanted.get(c.layer) ?? []), c.artifact]);
  for (const [name, ids] of wanted) {
    const layer = meta.layers.find((l) => l.name === name);
    if (!layer) continue;
    const levels = layer.levels.length > 0 ? layer.levels.map((l) => l.level) : [0];
    const rows = ids.flatMap((mosaicaId) => levels.map((rung) => ({mosaicaId, rung})));
    store.setAggregate(NAMES + name, {groupings: artifactGroupings(layer, rows, meta.selection), subject: 'visible'});
  }
}

/**
 * Give each unlabelled `member_of` clause the name {@link askMemberNames} found for its artifact,
 * and drop each request once it has answered or been refused. A clause left unnamed reads as a
 * clicked unnamed cluster's does.
 */
export function nameMembers(store: Store): void {
  const titles = new Map<string, string>();
  const answered: string[] = [];
  for (const [id, entry] of store.get('aggregates')) {
    if (!id.startsWith(NAMES) || (entry.status !== 'shown' && entry.status !== 'refused')) continue;
    // A refused read is dropped too, so that the store does not send it again at every requery.
    answered.push(id);
    const layer = id.slice(NAMES.length);
    for (const t of entry.result?.tables ?? []) for (const g of listedGroups(t)) if (g.title) titles.set(memberKey(layer, BigInt(g.key)), g.title);
  }
  if (answered.length === 0) return;
  // Called from the store's own publication, so the store is changed after it, in a microtask.
  queueMicrotask(() => {
    for (const id of answered) if (store.get('aggregates').has(id)) store.setAggregate(id, null);
    const members = store.get('filters').members;
    const named = members.map((c) => {
      const title = c.label === undefined ? titles.get(memberKey(c.layer, c.artifact)) : undefined;
      return title === undefined ? c : {...c, label: title};
    });
    if (named.some((c, i) => c !== members[i])) store.setMembers(named);
  });
}

/**
 * The store's draft for the `filters` traitlet's expression, which is in the wire's form, with every
 * control in the `filter` position and none in `highlight`. This inverts `composeFilters`: the
 * expression is an `all_of` over the columns, or one column's expression alone, and each column
 * holds one leaf, except a text column, which may hold any expression on itself. A text column's
 * expression becomes the query that writes it where there is one, and is otherwise kept as sent
 * and sent back as it is. Anything else (`none_of`, an `any_of` across columns, two leaves on
 * another column, an unknown column, an operator the column's family cannot hold) throws with a
 * reason, which goes to the kernel as an `error` message. `null` is no filter.
 */
export function draftOf(expr: FilterExpr | null, operands: FilterOperandSet[]): FilterDraft {
  const draft = emptyDraft(operands);
  if (expr === null) return draft;
  const top: FilterExpr[] = columnOf(expr) === null && 'all_of' in expr && Array.isArray(expr.all_of) ? (expr.all_of as FilterExpr[]) : [expr];
  const byColumn = new Map<string, FilterExpr[]>();
  for (const part of top) {
    const column = columnOf(part);
    if (holdsMemberOf(part)) {
      throw new Error(
        `the map holds a member_of filter at the top of the expression or in its top all_of, alone or alone in none_of; got ${JSON.stringify(part)}`
      );
    }
    if (column === null) throw new Error(`each part of a filter set from the widget names one column; got ${JSON.stringify(part)}`);
    byColumn.set(column, [...(byColumn.get(column) ?? []), part]);
  }
  for (const [column, parts] of byColumn) {
    const control = draft.filter[column];
    if (!control) throw new Error(`${column} is not a filterable column of this view`);
    if (control.family === 'text') {
      const held = parts.length === 1 ? parts[0]! : {all_of: parts};
      const query = textQueryOf(column, held, control.phrase);
      draft.filter[column] = query === null ? {...control, expr: held} : {...control, query};
      continue;
    }
    if (parts.length !== 1) throw new Error(`two leaves on ${column}; the widget holds one per column`);
    const leaf = parts[0]!;
    if (Object.keys(leaf)[0] !== column) throw new Error(`${column}: only a text column holds a combination of leaves`);
    draft.filter[column] = controlOf(column, control, (leaf as Record<string, unknown>)[column] as Record<string, unknown>);
  }
  return draft;
}

/** One leaf back into a control of the family `control` has, for a family other than text. */
function controlOf(column: string, control: Exclude<ColumnDraft, {family: 'text'}>, op: Record<string, unknown>): ColumnDraft {
  const names = Object.keys(op);
  if (names.length !== 1) throw new Error(`${column}: an operator has exactly one key; got ${names.join(', ')}`);
  const name = names[0]!;
  const value = op[name];
  const bad = () => new Error(`${column}: a ${control.family} column cannot hold ${name}`);
  switch (control.family) {
    case 'keyword': {
      if ((name === 'eq' || name === 'prefix' || name === 'contains') && typeof value === 'string') {
        return {family: control.family, needle: value, op: name};
      }
      throw bad();
    }
    case 'category': {
      if (name === 'in' && Array.isArray(value)) return {family: 'category', keys: value.map(String)};
      if (name === 'eq') return {family: 'category', keys: [String(value)]};
      throw bad();
    }
    case 'numeric': {
      if (name === 'range' && value && typeof value === 'object') {
        const r = value as {gte?: unknown; lte?: unknown; gt?: unknown; lt?: unknown};
        if (r.gt !== undefined || r.lt !== undefined) throw new Error(`${column}: the widget's range is inclusive (gte, lte)`);
        if ([r.gte, r.lte].some((bound) => bound !== undefined && typeof bound !== 'number')) {
          throw new Error(`${column}: the widget's range holds numbers; write each bound as a number, a timestamp's in microseconds since the Unix epoch`);
        }
        return {family: 'numeric', gte: (r.gte as number | undefined) ?? null, lte: (r.lte as number | undefined) ?? null};
      }
      throw bad();
    }
  }
}

function sameBbox(a: number[] | null, b: number[] | null): boolean {
  if (a === b) return true;
  if (!a || !b || a.length !== b.length) return false;
  return a.every((v, i) => v === b[i]);
}

/** How a store is built: `createStore`, unless a test injects one. */
export type StoreFactory = (options: {viewerUrl: string; authorise: TokenSupplier; view?: string; artifacts?: {perTile: number}}) => Store;

function buildStore(model: WidgetModel, state: ModelState): Store | null {
  const url = model.get('url');
  if (typeof url !== 'string' || !url) return null;
  const view = model.get('view');
  const perTile = model.get('artifacts_per_tile');
  return state.storeFactory({
    viewerUrl: url,
    authorise: state.supplier,
    ...(typeof view === 'string' && view ? {view} : {}),
    ...(typeof perTile === 'number' ? {artifacts: {perTile}} : {})
  });
}

function heightOf(model: WidgetModel): string {
  const h = model.get('height');
  return typeof h === 'number' ? `${h}px` : String(h || '480px');
}

/** Whether `inner` is `outer` or inside it, across shadow roots. */
function within(outer: Element, inner: Element): boolean {
  for (let at: Node | null = inner; at; at = at instanceof ShadowRoot ? at.host : at.parentNode) if (at === outer) return true;
  return false;
}

/**
 * Size `explorer`: `height` in the notebook's flow, and while an element around it is shown full
 * screen, as a notebook's output can be, the room from its top to the bottom of that element. The
 * whole page shown full screen leaves the notebook's flow as it is.
 */
function fitHeight(explorer: HTMLElement, model: WidgetModel): void {
  const full = typeof document === 'undefined' ? null : document.fullscreenElement;
  if (full && full !== document.documentElement && full !== document.body && within(full, explorer)) {
    const room = Math.min(full.getBoundingClientRect().bottom, innerHeight) - explorer.getBoundingClientRect().top;
    explorer.style.setProperty('--mosaica-explorer-height', `${Math.max(MIN_FULL_SCREEN, Math.floor(room))}px`);
  } else {
    explorer.style.setProperty('--mosaica-explorer-height', heightOf(model));
  }
}

/** The shortest the explorer is made while shown full screen, its own least height. */
const MIN_FULL_SCREEN = 320;

export function initialize({model, storeFactory = createStore}: {model: WidgetModel; storeFactory?: StoreFactory}): () => void {
  const listeners: ((msg: KernelMessage, buffers: readonly unknown[]) => void)[] = [];
  const onCustom = (...args: unknown[]) => {
    const msg = args[0] as KernelMessage;
    const buffers = Array.isArray(args[1]) ? (args[1] as unknown[]) : [];
    for (const l of listeners) l(msg, buffers);
  };
  model.on('msg:custom', onCustom);

  // A `bbox` the kernel held when the model was made is fitted as one it sets later is.
  const stated = model.get('bbox');
  const state: ModelState = {
    supplier: tokenSupplier(model, (cb) => listeners.push(cb)),
    storeFactory,
    views: new Map(),
    active: null,
    pendingFit: Array.isArray(stated) && stated.length === 4 ? (stated as [number, number, number, number]) : null,
    syncingUp: false,
    images: new Map(),
    dispose() {
      for (const v of state.views.values()) teardown(v);
      state.views.clear();
      state.active = null;
      for (const {url} of state.images.values()) URL.revokeObjectURL(url);
      state.images.clear();
      model.off('msg:custom', onCustom);
    }
  };
  states.set(model, state);

  // The local images: kept while `_basemap` lists them under the read they came from.
  const showBasemap = () => {
    for (const v of state.views.values()) applyBasemap(v.explorer, model, state.images);
  };
  /** The reads asked for and not yet arrived, as `<read>:<path>`, so none is asked for twice. */
  const requested = new Set<string>();
  listeners.push((msg, buffers) => {
    if (msg.type !== 'basemap-image' || basemapOf(model).images[msg.path] !== msg.read) return;
    if (state.images.get(msg.path)?.read === msg.read) return;
    const bytes = buffers[0];
    if (!(bytes instanceof ArrayBuffer || ArrayBuffer.isView(bytes))) return;
    const held = state.images.get(msg.path);
    if (held) URL.revokeObjectURL(held.url);
    state.images.set(msg.path, {read: msg.read, url: URL.createObjectURL(new Blob([bytes as BlobPart], {type: msg.mime}))});
    requested.delete(`${msg.read}:${msg.path}`);
    showBasemap();
  });
  /** Drop the images no longer listed, and ask for the reads listed, not held and not asked for. */
  const pullImages = () => {
    const listed = basemapOf(model).images;
    for (const [path, {url}] of state.images) {
      if (Object.hasOwn(listed, path)) continue;
      URL.revokeObjectURL(url);
      state.images.delete(path);
    }
    for (const pending of requested) {
      const at = pending.indexOf(':');
      if (listed[pending.slice(at + 1)] !== Number(pending.slice(0, at))) requested.delete(pending);
    }
    const reads = Object.fromEntries(Object.entries(listed).filter(([path, read]) => state.images.get(path)?.read !== read && !requested.has(`${read}:${path}`)));
    for (const [path, read] of Object.entries(reads)) requested.add(`${read}:${path}`);
    if (Object.keys(reads).length > 0) model.send({type: 'basemap-images', reads} satisfies PageMessage);
  };
  model.on('change:_basemap', () => {
    pullImages();
    showBasemap();
  });
  pullImages();

  const report = (what: string, e: unknown) => {
    model.send({type: 'error', what, detail: e instanceof Error ? e.message : String(e)} satisfies PageMessage);
  };

  // Up-sync: one `save_changes` per settle or region answer.
  const syncUp = (patch: Record<string, unknown>) => {
    state.syncingUp = true;
    try {
      let dirty = false;
      for (const [k, v] of Object.entries(patch)) {
        if (JSON.stringify(model.get(k)) === JSON.stringify(v)) continue;
        model.set(k, v);
        dirty = true;
      }
      if (dirty) model.save_changes();
    } finally {
      state.syncingUp = false;
    }
  };

  const applyFilters = (store: Store, expr: FilterExpr | null) => {
    const meta = store.get('meta');
    // Before `/v1/meta` there is no operand list to check against; `follow` applies the kernel's
    // expression when meta arrives.
    if (!meta) return;
    try {
      // The traitlet is the `filters` expression, so the highlight is kept, and so are the
      // highlight clauses beside the filter's `member_of` clauses.
      const {rest, members} = membersOf(expr);
      const draft = draftOf(rest, meta.filterOperands);
      const held = store.get('filters');
      // Each call fetches the map again, so a part the kernel left as it was is not set again.
      if (JSON.stringify(draft.filter) !== JSON.stringify(held.draft.filter)) store.setFilters({...draft, highlight: held.draft.highlight});
      // A clause the store holds already keeps the name it has, so that only new ones are asked for.
      const labels = new Map(held.members.filter((c) => c.verb === 'filter' && c.label !== undefined).map((c) => [memberKey(c.layer, c.artifact), c.label!]));
      const named = members.map((c) => {
        const label = labels.get(memberKey(c.layer, c.artifact));
        return label === undefined ? c : {...c, label};
      });
      store.setMembers([...held.members.filter((c) => c.verb !== 'filter'), ...named]);
      askMemberNames(store, named);
    } catch (e) {
      report('filters', e);
    }
  };

  /**
   * A `cluster:<layer>` naming no layer this view can colour by is reported, as a bad filter is;
   * the store still takes it and draws the points uniform. Checked once `/v1/meta` is in hand.
   */
  const checkColour = (store: Store, colourBy: unknown) => {
    const meta = store.get('meta');
    if (!meta || typeof colourBy !== 'string' || !colourBy.startsWith(CLUSTER_PREFIX)) return;
    const offered = colourLayers(meta.layers).map((l) => l.name);
    const layer = colourBy.slice(CLUSTER_PREFIX.length);
    if (offered.includes(layer)) return;
    const instead = offered.length === 0 ? 'this view has no layer to colour by' : `use one of ${offered.map((n) => CLUSTER_PREFIX + n).join(', ')}`;
    report('colour_by', new Error(`${layer} is not a layer this view can colour by; ${instead}`));
  };

  /** The size range and scale the kernel holds, shared with every element over `store`; `null` leaves the choice standing. */
  const applySizing = (store: Store) => {
    const patch: Partial<Sizing> = {};
    const min = model.get('size_min');
    const max = model.get('size_max');
    const scale = model.get('size_scale');
    if (typeof min === 'number') patch.min = min;
    if (typeof max === 'number') patch.max = max;
    if (typeof scale === 'string') patch.scale = scale as Sizing['scale'];
    setSizing(store, patch);
  };

  /** The column the kernel sizes by, `null` for one size, with a sample kept under rank. */
  const applySizeBy = (store: Store) => {
    const sizeBy = model.get('size_by');
    store.setSizeBy(typeof sizeBy === 'string' && sizeBy ? sizeBy : null, {rank: sizingOf(store).scale === 'rank'});
  };

  /** The cluster palette the kernel holds; `null` leaves the store's. */
  const applyPalette = (store: Store) => {
    const palette = model.get('palette');
    if (typeof palette === 'string' && palette) store.setPalette(palette as PaletteName);
  };

  // Down-sync: what the kernel holds, applied to one store (a new one, or one whose meta arrived).
  const applyControls = (store: Store) => {
    // `null` leaves the explorer's default; `[]` is none.
    const layers = model.get('layers');
    if (Array.isArray(layers)) store.setLayers(layers.map(String));
    const colourBy = model.get('colour_by');
    if (typeof colourBy === 'string' && colourBy) store.setColourBy(colourBy);
    applyPalette(store);
    applySizing(store);
    const sizeBy = model.get('size_by');
    if (typeof sizeBy === 'string' && sizeBy) applySizeBy(store);
    const filters = model.get('filters');
    if (filters !== null && filters !== undefined) applyFilters(store, filters as FilterExpr);
  };

  const fit = (bbox: [number, number, number, number]) => {
    let done = false;
    for (const v of state.views.values()) done = (v.explorer.map?.fitBbox(bbox, 1) ?? false) || done;
    state.pendingFit = done ? null : bbox;
  };

  /** Follow one view's store: its settles and answers sync up while it is the active view. */
  const follow = (v: ViewState, store: Store) => {
    let lastComposition: object | null = null;
    let lastRegion: object | null = null;
    let lastSelection: object | null = null;
    let metaSeen = false;
    v.unsubscribe = store.subscribe(() => {
      nameMembers(store);
      if (!metaSeen && store.get('meta')) {
        metaSeen = true;
        const filters = model.get('filters');
        if (filters !== null && filters !== undefined) applyFilters(store, filters as FilterExpr);
        checkColour(store, model.get('colour_by'));
        if (state.pendingFit) fit(state.pendingFit);
      }
      if (state.active !== v) return;
      const status = store.get('status');
      const view = store.get('view');
      const patch: Record<string, unknown> = {};
      // The settle: a new composition presented as `shown` or `empty`. The control traits are read
      // from the store, so the kernel holds what the store applied, whichever side set it.
      const settled = (status.status === 'shown' || status.status === 'empty') && view.composition !== lastComposition;
      if (settled) {
        lastComposition = view.composition;
        patch.bbox = v.lastBbox;
        patch.view = view.id;
        patch.layers = store.get('artifacts').layers;
        patch.colour_by = store.get('legend').colourBy;
        patch.palette = store.get('artifacts').palette;
        const sizing = sizingOf(store);
        patch.size_by = store.get('legend').sizeBy;
        patch.size_min = sizing.min;
        patch.size_max = sizing.max;
        patch.size_scale = sizing.scale;
        const {expr, members} = store.get('filters');
        patch.filters = withMembers(expr, members, 'filter');
      }
      const region = store.get('region');
      if (region !== lastRegion && (region === null || region.status !== 'loading')) {
        lastRegion = region;
        patch.region = region
          ? {
              shape: region.shape,
              status: region.status,
              visible: region.visible,
              matched: region.matched,
              served: region.served,
              verdict: region.verdict,
              refusal: region.refusal
            }
          : null;
      }
      const selection = store.get('selection');
      if (selection !== lastSelection) {
        lastSelection = selection;
        patch.selected = selection.item ? idString(selection.item.id) : null;
        patch.selected_artifact = selection.artifact ? idString(selection.artifact.id) : null;
      }
      if (Object.keys(patch).length) syncUp(patch);
    });
  };

  const teardown = (v: ViewState) => {
    v.unsubscribe?.();
    v.unsubscribe = null;
    v.store?.dispose();
    v.store = null;
    v.explorer.store = null;
  };

  /** Give a view a store, at mount and again when `url` changes. */
  const equip = (v: ViewState) => {
    teardown(v);
    v.store = buildStore(model, state);
    if (v.store) {
      follow(v, v.store);
      applyControls(v.store);
    }
    v.explorer.store = v.store;
  };

  const rebuildAll = () => {
    for (const v of state.views.values()) equip(v);
  };
  model.on('change:url', rebuildAll);
  model.on('change:artifacts_per_tile', rebuildAll);
  // A view change calls `setCurrentView` on each store; a store holds several views, so stepping
  // through a roster rebuilds nothing. A `url` change rebuilds, since a `mosaica_id` from one
  // bundle means nothing in another. A switch made in one view reaches only that view's store,
  // through the echo guard; one written from the kernel reaches every view.
  model.on('change:view', () => {
    if (state.syncingUp) return;
    const view = model.get('view');
    if (typeof view !== 'string' || !view) return;
    for (const v of state.views.values()) v.store?.setCurrentView(view);
  });
  model.on('change:layers', () => {
    if (state.syncingUp) return;
    const layers = model.get('layers');
    if (!Array.isArray(layers)) return;
    for (const v of state.views.values()) v.store?.setLayers(layers.map(String));
  });
  model.on('change:colour_by', () => {
    if (state.syncingUp) return;
    const c = model.get('colour_by');
    for (const v of state.views.values()) {
      if (!v.store) continue;
      v.store.setColourBy(typeof c === 'string' && c ? c : null);
      checkColour(v.store, c);
    }
  });
  model.on('change:palette', () => {
    if (state.syncingUp) return;
    for (const v of state.views.values()) if (v.store) applyPalette(v.store);
  });
  model.on('change:size_by', () => {
    if (state.syncingUp) return;
    for (const v of state.views.values()) if (v.store) applySizeBy(v.store);
  });
  for (const key of ['size_min', 'size_max', 'size_scale']) {
    model.on(`change:${key}`, () => {
      if (state.syncingUp) return;
      for (const v of state.views.values()) if (v.store) applySizing(v.store);
    });
  }
  model.on('change:filters', () => {
    if (state.syncingUp) return;
    const expr = (model.get('filters') as FilterExpr | null) ?? null;
    for (const v of state.views.values()) if (v.store) applyFilters(v.store, expr);
  });
  model.on('change:bbox', () => {
    if (state.syncingUp) return;
    const bbox = model.get('bbox');
    if (!Array.isArray(bbox) || bbox.length !== 4 || sameBbox(bbox as number[], state.active?.lastBbox ?? null)) return;
    fit(bbox as [number, number, number, number]);
  });
  model.on('change:height', () => {
    for (const v of state.views.values()) fitHeight(v.explorer, model);
  });
  model.on('change:title_field', () => {
    for (const v of state.views.values()) v.explorer.titleField = titleFieldOf(model);
  });
  for (const key of ['budget', 'budget_min', 'budget_max', 'cluster_budget', 'cluster_budget_min', 'cluster_budget_max']) {
    model.on(`change:${key}`, () => {
      if (state.syncingUp) return;
      for (const v of state.views.values()) applyBudget(v.explorer, model);
    });
  }
  for (const key of ['value_colours', 'cluster_colours']) {
    model.on(`change:${key}`, () => {
      if (state.syncingUp) return;
      for (const v of state.views.values()) applyColours(v.explorer, model);
    });
  }
  model.on('destroy', () => state.dispose());

  // What `render` calls, kept on the state so a test can mount through it.
  mounts.set(state, (explorer) => {
    const v: ViewState = {explorer, store: null, unsubscribe: null, lastBbox: null};
    state.views.set(explorer, v);
    state.active = v;
    equip(v);
    // Most points sets the store's budget; the kernel learns it as it is let go, not per frame.
    // The explorer's property follows, so the kernel setting the old budget back is a change.
    const onBudget = (e: Event) => {
      const {budget} = (e as CustomEvent<{budget: number}>).detail;
      explorer.budget = budget;
      syncUp({budget});
    };
    explorer.addEventListener('mosaica-budgetchange', onBudget);
    // Most clusters likewise.
    const onClusterBudget = (e: Event) => {
      const {budget} = (e as CustomEvent<{budget: number}>).detail;
      explorer.clusterBudget = budget;
      syncUp({cluster_budget: budget});
    };
    explorer.addEventListener('mosaica-clusterbudgetchange', onClusterBudget);
    // A palette chosen in the Colour section reaches the kernel at once.
    const onPalette = (e: Event) => syncUp({palette: (e as CustomEvent<{palette: PaletteName}>).detail.palette});
    explorer.addEventListener('mosaica-clusterpalettechange', onPalette);
    // A colour chosen for values or clusters reaches the kernel at once, as every colour the store
    // now holds, which the event's change is already in; the kernel's copy may be `None`, which
    // left the map's choices standing. The explorer's property follows, so a store it builds
    // later, or a view mounted after, starts with them.
    const onValueColour = () => {
      if (!v.store) return;
      const next = structuredClone(colouringOf(v.store).values) as Record<string, Record<string, string>>;
      explorer.valueColours = next;
      syncUp({value_colours: next});
    };
    const onClusterColour = () => {
      if (!v.store) return;
      const next = clusterColoursOf(v.store);
      explorer.clusterColours = next;
      syncUp({cluster_colours: next});
    };
    explorer.addEventListener('mosaica-valuecolour', onValueColour);
    explorer.addEventListener('mosaica-clustercolour', onClusterColour);
    // A basemap the map refuses reaches the kernel as a bad filter does. The note that one value
    // draws nothing under a view switched to later stays in the map's corner.
    const onBasemap = (e: Event) => {
      const {refusal} = (e as CustomEvent<{refusal: string | null}>).detail;
      if (refusal) report('basemap', new Error(refusal));
    };
    explorer.addEventListener('mosaica-basemapchange', onBasemap);
    return () => {
      explorer.removeEventListener('mosaica-basemapchange', onBasemap);
      explorer.removeEventListener('mosaica-valuecolour', onValueColour);
      explorer.removeEventListener('mosaica-clustercolour', onClusterColour);
      explorer.removeEventListener('mosaica-budgetchange', onBudget);
      explorer.removeEventListener('mosaica-clusterbudgetchange', onClusterBudget);
      explorer.removeEventListener('mosaica-clusterpalettechange', onPalette);
      state.views.delete(explorer);
      teardown(v);
      if (state.active === v) state.active = [...state.views.values()].at(-1) ?? null;
    };
  });
  return () => state.dispose();
}

/** The record field that titles a point, as the kernel set it; empty where it set none. */
function titleFieldOf(model: WidgetModel): string {
  const f = model.get('title_field');
  return typeof f === 'string' ? f : '';
}

/**
 * The point budget, the cluster budget and the range a control for each offers, as the kernel set
 * them; a value it left unset leaves the explorer's.
 */
function applyBudget(explorer: MosaicaExplorer, model: WidgetModel): void {
  const [budget, min, max, clusters, clustersMin, clustersMax] = ['budget', 'budget_min', 'budget_max', 'cluster_budget', 'cluster_budget_min', 'cluster_budget_max'].map((key) => model.get(key));
  if (typeof budget === 'number') explorer.budget = budget;
  if (typeof min === 'number') explorer.budgetMin = min;
  if (typeof max === 'number') explorer.budgetMax = max;
  if (typeof clusters === 'number') explorer.clusterBudget = clusters;
  if (typeof clustersMin === 'number') explorer.clusterBudgetMin = clustersMin;
  if (typeof clustersMax === 'number') explorer.clusterBudgetMax = clustersMax;
}

/** The cluster colours `store` holds, by layer and `mosaica_id`, as `#rrggbb`. */
function clusterColoursOf(store: Store): Record<string, Record<string, string>> {
  const out: Record<string, Record<string, string>> = {};
  for (const [layer, ids] of store.get('artifacts').overrides) {
    if (ids.size > 0) out[layer] = Object.fromEntries([...ids].map(([id, rgba]) => [idString(id), hexOf(rgba)]));
  }
  return out;
}

/** The value and cluster colours the kernel set; one it left unset leaves the explorer's. */
function applyColours(explorer: MosaicaExplorer, model: WidgetModel): void {
  const values = model.get('value_colours');
  const clusters = model.get('cluster_colours');
  if (values && typeof values === 'object') explorer.valueColours = values as Record<string, Record<string, string>>;
  if (clusters && typeof clusters === 'object') explorer.clusterColours = clusters as Record<string, Record<string, string>>;
}

/** The basemap the kernel set, and the local images it names, by path, to the number of the read. */
function basemapOf(model: WidgetModel): {value: unknown; images: Readonly<Record<string, number>>} {
  const synced = model.get('_basemap') as {value?: unknown; images?: Record<string, number>} | null | undefined;
  return {value: synced?.value ?? null, images: synced?.images ?? {}};
}

/**
 * Give the explorer the basemap the kernel set, with each local image's object URL in place of its
 * path, or `none` while a listed path has no bytes yet. `null` leaves the map's default.
 */
function applyBasemap(explorer: MosaicaExplorer, model: WidgetModel, images: ReadonlyMap<string, {url: string}>): void {
  const {value, images: listed} = basemapOf(model);
  let waiting = false;
  // A local image's `image` is `{local: path}`, which no other value holds.
  const local = (one: unknown): unknown => {
    const image = typeof one === 'object' && one !== null ? (one as {image?: unknown}).image : undefined;
    const path = typeof image === 'object' && image !== null ? (image as {local?: unknown}).local : undefined;
    if (typeof path !== 'string') return one;
    const held = Object.hasOwn(listed, path) ? images.get(path) : undefined;
    if (!held) waiting = true;
    return {...(one as object), image: held?.url ?? ''};
  };
  const mapping = typeof value === 'object' && value !== null && !isSingle(value);
  const given = mapping ? Object.fromEntries(Object.entries(value).map(([view, one]) => [view, local(one)])) : local(value);
  explorer.basemap = (waiting ? 'none' : given) as MosaicaExplorer['basemap'];
}

const mounts = new WeakMap<ModelState, (explorer: MosaicaExplorer) => () => void>();

export function render({model, el, signal}: {model: WidgetModel; el: HTMLElement; signal?: AbortSignal}): () => void {
  let state = states.get(model);
  if (!state) {
    // An anywidget without `initialize` (before 0.9): set up on first render.
    initialize({model});
    state = states.get(model)!;
  }
  const explorer = document.createElement('mosaica-explorer') as MosaicaExplorer;
  explorer.layout = (model.get('explorer_layout') as 'docked' | 'overlay') || 'docked';
  explorer.titleField = titleFieldOf(model);
  applyBudget(explorer, model);
  applyColours(explorer, model);
  applyBasemap(explorer, model, state.images);
  el.append(explorer);
  fitHeight(explorer, model);
  const refit = () => fitHeight(explorer, model);
  document.addEventListener('fullscreenchange', refit);
  addEventListener('resize', refit);
  const unmount = mounts.get(state)!(explorer);
  const v = state.views.get(explorer)!;
  const onView = (e: Event) => {
    v.lastBbox = (e as CustomEvent<{bbox: [number, number, number, number]}>).detail.bbox;
    state!.active = v;
  };
  explorer.addEventListener('mosaica-viewchange', onView);
  if (state.pendingFit) {
    const pending = state.pendingFit;
    void explorer.updateComplete.then(() => {
      if (state!.pendingFit === pending && explorer.map?.fitBbox(pending, 1)) state!.pendingFit = null;
    });
  }
  const cleanup = () => {
    document.removeEventListener('fullscreenchange', refit);
    removeEventListener('resize', refit);
    explorer.removeEventListener('mosaica-viewchange', onView);
    unmount();
    explorer.remove();
  };
  signal?.addEventListener('abort', cleanup, {once: true});
  return cleanup;
}

export default {initialize, render};
