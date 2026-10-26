# Reading

Use `connect` for a reader whose access terms come from its token, or `Database.viewer` to read your own database as someone who holds the terms you name. Every read covers only the items the reader may see, by the rule that [`Viewer`](#mosaica._viewer.Viewer) states.

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
