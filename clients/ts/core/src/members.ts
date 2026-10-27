import type {ClauseVerb} from './filters.js';
import type {FilterExpr, MemberOfOperand} from './types.js';

/**
 * The `member_of` leaf and the clauses a client holds of it.
 *
 * A `member_of` clause names one artifact of one layer and asks for its members. It composes like
 * any other leaf and sits in `filters` or in `highlight` alike, so narrowing the map to a cluster
 * and lighting that cluster's members are one clause in two positions, and an artifact can hold a
 * clause in each at once. A drawn `region` asks about a shape and this asks about membership; for
 * an artifact whose members are spread across the map, its shape is the map's outline and the two
 * differ.
 */

/**
 * Build the operand of a `member_of` filter for one artifact, named by its `mosaica_id` or by the
 * key it was published under. A `mosaica_id` becomes a decimal string, as the server expects;
 * `{key, level}` is sent as given, `level` left out for level 0.
 *
 * @category Filters
 */
export function memberOf(layer: string, artifact: bigint | {key: string; level?: number}): MemberOfOperand {
  if (typeof artifact === 'bigint') return {layer, artifact: artifact.toString()};
  return artifact.level === undefined ? {layer, key: artifact.key} : {layer, key: artifact.key, level: artifact.level};
}

/**
 * A filter on membership of one artifact, such as a cluster: keep or highlight the items inside
 * it, or those outside it. Pass the list of clauses to {@link Store.setMembers}. An artifact can
 * have one filter clause and one highlight clause at the same time.
 *
 * @category Filters
 */
export type MemberClause = {
  /** The artifact's layer. */
  layer: string;
  /** The artifact's `mosaica_id`. */
  artifact: bigint;
  /** Whether the clause selects the items outside the artifact instead of its members. */
  outside: boolean;
  /** Whether the clause filters or highlights. */
  verb: ClauseVerb;
  /**
   * A name to show for the artifact, such as the one shown by the card or panel that created the
   * clause. It is not sent to the server. The map does not draw a filter layer's artifacts, so
   * this may be the only name available for them.
   */
  label?: string;
};

/**
 * A string key for one artifact, `<layer> <artifact>`, for a `Map` or `Set`. A filter clause and a
 * highlight clause on the same artifact have the same key.
 *
 * @category Filters
 */
export function memberKey(layer: string, artifact: bigint): string {
  return `${layer} ${artifact}`;
}

/**
 * Turn a clause into a filter expression: `member_of`, wrapped in `none_of` if the clause is
 * `outside`.
 *
 * @category Filters
 */
export function memberLeaf(clause: MemberClause): FilterExpr {
  const leaf: FilterExpr = {member_of: memberOf(clause.layer, clause.artifact)};
  return clause.outside ? {none_of: [leaf]} : leaf;
}

/**
 * Combine `expr` with the clauses set to `verb`, using `all_of`. Two artifacts combine to the items
 * in both; to show the members of either, use their parent. Returns `expr` unchanged if no clause
 * is set to `verb`.
 *
 * @category Filters
 */
export function withMembers(expr: FilterExpr | null, clauses: readonly MemberClause[], verb: ClauseVerb): FilterExpr | null {
  const leaves = clauses.filter((c) => c.verb === verb).map(memberLeaf);
  if (leaves.length === 0) return expr;
  const all = expr === null ? leaves : [expr, ...leaves];
  return all.length === 1 ? all[0]! : {all_of: all};
}

/**
 * Return `clauses` with `clause` added at the end. It replaces any clause on the same artifact with
 * the same `verb`, and leaves one with the other `verb`.
 *
 * @category Filters
 */
export function withMember(clauses: readonly MemberClause[], clause: MemberClause): MemberClause[] {
  return [...withoutMember(clauses, clause.layer, clause.artifact, clause.verb), clause];
}

/**
 * Return `clauses` without the clause on this artifact with this `verb`, if there is one.
 *
 * @category Filters
 */
export function withoutMember(clauses: readonly MemberClause[], layer: string, artifact: bigint, verb: ClauseVerb): MemberClause[] {
  const key = memberKey(layer, artifact);
  return clauses.filter((c) => c.verb !== verb || memberKey(c.layer, c.artifact) !== key);
}
