# Reading

Use `connect` for a reader whose access terms come from its token, or `Database.viewer` to read your own database as someone who holds the terms you name. Every read covers only the items the reader may see, by the rule that [`Viewer`](#mosaica._viewer.Viewer) states.

| To | Call |
|---|---|
| Narrow a view by a filter or a box | [`Selection.filter`](#mosaica._viewer.Selection.filter), [`Selection.within`](#mosaica._viewer.Selection.within) |
| Count, break down, bin or grid a selection | [`Selection.count`](#mosaica._viewer.Selection.count), [`Selection.aggregate`](#mosaica._viewer.Selection.aggregate), [`Viewer.aggregate`](#mosaica._viewer.Viewer.aggregate) |
| Read the points a map draws | [`Selection.sample`](#mosaica._viewer.Selection.sample) |
| Read records | [`Selection.items`](#mosaica._viewer.Selection.items), [`Viewer.items`](#mosaica._viewer.Viewer.items), [`Viewer.item`](#mosaica._viewer.Viewer.item) |
| List a category's values | [`Viewer.categories`](#mosaica._viewer.Viewer.categories) |

The [filters](../filters.md) page lists every operator a filter takes, and the [aggregations](../aggregations.md) page lists every figure and grouping. [Count and chart from Python](../../guides/analysis.md) is the guide to both.

The `Database` methods on this page read with the operator's own token, which [`authorise`](connecting.md#mosaica._auth.authorise) describes under `read_all`.

## Database

::: mosaica._database.Database.view
    options:
      heading_level: 3

::: mosaica._database.Database.viewer
    options:
      heading_level: 3

::: mosaica._database.Database.map
    options:
      heading_level: 3

::: mosaica._database.Database.meta
    options:
      heading_level: 3

::: mosaica._database.Database.item
    options:
      heading_level: 3

::: mosaica._database.Database.items
    options:
      heading_level: 3

::: mosaica._database.Database.lookup
    options:
      heading_level: 3

::: mosaica._database.Database.artifacts
    options:
      heading_level: 3

::: mosaica._database.Database.browse_artifacts
    options:
      heading_level: 3

::: mosaica._database.Database.artifact
    options:
      heading_level: 3

::: mosaica._database.Database.viewport_artifacts
    options:
      heading_level: 3

::: mosaica._database.Database.aggregate
    options:
      heading_level: 3

::: mosaica._database.Database.categories
    options:
      heading_level: 3

::: mosaica._viewer.Viewer
    options:
      merge_init_into_class: true

::: mosaica._viewer.Selection

::: mosaica._viewer.Sample

::: mosaica._viewer.Batches

::: mosaica._viewer.PartialRead
