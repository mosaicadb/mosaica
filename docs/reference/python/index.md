# Python client

Use the `mosaica` package to make a Mosaica database from data frames and files, to read one from Python or a notebook, and to run a database's control plane. It returns every table as a pyarrow table, which pandas and polars both read.

The notebook map needs the `widget` extra, `mosaica[widget]`. Because `Map`, `create`, `open` and `Database` are loaded only when first used, `import mosaica` works without anywidget or pyarrow installed.

These are the names `mosaica` exports:

| Name | What it is | Page |
|---|---|---|
| `connect` | A function that connects to a database someone else runs, as a reader | [Connecting and creating](connecting.md) |
| `create`, `open` | Functions that make a new database in a directory, or open one saved there | [Connecting and creating](connecting.md) |
| `Database` | A database in a directory, which runs its own server | [Connecting and creating](connecting.md); its methods are on the task pages |
| `Token`, `login`, `logout`, `authorise`, `revoke` | A token, and the functions that make and end one | [Connecting and creating](connecting.md) |
| `Refusal` | The exception raised when a request is refused | [Connecting and creating](connecting.md) |
| `Control`, `Answer` | A client for a running database's control plane, and the answer each of its calls returns | [Operating](operating.md) |
| `Viewer`, `Selection`, `Sample` | A reader of a database, a part of one of its views, and the points a map draws | [Reading](reading.md) |
| `Batches`, `PartialRead` | The pages of a large read of records, and the exception raised when a read stops part of the way | [Reading](reading.md) |
| `Map` | An interactive map shown in a notebook cell | [The notebook map](widget.md) |
| `__version__` | The package's version, as a string | |

The package's calls return these types too. They are not imported by name.

| Type | What it is | Page |
|---|---|---|
| `Declared`, `DeclaredColumn` | What `Database.declare_columns` declared | [Declaring](declaring.md) |
| `Insert` | What `Database.insert` handed over | [Inserting and committing](inserting.md) |
| `Report`, `CommitReport`, `PagedReport` | What `Database.check` and `Database.commit` found or did | [Inserting and committing](inserting.md) |
| `Finding` | A problem that `Database.check` or `Database.commit` found | [Inserting and committing](inserting.md) |
| `ChangeReport` | What a deletion, a suppression or `Database.leave` did | [Operating](operating.md) |
| `Listening` | The addresses a database's server listens on | [Operating](operating.md) |
| `ServeRefused` | The exception raised when a server a `Database` started reports no addresses | [Operating](operating.md) |

The methods of `Database` are on four pages: [declaring](declaring.md), [inserting and committing](inserting.md), [reading](reading.md) and [operating](operating.md). The rest are on [connecting and creating](connecting.md).
