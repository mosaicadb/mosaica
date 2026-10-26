import type {FilterExpr, FilterOperandSet, FilterOperator} from './types.js';

/**
 * A filter draft, and how it becomes the expression `/v1/viewport` takes.
 *
 * A draft holds controls, including empty ones; an expression holds only predicates. An empty
 * control means no constraint: `{range: {}}` and `{in: []}` would be refused or match nothing.
 * Populated controls are joined by `all_of`, and the values within one category control by `in`.
 * The controls and their units are the renderer's.
 */

/**
 * One alternative of a text query, as {@link textTerms} returns it.
 *
 * @category Filters
 */
export type TextTerms = {
  /** Words that must all appear, in any order. */
  words: string[];
  /** Phrases that must each appear, with their words together and in order. */
  phrases: string[];
};

/**
 * Parse a text control's query into the alternatives it asks for. Plain words must all appear.
 * Text in double quotes (straight or curly) at the start of a word is a phrase, and an unclosed
 * quote runs to the end. `OR` in capitals, as a word on its own, separates alternatives, any one
 * of which is enough, so `a b OR c` matches items with both a and b, or with c. Nothing else is
 * syntax: parentheses, and quotes inside a word, are part of the word. Alternatives with no words
 * or phrases are dropped.
 *
 * @param query - The text typed into the control.
 * @param phrase - Whether the column supports phrase search. With `false`, quoted words are read as
 *   plain words. Defaults to `true`.
 *
 * @category Filters
 */
export function textTerms(query: string, phrase = true): TextTerms[] {
  const out: TextTerms[] = [];
  let current: TextTerms = {words: [], phrases: []};
  const close = () => {
    if (current.words.length > 0 || current.phrases.length > 0) out.push(current);
    current = {words: [], phrases: []};
  };
  const token = /["“”]([^"“”]*)["“”]?|(\S+)/g;
  for (const m of query.matchAll(token)) {
    if (m[1] !== undefined) {
      const words = m[1].trim().split(/\s+/).filter(Boolean);
      if (!phrase) current.words.push(...words);
      else if (words.length > 0) current.phrases.push(words.join(' '));
    } else if (m[2] === 'OR') close();
    else current.words.push(m[2]!);
  }
  close();
  return out;
}

/**
 * What a filter control does with the items it matches.
 *
 * - `filter`: the map and its counts are limited to the matching items.
 * - `highlight`: the matching items are drawn at full strength and the rest dimmed.
 *
 * @category Filters
 */
export type ClauseVerb = 'filter' | 'highlight';

/**
 * The state of one filter control, with one variant per kind of column. An empty control, such as
 * a blank query or no selected keys, does not filter ({@link isPopulated}).
 *
 * @category Filters
 */
export type ColumnDraft =
  /**
   * A text search for `query`, parsed by {@link textTerms}. `phrase` says whether the column
   * supports phrases. If `expr` is set, it is sent in place of the query until the control is
   * cleared; use it to filter by an expression a query cannot produce.
   */
  | {family: 'text'; query: string; phrase: boolean; expr?: FilterExpr}
  /** Match `needle` against the whole value (`eq`), its start (`prefix`) or any part (`contains`). */
  | {family: 'keyword'; needle: string; op: 'eq' | 'prefix' | 'contains'}
  /**
   * Match any of the selected category keys. A key the viewer may not see matches nothing, the same
   * as a key that does not exist.
   */
  | {family: 'category'; keys: string[]}
  /** Match values between inclusive bounds in the column's own units. `null` leaves a side open. */
  | {family: 'numeric'; gte: number | null; lte: number | null};

/**
 * The state of a filter panel: its filter controls and its highlight controls, each keyed by
 * column name. Pass it to {@link Store.setFilters}. A column may have a control in both, so you
 * can limit the map to some values and highlight a few of those. The highlight applies within what
 * the filters keep. {@link emptyDraft} creates one, and {@link composeFilters} turns it into
 * filter expressions.
 *
 * @category Filters
 */
export type FilterDraft = {
  /** The filter controls. {@link emptyDraft} creates one per filterable column. */
  filter: Record<string, ColumnDraft>;
  /** The highlight controls. */
  highlight: Record<string, ColumnDraft>;
};

/**
 * Whether a control filters anything: a text query that is not blank, a keyword that is not empty,
 * at least one category key, or at least one numeric bound.
 *
 * @category Filters
 */
export function isPopulated(draft: ColumnDraft): boolean {
  switch (draft.family) {
    case 'text':
      return draft.expr !== undefined || textTerms(draft.query, draft.phrase).length > 0;
    case 'keyword':
      return draft.needle.length > 0;
    case 'category':
      return draft.keys.length > 0;
    case 'numeric':
      return draft.gte !== null || draft.lte !== null;
  }
}

/**
 * The expression one populated control on `column` becomes. A text query's words go as one
 * `match`, each phrase as a `phrase`, the two joined by `all_of`, and alternatives by `any_of`.
 */
function expressionOf(column: string, draft: ColumnDraft): FilterExpr {
  if (draft.family !== 'text') return {[column]: operatorOf(draft)} as FilterExpr;
  if (draft.expr !== undefined) return draft.expr;
  const alternatives = textTerms(draft.query, draft.phrase).map(({words, phrases}): FilterExpr => {
    const leaves = [...(words.length > 0 ? [{match: words.join(' ')}] : []), ...phrases.map((phrase) => ({phrase}))].map((op) => ({[column]: op}) as FilterExpr);
    return leaves.length === 1 ? leaves[0]! : {all_of: leaves};
  });
  return alternatives.length === 1 ? alternatives[0]! : {any_of: alternatives};
}

/**
 * The text query that produces `expr` on `column`: the reverse of what a text control sends.
 * Returns `null` if no query produces exactly that expression, for example if a `match` contains
 * `OR` or a quote. Use it to show an expression set from elsewhere in a search box.
 *
 * @category Filters
 */
export function textQueryOf(column: string, expr: FilterExpr, phrase: boolean): string | null {
  const alternatives = 'any_of' in expr ? (expr.any_of as FilterExpr[]) : [expr];
  const parts: string[] = [];
  for (const alternative of alternatives) {
    const leaves = 'all_of' in alternative ? (alternative.all_of as FilterExpr[]) : [alternative];
    const words: string[] = [];
    const phrases: string[] = [];
    for (const leaf of leaves) {
      const op = (leaf as Record<string, unknown>)[column] as Record<string, unknown> | undefined;
      if (Object.keys(leaf).length !== 1 || !op || typeof op !== 'object') return null;
      if (typeof op['match'] === 'string') words.push(op['match']);
      else if (typeof op['phrase'] === 'string') phrases.push(`"${op['phrase']}"`);
      else return null;
    }
    parts.push([...words, ...phrases].join(' '));
  }
  const query = parts.join(' OR ');
  const draft: ColumnDraft = {family: 'text', query, phrase};
  return isPopulated(draft) && JSON.stringify(expressionOf(column, draft)) === JSON.stringify(expr) ? query : null;
}

/**
 * The operator one populated control of a family other than text becomes. A category control with
 * one key still sends `in`, which asks the same as `eq`.
 */
function operatorOf(draft: Exclude<ColumnDraft, {family: 'text'}>): FilterOperator {
  switch (draft.family) {
    case 'keyword':
      return draft.op === 'eq'
        ? {eq: draft.needle}
        : draft.op === 'prefix'
          ? {prefix: draft.needle}
          : {contains: draft.needle};
    case 'category':
      return {in: draft.keys};
    case 'numeric': {
      const range: {gte?: number; lte?: number} = {};
      if (draft.gte !== null) range.gte = draft.gte;
      if (draft.lte !== null) range.lte = draft.lte;
      return {range};
    }
  }
}

/**
 * Build the filter expression for one side of a draft: `null` if no control is populated, the
 * single clause if one is, and `all_of` over the clauses if several are.
 *
 * @param draft - The filter panel's state.
 * @param verb - Which controls to use. Defaults to `filter`.
 *
 * @category Filters
 */
export function composeFilters(draft: FilterDraft, verb: ClauseVerb = 'filter'): FilterExpr | null {
  const leaves: FilterExpr[] = [];
  // The filter position is seeded in `/v1/meta`'s declaration order, and both positions follow it,
  // so two sessions filtering alike send the same body. A column the seed lacks comes after, in the
  // order it was added.
  const controls = draft[verb];
  for (const column of new Set([...Object.keys(draft.filter), ...Object.keys(controls)])) {
    const control = controls[column];
    if (!control || !isPopulated(control)) continue;
    leaves.push(expressionOf(column, control));
  }
  if (leaves.length === 0) return null;
  if (leaves.length === 1) return leaves[0]!;
  return {all_of: leaves};
}

/**
 * How many controls are populated, for a panel heading. A column populated as both filter and
 * highlight counts twice. Pass `verb` to count only one side.
 *
 * @category Filters
 */
export function activeCount(draft: FilterDraft, verb?: ClauseVerb): number {
  const verbs: ClauseVerb[] = verb === undefined ? ['filter', 'highlight'] : [verb];
  return verbs.reduce((n, v) => n + Object.values(draft[v]).filter(isPopulated).length, 0);
}

/**
 * Return a copy of `draft` with `column`'s `verb` control emptied. The control keeps its kind and
 * keyword operator, and the column's control on the other side is unchanged. Returns `draft`
 * itself if there is no such control.
 *
 * @category Filters
 */
export function withoutClause(draft: FilterDraft, column: string, verb: ClauseVerb): FilterDraft {
  const control = draft[verb][column];
  if (!control) return draft;
  const empty: ColumnDraft =
    control.family === 'text'
      ? {family: 'text', query: '', phrase: control.phrase}
      : control.family === 'keyword'
        ? {...control, needle: ''}
        : control.family === 'category'
          ? {family: 'category', keys: []}
          : {family: 'numeric', gte: null, lte: null};
  return {...draft, [verb]: {...draft[verb], [column]: empty}};
}

/**
 * Create a draft with one empty filter control per filterable column, and no highlight controls.
 * Pass `meta.filterOperands`. A column gets a control only if it supports the operator the control
 * needs: `match` for text, `in` for category and `range` for numeric. A text control's `phrase` is
 * set if the column supports `phrase`. A keyword control starts with whichever of `eq`, `prefix` and
 * `contains` comes first in the column's operands.
 *
 * A group-scoped column is keyed by its bare name, which works only under a view of its own group
 * or of a group that shares its views. To filter it under another view, key the control as
 * `column@key`, where `key` names one of the group's views; the server refuses the bare name there
 * with `422`.
 *
 * @category Filters
 */
export function emptyDraft(operands: FilterOperandSet[]): FilterDraft {
  const draft: Record<string, ColumnDraft> = {};
  for (const {column, family, operands: ops} of operands) {
    switch (family) {
      case 'text':
        if (ops.includes('match')) draft[column] = {family: 'text', query: '', phrase: ops.includes('phrase')};
        break;
      case 'keyword': {
        const op = ops.find((o): o is 'eq' | 'prefix' | 'contains' => o === 'eq' || o === 'prefix' || o === 'contains');
        if (op) draft[column] = {family, needle: '', op};
        break;
      }
      case 'category':
        if (ops.includes('in')) draft[column] = {family: 'category', keys: []};
        break;
      case 'numeric':
        if (ops.includes('range')) draft[column] = {family: 'numeric', gte: null, lte: null};
        break;
    }
  }
  return {filter: draft, highlight: {}};
}
