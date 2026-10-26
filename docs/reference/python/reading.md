# Reading

A reader holds a set of access terms and sees an item when those terms satisfy one of the item's labels, so every count, map, record and annotation it is given covers only the items it may see. Use `connect` for a reader whose terms come from its token, or `Database.viewer` to read your own database as someone holding the terms you name.

A `Database` itself reads with its operator's token, which holds `read-all`. It therefore sees every item that is not deleted or suppressed, including items committed after the token was made, once their commit is flushed.

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
