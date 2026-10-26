"""The reports that `check()`, `commit()`, `insert()`, `declare_columns()` and the change methods
return.

No call prints anything. Each report shows as a short summary of what happened, in numbers, so
that a notebook cell ending in one displays it and `print()` shows the same lines. The summary
carries every refusal and every finding in full, and the detail is in the report's attributes,
which each class's docstring lists.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import re
from typing import Sequence

from ._columns import DeclaredColumn


class Summarised:
    """The base of every report, which shows as the lines that `summary()` returns."""

    def summary(self) -> list[str]:
        """The lines `print()` shows."""
        raise NotImplementedError

    def __str__(self) -> str:
        return "\n".join(self.summary())

    def __repr__(self) -> str:
        return str(self)


#: The check page's closing line, which counts the warnings above it.
_WARNINGS = re.compile(r"^check OK:.* (\d+) warning\(s\)$")


def _count(n: int, one: str, many: str | None = None) -> str:
    return f"{n:,} {one if n == 1 else (many or one + 's')}"


def _by_reason(refused: Sequence[dict]) -> dict:
    """How many refused rows or members give each reason."""
    counts: dict = {}
    for one in refused:
        counts[one["reason"]] = counts.get(one["reason"], 0) + 1
    return counts


#: Each reason the server gives for a refused row, as the words that follow its count.
REASONS = {
    "names_no_item": "naming no item",
    "names_two_items": "naming two items",
    "one_item_twice": "naming an item an earlier row names",
    "one_value_twice": "holding a unique value an earlier row holds",
    "unknown_mosaica_id": "naming a mosaica_id no item holds",
    "value_held": "holding a unique value an item outside the writer's terms holds",
    "label_not_held": "carrying a label the writer's terms do not satisfy",
}

#: How many refused rows a change report's summary lists one by one; `refused` holds them all.
LISTED = 50


def _reason(reason: str) -> str:
    return REASONS.get(reason, reason.replace("_", " "))


def _reasons_in_words(counts: dict) -> str:
    return ", ".join(f"{n:,} {_reason(reason)}" for reason, n in counts.items())


@dataclass(repr=False)
class Declared(Summarised):
    """What `declare_columns` declared.

    - `columns`: a `DeclaredColumn` for each column of the frame that was neither skipped nor
      already declared, including those no type fits, which were left undeclared.
    - `vocabularies`: the vocabularies this call declared for category columns, leaving out one
      declared before.
    """

    columns: list[DeclaredColumn] = field(default_factory=list)
    vocabularies: list[str] = field(default_factory=list)

    def summary(self) -> list[str]:
        """The lines `print()` shows: each column with its type, then the vocabularies."""

        def flags(column: DeclaredColumn) -> str:
            named = [flag for flag in ("render", "index") if getattr(column, flag)]
            return column.declared_as + "".join(f", {flag}" for flag in named)

        out = [
            f"declared {_count(len(self.columns), 'column')}: "
            + (", ".join(f"{c.name} ({flags(c)})" for c in self.columns) or "none")
        ]
        if self.vocabularies:
            out.append(
                f"and {_count(len(self.vocabularies), 'open vocabulary', 'open vocabularies')}, "
                "public, so every reader may list their values: " + ", ".join(self.vocabularies)
            )
        return out


@dataclass(repr=False)
class Report(Summarised):
    """The report of a `check()` before the first commit, which `CommitReport` extends.

    - `what`: `"check"` or `"commit"`.
    - `ok`: `True` when nothing was refused.
    - `rows`: the rows inserted into each view or view group.
    - `frames`: each view's name, with its extent in words.
    - `render_columns`: the columns sent with every point the map draws.
    - `notes`: what each insert read and ignored, and each declared column that no insert
      fills, with the reason. The first commit's build refuses such a column, although
      `check()` accepts it, unless it is unique or scoped to a view group.
    - `findings`: the `Finding`s found before anything was built.
    - `log`: the declaration check's output, followed on a commit by the build's.

    The summary lists every note, finding and warning, and carries the whole log when the
    report is not `ok`, because the log says why the check or build failed.
    """

    what: str
    ok: bool
    rows: dict = field(default_factory=dict)
    frames: list[tuple[str, str]] = field(default_factory=list)
    render_columns: list[str] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)
    findings: list = field(default_factory=list)
    log: str = ""

    def _rows_in_words(self) -> str:
        return "; ".join(f"{target}: {_count(n, 'row')}" for target, n in self.rows.items())

    def summary(self) -> list[str]:
        """The lines `print()` shows: the rows inserted, then the notes, findings and warnings, or
        the whole log when the check failed."""
        out = [
            f"check: {'ok' if self.ok else 'FAILED'}, inserted "
            f"{self._rows_in_words() or 'no rows'}"
        ]
        return out + self._problems()

    def _problems(self) -> list[str]:
        """The notes, the findings, and the log's warnings, or the whole log where not `ok`."""
        out = [f"  {note}" for note in self.notes]
        out += [str(finding) for finding in self.findings]
        if not self.ok:
            if self.log:
                out.append(self.log.rstrip())
            return out
        for line in self.log.splitlines():
            stripped = line.strip()
            counted = _WARNINGS.search(stripped)
            if stripped.startswith("WARNING") or (counted and int(counted.group(1)) > 0):
                out.append(f"  {stripped}")
        return out


@dataclass(repr=False)
class CommitReport(Report):
    """The report of the first `commit()`, which adds to a `Report` what the build made and
    where the database is served.

    - `views`: how many views the database serves under each name, counting a view group's
      views under the group.
    - `layers`, `labels`: the annotation layers and the label sets the database declares.
    - `items`, `minted`, `unclustered`: the build's counts of the items it built, the
      annotations it made from key columns, and the member rows left in no annotation by a null
      key. Each is `None` when the build's log does not give it.
    - `seconds`: how long the check, the build and the server's start took.
    - `viewer`, `session`, `control`: the addresses of the planes where readers read, where
      tokens are made and where the operator writes.
    - `identity`: in words, how the rows name their items.
    - `refused`: the rows the build left out, as one dictionary for each file and reason, with
      the file as `source`, the block that read it as `object`, the number of rows as `rows`,
      up to ten of their values as `values`, and `reason`, one of `names_no_item`,
      `names_two_items`, `one_item_twice`, `one_value_twice` and `unknown_mosaica_id`.
    """

    views: dict = field(default_factory=dict)
    layers: list[str] = field(default_factory=list)
    labels: list[str] = field(default_factory=list)
    items: int | None = None
    minted: int | None = None
    unclustered: int | None = None
    seconds: float | None = None
    viewer: str | None = None
    session: str | None = None
    control: str | None = None
    identity: str = ""
    refused: list = field(default_factory=list)

    def summary(self) -> list[str]:
        """The lines `print()` shows: the items built, the views and layers and the address served
        at, then the notes, findings and warnings."""
        if not self.ok:
            return ["commit: FAILED"] + self._problems()
        views = [
            name if n == 1 else f"{name} ({_count(n, 'view')})" for name, n in self.views.items()
        ]
        if self.items is None:
            out = [f"inserted {self._rows_in_words() or 'no rows'}"]
        else:
            out = [f"built {_count(self.items, 'item')} from {self._rows_in_words() or 'no rows'}"]
        if self.minted:
            out.append(f"  {_count(self.minted, 'annotation')} made from key columns")
        if self.unclustered:
            out.append(f"  {_count(self.unclustered, 'member row')} in no annotation")
        for one in self.refused:
            sample = f": {', '.join(one['values'])}" if one.get("values") else ""
            out.append(
                f"  {_count(int(one['rows']), 'row')} of {one['source']} left out "
                f"({one['object']}, {one['reason'].replace('_', ' ')}){sample}"
            )
        if views:
            out.append(f"  {_count(len(views), 'view')}: {', '.join(views)}")
        if self.layers:
            out.append(f"  {_count(len(self.layers), 'layer')}: {', '.join(self.layers)}")
        if self.labels:
            out.append(f"  {_count(len(self.labels), 'label set')}: {', '.join(self.labels)}")
        took = "" if self.seconds is None else f"in {self.seconds:.1f} s; "
        out.append(f"  {took}serving at {self.viewer}" if self.viewer else f"  {took}not serving")
        return out + self._problems()


@dataclass(repr=False)
class PagedReport(Summarised):
    """What a `check()` or `commit()` after the first one planned or did.

    `check()` returns it with `sent` false, with the requests a commit would send in `plan` and
    any problem found in `findings`. `commit()` returns the same plan with `sent` true, and
    counts what the requests did:

    - `rows_accepted`: the rows added to each view, each of which created an item or added one to
      the view. `rows` is their total.
    - `artifacts_minted`, `memberships_joined`: the annotations added and the memberships joined.
    - `items_edited`: the items already held whose value, label or position a row changed. An
      edited item keeps its `mosaica_id`, and placing an item in an annotation counts as a
      membership instead.
    - `items_unchanged`: the rows that named an item and left it as it was, apart from placing
      it in an annotation.
    - `values_bound`, `titles_set`: the vocabulary values added and the titles replaced, leaving
      out the values of a `derived` vocabulary, which a writer without `write-all` is not told.
    - `parts_filled`: the absent parts of held annotations, such as content, that were filled.
    - `already_present`: the declarations and vocabulary keys already held, which changed
      nothing.
    - `without_content`: the annotations added without the content their layer declares.
    - `clipped`: the rows the view's projection could not place, stored on the view's edge.
    - `clamped`: the rows outside a view's extent, moved onto its edge.
    - `refusals`: each request the server refused, with its status and answer.
    - `refused`: each row or member the server left out while applying the rest of its request.
      A row is `{"target", "view", "row", "reason"}`, where `row` is its position in the table
      inserted into `target`, and a member is `{"layer", "level", "view", "key", "list",
      "member", "reason"}`, where `member` is its row as sent. An annotation refused whole for
      its label has `list` and `member` set to `None`. `reason` is one of `names_no_item`,
      `names_two_items`, `one_item_twice`, `one_value_twice`, `unknown_mosaica_id`, `value_held`
      (a unique value held by an item outside the writer's terms) and `label_not_held` (a label
      the writer's terms do not satisfy), and `refused_by_reason` counts them.
    - `ignored_columns`: for each layer, the member columns that were not sent, or that the
      server ignored, because they are neither `mosaica_id` nor a unique attribute's column.
    - `mosaica_ids`: the id given to each row sent, or `None` for a refused row.
    - `artifact_ids`: the id given to each added annotation, by layer and then by
      `(level, view, key)`, or `None` for one refused for its label.
    - `replayed`: the requests the server had already carried out.
    - `publication`, `flush_wait`, `flush_reached`: the publication in which the changes can be
      read, how many seconds the commit waited for it, and whether the commit saw it arrive.

    `ok` is `True` when no request was refused and nothing was found before sending, so rows and
    members in `refused` leave it `True`. The summary counts them by reason and lists every
    finding and refused request.
    """

    sent: bool = False
    plan: list[str] = field(default_factory=list)
    findings: list = field(default_factory=list)
    rows_accepted: dict = field(default_factory=dict)
    artifacts_minted: int = 0
    #: Memberships the pages added, to artifacts this commit minted and to artifacts already held.
    memberships_joined: int = 0
    items_edited: int = 0
    items_unchanged: int = 0
    parts_filled: int = 0
    values_bound: int = 0
    titles_set: int = 0
    already_present: int = 0
    without_content: int = 0
    clipped: int = 0
    clamped: int = 0
    refusals: list = field(default_factory=list)
    refused: list = field(default_factory=list)
    ignored_columns: dict = field(default_factory=dict)
    mosaica_ids: list = field(default_factory=list)
    replayed: list = field(default_factory=list)
    publication: int | None = None
    artifact_ids: dict = field(default_factory=dict)
    flush_wait: float | None = None
    flush_reached: bool = True

    @property
    def ok(self) -> bool:
        """`True` when no request was refused and nothing was found before sending."""
        return not self.refusals and not self.findings

    @property
    def rows(self) -> int:
        """How many rows were added, over every view."""
        return sum(self.rows_accepted.values())

    @property
    def refused_by_reason(self) -> dict:
        """How many rows and members `refused` holds for each reason."""
        return _by_reason(self.refused)

    def summary(self) -> list[str]:
        """The lines `print()` shows: the requests planned, or what was added and when it could be
        read, then the findings and refusals."""
        if not self.sent:
            out = [
                f"check: {'ok' if self.ok else 'FAILED'}, "
                f"{_count(len(self.plan), 'request')} planned, nothing sent"
            ]
            return out + self._ignored() + [str(finding) for finding in self.findings]
        added = [f"{_count(n, 'row')} to {view}" for view, n in self.rows_accepted.items()]
        for n, what in (
            (self.artifacts_minted, "annotation"),
            (self.memberships_joined, "membership"),
            (self.values_bound, "vocabulary value"),
            (self.titles_set, "vocabulary title"),
        ):
            if n:
                added.append(_count(n, what))
        line = f"commit: {'ok' if self.ok else 'FAILED'}, added " + (", ".join(added) or "nothing")
        if self.items_edited:
            line += f", edited {_count(self.items_edited, 'item')}"
        if self.flush_wait is not None:
            at = "" if self.publication is None else f" for publication {self.publication}"
            if self.flush_reached:
                line += f"; visible after {self.flush_wait:.2f} s{at}"
            else:
                line += f"; not visible after waiting {self.flush_wait:.2f} s{at}"
        out = [line]
        if self.items_unchanged:
            placed = ", besides any annotation it was placed in" if self.memberships_joined else ""
            out.append(
                f"  {_count(self.items_unchanged, 'row')} named an item and left it as it was"
                f"{placed}"
            )
        if self.parts_filled:
            out.append(
                f"  {_count(self.parts_filled, 'part')} filled on annotations already held"
            )
        if self.already_present:
            out.append(
                f"  {_count(self.already_present, 'declaration or vocabulary key')} already "
                f"held, changing nothing"
            )
        if self.without_content:
            out.append(
                f"  annotations published without their declared content: {self.without_content:,}"
            )
        if self.clipped:
            out.append(f"  rows clipped onto the frame's edge by the projection: {self.clipped:,}")
        if self.clamped:
            out.append(f"  rows outside the extent, clamped onto its edge: {self.clamped:,}")
        if self.replayed:
            out.append(f"  requests replayed, adding nothing: {len(self.replayed):,}")
        rows = [one for one in self.refused if "row" in one]
        members = [one for one in self.refused if "member" in one]
        if rows:
            out.append(
                f"  {_count(len(rows), 'row')} refused and left out: "
                f"{_reasons_in_words(_by_reason(rows))}"
            )
        if members:
            out.append(
                f"  {_count(len(members), 'member')} refused and left out: "
                f"{_reasons_in_words(_by_reason(members))}"
            )
        out += self._ignored()
        out += [str(finding) for finding in self.findings]
        out += [
            f"refused {refusal['status']} on {refusal['what']}: {refusal['detail']}"
            for refusal in self.refusals
        ]
        return out

    def _ignored(self) -> list[str]:
        return [
            f"  layer '{layer}': {_count(len(names), 'member column')} naming no item, ignored: "
            f"{', '.join(names)}"
            for layer, names in self.ignored_columns.items()
        ]


@dataclass(repr=False)
class ChangeReport(Summarised):
    """What a call to `remove`, `suppress`, `unsuppress` or `leave` did.

    - `op`: the change, as `"delete"`, `"suppress"`, `"unsuppress"` or
      `"leave <layer>/<key> rank <rank>"`.
    - `requested`, `accepted`: how many rows the call was given, and how many were applied.
    - `refused`: each row refused because of how it named its item, as `{"row", "item",
      "reason"}`, where `row` is its position in what was given, `item` is the row as given,
      such as `{"mosaica_id": "17"}`, and `reason` is `names_no_item`, `names_two_items` or
      `unknown_mosaica_id`. The other rows were applied, and `refused_by_reason` counts these.
    - `ignored_columns`: the columns given that were not sent, or that the server ignored,
      because they are neither `mosaica_id` nor a unique attribute's column.
    - `refusals`: each request the server refused, with its status and answer.

    `ok` is `True` when every row given was applied.
    """

    op: str
    requested: int = 0
    accepted: int = 0
    refused: list = field(default_factory=list)
    ignored_columns: list = field(default_factory=list)
    refusals: list = field(default_factory=list)

    @property
    def ok(self) -> bool:
        """`True` when every row given was applied."""
        return not self.refusals and not self.refused

    @property
    def refused_by_reason(self) -> dict:
        """How many rows `refused` holds for each reason."""
        return _by_reason(self.refused)

    def summary(self) -> list[str]:
        """The lines `print()` shows: the rows given and applied, up to fifty refused rows with
        their reasons, the columns ignored, then the refused requests. The first line ends `ok`
        only when every row was applied."""
        if self.refusals:
            outcome = "FAILED"
        elif self.refused:
            outcome = f"{len(self.refused):,} refused"
        else:
            outcome = "ok"
        out = [f"{self.op}: {_count(self.requested, 'row')}, {self.accepted:,} applied, {outcome}"]
        for one in self.refused[:LISTED]:
            named = ", ".join(f"{name} {value}" for name, value in (one.get("item") or {}).items())
            out.append(f"  row {one['row']}{f' ({named})' if named else ''}: refused, "
                       f"{_reason(one['reason'])}")
        if len(self.refused) > LISTED:
            out.append(
                f"  and {len(self.refused) - LISTED:,} more refused rows, each in `refused`: "
                f"{_reasons_in_words(self.refused_by_reason)} in all"
            )
        if self.ignored_columns:
            out.append(
                f"  {_count(len(self.ignored_columns), 'column')} naming no item, ignored: "
                f"{', '.join(self.ignored_columns)}"
            )
        out += [f"refused {refusal['status']}: {refusal['detail']}" for refusal in self.refusals]
        return out


def render_columns_of(attributes: Sequence[dict]) -> list[str]:
    return [block["name"] for block in attributes if block.get("render")]
