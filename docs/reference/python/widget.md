# The notebook map

`Map` shows a database as an interactive map in a notebook cell. It needs the `widget` extra, `mosaica[widget]`, and you rarely make one directly, because `Database.map`, `Viewer.map` and `Selection.map` make one for you.

Give each map a `label` where a notebook shows more than one, so that a screen reader names them apart.

::: mosaica.widget.Map
    options:
      merge_init_into_class: true

::: mosaica.widget.side_by_side

::: mosaica.widget.bundle_path
