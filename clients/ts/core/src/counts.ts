/**
 * Numbers typed by what they are. A masked map shows two kinds of figure, and a panel that confuses
 * them presents a sample as a set: "12,040 of 12,040" against a cluster is false. The formatters
 * render each kind correctly, so a customer drawing their own panel can rely on the type.
 */

/**
 * How many marks are drawn out of how many items there are, such as `1,204 of 29,935`. The map
 * draws a sample of the items, so the two usually differ. The store publishes one as
 * `view.served`, `marks.count` and `region.served`. Show `shown` with `total`, since `shown` alone
 * reads as the size of the set.
 *
 * @category Counts
 */
export type Count = {
  /** How many marks were drawn. */
  shown: number;
  /** How many items the marks were drawn from, counting only items the viewer may see. */
  total: number;
  /**
   * Whether the figures are exact. `false` before anything has arrived and while a region waits
   * for its counts, and {@link formatCount} then returns an empty string.
   */
  exact: boolean;
};

/**
 * A count of items the viewer may see, such as `visible`, `matched` and `highlighted` in the
 * `view` projection, or a region's counts.
 *
 * @category Counts
 */
export type Masked = {
  /** The count. */
  value: number;
  /**
   * Whether `value` is exact. A region's count is an estimate if the server counted the tiles
   * covering the region instead of the region itself. {@link formatMasked} shows an estimate with
   * `≈`.
   */
  exact: boolean;
};

/**
 * Options for {@link formatCount} and {@link formatMasked}.
 *
 * @category Counts
 */
export type FormatOptions = {
  /**
   * Whether the figure is out of date (`status.stale` in the store). A stale figure formats as an
   * empty string. Defaults to `false`.
   */
  stale?: boolean;
  /** The locale for thousands separators, as `Number.prototype.toLocaleString` takes it. Defaults to `en-GB`. */
  locale?: string;
  /**
   * Whether to shorten figures of a thousand or more to one decimal place, such as `16.8M` for
   * 16,822,190. Defaults to `false`.
   */
  compact?: boolean;
};

/** ICU writes British English's compact suffixes in lower case (`16.8m`); the figures use capitals. */
const fmt = (n: number, opts: FormatOptions) =>
  opts.compact
    ? n.toLocaleString(opts.locale ?? 'en-GB', {notation: 'compact', maximumFractionDigits: 1}).replace(/(\d)([kmbt])$/, (_, d: string, u: string) => d + u.toUpperCase())
    : n.toLocaleString(opts.locale ?? 'en-GB');

/**
 * The {@link Count} before anything has arrived: zero of zero, not exact.
 *
 * @category Counts
 */
export const NO_COUNT: Count = {shown: 0, total: 0, exact: false};
/**
 * The {@link Masked} count before anything has arrived: zero, not exact.
 *
 * @category Counts
 */
export const NO_MASKED: Masked = {value: 0, exact: false};

/**
 * Format a count as `<shown> of <total>`, such as `1,204 of 29,935`. Returns an empty string if
 * the count is not exact or `opts.stale` is set.
 *
 * @category Counts
 */
export function formatCount(count: Count, opts: FormatOptions = {}): string {
  if (opts.stale || !count.exact) return '';
  return `${fmt(count.shown, opts)} of ${fmt(count.total, opts)}`;
}

/**
 * Format a masked count as one figure, such as `29,935`, or `≈ 29,935` if it is not exact. Returns
 * an empty string if `opts.stale` is set.
 *
 * @category Counts
 */
export function formatMasked(masked: Masked, opts: FormatOptions = {}): string {
  if (opts.stale) return '';
  return masked.exact ? fmt(masked.value, opts) : `≈ ${fmt(masked.value, opts)}`;
}
