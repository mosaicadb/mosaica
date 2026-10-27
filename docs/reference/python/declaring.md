# Declaring

Declaring says what a database will hold before any data arrives: its views, columns, vocabularies and annotation layers, and the size of the shards its items are stored in. Each `declare_*` method adds a block to the declaration, which the next `check()` or `commit()` writes out as `schema.toml` and checks. The data itself comes later, through [`insert`](inserting.md).

## Database

::: mosaica._database.Database.declare_view
    options:
      heading_level: 3

::: mosaica._database.Database.declare_view_group
    options:
      heading_level: 3

::: mosaica._database.Database.declare_attribute
    options:
      heading_level: 3

::: mosaica._database.Database.declare_unique
    options:
      heading_level: 3

::: mosaica._database.Database.declare_vocabulary
    options:
      heading_level: 3

::: mosaica._database.Database.declare_shards
    options:
      heading_level: 3

::: mosaica._database.Database.declare_columns
    options:
      heading_level: 3

::: mosaica._database.Database.declare_layer
    options:
      heading_level: 3

::: mosaica._database.Database.declare_labels
    options:
      heading_level: 3

::: mosaica._database.Database.declare
    options:
      heading_level: 3

::: mosaica._database.Database.declaration
    options:
      heading_level: 3

::: mosaica._database.Database.write
    options:
      heading_level: 3

::: mosaica._reports.Declared
    options:
      show_bases: false

::: mosaica._columns.DeclaredColumn
