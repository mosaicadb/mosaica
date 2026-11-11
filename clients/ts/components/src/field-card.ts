import {css, html, nothing, type PropertyValues, type TemplateResult} from 'lit';
import {property, state} from 'lit/decorators.js';
import {repeat} from 'lit/directives/repeat.js';
import {
  CLUSTER_PREFIX,
  PALETTES,
  artifactColour,
  artifactName,
  colourLayers,
  composeFilters,
  isPopulated,
  memberLeaf,
  paletteSize,
  withMember,
  withoutMember,
  type AggregateEntry,
  type AggregateSpec,
  type AggregateTable,
  type ClauseVerb,
  type ColumnDraft,
  type FieldSummary,
  type FilterDraft,
  type Layer,
  type MemberClause,
  type Meta,
  type PaletteName,
  type Store
} from '@mosaicajs/client';
import {CATEGORY_PALETTES, RAMPS} from '@mosaicajs/deck';
import {colourOfFraction, css as rgb, fractionOf, hexOf} from '@mosaicajs/deck/internal';
import {levelFollowsZoom} from '@mosaicajs/client/internal';
import {HeldAggregate, artifactGroupings, countsByKey, isLinked, levelOf, listedGroups, rankedGrouping} from './aggregate.js';
import {BOOL_KEYS, BOOL_NUMBERS, BOOL_WORDS, MosaicaElement, UNNAMED, boolClause, boolKeys, columnCaption, countText, dateRangeText, emit, focusLost, idString, isBoolColumn, keyTitle, parseDateText, shortDateText, type BoolKey} from './base.js';
import {ColourPicker, pickerStyles} from './colour-picker.js';
import {clusterColour, colouringOf, holdColours, paletteValueColour, setClusterColours, setValueColours, valueColour, valueMet, watchChoices} from './colouring.js';
import {attachContextRoot, defineOnce} from './define.js';
import {icon} from './icons.js';
import {Lineage, focusAdded, spokenCounts, type Node as BrowseNode} from './lineage.js';
import {exportparts} from './parts.js';
import {chrome, tokens} from './tokens.js';
import './filter.js';
import './cluster-filter.js';

/** The rows, crumbs and chips whose place keeps focus across a render, by part. */
const HOLDERS = '[part="row"], [part="crumb"], [part="chip"]';
/** The rows a category or cluster card lists, and the rows "N more" opens it to. */
const ROWS = 5;
const MORE_ROWS = 20;
/** The rows of a browse page of a linked layer's card, which More adds to. */
const PAGE_ROWS = 20;
/** The narrowest a cluster's bar is drawn where it is not empty, in pixels. */
const MIN_BAR_PX = 3;
/** The values the counts over everything matching are asked for, so the rows in view find theirs. */
const MATCH_TOP = 100;
/** A histogram's bins, and the items its counts are taken from beyond which they are scaled. */
const BINS = 20;
const SAMPLE = 100_000;
/** The histogram's height in pixels. */
const PLOT_HEIGHT = 56;
/** The bars of a folded card's small chart. */
const SPARK_BARS = 8;


/**
 * A number as an axis prints it: in exponent form from a million up and below a thousandth, else
 * to two decimal places at 1 or more and three significant figures below 1. With `whole`, rounded
 * to a whole number first, for an integer column.
 */
function formatNumber(n: number, whole: boolean): string {
  const v = whole ? Math.round(n) : n;
  const size = Math.abs(v);
  if (size >= 1e6 || (v !== 0 && size < 1e-3)) return v.toExponential(2);
  return v.toLocaleString('en-GB', size >= 1 ? {maximumFractionDigits: 2} : {maximumSignificantDigits: 3});
}

/** What a card draws: a category's or a layer's top values, a histogram, a bool's two values, or a search box. */
type Kind = 'category' | 'cluster' | 'histogram' | 'bool' | 'search';

/**
 * How a cluster card lists its layer: `ranked` by count with the layer's own clauses left out, the
 * `children` of its position from browse pages, or the `overlap`, ranked under every clause at the
 * level after its position's, inside the position.
 */
type Listing = 'ranked' | 'children' | 'overlap';

/** One step of a card's position: the cluster stepped into, its level, and how many children its row said it has. */
type Step = {id: bigint; name: string; rung: number; children: number | null};

/** A row's button that steps into it: its accessible name, the figure it shows, and the step it takes. */
type Stepper = {label: string; count: number | null; step: Step};

/** One bin of a histogram, its edges in the column's units (microseconds on a timestamp). */
type Bin = {lower: number; upper: number; count: number};

/**
 * One row of a category, bool or cluster card; `own` is a cluster's palette colour, `#rrggbb`, where it
 * is coloured, and `unmet` marks a category value the map has given no colour yet.
 */
type Row = {key: string; name: string; sub: number | null; all: number | null; swatch: string | null; own: string | null; unmet: boolean};

/** A range being dragged across the histogram, by bin, `from` where the drag began. */
type Brush = {from: number; to: number};

/**
 * One end of the range box: what its field holds, the value that stands for (`null` leaves the end
 * open, `undefined` is text that is no value), and whether a mistake in it is shown.
 */
type End = {text: string; value: number | null | undefined; checked: boolean};
/** The range box's two ends. */
type RangeEntry = {from: End; to: End};

/** The marks that may group a number's thousands: a comma, a space and a narrow no-break space. */
const GROUP_MARKS = /[, \u202f]/g;
/** Digits in threes between one such mark, as a count is written: `1,234,567` and `1 250.5`, not `1,5` or `1,234 567`. */
const GROUPED = /^[+-]?\d{1,3}([, \u202f])\d{3}(\1\d{3})*(\.\d*)?$/;

/**
 * The value a range box's field stands for; `upper` reads a date as the last instant of the period
 * it names. In a number a comma or a space is taken only where it groups thousands, so `1,5` is no
 * number.
 */
function endValue(text: string, timestamp: boolean, upper: boolean): number | null | undefined {
  const t = text.trim();
  if (t === '') return null;
  if (timestamp) return parseDateText(t, upper) ?? undefined;
  const n = Number(GROUPED.test(t) ? t.replace(GROUP_MARKS, '') : t);
  return Number.isFinite(n) ? n : undefined;
}

/**
 * A value as a range box's field holds it. A number is written in full, so that typing what the
 * field shows gives the same number; the axis rounds it and writes a large one with an exponent.
 */
function endText(v: number, timestamp: boolean): string {
  return timestamp ? shortDateText(v) : String(v);
}

/** Whether both ends hold a value and the lower is above the upper. */
function backwards(entry: RangeEntry): boolean {
  return typeof entry.from.value === 'number' && typeof entry.to.value === 'number' && entry.from.value > entry.to.value;
}

/** A range as the card says it: an open end as a bound, and no end as every value. */
function rangeWords(gte: number | null, lte: number | null, timestamp: boolean, whole: boolean): string {
  if (gte === null && lte === null) return 'Any value';
  if (timestamp) return dateRangeText(gte, lte);
  if (lte === null) return `≥ ${formatNumber(gte!, whole)}`;
  if (gte === null) return `≤ ${formatNumber(lte, whole)}`;
  return `${formatNumber(gte, whole)} – ${formatNumber(lte, whole)}`;
}

/**
 * The items `bins` count in a range whose ends are bin edges or open; `null` for a range that cuts
 * a bin. With `discrete`, a bin's last value is one below the next bin's edge.
 */
function countWithin(bins: Bin[], gte: number | null, lte: number | null, discrete: boolean): number | null {
  const first = gte === null ? 0 : bins.findIndex((b) => b.lower === gte);
  const last = lte === null ? bins.length - 1 : bins.findIndex((b) => b.upper - (discrete ? 1 : 0) === lte);
  return first < 0 || last < first ? null : bins.slice(first, last + 1).reduce((t, b) => t + b.count, 0);
}

/** Whether a cluster card lists `layer` by its children: a layer with parent edges that the browse route serves. */
function drills(layer: Layer): boolean {
  return isLinked(layer) && layer.depsOn.length === 0;
}

/** The level of `layer` after `rung`, the next finer one, where it has one. */
function levelAfter(layer: Layer, rung: number): number | undefined {
  return layer.levels
    .map((l) => l.level)
    .filter((l) => l > rung)
    .sort((a, b) => a - b)[0];
}

/** A level's title, or its number where it has none. */
function levelTitle(layer: Layer, level: number): string {
  return layer.levels.find((l) => l.level === level)?.title || `Level ${level}`;
}

/** `n` and a noun, singular for one. */
function counted(n: number, one: string, many: string): string {
  return `${n.toLocaleString('en-GB')} ${n === 1 ? one : many}`;
}

/** "≈" where either table's counts were taken from a sample, else nothing. */
function approxMark(...tables: (AggregateTable | undefined)[]): string {
  return tables.some((t) => t?.sample?.sampled) ? '≈' : '';
}

/** The listed bins of a histogram table, in order, with the edges in the column's units. */
function binsOf(table: AggregateTable | undefined, timestamp: boolean): Bin[] {
  if (!table) return [];
  const rows = table.rows;
  const group = rows.getChild('group');
  const lower = rows.getChild('lower');
  const upper = rows.getChild('upper');
  const count = rows.getChild('count');
  if (!group || !lower || !upper || !count) return [];
  // Arrow reads a timestamp in milliseconds; a clause on one is in microseconds.
  const unit = timestamp ? 1000 : 1;
  const out: Bin[] = [];
  for (let i = 0; i < rows.numRows; i++) {
    if (group.get(i) !== 'listed') continue;
    const lo = lower.get(i) as number | bigint | Date | null;
    const hi = upper.get(i) as number | bigint | Date | null;
    if (lo === null || hi === null) continue;
    out.push({lower: Number(lo instanceof Date ? lo.getTime() : lo) * unit, upper: Number(hi instanceof Date ? hi.getTime() : hi) * unit, count: Number(count.get(i) as bigint | number)});
  }
  return out;
}

/**
 * Use a field card on its own when you want one field's chart in a place of your choosing.
 * `<mosaica-filter-panel>` shows one for each field. Set `field` to a column `meta.filterOperands`
 * lists, or to `cluster:<layer>` for a layer's clusters.
 *
 * On a column's card and a flat layer's, the solid bars count the items in view that the filters
 * match, or the highlighted items while there is a highlight. The pale bars count every item the
 * filters match. Neither count applies the card's own filter, so a value the viewer has filtered
 * out keeps its count.
 *
 * What the card shows depends on the kind of field:
 *
 * - A category card, and the card of a `flat` layer, lists the values with most items in view,
 *   each with its counts. A row's Highlight and Filter buttons take the counts' place while the
 *   pointer is over the row or the keyboard's focus is in it, and are always drawn on a screen with
 *   no pointer that hovers. A button is filled, and its icon solid, while its value is highlighted
 *   or filtered to, and before the counts a tick marks a value the filter keeps and a pen one that
 *   is highlighted. A screen reader is given each count with what it counts.
 * - The card of a `tiered`, `nested` or `dag` layer walks the layer by its parent edges, and
 *   walking it leaves the map's filter as it is. A breadcrumb above the rows holds the card's
 *   position, from All, the layer's top level, down to the cluster whose children are listed. Each
 *   crumb is a button that steps back to it, and a tick marks a crumb whose cluster the filter is
 *   on. The rows are the served children of the position, in the server's order, a page at a time:
 *   More loads the next page. Each row counts the items that match every filter, this layer's
 *   clauses among them, beside every item of it the viewer may see, so a row whose count is 0 is
 *   still listed, and each count is what filtering to the row would leave. A row with children
 *   ends in a button with their number, which steps into the row. Filtering to a row adds a
 *   clause, and a layer's clauses combine with AND. The rows of a `tiered` layer stand under the
 *   titles of their levels, and a row of a `dag` layer names its other served parents. Once every
 *   child of the position has loaded, a last row counts the items of the position that match every
 *   filter and are in none of its children.
 * - The card of a `stacked` layer lists the clusters with most items in view, under every filter,
 *   this layer's clauses among them, at the chosen level or at the level the map draws. A row at a
 *   level with a finer level after it ends in a button that steps into it, and the card then lists
 *   the clusters of that finer level by their items inside the row, under a heading that says so.
 * - On the card of a `tiered`, `nested`, `dag` or `stacked` layer, the layer's filter and highlight
 *   clauses stand in a line of their own under the breadcrumb, each with a × that removes it, so a
 *   clause set by another control is shown wherever the card's position is.
 * - A number or date card draws a histogram of up to 20 bins, and the viewer presses a bin or drags
 *   across several to pick a range to filter or highlight by. From the keyboard, they press Shift
 *   and the arrow keys on the focused plot, then Enter. Either opens a box under the range with its
 *   two ends in fields that take a typed number or date. A field left empty leaves that end open.
 *   The box closes on Escape, on a press outside it and as focus leaves it.
 *   A field that holds no number or date, or a lower end above the upper, is not applied, and the
 *   box says what to type. Set range, a button drawn only while the keyboard has it, opens the
 *   same box with nothing chosen. A screen reader is given the range each of the filter and the
 *   highlight holds, and the bins' counts in a table. Counts over about 100,000 items come from a
 *   sample, so they are marked "≈".
 * - A bool card lists yes and no as a category card lists values. Its counts are taken as a
 *   histogram's, so those over about 100,000 items are marked "≈".
 * - A text or keyword field's card is a search box, `<mosaica-filter>`.
 *
 * While the paint button colours the map by the field, clicking a value's colour opens a picker
 * that changes the colour on the map.
 *
 * A long name is one line, cut with an ellipsis. Where `--mosaica-long-text` is `normal`, as in the
 * explorer's narrow form, the card's name, what it notes and its rows' names wrap.
 *
 * @summary A card that charts one field's values and filters or highlights by them.
 * @tagname mosaica-field-card
 * @category Elements
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-filterchange']>} mosaica-filterchange - A row's
 *   Highlight or Filter, or the range box, changed the field's clause.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-clausechange']>} mosaica-clausechange - A cluster
 *   row, its Highlight or its Filter, or a clause's × put a `member_of` clause on or took it off.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-colourchange']>} mosaica-colourchange - The paint
 *   button changed what the map is coloured by.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-levelchange']>} mosaica-levelchange - The Level
 *   choice of a levelled layer's card changed. The detail names the card's layer, and a `level` of
 *   `null` for Follow zoom.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-valuecolour']>} mosaica-valuecolour - A category
 *   value's colour was chosen or reset in the colour picker.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-clustercolour']>} mosaica-clustercolour - A
 *   cluster's colour was chosen or reset in the colour picker.
 * @fires {CustomEvent<MosaicaEventDetails['mosaica-fold']>} mosaica-fold - The fold button folded or
 *   opened the card.
 * @csspart head - The heading: the name, what it notes, and the buttons.
 * @csspart title - The field's name, a heading that names the card.
 * @csspart sub - What the heading notes: the field's figures, or the Level choice.
 * @csspart level-select - The Level choice, on a levelled layer's card. Where no level declares a
 *   zoom range, its first option is Follow zoom.
 * @csspart spark - The small chart of a folded card.
 * @csspart paint - The paint button, with `aria-pressed`.
 * @csspart fold - The fold button, with `aria-expanded`.
 * @csspart crumbs - The breadcrumb of a layer's card that steps: a navigation landmark named by
 *   the layer, holding a list of the card's position from All down.
 * @csspart crumb - One crumb, with `data-id`, which is empty for All, and `data-filtered` while the
 *   layer's filter is on its cluster.
 * @csspart crumb-name - A crumb's button, which steps back to it, with `aria-current` on the
 *   deepest. A tick before the name marks a cluster the filter is on.
 * @csspart chips - The layer's filter and highlight clauses on a card that steps, a list, while it
 *   has any.
 * @csspart chip - One clause, with `data-id` and `data-verb` (`filter` or `highlight`).
 * @csspart chip-name - A clause's cluster, after a tick for a filter or a pen for a highlight.
 * @csspart chip-remove - The × that removes a clause.
 * @csspart level - A heading over a list of rows: a level's title on a `tiered` layer's card, or
 *   what a `stacked` layer's card lists inside its position.
 * @csspart rows - A category or cluster card's rows, one list for each level on a `tiered` layer's
 *   card.
 * @csspart row - One row, with `data-key` and `data-state`: `in` if the field's filter keeps the
 *   value, `out` if it leaves it out, `lit` if it is highlighted, otherwise empty.
 * @csspart swatch - A row's colour button, while the map is coloured by the field: a 24 px target
 *   with the colour, set as `--c`, in a 10 px square at its middle. It opens the colour picker. It
 *   carries `data-unmet`, and the square is drawn as an outline, for a value the map has not drawn
 *   yet and so has not given a colour.
 * @csspart name - A row's name, with `data-unnamed` for a cluster that has none, and the whole
 *   name as its `title`.
 * @csspart step - A row's button that steps into it, with the number of its children on a layer
 *   with parent edges.
 * @csspart also - The other served parents a `dag` row of a layer's card is under.
 * @csspart remainder - The last row of the last list, which counts the position's items in none of
 *   its children.
 * @csspart refusal - The words "Clusters unavailable", with `data-code`, where the rows were
 *   refused.
 * @csspart bar-match - A row's or a bin's pale bar, its share of the whole match.
 * @csspart bar-subject - A row's or a bin's solid bar, its share of the subject.
 * @csspart count - A row's counts as they are drawn, after its tick and its pen, with what they
 *   count as its `title`. A screen reader is given them beside it, each with what it counts.
 * @csspart verbs - A row's Highlight and Filter buttons, in its counts' place.
 * @csspart highlight - A row's Highlight button, with `aria-pressed`.
 * @csspart filter - A row's Filter button, with `aria-pressed`.
 * @csspart more - The "N more" button. On a layer's card walked by its parent edges it loads the
 *   next page and moves focus to the first row it added; elsewhere it shows more rows, or fewer. Its
 *   accessible name says what it shows.
 * @csspart plot - The histogram, a focusable group.
 * @csspart bin - One bin, with `data-index`.
 * @csspart band - An outlined range on the histogram, with `data-verb` (`filter`, `highlight`, or
 *   `brush` while one is being chosen). The highlight's is hatched.
 * @csspart axis - The values under the histogram.
 * @csspart ramp - The colour ramp under the histogram, while the map is coloured by the field.
 * @csspart range - The Set range button, with `aria-expanded`, drawn over the start of the axis
 *   while it has the keyboard's focus.
 * @csspart brush - The range box, which a chosen range or Set range opens over what is under the
 *   histogram.
 * @csspart brush-from - The field for its lower end, with `aria-invalid` while what it holds is not
 *   applied.
 * @csspart brush-to - The field for its upper end, with `aria-invalid` while what it holds is not
 *   applied.
 * @csspart brush-filter - Its Filter button.
 * @csspart brush-highlight - Its Highlight button.
 * @csspart colour-popover - The colour picker, while it is open.
 * @csspart choice - A colour in the picker's palette rows, with `aria-pressed`.
 * @csspart sv - The picker's saturation and brightness area, a slider in two directions.
 * @csspart hue - The picker's hue bar, a slider.
 * @csspart hex - The picker's hex field.
 * @csspart reset - The picker's Reset button.
 * @csspart filter-<part> - A part of the inner `<mosaica-filter>`.
 * @csspart cluster-filter-<part> - A part of the inner `<mosaica-cluster-filter>`.
 */
export class MosaicaFieldCard extends MosaicaElement {
  static override styles = [
    tokens,
    chrome,
    pickerStyles,
    css`
      :host {
        display: block;
        padding: 10px var(--_mosaica-panel-inline, 14px) 12px;
        border-bottom: 1px solid var(--_mosaica-line-2);
      }
      :host([folded]) {
        padding-top: 6px;
        padding-bottom: 6px;
      }
      [part='head'] {
        display: flex;
        align-items: center;
        gap: 8px;
        min-height: 24px;
      }
      /* The name, the mark after it and what the name notes stand in the heading's own row: the
         group draws no box of its own while a long name is cut. */
      .named {
        display: contents;
      }
      /* Not a section heading, as the shared rule for a title part draws one. */
      [part='title'] {
        display: block;
        flex: none;
        margin: 0;
        font-size: 13px;
        font-weight: 600;
        letter-spacing: 0;
        text-transform: none;
        color: var(--_mosaica-ink);
        white-space: var(--_mosaica-long-text, nowrap);
        overflow: hidden;
        text-overflow: ellipsis;
      }
      [part='sub'] {
        flex: 1 1 auto;
        min-width: 0;
        font-size: 12px;
        color: var(--_mosaica-ink-3);
        white-space: var(--_mosaica-long-text, nowrap);
        overflow: hidden;
        text-overflow: ellipsis;
      }
      [part='sub'].choice {
        flex: 0 1 auto;
        margin-right: auto;
      }
      /* 24 px to press, which the heading does not grow for. */
      [part='sub'] select {
        padding-block: 4px;
        margin-block: -4px;
        font-size: 12px;
        color: var(--_mosaica-ink-3);
      }
      [part='spark'] {
        flex: none;
        margin-left: auto;
        display: flex;
        align-items: flex-end;
        gap: 1px;
        width: 60px;
        height: 14px;
      }
      [part='spark'] span {
        flex: 1 1 0;
        min-height: 1px;
        border-radius: 1px 1px 0 0;
        background: var(--_mosaica-bar-match);
        forced-color-adjust: none;
      }
      :host([compact]) [part='spark'] {
        width: 72px;
      }
      /* Folded in the compact layout, a card is its name, a drop where it colours the map, its
         chart and its chevron. */
      :host([compact][folded]) [part='paint'],
      :host([compact][folded]) [part='sub'] {
        display: none;
      }
      :host([compact][folded]) [part='head'] {
        gap: 6px;
      }
      :host([compact][folded]) [part='fold'] {
        width: 16px;
      }
      :host([compact][folded]) [part='title'] {
        flex: 0 1 auto;
        min-width: 0;
      }
      .painted {
        display: inline-flex;
        flex: none;
        margin-left: -2px;
      }
      .painted path {
        fill: currentColor;
      }
      :host([compact]) [part='spark'] span {
        background: var(--_mosaica-bar);
      }
      [part='paint'],
      [part='fold'] {
        position: relative;
        flex: none;
        width: 22px;
        height: 22px;
        display: grid;
        place-items: center;
        border-radius: 5px;
        color: var(--_mosaica-ink-2);
      }
      /* Each small button takes a press from 24 px around its middle, drawn no larger for it. */
      [part='paint']::after,
      [part='fold']::after,
      [part='verbs'] button::after,
      [part='more']::after {
        content: '';
        position: absolute;
        inset: -2px;
      }
      [part='fold']::after {
        inset: -1px;
      }
      :host([compact][folded]) [part='fold']::after {
        inset: -1px -4px;
      }
      [part='paint'] {
        border: 1px solid var(--_mosaica-line);
        background: var(--_mosaica-surface);
        color: color-mix(in srgb, var(--_mosaica-ink) 82%, var(--_mosaica-surface));
      }
      [part='paint'][aria-pressed='true'] {
        border-color: var(--_mosaica-accent);
        background: var(--_mosaica-accent);
        color: var(--_mosaica-accent-ink);
      }
      [part='fold']:hover,
      [part='paint'][aria-pressed='false']:hover {
        background: var(--_mosaica-surface-2);
      }
      /* Where a host has long names wrap, the name and what follows it are one group: what follows
         goes under the name only where the two do not fit on a line, and the chart and the buttons
         go under the group, at the right, where the name's longest word does not fit beside them. */
      @container style(--_mosaica-long-text: normal) {
        [part='head'] {
          flex-wrap: wrap;
          justify-content: flex-end;
          gap: 2px 8px;
        }
        .named {
          flex: 1000 1 min-content;
          min-width: 0;
          max-width: max-content;
          margin-right: auto;
          display: flex;
          flex-wrap: wrap;
          align-items: center;
          gap: 0 8px;
        }
        [part='title'] {
          flex: 0 1 auto;
          min-width: 0;
          overflow-wrap: break-word;
        }
        [part='sub'] {
          overflow-wrap: anywhere;
        }
        /* Two thirds of its width where the name beside it needs the rest. */
        [part='spark'] {
          flex: 1 0 40px;
          width: auto;
          max-width: 60px;
        }
        :host([compact]) [part='spark'] {
          flex-basis: 48px;
          max-width: 72px;
        }
        :host([compact][folded]) [part='head'],
        :host([compact][folded]) .named {
          column-gap: 6px;
        }
      }
      .body {
        margin-top: 6px;
        display: flex;
        flex-direction: column;
        gap: 2px;
      }
      .search {
        display: block;
        margin-bottom: 4px;
        --_mosaica-input-height: 28px;
      }
      [part~='row'] {
        position: relative;
        display: grid;
        grid-template-columns: 12px minmax(0, 1fr) auto;
        align-items: center;
        column-gap: 8px;
        padding: 4px 6px;
        margin: 0 -6px;
        border-radius: 6px;
        cursor: pointer;
      }
      [part~='row']:hover,
      [part~='row']:focus-within {
        background: var(--_mosaica-surface-2);
      }
      /* A value the field's filter leaves out: its words faint, its colour dimmed. */
      [part~='row'][data-state='out'] [part='name'] {
        color: var(--_mosaica-ink-3);
      }
      [part~='row'][data-state='out'] [part='swatch'] {
        opacity: 0.55;
      }
      [part~='row'][data-state='in'] [part='name'] {
        font-weight: 600;
      }
      [part~='row'][data-state='lit'] [part='name'] {
        font-weight: 600;
        color: var(--_mosaica-highlight);
      }
      .mark {
        width: 12px;
        display: grid;
        place-items: center;
      }
      /* A 24 px target around a 10 px square of the value's colour, reaching into the row's
         padding and the gap beside it. */
      [part='swatch'] {
        width: 24px;
        height: 24px;
        margin: -6px;
        display: grid;
        place-items: center;
        border-radius: 5px;
      }
      [part='swatch']::before {
        content: '';
        width: 10px;
        height: 10px;
        border-radius: 2px;
        background: var(--c);
        forced-color-adjust: none;
      }
      /* A value the map has given no colour yet: an outline, not a grey that reads as a colour. */
      [part='swatch'][data-unmet]::before {
        background: none;
        box-shadow: inset 0 0 0 1px var(--_mosaica-line-control);
      }
      .label {
        display: flex;
        flex-direction: column;
        gap: 3px;
        min-width: 0;
      }
      /* The name on one line, cut at its end, or wrapped where long names wrap. */
      .names {
        min-width: 0;
        white-space: var(--_mosaica-long-text, nowrap);
        overflow: hidden;
        text-overflow: ellipsis;
        overflow-wrap: anywhere;
      }
      .bars {
        position: relative;
        height: 5px;
      }
      .bars span {
        position: absolute;
        left: 0;
        top: 0;
        height: 5px;
        border-radius: 2px;
      }
      [part='bar-match'] {
        background: var(--_mosaica-bar-match);
        forced-color-adjust: none;
      }
      [part='bar-subject'] {
        background: var(--_bar, var(--_mosaica-bar));
        forced-color-adjust: none;
      }
      :host([highlighting]) {
        --_bar: var(--_mosaica-bar-highlight);
      }
      [part='count'] {
        text-align: right;
        font-size: 12px;
        font-variant-numeric: tabular-nums;
        white-space: nowrap;
        color: var(--_mosaica-ink-3);
      }
      [part='count'] .sub {
        font-weight: 500;
        color: var(--_mosaica-ink);
      }
      :host([highlighting]) [part='count'] .sub {
        color: var(--_mosaica-highlight);
      }
      /* Before a row's counts, a tick where the filter keeps its value and a pen where it is
         highlighted, so neither is told by a colour alone. */
      .cue {
        display: inline-flex;
        margin-right: 4px;
        vertical-align: -1.5px;
        color: var(--_mosaica-ink);
      }
      .cue.lit {
        color: var(--_mosaica-highlight);
      }
      /* The verbs share the counts' cell and take their place while the row is hovered or the
         keyboard's focus is in it. Hidden, they stay in the tab order, so Tab reaches them and
         shows them. */
      [part='count'],
      [part='verbs'] {
        grid-column: 3;
        grid-row: 1;
      }
      [part='verbs'] {
        display: flex;
        justify-self: end;
        gap: 4px;
        opacity: 0;
        pointer-events: none;
      }
      [part~='row']:hover [part='verbs'],
      [part~='row']:has(:focus-visible) [part='verbs'] {
        opacity: 1;
        pointer-events: auto;
      }
      [part~='row']:hover [part='count'],
      [part~='row']:has(:focus-visible) [part='count'] {
        visibility: hidden;
      }
      [part='verbs'] button {
        position: relative;
        width: 22px;
        height: 22px;
        display: grid;
        place-items: center;
        border: 1px solid var(--_mosaica-line);
        border-radius: 5px;
        background: var(--_mosaica-surface);
        color: color-mix(in srgb, var(--_mosaica-ink) 82%, var(--_mosaica-surface));
      }
      /* On, a verb is filled and its icon solid. */
      [part='verbs'] button[aria-pressed='true'] {
        border-color: var(--_mosaica-accent);
        background: var(--_mosaica-accent);
        color: var(--_mosaica-accent-ink);
      }
      [part='verbs'] [part='highlight'][aria-pressed='true'] {
        border-color: var(--_mosaica-highlight);
        background: var(--_mosaica-highlight);
      }
      [part='verbs'] button[aria-pressed='true'] svg {
        fill: currentColor;
      }
      /* A screen with no pointer that hovers cannot show the verbs on hover: there they are
         always drawn, beside the counts. */
      @media (hover: none) {
        [part~='row'] {
          grid-template-columns: 12px minmax(0, 1fr) auto auto;
        }
        [part='verbs'] {
          grid-column: 4;
          opacity: 1;
          pointer-events: auto;
        }
        [part~='row'].steps {
          grid-template-columns: 12px minmax(0, 1fr) auto auto minmax(30px, auto);
        }
        .step {
          grid-column: 5;
        }
        [part~='row']:hover [part='count'],
        [part~='row']:has(:focus-visible) [part='count'] {
          visibility: visible;
        }
      }
      [part='more'] {
        position: relative;
        align-self: flex-start;
        margin: 4px 0 0 20px;
      }
      [part='more']::after {
        inset: -4px 0;
      }
      .none {
        padding: 4px 0 0 20px;
        font-size: 12px;
        color: var(--_mosaica-ink-3);
      }
      /* The breadcrumb: each crumb a button with its name, cut where long. */
      [part='crumbs'] ol,
      [part='chips'] {
        list-style: none;
        margin: 0;
        padding: 0;
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: 2px 4px;
        font-size: 12px;
      }
      [part='crumb'] {
        display: inline-flex;
        align-items: center;
        gap: 2px;
        min-width: 0;
        max-width: 100%;
      }
      .sep {
        margin-right: 2px;
        color: var(--_mosaica-ink-3);
      }
      /* 24 px high, the name cut where long. */
      [part='crumb-name'] {
        display: inline-flex;
        align-items: center;
        min-width: 0;
        max-width: 100%;
        min-height: 24px;
        padding: 0 4px;
        margin: 0 -4px;
        border-radius: 4px;
        color: var(--_mosaica-ink-2);
      }
      [part='crumb-name']:hover {
        background: var(--_mosaica-surface-2);
      }
      [part='crumb-name'][aria-current] {
        font-weight: 600;
        color: var(--_mosaica-ink);
      }
      .crumb-text {
        min-width: 0;
        white-space: var(--_mosaica-long-text, nowrap);
        overflow: hidden;
        text-overflow: ellipsis;
        overflow-wrap: anywhere;
      }
      /* The layer's clauses, each a chip with its ×, on a line apart from the breadcrumb. */
      [part='chips'] {
        gap: 4px;
      }
      [part='chip'] {
        display: inline-flex;
        align-items: center;
        gap: 4px;
        min-width: 0;
        max-width: 100%;
        height: 22px;
        padding: 0 0 0 6px;
        border: 1px solid var(--_mosaica-line);
        border-radius: 5px;
        background: var(--_mosaica-surface);
      }
      [part='chip'][data-verb='highlight'] {
        border-color: color-mix(in srgb, var(--_mosaica-highlight) 18%, var(--_mosaica-highlight-soft));
        background: var(--_mosaica-highlight-soft);
        color: var(--_mosaica-highlight);
      }
      [part='chip'] .cue {
        margin-right: 0;
        color: inherit;
      }
      [part='chip-name'] {
        min-width: 0;
        white-space: var(--_mosaica-long-text, nowrap);
        overflow: hidden;
        text-overflow: ellipsis;
        overflow-wrap: anywhere;
      }
      /* Drawn 20 px square, with a 24 px target around it. */
      [part='chip-remove'] {
        position: relative;
        flex: none;
        width: 20px;
        height: 20px;
        display: grid;
        place-items: center;
        border-radius: 4px;
        color: inherit;
      }
      [part='chip-remove']::after {
        content: '';
        position: absolute;
        inset: -2px;
      }
      [part='chip-remove']:hover {
        background: var(--_mosaica-surface-2);
      }
      [part='level'] {
        margin: 4px 0 0;
        font-size: 12px;
        font-weight: 600;
        line-height: 1.45;
        letter-spacing: 0;
        text-transform: none;
        color: var(--_mosaica-ink-2);
      }
      [part='also'] {
        padding-left: 20px;
        font-size: 12px;
        color: var(--_mosaica-ink-3);
        overflow-wrap: anywhere;
      }
      [part='remainder'] {
        display: grid;
        grid-template-columns: 12px minmax(0, 1fr) auto;
        align-items: center;
        column-gap: 8px;
        padding: 4px 6px;
        margin: 0 -6px;
        font-size: 12px;
        color: var(--_mosaica-ink-2);
      }
      /* A row that steps has a fourth column, kept on a row with nothing to step into so the
         counts stand in line. */
      [part~='row'].steps {
        grid-template-columns: 12px minmax(0, 1fr) auto minmax(30px, auto);
      }
      .step {
        grid-column: 4;
        grid-row: 1;
        justify-self: end;
      }
      /* 22 px high, with a 24 px target around it. */
      [part='step'] {
        position: relative;
        display: inline-flex;
        align-items: center;
        justify-content: flex-end;
        gap: 3px;
        min-width: 24px;
        height: 22px;
        padding: 0 4px;
        border-radius: 5px;
        font-size: 12px;
        font-variant-numeric: tabular-nums;
        color: var(--_mosaica-ink-2);
      }
      [part='step']::after {
        content: '';
        position: absolute;
        inset: -1px 0;
      }
      [part='step']:hover {
        background: var(--_mosaica-surface-3);
        color: var(--_mosaica-ink);
      }
      /* The histogram. */
      .hist {
        position: relative;
        margin-top: 8px;
      }
      [part='plot'] {
        position: relative;
        height: ${PLOT_HEIGHT}px;
        display: flex;
        align-items: flex-end;
        gap: 2px;
        cursor: crosshair;
        touch-action: pan-y;
        user-select: none;
        border-radius: 2px;
      }
      [part~='bin'] {
        position: relative;
        flex: 1 1 0;
        height: ${PLOT_HEIGHT}px;
      }
      [part~='bin'] span {
        position: absolute;
        bottom: 0;
        border-radius: 1px 1px 0 0;
      }
      [part~='bin'] [part='bar-match'] {
        left: 0;
        right: 0;
      }
      [part~='bin'] [part='bar-subject'] {
        left: 2px;
        right: 2px;
      }
      [part~='band'] {
        position: absolute;
        top: -4px;
        bottom: -2px;
        border-radius: 4px;
        border: 1.5px solid var(--_mosaica-ink);
        background: color-mix(in srgb, var(--_mosaica-ink) 6%, transparent);
        pointer-events: none;
      }
      [part~='band'][data-verb='brush'] {
        border-style: dashed;
      }
      /* Hatched, so it differs from the filter's range by more than its colour. */
      [part~='band'][data-verb='highlight'] {
        border-color: var(--_mosaica-highlight);
        background: repeating-linear-gradient(135deg, color-mix(in srgb, var(--_mosaica-highlight) 24%, transparent) 0 1.5px, color-mix(in srgb, var(--_mosaica-highlight) 6%, transparent) 1.5px 6px);
      }
      [part='ramp'] {
        height: 6px;
        margin-top: 4px;
        border-radius: 2px;
        forced-color-adjust: none;
      }
      [part='axis'] {
        display: flex;
        justify-content: space-between;
        margin-top: 4px;
        font-size: 12px;
        color: var(--_mosaica-ink-2);
        font-variant-numeric: tabular-nums;
      }
      /* The button that opens the range box without a drag is drawn only while the keyboard has
         it, over the start of the axis, so the card is no taller for it. */
      [part='range'] {
        position: absolute;
        left: 0;
        top: ${PLOT_HEIGHT + 2}px;
        z-index: 1;
      }
      /* Off screen it has no box either, which the shared button's height and padding would give it. */
      [part='range']:not(:focus) {
        width: 1px;
        height: 1px;
        padding: 0;
        border: 0;
      }
      [part='brush'] {
        position: absolute;
        top: ${PLOT_HEIGHT + 8}px;
        z-index: 2;
        display: flex;
        flex-direction: column;
        gap: 8px;
        padding: 10px;
        background: var(--_mosaica-surface);
        border: 1px solid var(--_mosaica-line);
        border-radius: var(--_mosaica-radius);
        box-shadow: 0 6px 20px rgba(0, 0, 0, 0.12);
        font-size: 12px;
        white-space: nowrap;
        color: var(--_mosaica-ink-2);
      }
      [part='brush'] b {
        color: var(--_mosaica-ink);
        font-weight: 600;
      }
      .chosen:empty {
        display: none;
      }
      /* The two fields share the width the range's words and the buttons give the box, each wide
         enough for a date. */
      .ends {
        display: flex;
        align-items: center;
        gap: 6px;
      }
      .ends input {
        flex: 1 1 0;
        width: 0;
        min-width: 84px;
        height: 26px;
        padding: 0 6px;
        font-size: 12px;
      }
      /* A field that is not applied: a heavier border, with the words under it. */
      .ends input[aria-invalid='true'] {
        padding: 0 5px;
        border: 2px solid var(--_mosaica-refuse);
      }
      /* The words wrap inside the box, and make it no wider. With nothing to say they take no room. */
      .mistakes {
        display: flex;
        flex-direction: column;
        margin-top: -8px;
      }
      .mistakes span:not(:empty) {
        width: 0;
        min-width: 100%;
        margin-top: 8px;
        white-space: normal;
        color: var(--_mosaica-refuse);
        font-weight: 500;
      }
      [part='brush'] .acts {
        display: flex;
        gap: 6px;
      }
      [part='brush'] .acts button {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 5px 10px;
        border-radius: 6px;
        font-size: 12px;
        font-weight: 500;
      }
      [part='brush-filter'] {
        background: var(--_mosaica-accent);
        color: var(--_mosaica-accent-ink);
      }
      [part='brush-highlight'] {
        border: 1px solid color-mix(in srgb, var(--_mosaica-highlight) 18%, var(--_mosaica-highlight-soft)) !important;
        background: var(--_mosaica-highlight-soft);
        color: var(--_mosaica-highlight);
      }
      .text {
        margin-top: 6px;
      }
      /* Without a colouring, a bar is not content, and takes a system colour. */
      @media (forced-colors: active) {
        [part='bar-match'],
        [part='spark'] span {
          background: GrayText;
        }
        [part='bar-subject'],
        :host([compact]) [part='spark'] span {
          background: CanvasText;
        }
        :host([highlighting]) [part='bar-subject'] {
          background: Highlight;
        }
      }
    `
  ];

  /** The field: a column `meta.filterOperands` lists, or `cluster:<layer>` for a layer's clusters. */
  @property() accessor field = '';
  /** Shows the heading alone, with a small chart of the counts in view. */
  @property({type: Boolean, reflect: true}) accessor folded = false;
  /** Draws a folded card in the compact style the explorer uses in narrow containers. */
  @property({type: Boolean, reflect: true}) accessor compact = false;
  /**
   * The level a card for a layer with levels counts at. `null` is the level the map draws: where no
   * level declares a zoom range, the finest that is not crowded in view, else the coarsest served.
   */
  @property({type: Number, attribute: 'cluster-level'}) accessor level: number | null = null;

  /** Whether "N more" was pressed. @internal */
  @state() accessor expanded = false;
  /**
   * Where a cluster card that steps stands in its layer: the clusters stepped into, the deepest
   * last, and none for the top level. @internal
   */
  @state() accessor position: readonly Step[] = [];
  /** What the card says loaded after the last step. @internal */
  @state() accessor said = '';
  /** A range being chosen on the histogram. @internal */
  @state() accessor brush: Brush | null = null;
  /** The range box's two fields, while it is open. @internal */
  @state() accessor entry: RangeEntry | null = null;
  /** The part that opened the range box, which takes focus as it closes. */
  private entryOpener: 'plot' | 'range' = 'plot';
  /** The ends of the last range the box could apply, as shares of the plot's width, which place the box. */
  private boxAt: [number, number] = [0, 1];
  /** The control in the card that held focus as a render began, and the row it was in. */
  private focusedBefore: {el: HTMLElement; holder: string; row: number} | null = null;
  /**
   * A step whose rows have not landed: the cluster stepped out of, whose row then takes focus, or
   * `null` for a step in, after which the first row does. `fresh` until the render after the step.
   */
  private landing: {out: bigint | null; fresh: boolean} | null = null;

  private readonly subjectCounts = new HeldAggregate('field-subject');
  private readonly matchCounts = new HeldAggregate('field-match');
  private readonly figures = new HeldAggregate('field-summary');
  /** The whole match's counts of the values in view its commonest do not list. */
  private readonly outsideCounts = new HeldAggregate('field-outside');
  private unwatchChoices: (() => void) | null = null;
  /** A linked layer's rows: the children of the position, or the roots, a page at a time. */
  private readonly lineage = new Lineage({
    store: () => this.resolvedStore,
    layer: () => {
      const layer = this.declaredLayer(this.resolvedStore?.get('meta') ?? null);
      return layer && drills(layer) ? layer : null;
    },
    limit: () => PAGE_ROWS,
    isOpen: () => false,
    changed: () => this.requestUpdate()
  });
  /** The items of the position in none of its children, once every child has loaded. */
  private readonly remainder = new HeldAggregate('field-remainder');
  private remainderAsked: AggregateSpec | null = null;
  /** The counts in view asked last. */
  private subjectAsked: AggregateSpec | null = null;
  private readonly picker = new ColourPicker(this, () => this.getBoundingClientRect());

  /** What the card last drew from, so a publish that changes none of it draws nothing. */
  private drawnFrom: readonly unknown[] = [];

  /**
   * Draw again only when what the card reads changed: its counts, the clauses, the drawn region,
   * the colouring, the view, the palette and chosen colours a layer's clusters take, or the level a
   * levelled layer is drawn at. The store publishes as each frame arrives, which changes none of
   * these.
   */
  protected override onStoreChange(): void {
    const s = this.resolvedStore;
    const layer = this.declaredLayer(s?.get('meta') ?? null);
    const now = s ? [s.get('aggregates'), s.get('filters'), s.get('region'), s.get('legend'), s.get('meta'), s.get('view').id, s.get('artifacts').palette, s.get('artifacts').overrides, layer ? this.levelOf(layer) : undefined] : [];
    if (now.length === this.drawnFrom.length && now.every((v, i) => v === this.drawnFrom[i])) return;
    this.drawnFrom = now;
    super.onStoreChange();
  }

  protected override onStoreAdopted(store: Store | null): void {
    this.drawnFrom = [];
    this.unwatchChoices?.();
    this.unwatchChoices = store ? watchChoices(store, () => this.requestUpdate()) : null;
  }

  protected override resetServerData(): void {
    this.lineage.reset();
    this.position = [];
    this.landing = null;
    this.said = '';
    this.brush = null;
    this.entry = null;
    this.picker.close(false);
  }

  override disconnectedCallback(): void {
    this.subjectCounts.set(null, null);
    this.matchCounts.set(null, null);
    this.outsideCounts.set(null, null);
    this.figures.set(null, null);
    this.remainder.set(null, null);
    document.removeEventListener('pointerdown', this.onPressOutsideBrush, true);
    super.disconnectedCallback();
  }

  /** The layer a cluster card shows; `null` on a column's card. */
  private get layerName(): string | null {
    return this.field.startsWith(CLUSTER_PREFIX) ? this.field.slice(CLUSTER_PREFIX.length) : null;
  }

  private declaredLayer(meta: Meta | null): Layer | null {
    const name = this.layerName;
    return name === null ? null : (meta?.layers.find((l) => l.name === name) ?? null);
  }

  /** What the card draws, from the field's family in `meta`; `null` where `meta` offers no such field. */
  private kindOf(meta: Meta | null): Kind | null {
    if (!meta) return null;
    if (this.layerName !== null) return this.declaredLayer(meta) ? 'cluster' : null;
    const o = meta.filterOperands.find((x) => x.column === this.field);
    if (!o) return null;
    if (o.family === 'numeric') return isBoolColumn(meta, this.field) ? 'bool' : 'histogram';
    return o.family === 'category' ? 'category' : 'search';
  }

  private isTimestamp(meta: Meta | null): boolean {
    return meta?.declaredScalars.find((c) => c.name === this.field)?.arrowType === 'timestamp_us';
  }

  /** Whether the map can be coloured by this field. */
  private colourable(meta: Meta, kind: Kind): boolean {
    const name = this.layerName;
    if (name !== null) return colourLayers(meta.layers).some((l) => l.name === name);
    return (kind === 'category' || kind === 'histogram') && meta.declaredScalars.some((c) => c.name === this.field && c.render);
  }

  /** The level a levelled layer's card counts and browses at: the one chosen, else the one the map draws. */
  private levelOf(layer: Layer): number | undefined {
    const s = this.resolvedStore;
    return levelOf(layer, this.level, s?.get('artifacts').levelsInView, s?.clusterBudget ?? null);
  }

  /** How the card lists `layer`. */
  private listing(layer: Layer): Listing {
    if (layer.hierarchy.kind === 'flat') return 'ranked';
    return drills(layer) ? 'children' : 'overlap';
  }

  /** A cluster's name as a page or a clause gave it, or `null` where none has. */
  private knownName(store: Store, id: bigint): string | null {
    return this.lineage.knownName(id) ?? this.clauseName(store, id);
  }

  /**
   * The level a ranked list counts at. The overlap list counts at the level after its position's,
   * and at the chosen or drawn level at the top.
   */
  private listLevel(layer: Layer): number | undefined {
    const deepest = this.listing(layer) === 'overlap' ? this.position.at(-1) : undefined;
    return deepest ? levelAfter(layer, deepest.rung) : this.levelOf(layer);
  }

  /**
   * The rows a category or flat layer's card lists first, whatever their counts: those its clauses
   * name. A card that steps shows its layer's clauses apart from its rows.
   */
  private pinned(store: Store): string[] {
    const layer = this.declaredLayer(store.get('meta'));
    return layer && this.listing(layer) !== 'ranked' ? [] : [...this.clauseKeys(store, 'filter'), ...this.clauseKeys(store, 'highlight')];
  }

  /** Point the browse pages at the position, with slots while the map is coloured by the layer, and fetch them where that moved. */
  private browseIfStale(): void {
    const s = this.resolvedStore;
    const layer = s && this.isConnected ? this.declaredLayer(s.get('meta')) : null;
    if (!s || !layer || !drills(layer)) return;
    this.lineage.top = this.position.at(-1)?.id ?? null;
    this.lineage.paletteSize = s.get('legend').colourBy === this.field ? paletteSize(s.get('artifacts').palette) : null;
    this.lineage.loadIfStale();
  }

  /**
   * The count of the position's items in none of its children, under every filter: a filter of its
   * own holds the position's `member_of` and one `none_of` over the children. Asked only once every
   * child has loaded.
   */
  private remainderSpec(layer: Layer): AggregateSpec | null {
    const deepest = this.position.at(-1);
    const {roots} = this.lineage;
    if (this.folded || this.listing(layer) !== 'children' || !deepest || !roots || roots.length === 0 || !this.lineage.complete) return null;
    const leaf = (artifact: bigint) => memberLeaf({layer: layer.name, artifact, outside: false, verb: 'filter'});
    const ids = [...new Set(roots.map((n) => n.row.mosaicaId))].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
    return {groupings: [{count: true}], filters: {all_of: [leaf(deepest.id), {none_of: ids.map(leaf)}]}};
  }

  /** The filter that keeps an overlap list inside its position, where it has one. */
  private within(layer: Layer): Pick<AggregateSpec, 'filters'> {
    const deepest = this.position.at(-1);
    return deepest ? {filters: memberLeaf({layer: layer.name, artifact: deepest.id, outside: false, verb: 'filter'})} : {};
  }

  /** The keys of the field's clause in `verb`: a category's values, a bool's, or a layer's clusters by id. */
  private clauseKeys(store: Store, verb: ClauseVerb): Set<string> {
    const name = this.layerName;
    if (name !== null) return new Set(store.get('filters').members.filter((m) => m.layer === name && m.verb === verb && !m.outside).map((m) => idString(m.artifact)));
    const held = store.get('filters').draft[verb][this.field];
    if (isBoolColumn(store.get('meta'), this.field)) return new Set(boolKeys(held));
    return new Set(held?.family === 'category' ? held.keys : []);
  }

  /** The groupings a category or cluster card counts; the values its clauses name are always counted. */
  private rowGroupings(store: Store, meta: Meta, kind: Kind, top: number): AggregateSpec['groupings'] | null {
    const named = [...new Set(this.pinned(store))];
    if (kind === 'category') {
      const values = named.slice(0, meta.selection.maxAggregateNamed).sort();
      return [{count: true, by: {field: this.field, top}}, ...(values.length > 0 ? [{count: true as const, by: {field: this.field, values}}] : [])];
    }
    const layer = this.declaredLayer(meta);
    if (!layer) return null;
    return [this.ranked(layer, top), ...this.namedGroupings(layer, meta, named.map((id) => BigInt(id)))];
  }

  /** The grouping ranking a layer's `top` clusters at the cut the map draws or at {@link listLevel}; each row's slot colours its swatch. */
  private ranked(layer: Layer, top: number): AggregateSpec['groupings'][number] {
    return rankedGrouping(layer, top, this.listLevel(layer) ?? null);
  }

  /** The groupings counting `ids` of `layer`, each at the level the list counts at, by name, leaving room for one more beside them. */
  private namedGroupings(layer: Layer, meta: Meta, ids: bigint[]): AggregateSpec['groupings'] {
    const level = this.listLevel(layer) ?? 0;
    const rows = [...new Set(ids)].map((id) => ({mosaicaId: id, rung: level}));
    return rows.length === 0 ? [] : artifactGroupings(layer, rows, meta.selection, 'drawn').slice(0, meta.selection.maxAggregateGroupings - 1);
  }

  /**
   * The values in view that the whole match's commonest values do not list, which a registration of
   * their own asks for by name, so each row has both its counts. It reads the two answers and
   * neither reads it, so asking does not change what is asked. Nothing is named while either
   * answer is held for another store, as just after the card adopts a new one.
   */
  private outsideSpec(store: Store, meta: Meta): AggregateSpec | null {
    const matchTop = this.matchCounts.entryFor(store)?.result?.tables[0];
    const inView = this.subjectCounts.entryFor(store)?.result?.tables[0];
    if (!matchTop || !inView) return null;
    const listed = new Set(listedGroups(matchTop).map((g) => g.key));
    const named = new Set([...this.clauseKeys(store, 'filter'), ...this.clauseKeys(store, 'highlight')]);
    const values = listedGroups(inView)
      .map((g) => g.key)
      .filter((k) => !listed.has(k) && !named.has(k))
      .slice(0, meta.selection.maxAggregateNamed)
      .sort();
    return values.length === 0 ? null : {groupings: [{count: true, by: {field: this.field, values}}], without: this.field};
  }

  /**
   * The specs the card keeps registered: the subject's counts, the whole match's and a number's
   * figures. They are read in a render as well, so the overlap list reads only the answers to what
   * its position asks.
   */
  private specs(store: Store, meta: Meta, kind: Kind): {subject: AggregateSpec | null; match: AggregateSpec | null; figures: AggregateSpec | null} {
    const layer = this.declaredLayer(meta);
    const listing = layer ? this.listing(layer) : null;
    // A children list counts from its browse pages, and the overlap list under every clause.
    if (kind === 'search' || listing === 'children') return {subject: null, match: null, figures: null};
    const leaveOut = listing === 'overlap' ? this.within(layer!) : this.layerName !== null ? {withoutMembersOf: this.layerName} : {without: this.field};
    if (kind === 'histogram' || kind === 'bool') {
      const bins: AggregateSpec['groupings'] = [{count: true, by: {field: this.field, bins: kind === 'bool' ? BOOL_KEYS.length : BINS, sample: SAMPLE}}];
      return {
        subject: {groupings: bins, subject: 'view', highlighted: true, ...leaveOut},
        match: this.folded ? null : {groupings: bins, ...leaveOut},
        figures: kind === 'bool' ? null : {groupings: [{summary: [this.field]}], subject: 'visible'}
      };
    }
    const groupings = this.rowGroupings(store, meta, kind, this.expanded ? MORE_ROWS : ROWS);
    const subject = groupings ? {groupings, subject: 'view' as const, highlighted: true, ...leaveOut} : null;
    // A layer's whole match counts the clusters the subject lists and its clauses name, and ranks
    // one more, whose table says how many there are; the search box names that number.
    const answered = listing === 'overlap' ? this.subjectCounts.entryOf(store, subject) : this.subjectCounts.entryFor(store);
    const listed = [...(countsByKey(answered)?.keys() ?? []), ...this.pinned(store)];
    const match =
      kind === 'category'
        ? this.rowGroupings(store, meta, kind, Math.min(MATCH_TOP, meta.selection.maxAggregateTop))
        : layer
          ? [...this.namedGroupings(layer, meta, listed.map((id) => BigInt(id))), this.ranked(layer, 1)]
          : null;
    return {
      subject,
      match: match && !this.folded ? {groupings: match, ...leaveOut} : null,
      figures: null
    };
  }

  protected override willUpdate(changed: PropertyValues<this>): void {
    super.willUpdate(changed);
    if (changed.has('field')) {
      this.position = [];
      this.landing = null;
      this.said = '';
    }
    // Before the render, so the rows of a position stepped out of are not drawn under the new one.
    this.browseIfStale();
    this.land();
    // The element of the card that holds focus, itself or in a search box's own shadow root.
    let el = this.isConnected ? document.activeElement : null;
    while (el && el.getRootNode() !== this.shadowRoot) el = el.shadowRoot?.activeElement ?? null;
    const row = el?.closest(HOLDERS) ?? null;
    const holder = row?.getAttribute('part') ?? '';
    this.focusedBefore = el ? {el: el as HTMLElement, holder, row: row ? Array.from(this.renderRoot.querySelectorAll(`[part="${holder}"]`)).indexOf(row) : -1} : null;
  }

  /** The deepest crumb's button. */
  private deepestCrumb(): HTMLElement | null {
    return this.renderRoot.querySelector<HTMLElement>('[part="crumb-name"][aria-current]');
  }

  /**
   * Give focus back where the render took it away. A row, crumb or chip that moved, as its value
   * joined or left a clause, keeps it; for one that went, the same button of the row, crumb or chip
   * now in its place takes it, else the one before's, else the search box, else the fold button of
   * a card with none. Focus in the search box stays there. Just after a step, focus in the card
   * goes to the deepest crumb until the rows land.
   */
  private keepFocus(): void {
    const before = this.focusedBefore;
    this.focusedBefore = null;
    if (this.landing?.fresh) {
      this.landing.fresh = false;
      if (before) this.deepestCrumb()?.focus();
      return;
    }
    if (!before || !focusLost()) return;
    if (before.el.isConnected) return before.el.focus();
    const search = this.renderRoot.querySelector<HTMLElement>('.search');
    if (before.row < 0) {
      if (before.el.classList.contains('search')) search?.focus();
      return;
    }
    const rows = Array.from(this.renderRoot.querySelectorAll(`[part="${before.holder}"]`));
    const same = (rows[before.row] ?? rows[before.row - 1])?.querySelector<HTMLElement>(`[part="${before.el.getAttribute('part')}"]`);
    (same ?? search ?? this.renderRoot.querySelector<HTMLElement>('[part="fold"]'))?.focus();
  }

  /**
   * The keys of the rows a card that steps lists for its position, once they have landed; `null`
   * while they are on their way.
   */
  private landed(s: Store, layer: Layer): string[] | null {
    if (this.listing(layer) === 'children') return this.lineage.settled ? this.lineage.roots!.map((n) => idString(n.row.mosaicaId)) : null;
    const entry = this.subjectCounts.entryOf(s, this.subjectAsked);
    if (!entry?.result && !entry?.refusal) return null;
    return this.rows(s, 'cluster', entry, undefined, false).map((r) => r.key);
  }

  /**
   * Once the rows of a step have landed, say what loaded, and once they are drawn move focus from
   * the deepest crumb to the row stepped out of or, after a step in, to the first row. Focus that
   * has gone elsewhere meanwhile stays there.
   */
  private land(): void {
    const landing = this.landing;
    const s = this.resolvedStore;
    const layer = s ? this.declaredLayer(s.get('meta')) : null;
    // The render of the step itself still holds the answers to the position before.
    if (!landing || landing.fresh || !s || !layer) return;
    const keys = this.landed(s, layer);
    if (keys === null) return;
    this.landing = null;
    this.said = this.loadedText(layer, keys.length);
    const crumb = this.deepestCrumb();
    void this.updateComplete.then(() => {
      const active = this.shadowRoot?.activeElement ?? null;
      if (active !== crumb && !focusLost()) return;
      const row = (key: string) => this.renderRoot.querySelector(`[part="row"][data-key="${key}"]`);
      const to = landing.out !== null ? row(idString(landing.out))?.querySelector<HTMLElement>('[part="step"]') : keys[0] === undefined ? null : row(keys[0])?.querySelector<HTMLElement>('[part="highlight"]');
      to?.focus();
    });
  }

  /** What a card that steps says has loaded: how many rows, of what. */
  private loadedText(layer: Layer, n: number): string {
    const deepest = this.position.at(-1);
    if (this.listing(layer) === 'overlap') return `${this.overlapTitle(layer) ?? layer.title ?? layer.name}: ${counted(n, 'cluster', 'clusters')}`;
    if (this.lineage.refusal) return '';
    if (!deepest) return counted(n, 'top-level cluster', 'top-level clusters');
    const total = deepest.children !== null && deepest.children > n ? ` of ${deepest.children.toLocaleString('en-GB')}` : '';
    return `${n.toLocaleString('en-GB')}${total} ${n === 1 && !total ? 'child' : 'children'} of ${deepest.name}`;
  }

  protected override updated(changed: PropertyValues<this>): void {
    super.updated(changed);
    this.keepFocus();
    const s = this.resolvedStore;
    const meta = s?.get('meta') ?? null;
    const kind = this.kindOf(meta);
    const live = this.isConnected && s !== null && meta !== null && kind !== null;
    const layer = live ? this.declaredLayer(meta) : null;
    // A children list's counts ignore the highlight, so they are not drawn as highlighted counts.
    this.toggleAttribute('highlighting', s !== null && this.highlighting(s) && !(layer && this.listing(layer) === 'children'));
    const specs = live ? this.specs(s, meta, kind) : {subject: null, match: null, figures: null};
    this.subjectAsked = specs.subject;
    this.remainderAsked = layer ? this.remainderSpec(layer) : null;
    this.subjectCounts.set(s, specs.subject);
    this.remainder.set(s, this.remainderAsked);
    this.matchCounts.set(s, specs.match);
    this.outsideCounts.set(s, live && kind === 'category' && !this.folded ? this.outsideSpec(s, meta) : null);
    this.figures.set(s, specs.figures);
    if (changed.has('entry')) {
      if (this.entry) document.addEventListener('pointerdown', this.onPressOutsideBrush, true);
      else document.removeEventListener('pointerdown', this.onPressOutsideBrush, true);
    }
  }

  /** A press outside an open range box closes it, as Escape does; Set range closes it itself. */
  private onPressOutsideBrush = (e: PointerEvent): void => {
    const path = e.composedPath();
    if (Array.from(this.renderRoot.querySelectorAll('[part="brush"], [part="range"]')).some((el) => path.includes(el))) return;
    this.entry = null;
  };

  /** Whether a highlight is set, in which case the subject is the highlighted items. */
  private highlighting(store: Store): boolean {
    const {draft, members} = store.get('filters');
    return Object.values(draft.highlight).some((d) => isPopulated(d)) || members.some((m) => m.verb === 'highlight');
  }

  // ---- clauses ---------------------------------------------------------------------------------

  /** Put a category value in or out of the field's clause in `verb`. */
  private toggleValue(key: string, verb: ClauseVerb): void {
    const s = this.resolvedStore;
    if (!s) return;
    const draft = s.get('filters').draft;
    const held = draft[verb][this.field];
    const was = held?.family === 'category' ? held.keys : [];
    const keys = was.includes(key) ? was.filter((k) => k !== key) : [...was, key];
    this.setClause(verb, {family: 'category', keys});
  }

  /** Put a bool's value in or out of the field's clause in `verb`. */
  private toggleBool(key: BoolKey, verb: ClauseVerb): void {
    const s = this.resolvedStore;
    if (!s) return;
    const was = boolKeys(s.get('filters').draft[verb][this.field]);
    this.setClause(verb, boolClause(was.includes(key) ? was.filter((k) => k !== key) : [...was, key]));
  }

  /** Replace the field's clause in `verb`, and report what that position composes. */
  private setClause(verb: ClauseVerb, clause: ColumnDraft): void {
    const s = this.resolvedStore;
    if (!s) return;
    const draft = s.get('filters').draft;
    const next: FilterDraft = {...draft, [verb]: {...draft[verb], [this.field]: clause}};
    s.setFilters(next);
    emit(this, 'mosaica-filterchange', {column: this.field, verb, expr: composeFilters(next, verb)});
  }

  /** Put a cluster's `member_of` clause in `verb` on or off. */
  private toggleCluster(id: string, name: string | null, verb: ClauseVerb): void {
    const s = this.resolvedStore;
    const layer = this.layerName;
    if (!s || layer === null) return;
    const artifact = BigInt(id);
    const members = s.get('filters').members;
    const on = members.some((m) => m.layer === layer && m.artifact === artifact && m.verb === verb && !m.outside);
    s.setMembers(on ? withoutMember(members, layer, artifact, verb) : withMember(members, {layer, artifact, outside: false, verb, ...(name === null ? {} : {label: name})}));
    emit(this, 'mosaica-clausechange', {id, layer, outside: false, verb, on: !on});
  }

  /** Take one of this layer's clauses off. */
  private removeClause(clause: MemberClause): void {
    const s = this.resolvedStore;
    const layer = this.layerName;
    if (!s || layer === null) return;
    s.setMembers(withoutMember(s.get('filters').members, layer, clause.artifact, clause.verb));
    emit(this, 'mosaica-clausechange', {id: idString(clause.artifact), layer, outside: clause.outside, verb: clause.verb, on: false});
  }

  /** Step into a cluster, so the card lists what is inside it. The filter is left as it is. */
  private stepIn(step: Step): void {
    this.position = [...this.position, step];
    this.stepped(null);
  }

  /** Step back to the crumb at `depth`, where 0 is All. */
  private stepTo(depth: number): void {
    if (depth >= this.position.length) return;
    const out = this.position[depth]!.id;
    this.position = this.position.slice(0, depth);
    this.stepped(out);
  }

  private stepped(out: bigint | null): void {
    this.landing = {out, fresh: true};
    this.said = '';
    this.expanded = false;
  }

  /** Colour the map by this field, or by nothing where it already is. */
  private paint(): void {
    const s = this.resolvedStore;
    if (!s) return;
    const colourBy = s.get('legend').colourBy === this.field ? null : this.field;
    s.setColourBy(colourBy);
    emit(this, 'mosaica-colourchange', {colourBy});
  }

  private chooseLevel(value: string): void {
    const layer = this.layerName;
    if (layer === null) return;
    const level = value === '' ? null : Number(value);
    this.level = level;
    emit(this, 'mosaica-levelchange', {layer, level});
  }

  private fold(): void {
    this.folded = !this.folded;
    this.brush = null;
    this.entry = null;
    emit(this, 'mosaica-fold', {field: this.field, folded: this.folded});
  }

  // ---- render ----------------------------------------------------------------------------------

  override render(): TemplateResult | typeof nothing {
    const s = this.resolvedStore;
    const meta = s?.get('meta') ?? null;
    const kind = this.kindOf(meta);
    if (!s || !meta || !kind) return nothing;
    const layer = this.declaredLayer(meta);
    const title = layer ? layer.title || layer.name : columnCaption(this.field);
    const colouring = s.get('legend').colourBy === this.field;
    const paint = this.colourable(meta, kind)
      ? html`<button part="paint" type="button" aria-pressed=${colouring ? 'true' : 'false'} aria-label=${`Colour by ${title}`} title=${colouring ? 'Stop colouring by this' : 'Colour by this'} @click=${() => this.paint()}>${icon('drop', 13, 2)}</button>`
      : nothing;
    const fold = html`<button part="fold" type="button" aria-expanded=${this.folded ? 'false' : 'true'} aria-label=${this.folded ? `Open ${title}` : `Fold ${title}`} @click=${() => this.fold()}>${icon(this.folded ? 'chev' : 'chevup', 12, 2)}</button>`;
    const head = (sub: TemplateResult | string | typeof nothing) =>
      html`<div part="head"><div class="named"><h3 part="title" id="title">${title}</h3>${this.folded && this.compact && colouring ? html`<span class="painted" role="img" aria-label="Colouring the map">${icon('drop', 11, 2)}</span>` : nothing}${sub}</div>${this.folded ? this.spark(s, kind, colouring) : nothing}${paint}${fold}</div>`;
    const card =
      kind === 'histogram'
        ? this.histogram(s, meta, title, head, colouring)
        : kind === 'search'
          ? html`${head(html`<span part="sub"></span>`)}${this.folded ? nothing : html`<mosaica-filter class="text" exportparts=${exportparts('filter')} column=${this.field} placeholder=${`Search ${title.toLowerCase()}`} .store=${s}></mosaica-filter>`}`
          : this.valueCard(s, kind, layer, title, head, colouring);
    return html`<div role="group" aria-labelledby="title">${card}</div>`;
  }

  /**
   * A folded card's small chart: the subject's counts, largest first for values, in order for bins.
   * A children list charts its first rows as it lists them.
   */
  private spark(s: Store, kind: Kind, colouring: boolean): TemplateResult | typeof nothing {
    const layer = this.declaredLayer(s.get('meta'));
    const entry = this.subjectCounts.entry();
    const table = entry?.result?.tables[0];
    const nodes = layer && this.listing(layer) === 'children' ? this.lineage.roots : null;
    if (!table && !nodes) return html`<span part="spark" aria-hidden="true"></span>`;
    let bars: {n: number; colour: string | null}[];
    if (layer && nodes) {
      bars = this.childRows(s, layer, nodes.slice(0, SPARK_BARS), colouring).map((r) => ({n: r.sub ?? 0, colour: r.swatch}));
    } else if (kind === 'histogram' && table) {
      const bins = binsOf(table, this.isTimestamp(s.get('meta')));
      const per = Math.max(1, Math.ceil(bins.length / SPARK_BARS));
      bars = [];
      for (let i = 0; i < bins.length; i += per) bars.push({n: bins.slice(i, i + per).reduce((t, b) => t + b.count, 0), colour: null});
    } else {
      const rows = this.rows(s, kind, entry, undefined, colouring).slice(0, SPARK_BARS);
      bars = rows.map((r) => ({n: r.sub ?? 0, colour: r.swatch}));
    }
    const top = Math.max(1, ...bars.map((b) => b.n));
    return html`<span part="spark" aria-hidden="true">${bars.map((b) => html`<span style=${`height:${Math.max(1, Math.round((b.n / top) * 14))}px${b.colour ? `;background:${b.colour}` : ''}`}></span>`)}</span>`;
  }

  /**
   * The rows of a bool card, yes then no; or of a category or cluster card: the values its
   * clauses name, then the rest by their count in the subject, then in the whole match.
   */
  private rows(s: Store, kind: Kind, subject: AggregateEntry | undefined, match: AggregateEntry | undefined, colouring: boolean): Row[] {
    const sub = this.counts(kind, subject);
    const all = this.counts(kind, match);
    if (kind === 'bool') {
      return BOOL_KEYS.map((key) => ({key, name: BOOL_WORDS[key], sub: sub?.get(key) ?? null, all: all?.get(key) ?? null, swatch: null, own: null, unmet: false}));
    }
    if (all && kind === 'category') for (const [k, n] of countsByKey(this.outsideCounts.entryFor(s)) ?? []) if (!all.has(k)) all.set(k, n);
    const named = this.pinned(s);
    if (kind === 'category') {
      const titles = new Map<string, string | null>();
      for (const t of [...(subject?.result?.tables ?? []), ...(match?.result?.tables ?? [])]) for (const g of listedGroups(t)) if (!titles.has(g.key) || g.title) titles.set(g.key, g.title);
      const order = [...new Set([...named, ...(sub ? [...sub.keys()] : []), ...(all ? [...all.keys()] : [])])];
      const keys = order.filter((k) => named.includes(k) || (sub?.get(k) ?? 0) > 0 || (!sub && (all?.get(k) ?? 0) > 0));
      keys.sort((a, b) => Number(named.includes(b)) - Number(named.includes(a)) || (sub?.get(b) ?? 0) - (sub?.get(a) ?? 0) || (all?.get(b) ?? 0) - (all?.get(a) ?? 0));
      return keys.map((key) => ({
        key,
        name: titles.get(key) ?? keyTitle(s, this.field, key),
        sub: sub?.get(key) ?? null,
        all: all?.get(key) ?? null,
        swatch: colouring ? valueColour(s, this.field, key) : null,
        own: null,
        unmet: colouring && colouringOf(s).values[this.field]?.[key] === undefined && !valueMet(s, this.field, key)
      }));
    }
    const {palette, overrides} = s.get('artifacts');
    const titles = new Map<string, string>();
    // Each slot with the palette its answer was asked for, which holds while the next is asked for.
    const slots = new Map<string, {slot: number; palette: PaletteName}>();
    for (const entry of [subject, match]) {
      for (const t of entry?.result?.tables ?? []) {
        for (const g of listedGroups(t)) {
          if (g.title) titles.set(g.key, g.title);
          if (g.slot !== null) slots.set(g.key, {slot: g.slot, palette: entry!.palette ?? palette});
        }
      }
    }
    const ids = [...new Set([...named, ...(sub ? [...sub.keys()] : [])])];
    const keys = ids.filter((k) => named.includes(k) || (sub?.get(k) ?? 0) > 0);
    keys.sort((a, b) => Number(named.includes(b)) - Number(named.includes(a)) || (sub?.get(b) ?? 0) - (sub?.get(a) ?? 0) || (all?.get(b) ?? 0) - (all?.get(a) ?? 0));
    return keys.map((key) => {
      const id = BigInt(key);
      const held = slots.get(key);
      const own = artifactColour(held?.palette ?? palette, held?.slot ?? null);
      const swatch = colouring ? rgb(overrides.get(this.layerName!)?.get(id) ?? own) : null;
      return {key, name: titles.get(key) ?? this.clauseName(s, id) ?? UNNAMED, sub: sub?.get(key) ?? null, all: all?.get(key) ?? null, swatch, own: colouring ? hexOf(own) : null, unmet: false};
    });
  }

  /** Each row's count in `entry`: a bool's from its bins for false and true, else by each group's key. */
  private counts(kind: Kind, entry: AggregateEntry | undefined): Map<string, number> | null {
    if (kind !== 'bool') return countsByKey(entry);
    const table = entry?.result?.tables[0];
    if (!table) return null;
    return new Map(BOOL_KEYS.map((k) => [k, binsOf(table, false).find((b) => b.lower === BOOL_NUMBERS[k])?.count ?? 0]));
  }

  /** The name a cluster's clause carries, for a cluster no table lists. */
  private clauseName(s: Store, id: bigint): string | null {
    return s.get('filters').members.find((m) => m.artifact === id && m.layer === this.layerName)?.label ?? null;
  }

  private valueCard(s: Store, kind: Kind, layer: Layer | null, title: string, head: (sub: TemplateResult | typeof nothing) => TemplateResult, colouring: boolean): TemplateResult {
    const levelled = layer ? this.levelOf(layer) : undefined;
    // Follow zoom is offered where no level declares a zoom range, and is chosen while no level is.
    const follows = layer !== null && levelFollowsZoom(layer);
    const following = follows && !layer.levels.some((l) => l.level === this.level);
    const sub =
      layer && levelled !== undefined && layer.levels.length > 1
        ? html`<span part="sub" class="choice"><select part="level-select" aria-label=${`${title} level`} @change=${(e: Event) => this.chooseLevel((e.target as HTMLSelectElement).value)}>
              ${follows ? html`<option value="" ?selected=${following}>Follow zoom</option>` : nothing}
              ${layer.levels.map((l) => html`<option value=${l.level} ?selected=${!following && levelled === l.level}>${l.title || `Level ${l.level}`}</option>`)}
            </select>${icon('chev', 12, 1.4)}</span>`
        : html`<span part="sub"></span>`;
    // One template under the heading, whatever the card lists, so the heading, the search box and
    // the breadcrumb are the same elements as the list below them changes.
    const body = this.folded ? nothing : layer && this.listing(layer) !== 'ranked' ? this.clusterBody(s, layer, title, colouring) : this.valueBody(s, kind, layer, title, colouring);
    return html`${head(sub)}${body}${this.folded ? nothing : this.picker.render()}`;
  }

  /** The body of a category, bool or `flat` layer's card: the search box, the rows and More. */
  private valueBody(s: Store, kind: Kind, layer: Layer | null, title: string, colouring: boolean): TemplateResult {
    const {list, after, values} = this.rankedList(s, kind, layer, title, colouring);
    const search =
      kind === 'category'
        ? html`<mosaica-filter class="search" exportparts=${exportparts('filter')} column=${this.field} placeholder=${values === null ? 'Search values' : `Search ${values.toLocaleString('en-GB')} values`} .store=${s}></mosaica-filter>`
        : kind === 'bool'
          ? nothing
          : this.clusterSearch(s, layer!, values);
    return html`<div class="body">${search}${list}${after}</div>`;
  }

  /**
   * The body of a card that steps: the search box, the breadcrumb, the layer's clauses, the one
   * status region, and the rows of the position with what follows them.
   */
  private clusterBody(s: Store, layer: Layer, title: string, colouring: boolean): TemplateResult {
    const children = this.listing(layer) === 'children';
    const {list, after} = children ? this.childList(s, layer, colouring) : this.rankedList(s, 'cluster', layer, title, colouring);
    const {roots, refusal, loading} = this.lineage;
    const note = !children
      ? nothing
      : refusal
        ? html`<span class="none" part="refusal" data-code=${refusal.code}>Clusters unavailable</span>`
        : roots === null
          ? loading
            ? html`<span class="sr-only">Loading</span>`
            : nothing
          : roots.length === 0
            ? html`<span class="none">No clusters</span>`
            : nothing;
    return html`<div class="body">${this.clusterSearch(s, layer, null)}${this.crumbsTemplate(s, title)}${this.chipsTemplate(s, title)}<div class="note" role="status">${note}<span class="sr-only">${this.said}</span></div>${list}${after}</div>`;
  }

  /** What an overlap list inside a position lists, which heads it; `null` at the top. */
  private overlapTitle(layer: Layer): string | null {
    const deepest = this.position.at(-1);
    const level = this.listLevel(layer);
    return deepest && level !== undefined ? `${levelTitle(layer, level)} overlapping ${deepest.name}` : null;
  }

  /**
   * A list ranked by count: a category's or a bool's values, a flat layer's clusters, or the
   * overlap; then what an empty answer says and the More that shows more rows. `values` is how
   * many values or clusters the whole match has, where its answer says.
   */
  private rankedList(s: Store, kind: Kind, layer: Layer | null, title: string, colouring: boolean): {list: TemplateResult; after: TemplateResult; values: number | null} {
    // A filter on the overlap list joins the others with AND, so it leaves no row out.
    const anded = layer !== null && this.listing(layer) === 'overlap';
    // Just after a step, the answers held are to what the position before asked.
    const stale = anded && this.landing !== null;
    const subject = stale ? undefined : this.subjectCounts.entry();
    const match = stale ? undefined : this.matchCounts.entry();
    const all = this.rows(s, kind, subject, match, colouring);
    const filtered = this.clauseKeys(s, 'filter');
    const lit = this.clauseKeys(s, 'highlight');
    const pinned = new Set(this.pinned(s));
    const shown = all.slice(0, Math.max(this.expanded ? MORE_ROWS : ROWS, all.filter((r) => pinned.has(r.key)).length));
    // The shares are of each set's total, or of what the field's own filter keeps where it has one.
    const keptTotal = (counts: Map<string, number> | null, table: AggregateTable | undefined) =>
      !anded && filtered.size > 0 && counts ? [...filtered].reduce((t, k) => t + (counts.get(k) ?? 0), 0) : (table?.total ?? 0);
    const subTotal = keptTotal(this.counts(kind, subject), subject?.result?.tables[0]);
    const allTotal = keptTotal(this.counts(kind, match), match?.result?.tables[0]);
    const same = subTotal === allTotal;
    // A bool's counts are a histogram's, so they may come from a sample.
    const approx = approxMark(subject?.result?.tables[0], match?.result?.tables[0]);
    const highlighting = this.highlighting(s);
    const subjectWord = highlighting ? 'highlighted' : 'in view';
    const pct = (n: number | null, total: number) => (n === null || total <= 0 ? 0 : Math.min(100, (100 * n) / total));
    // A layer's clusters are often a few per cent each; a bar that is not empty stays visible.
    const width = (share: number) => (share > 0 && kind === 'cluster' ? `max(${MIN_BAR_PX}px, ${share.toFixed(1)}%)` : `${share.toFixed(1)}%`);
    const groups = kind === 'category' ? (subject?.result?.tables[0]?.groups ?? null) : all.length;
    const more = groups === null ? 0 : groups - shown.length;
    // On the overlap list, a row at a level with a finer one after it steps into it.
    const level = anded ? this.listLevel(layer) : undefined;
    const next = level === undefined ? undefined : levelAfter(layer!, level);
    const stepper = (r: Row): Stepper | undefined =>
      next === undefined ? undefined : {label: `Open ${r.name}, ${levelTitle(layer!, next)} inside it`, count: null, step: {id: BigInt(r.key), name: r.name, rung: level!, children: null}};

    const row = (r: Row) => {
      const inFilter = filtered.has(r.key);
      const out = !anded && filtered.size > 0 && !inFilter;
      const onLit = lit.has(r.key);
      const sub = out || r.sub === null ? null : approx + countText(r.sub);
      const all = r.all === null || (!out && same) ? null : approx + countText(r.all);
      return this.rowTemplate(r, kind, {
        state: out ? 'out' : inFilter ? 'in' : onLit ? 'lit' : '',
        inFilter,
        onLit,
        drawn: out ? (all ?? '') : null,
        sub,
        all,
        said: [sub === null ? '' : `${sub} ${subjectWord}`, all === null ? '' : `${all} matching`].filter(Boolean).join(', '),
        barMatch: width(out ? 0 : pct(r.all, allTotal)),
        barSubject: width(out ? 0 : pct(r.sub, subTotal)),
        step: stepper(r)
      });
    };
    const values = (kind === 'category' ? match?.result?.tables[0] : match?.result?.tables.at(-1))?.groups ?? null;
    const noun = kind === 'bool' ? 'values' : layer ? `${title} clusters` : `${title} values`;
    const moreText = this.expanded ? 'Show fewer' : `${more.toLocaleString('en-GB')} more${highlighting ? '' : ' in view'}`;
    const moreButton =
      more > 0 || this.expanded
        ? html`<button part="more" class="more-link" type="button" aria-label=${`${moreText}: ${noun}`} @click=${() => (this.expanded = !this.expanded)}>${moreText}</button>`
        : nothing;
    // An answer with nothing in it says so, where a list would otherwise stand empty.
    const empty = shown.length === 0 && subject?.result ? html`<span class="none">${subTotal === 0 ? 'None in view' : 'None counted'}</span>` : nothing;
    const heading = anded ? this.overlapTitle(layer) : null;
    const items = repeat(
      shown,
      (r) => r.key,
      (r) => html`<div role="listitem">${row(r)}</div>`
    );
    const list =
      heading === null
        ? html`<div part="rows" role="list" aria-label=${`${title}: ${kind === 'bool' ? 'values' : layer ? 'commonest clusters' : 'commonest values'}`}>${items}</div>`
        : html`<h4 part="level" id="overlap">${heading}</h4><div part="rows" role="list" aria-labelledby="overlap">${items}</div>`;
    return {list, after: html`${empty}${moreButton}`, values};
  }

  /** The search box over a layer's clusters, which names how many there are where a count says. */
  private clusterSearch(s: Store, layer: Layer, values: number | null): TemplateResult {
    return html`<mosaica-cluster-filter class="search" exportparts=${exportparts('cluster-filter')} layer=${layer.name} placeholder=${values === null ? 'Search clusters' : `Search ${values.toLocaleString('en-GB')} clusters`} .store=${s}></mosaica-cluster-filter>`;
  }

  /**
   * One row of a category, bool or cluster card. `sub` and `all` are its two counts as drawn, or
   * `drawn` alone in their place, and `said` gives them to a screen reader with what each counts.
   * `step` is the button that steps into the row, `null` on a row of a card that steps with nothing
   * to step into. A press on the row filters to its value or cluster, as its Filter button does;
   * the keyboard reaches the buttons.
   */
  private rowTemplate(
    r: Row,
    kind: Kind,
    look: {state: string; inFilter: boolean; onLit: boolean; drawn: string | null; sub: string | null; all: string | null; said: string; barMatch: string; barSubject: string; step?: Stepper | null}
  ): TemplateResult {
    const {inFilter, onLit, step} = look;
    const mark = r.swatch
      ? html`<button part="swatch" type="button" style=${`--c:${r.swatch}`} ?data-unmet=${r.unmet} aria-haspopup="dialog" aria-label=${`Colour of ${r.name}`} @click=${(e: Event) => this.openPicker(e, r, kind)}></button>`
      : inFilter
        ? icon('check', 12, 2.6)
        : nothing;
    const verb = (v: ClauseVerb) => () =>
      kind === 'category' ? this.toggleValue(r.key, v) : kind === 'bool' ? this.toggleBool(r.key as BoolKey, v) : this.toggleCluster(r.key, r.name === UNNAMED ? null : r.name, v);
    // A tick where the filter keeps the value and the swatch stands where the tick would, and a pen where it is highlighted.
    const cues = html`${inFilter && r.swatch ? html`<span class="cue">${icon('check', 11, 2.4)}</span>` : nothing}${onLit ? html`<span class="cue lit">${icon('highlight', 11, 1.6)}</span>` : nothing}`;
    // The figures as they are drawn, and then for a reader, each with what it counts.
    const counts = html`<span part="count" aria-hidden="true" title=${look.said}>${cues}${look.drawn ?? html`<span class="sub">${look.sub ?? ''}</span>${look.all === null ? '' : ` / ${look.all}`}`}</span
      ><span class="sr-only">${look.said}</span>`;
    const stepButton =
      step === undefined
        ? nothing
        : step === null
          ? html`<span class="step"></span>`
          : html`<span class="step"><button part="step" type="button" aria-label=${step.label} title="Open" @click=${() => this.stepIn(step.step)}>${step.count === null ? '' : countText(step.count)}<span aria-hidden="true">›</span></button></span>`;
    return html`<div part="row" class=${step === undefined ? '' : 'steps'} data-key=${r.key} data-state=${look.state}
      @click=${(e: Event) => {
        if (!(e.target as HTMLElement).closest('button')) verb('filter')();
      }}>
      <span class="mark">${mark}</span>
      <span class="label">
        <span class="names"><span part="name" title=${r.name} ?data-unnamed=${r.name === UNNAMED}>${r.name}</span></span>
        <span class="bars" aria-hidden="true"><span part="bar-match" style=${`width:${look.barMatch}`}></span><span part="bar-subject" style=${`width:${look.barSubject}${r.swatch && !r.unmet ? `;background:${r.swatch}` : ''}`}></span></span>
      </span>
      ${counts}
      <span part="verbs">
        <button part="highlight" type="button" aria-pressed=${onLit ? 'true' : 'false'} aria-label=${onLit ? `Stop highlighting ${r.name}` : `Highlight ${r.name}`} title=${onLit ? 'Stop highlighting' : 'Highlight'} @click=${verb('highlight')}>${icon('highlight', 13, 1.5)}</button>
        <button part="filter" type="button" aria-pressed=${inFilter ? 'true' : 'false'} aria-label=${inFilter ? `Stop filtering to ${r.name}` : `Filter to ${r.name}`} title=${inFilter ? 'Stop filtering' : 'Filter'} @click=${verb('filter')}>${icon('filter', 13, 1.6)}</button>
      </span>
      ${stepButton}
    </div>`;
  }

  /** The rows of a children list, in the server's order: each with its matched count over its masked one where a filter is set, else its masked count alone. */
  private childRows(s: Store, layer: Layer, nodes: readonly BrowseNode[], colouring: boolean): Row[] {
    const {palette, overrides} = s.get('artifacts');
    return nodes.map(({row}) => {
      const own = artifactColour(palette, row.slot);
      const masked = Number(row.maskedCount);
      return {
        key: idString(row.mosaicaId),
        name: artifactName(row) ?? UNNAMED,
        sub: row.matchedCount === null ? masked : Number(row.matchedCount),
        all: row.matchedCount === null ? null : masked,
        swatch: colouring ? rgb(overrides.get(layer.name)?.get(row.mosaicaId) ?? own) : null,
        own: colouring ? hexOf(own) : null,
        unmet: false
      };
    });
  }

  /**
   * The rows of a linked layer's position: its children, or the roots, grouped under their levels'
   * headings on a `tiered` layer, with the remainder as the last list's last row; then More.
   */
  private childList(s: Store, layer: Layer, colouring: boolean): {list: TemplateResult | typeof nothing; after: TemplateResult | typeof nothing} {
    const nodes = this.lineage.roots ?? [];
    const deepest = this.position.at(-1);
    const filtered = this.clauseKeys(s, 'filter');
    const lit = this.clauseKeys(s, 'highlight');
    const rows = this.childRows(s, layer, nodes, colouring);
    // Each bar is a share of the largest row's masked count, the solid part what matches.
    const largest = Math.max(1, ...nodes.map((n) => Number(n.row.maskedCount)));
    const width = (n: number | null) => {
      const share = n === null ? 0 : Math.min(100, (100 * n) / largest);
      return share > 0 ? `max(${MIN_BAR_PX}px, ${share.toFixed(1)}%)` : '0%';
    };
    const row = (r: Row, i: number) => {
      const node = nodes[i]!;
      const inFilter = filtered.has(r.key);
      const onLit = lit.has(r.key);
      const also = this.lineage.alsoUnder(node);
      const n = node.row.childCount;
      return html`<div role="listitem">${this.rowTemplate(r, 'cluster', {
        state: inFilter ? 'in' : onLit ? 'lit' : '',
        inFilter,
        onLit,
        drawn: null,
        sub: countText(r.sub ?? 0),
        all: r.all === null ? null : countText(r.all),
        said: spokenCounts(node.row),
        barMatch: width(r.all ?? r.sub),
        barSubject: width(r.sub),
        step: n > 0 ? {label: `Open ${r.name}, ${counted(n, 'child', 'children')}`, count: n, step: {id: node.row.mosaicaId, name: r.name, rung: node.row.rung, children: n}} : null
      })}${also.length > 0 ? html`<div part="also">also under ${also.join(', ')}</div>` : nothing}</div>`;
    };
    const left = this.remainderAsked ? this.remainder.entryOf(s, this.remainderAsked)?.result?.tables[0]?.total : undefined;
    const rest = this.remainderAsked
      ? html`<div role="listitem"><div part="remainder"><span class="mark"></span><span class="rest">Not in any of these</span>${
          left === undefined ? nothing : html`<span part="count" aria-hidden="true"><span class="sub">${countText(left)}</span></span><span class="sr-only">${countText(left)} matching</span>`
        }</div></div>`
      : nothing;
    const scope = deepest ? `children of ${deepest.name}` : 'top level';
    // On a `tiered` layer one parent can have children at several levels, each listed under its title.
    const levels = layer.hierarchy.kind === 'tiered' ? [...new Set(nodes.map((n) => n.row.rung))].sort((a, b) => a - b) : [null];
    const list =
      nodes.length === 0
        ? nothing
        : html`${levels.map((rung, li) => {
            const at = nodes.flatMap((n, i) => (rung === null || n.row.rung === rung ? [i] : []));
            const items = html`${repeat(
              at,
              (i) => rows[i]!.key,
              (i) => row(rows[i]!, i)
            )}${li === levels.length - 1 ? rest : nothing}`;
            return rung === null
              ? html`<div part="rows" role="list" aria-label=${`${layer.title || layer.name}: ${scope}`}>${items}</div>`
              : html`<h4 part="level" id=${`level-${rung}`}>${levelTitle(layer, rung)}</h4><div part="rows" role="list" aria-labelledby=${`level-${rung}`}>${items}</div>`;
          })}`;
    // How many children are still to come, where the position's row was listed with its count.
    const remaining = deepest?.children == null ? 0 : deepest.children - nodes.length;
    const moreText = remaining > 0 ? `${remaining.toLocaleString('en-GB')} more` : 'More';
    const after = this.lineage.hasMore
      ? html`<button part="more" class="more-link" type="button" aria-label=${`${moreText} ${deepest ? `children of ${deepest.name}` : 'top-level clusters'}`} @click=${(e: Event) => void this.moreChildren(e.currentTarget as HTMLElement)}>${moreText}</button>`
      : nothing;
    return {list, after};
  }

  /** The breadcrumb: All and then each step of the position, each a button that steps back to it. */
  private crumbsTemplate(s: Store, title: string): TemplateResult {
    const filtered = this.clauseKeys(s, 'filter');
    const steps: {id: bigint | null; name: string}[] = [{id: null, name: 'All'}, ...this.position];
    return html`<nav part="crumbs" aria-label=${`${title} breadcrumb`}><ol>${repeat(
      steps,
      (step, i) => `${i}:${step.id ?? ''}`,
      (step, i) => {
        const on = step.id !== null && filtered.has(idString(step.id));
        return html`<li part="crumb" data-id=${step.id === null ? '' : idString(step.id)} ?data-filtered=${on}>${i > 0 ? html`<span class="sep" aria-hidden="true">›</span>` : nothing}<button
            part="crumb-name" type="button" title=${step.name} aria-current=${i === steps.length - 1 ? 'location' : nothing} @click=${() => this.stepTo(i)}>${
            on ? html`<span class="cue">${icon('check', 11, 2.4)}</span>` : nothing
          }<span class="crumb-text">${step.name}</span>${on ? html`<span class="sr-only">, filtered to</span>` : nothing}</button></li>`;
      }
    )}</ol></nav>`;
  }

  /** The layer's clauses, in the order they were added, each with a × that removes it. */
  private chipsTemplate(s: Store, title: string): TemplateResult | typeof nothing {
    const clauses = s.get('filters').members.filter((m) => m.layer === this.layerName);
    if (clauses.length === 0) return nothing;
    return html`<ul part="chips" aria-label=${`${title}: filters and highlights`}>${repeat(
      clauses,
      (c) => `${c.verb}:${c.artifact}`,
      (c) => {
        const name = c.label ?? this.knownName(s, c.artifact) ?? UNNAMED;
        const lit = c.verb === 'highlight';
        const said = `${lit ? 'Highlighted' : 'Filtered to'}${c.outside ? ' outside' : ''}`;
        return html`<li part="chip" data-id=${idString(c.artifact)} data-verb=${c.verb}><span class="cue">${lit ? icon('highlight', 11, 1.6) : icon('check', 11, 2.4)}</span><span part="chip-name" title=${name}
            ><span class="sr-only">${said} </span>${c.outside ? 'not ' : ''}${name}</span
          ><button part="chip-remove" type="button" aria-label=${`${lit ? 'Stop highlighting' : 'Stop filtering to'}${c.outside ? ' outside' : ''} ${name}`} title=${lit ? 'Stop highlighting' : 'Stop filtering'} @click=${() => this.removeClause(c)}>${icon('close', 12)}</button></li>`;
      }
    )}</ul>`;
  }

  /** Load the next page of children, and move focus from More to the first row it added, unless focus has gone elsewhere meanwhile. */
  private async moreChildren(button: HTMLElement): Promise<void> {
    const first = (await this.lineage.loadMore())[0];
    if (first) await focusAdded(this, button, () => this.renderRoot.querySelector<HTMLElement>(`[part="row"][data-key="${idString(first.row.mosaicaId)}"] [part="highlight"]`));
  }

  // ---- histogram ---------------------------------------------------------------------------------

  private histogram(s: Store, meta: Meta, title: string, head: (sub: TemplateResult | string | typeof nothing) => TemplateResult, colouring: boolean): TemplateResult {
    const timestamp = this.isTimestamp(meta);
    const column = meta.declaredScalars.find((c) => c.name === this.field);
    const whole = column ? column.arrowType !== 'f32' && column.arrowType !== 'f64' : false;
    const value = (v: number) => (timestamp ? shortDateText(v) : formatNumber(v, whole));
    const summary = this.figures.entry()?.summaries[0]?.get(this.field) ?? null;
    const sub = html`<span part="sub" title=${this.summaryText(summary, timestamp, whole, false)}>${this.summaryText(summary, timestamp, whole, this.folded)}</span>`;
    if (this.folded) return head(sub);
    const subjectTable = this.subjectCounts.entry()?.result?.tables[0];
    const matchTable = this.matchCounts.entry()?.result?.tables[0];
    const subjectBins = binsOf(subjectTable, timestamp);
    const matchBins = binsOf(matchTable, timestamp);
    const bins = matchBins.length > 0 ? matchBins : subjectBins;
    const subTotal = subjectTable?.total ?? 0;
    const allTotal = matchTable?.total ?? 0;
    const subOf = (b: Bin) => subjectBins.find((x) => x.lower === b.lower)?.count ?? 0;
    const allOf = (b: Bin) => matchBins.find((x) => x.lower === b.lower)?.count ?? 0;
    const share = (n: number, total: number) => (total > 0 ? n / total : 0);
    const tallest = Math.max(1e-9, ...bins.map((b) => Math.max(share(allOf(b), allTotal), share(subOf(b), subTotal))));
    const height = (n: number, total: number) => `${((share(n, total) / tallest) * PLOT_HEIGHT).toFixed(1)}px`;
    const approx = approxMark(subjectTable, matchTable);
    const lo = bins[0]?.lower;
    const hi = bins.at(-1)?.upper;
    // A bin holds its lower edge and not its upper, so a whole number's or a date's last value is one below.
    const discrete = whole || timestamp;
    const at = (v: number) => (lo === undefined || hi === undefined || hi <= lo ? 0 : Math.min(1, Math.max(0, (v - lo) / (hi - lo))));
    const pct = (t: number) => `${(t * 100).toFixed(2)}%`;
    const band = (verb: string, from: number, to: number) => html`<span part="band" data-verb=${verb} style=${`left:calc(${pct(from)} - 1px);width:calc(${pct(to - from)} + 2px)`}></span>`;
    const between = (verb: string, gte: number | null, lte: number | null) => band(verb, gte === null ? 0 : at(gte), lte === null ? 1 : at(lte));
    const words = (gte: number | null, lte: number | null) => rangeWords(gte, lte, timestamp, whole);
    const draft = s.get('filters').draft;
    const held = (verb: ClauseVerb) => {
      const clause = draft[verb][this.field];
      return clause?.family === 'numeric' && isPopulated(clause) ? clause : null;
    };
    const clauseBand = (verb: ClauseVerb) => {
      const range = held(verb);
      return range && lo !== undefined ? between(verb, range.gte, range.lte) : nothing;
    };
    // The range each position holds, in words for a reader, since the bands are outlines alone.
    const applied = (verb: ClauseVerb) => {
      const range = held(verb);
      return range ? html`<span class="applied sr-only" data-verb=${verb}>${verb === 'filter' ? 'Filter' : 'Highlight'}: ${words(range.gte, range.lte)}. </span>` : nothing;
    };
    const brush = this.brush;
    const range = brush && bins.length > 0 ? this.brushRange(bins, brush, timestamp, whole) : null;
    const entry = this.entry;
    // The range the box's fields stand for, while it can be applied.
    const typed = entry && entry.from.value !== undefined && entry.to.value !== undefined && !backwards(entry) ? {gte: entry.from.value, lte: entry.to.value} : null;
    const brushBand =
      brush && range
        ? band('brush', Math.min(brush.from, brush.to) / bins.length, (Math.max(brush.from, brush.to) + 1) / bins.length)
        : typed && lo !== undefined
          ? between('brush', typed.gte, typed.lte)
          : nothing;
    const ramp = colouring && bins.length > 0 ? this.ramp(s, bins) : nothing;
    const middle = bins[Math.floor(bins.length / 2)]?.lower;
    const axis =
      lo !== undefined && hi !== undefined
        ? html`<div part="axis"><span>${this.edgeText(lo, lo, hi, timestamp, whole)}</span><span>${middle === undefined ? '' : this.edgeText(middle, lo, hi, timestamp, whole)}</span><span>${this.edgeText(hi, lo, hi, timestamp, whole)}</span></div>`
        : html`<div part="axis"><span class="skel"></span></div>`;

    /** Open the range box on the range being chosen, else on the range the field holds, else on none. */
    const open = (opener: 'plot' | 'range') => {
      const chosen = this.brush && bins.length > 0 ? this.brushRange(bins, this.brush, timestamp, whole) : null;
      const {gte, lte} = chosen ?? held('filter') ?? held('highlight') ?? {gte: null, lte: null};
      const end = (v: number | null): End => ({text: v === null ? '' : endText(v, timestamp), value: v, checked: false});
      this.entryOpener = opener;
      this.brush = null;
      this.entry = {from: end(gte), to: end(lte)};
      // A range that was dragged is ready to apply; one that was not is typed, over what the field holds.
      void this.updateComplete.then(() => {
        const to = this.renderRoot.querySelector<HTMLElement>(chosen ? '[part="brush-filter"]' : '[part="brush-from"]');
        to?.focus();
        if (to instanceof HTMLInputElement) to.select();
      });
    };
    const type = (which: 'from' | 'to', e: Event, checked: boolean) => {
      const text = (e.target as HTMLInputElement).value;
      if (this.entry) this.entry = {...this.entry, [which]: {text, value: endValue(text, timestamp, which === 'to'), checked}};
    };
    const mistake = (end: End, edge: number | undefined): string => {
      if (!end.checked || end.value !== undefined) return '';
      const said = `“${end.text.trim()}”`;
      if (!timestamp) return `${said} is not a number. Type a number such as ${edge ?? 0}${end.text.trim().search(GROUP_MARKS) < 0 ? '' : '; a comma or a space only groups thousands, as in 1,250'}.`;
      const day = shortDateText(edge ?? 0);
      return `${said} is not a date. Type a day such as ${day}, a month such as ${day.replace(/^\d+ /, '')}, or a year.`;
    };
    const last = hi === undefined ? undefined : discrete ? hi - 1 : hi;
    const fromMistake = entry ? mistake(entry.from, lo) : '';
    const toMistake = entry ? mistake(entry.to, last) : '';
    const order = !entry || !backwards(entry) || !(entry.from.checked || entry.to.checked) ? '' : timestamp ? 'From is later than To. Type an earlier From or a later To.' : 'From is above To. Type a lower From or a higher To.';
    const described = (own: string, id: string) => [own ? id : '', order ? 'order-error' : ''].filter(Boolean).join(' ') || nothing;
    const count = typed ? countWithin(bins, typed.gte, typed.lte, discrete) : null;
    // The box stands under the range it holds, and stays where it is while a field holds no range.
    if (typed && lo !== undefined) this.boxAt = [typed.gte === null ? 0 : at(typed.gte), typed.lte === null ? 1 : at(typed.lte)];
    const box = entry
      ? html`<div part="brush" role="dialog" aria-label=${`${title}: range`} style=${this.brushPlace()} @keydown=${this.onBrushKey} @focusout=${this.onBrushLeft}>
          <div class="chosen">${typed ? html`<b>${words(typed.gte, typed.lte)}</b>${count === null ? nothing : html` · ${approx}${countText(count)} items`}` : nothing}</div>
          <div class="ends">
            <input part="brush-from" type="text" autocomplete="off" aria-label=${`${title}: from`} title="From" placeholder=${lo === undefined ? nothing : endText(lo, timestamp)}
              aria-invalid=${fromMistake || order ? 'true' : nothing} aria-describedby=${described(fromMistake, 'from-error')} .value=${entry.from.text}
              @input=${(e: Event) => type('from', e, false)} @change=${(e: Event) => type('from', e, true)} />
            <span aria-hidden="true">–</span>
            <input part="brush-to" type="text" autocomplete="off" aria-label=${`${title}: to`} title="To" placeholder=${last === undefined ? nothing : endText(last, timestamp)}
              aria-invalid=${toMistake || order ? 'true' : nothing} aria-describedby=${described(toMistake, 'to-error')} .value=${entry.to.text}
              @input=${(e: Event) => type('to', e, false)} @change=${(e: Event) => type('to', e, true)} />
          </div>
          <div class="mistakes" role="status"><span id="from-error">${fromMistake}</span><span id="to-error">${toMistake}</span><span id="order-error">${order}</span></div>
          <div class="acts">
            <button part="brush-filter" type="button" @click=${() => this.applyRange('filter')}>${icon('filter', 12, 2)}Filter</button>
            <button part="brush-highlight" type="button" @click=${() => this.applyRange('highlight')}>${icon('highlight', 12, 1.8)}Highlight</button>
          </div>
        </div>`
      : nothing;
    const subjectName = this.highlighting(s) ? 'Highlighted' : 'In view';
    const span = (b: Bin) => `${value(b.lower)} – ${value(b.upper)}`;
    const binTitle = (b: Bin) => `${span(b)}: ${approx}${countText(subOf(b))} ${subjectName.toLowerCase()}, ${approx}${countText(allOf(b))} matching`;
    // The bins' counts for a reader, who has no pointer to hover a bar with.
    const table =
      bins.length === 0
        ? nothing
        : html`<div class="sr-only"><table>
            <caption>${title} by range</caption>
            <thead><tr><th scope="col">Range</th>${subjectTable ? html`<th scope="col">${subjectName}</th>` : nothing}${matchTable ? html`<th scope="col">All matching</th>` : nothing}</tr></thead>
            <tbody>${bins.map(
              (b) => html`<tr><th scope="row">${span(b)}</th>${subjectTable ? html`<td>${approx}${countText(subOf(b))}</td>` : nothing}${matchTable ? html`<td>${approx}${countText(allOf(b))}</td>` : nothing}</tr>`
            )}</tbody>
          </table></div>`;
    // While a range is being chosen, a reader hears it and its count as it moves.
    const said = range ? `${range.text}, ${approx}${countText(range.count)} items` : '';
    return html`${head(sub)}<div class="hist">
        <div part="plot" role="group" tabindex="0" aria-label=${`${title}: the arrow keys choose a range, Shift widens it and Enter opens it; Set range takes one typed`}
          @pointerdown=${(e: PointerEvent) => this.brushStart(e, bins.length)} @pointermove=${(e: PointerEvent) => this.brushMove(e, bins.length)}
          @pointerup=${(e: PointerEvent) => this.brushEnd(e, bins.length, open)} @pointercancel=${() => (this.brush = null)} @keydown=${(e: KeyboardEvent) => this.brushKey(e, bins.length, open)}>
          ${bins.map(
            (b, i) => html`<div part="bin" data-index=${i} title=${binTitle(b)} aria-hidden="true"><span part="bar-match" style=${`height:${height(allOf(b), allTotal)}`}></span><span part="bar-subject" style=${`height:${height(subOf(b), subTotal)}`}></span></div>`
          )}
          ${clauseBand('filter')}${clauseBand('highlight')}${brushBand}
        </div>
        ${ramp}${axis}
        <button part="range" class="btn small sr-only" type="button" aria-haspopup="dialog" aria-expanded=${entry ? 'true' : 'false'} aria-label=${`Set range of ${title}`}
          @click=${() => (this.entry ? (this.entry = null) : open('range'))}>Set range</button>
        ${applied('filter')}${applied('highlight')}${box}${table}
        <span class="sr-only" role="status" aria-live="polite">${said}</span>
      </div>`;
  }

  /** Where the range box stands: under its range, from the range's left end or, on the right half, its right, and never past the plot. */
  private brushPlace(): string {
    const [from, to] = this.boxAt;
    const room = 'calc(100% - 210px)';
    return from + to < 1 ? `left:clamp(0px, ${(from * 100).toFixed(2)}%, ${room})` : `right:clamp(0px, ${((1 - to) * 100).toFixed(2)}%, ${room})`;
  }

  /** The figures in the heading: how many hold a value and their mean, or while folded their range. */
  private summaryText(summary: FieldSummary | null, timestamp: boolean, whole: boolean, folded: boolean): string {
    if (!summary || summary.min === null || summary.max === null) return '';
    const unit = timestamp ? 1000 : 1;
    const min = Number(summary.min) * unit;
    const max = Number(summary.max) * unit;
    if (folded) return `${this.edgeText(min, min, max, timestamp, whole)} – ${this.edgeText(max, min, max, timestamp, whole)}`;
    const mean = summary.mean === null ? '' : ` · mean ${timestamp ? shortDateText(summary.mean * unit) : formatNumber(summary.mean, false)}`;
    return `n ${countText(Number(summary.values))}${mean}`;
  }

  /** An edge on the axis: a date as its year where the range spans years, else its day. */
  private edgeText(v: number, lo: number, hi: number, timestamp: boolean, whole: boolean): string {
    if (!timestamp) return formatNumber(v, whole);
    const years = (hi - lo) / (365.25 * 24 * 3600 * 1e6);
    return years >= 3 ? String(new Date(v / 1000).getUTCFullYear()) : shortDateText(v);
  }

  /** The ramp under the plot: each bin's middle in the colour the map gives it. */
  private ramp(s: Store, bins: Bin[]): TemplateResult {
    const colouring = colouringOf(s);
    const domain = s.get('legend').domains[this.field] ?? {min: bins[0]!.lower, max: bins.at(-1)!.upper};
    const diverging = RAMPS[colouring.ramp].diverging;
    const stops = bins.map((b, i) => `${rgb(colourOfFraction(Math.min(1, Math.max(0, fractionOf((b.lower + b.upper) / 2, domain, colouring.scale, diverging))), colouring.ramp, colouring.reverse))} ${(((i + 0.5) / bins.length) * 100).toFixed(1)}%`);
    return html`<div part="ramp" style=${`background:linear-gradient(to right, ${stops.join(', ')})`}></div>`;
  }

  /**
   * The range a brush covers, its words, and how many matching items lie in it. The bins span every
   * value the viewer may see, so a range reaching the first or the last bin leaves that end open.
   */
  private brushRange(bins: Bin[], brush: Brush, timestamp: boolean, whole: boolean): {gte: number | null; lte: number | null; text: string; count: number} {
    const first = Math.max(0, Math.min(brush.from, brush.to));
    const last = Math.min(bins.length - 1, Math.max(brush.from, brush.to));
    const from = bins[first]!.lower;
    const end = bins[last]!.upper;
    const to = whole || timestamp ? end - 1 : end;
    const count = bins.slice(first, last + 1).reduce((t, b) => t + b.count, 0);
    return {gte: first === 0 ? null : from, lte: last === bins.length - 1 ? null : to, text: rangeWords(from, to, timestamp, whole), count};
  }

  /**
   * Set the range box's range as the field's clause in `verb`. A field holding text that is no
   * value, or a lower end above the upper, sets nothing: both fields show what is wrong with them,
   * and the first that is wrong takes focus.
   */
  private applyRange(verb: ClauseVerb): void {
    const entry = this.entry;
    if (!entry) return;
    if (entry.from.value === undefined || entry.to.value === undefined || backwards(entry)) {
      this.entry = {from: {...entry.from, checked: true}, to: {...entry.to, checked: true}};
      void this.updateComplete.then(() => this.renderRoot.querySelector<HTMLElement>('[part="brush"] [aria-invalid="true"]')?.focus());
      return;
    }
    this.entry = null;
    this.setClause(verb, {family: 'numeric', gte: entry.from.value, lte: entry.to.value});
    this.focusOpener();
  }

  /** Give focus back to the plot or to Set range, whichever opened the range box, unless the box is open again by then. */
  private focusOpener(): void {
    void this.updateComplete.then(() => {
      if (!this.entry) this.renderRoot.querySelector<HTMLElement>(`[part="${this.entryOpener}"]`)?.focus();
    });
  }

  /** Which bin a pointer is over. */
  private binAt(e: PointerEvent, n: number): number {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    if (r.width <= 0 || n === 0) return 0;
    return Math.min(n - 1, Math.max(0, Math.floor(((e.clientX - r.left) / r.width) * n)));
  }

  private brushStart(e: PointerEvent, n: number): void {
    if (e.button !== 0 || n === 0) return;
    const i = this.binAt(e, n);
    this.brush = {from: i, to: i};
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    e.preventDefault();
  }

  private brushMove(e: PointerEvent, n: number): void {
    if (!this.brush) return;
    const to = this.binAt(e, n);
    if (to !== this.brush.to) this.brush = {...this.brush, to};
  }

  private brushEnd(e: PointerEvent, n: number, open: (opener: 'plot') => void): void {
    if (!this.brush) return;
    this.brush = {...this.brush, to: this.binAt(e, n)};
    open('plot');
  }

  /**
   * The arrow keys move a one-bin range, Shift widens it and Escape drops it. Enter opens the range
   * box, on that range or, with none chosen, on the range the field holds.
   */
  private brushKey(e: KeyboardEvent, n: number, open: (opener: 'plot') => void): void {
    if (n === 0) return;
    if (e.key === 'Escape' && (this.brush || this.entry)) {
      e.stopPropagation();
      this.brush = null;
      this.entry = null;
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      if (!this.entry) open('plot');
      return;
    }
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (step === 0) return;
    e.preventDefault();
    this.entry = null;
    const b = this.brush ?? {from: 0, to: 0};
    const to = Math.min(n - 1, Math.max(0, b.to + (this.brush ? step : 0)));
    this.brush = e.shiftKey ? {from: b.from, to} : {from: to, to};
  }

  /**
   * The box lies over what is under the plot, so it closes as focus goes on to that. Focus that
   * goes to nothing has not left, and a box already taken off the page has nothing to close.
   */
  private onBrushLeft = (e: FocusEvent): void => {
    const box = e.currentTarget as HTMLElement;
    const to = e.relatedTarget as Node | null;
    if (box.isConnected && to && !box.contains(to)) this.entry = null;
  };

  private onBrushKey = (e: KeyboardEvent): void => {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    this.entry = null;
    this.focusOpener();
  };

  // ---- colours ---------------------------------------------------------------------------------

  /** Open the colour picker on a row's swatch: a category value's, or a cluster's. */
  private openPicker(e: Event, r: Row, kind: Kind): void {
    const s = this.resolvedStore;
    const layer = this.layerName;
    if (!s) return;
    const from = e.currentTarget as HTMLElement;
    if (kind === 'category') {
      this.picker.open(from, {
        title: r.name,
        palette: CATEGORY_PALETTES[colouringOf(s).palette],
        own: paletteValueColour(s, this.field, r.key),
        current: () => valueColour(s, this.field, r.key),
        apply: (hex, final) => {
          setValueColours(s, this.field, [{value: r.key, colour: hex}]);
          if (final) emit(this, 'mosaica-valuecolour', {column: this.field, changes: [{value: r.key, colour: hex}]});
        },
        hold: () => holdColours(s)
      });
      return;
    }
    if (layer === null || r.own === null) return;
    const id = BigInt(r.key);
    const own = r.own;
    this.picker.open(from, {
      title: r.name,
      palette: PALETTES[s.get('artifacts').palette],
      own,
      current: () => clusterColour(s, layer, id, own),
      apply: (hex, final) => {
        setClusterColours(s, layer, [{mosaicaId: id, colour: hex}]);
        if (final) emit(this, 'mosaica-clustercolour', {layer, changes: [{mosaicaId: r.key, colour: hex}]});
      },
      hold: () => holdColours(s)
    });
  }
}

attachContextRoot();
defineOnce('mosaica-field-card', MosaicaFieldCard);

declare global {
  interface HTMLElementTagNameMap {
    'mosaica-field-card': MosaicaFieldCard;
  }
}
