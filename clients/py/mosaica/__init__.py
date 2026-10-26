"""Use this package to make a Mosaica database from data frames and files, to read one from
Python or a notebook, and to manage a running one. Mosaica serves an interactive map of points
to many viewers, each of whom sees only the items they may see.

`create` makes a database in a directory and `open` reopens one, and both give a `Database`.
You declare on it what the database holds, insert tables into what you declared, and commit
them: the first commit builds the database and starts a server over it, and each later commit
sends that server what was inserted since. A `Database` reads with its operator's own token, and
`Database.viewer` gives a reader that holds only the access terms you name.

`connect` gives a `Viewer`, which reads a database someone else runs with a token that its
operator issued, or that `login` or `authorise` made. `Control` is a client for a database's
control plane, which writes to it and manages its principals, credentials, groups, grants, OIDC
providers and sessions.

Every table the package returns is a pyarrow table, whose `.to_pandas()` gives a pandas
DataFrame when pandas is installed. Every `mosaica_id` in it, an item's or an annotation's, is
the id's decimal string, so one id is equal wherever it appears, and every call that takes an id
takes that string or the whole number. `Map` shows a database as an interactive map in a notebook
cell, and `side_by_side` shows several maps in one row; both need the `widget` extra, which
`pip install 'mosaica[widget]'` adds. `Map`, `side_by_side`, `create`, `open` and `Database` are
loaded when first used, so `import mosaica` works without anywidget or pyarrow.
"""

from __future__ import annotations

from ._auth import Token, authorise, login, logout, revoke
from ._control import Answer, Control
from ._refusal import Refusal
from ._viewer import Batches, PartialRead, Sample, Selection, Viewer, connect

__all__ = [
    "Answer",
    "Batches",
    "Control",
    "Database",
    "Map",
    "PartialRead",
    "Refusal",
    "Sample",
    "Selection",
    "Token",
    "Viewer",
    "authorise",
    "connect",
    "create",
    "login",
    "logout",
    "open",
    "revoke",
    "side_by_side",
    "__version__",
]
__version__ = "0.1.0rc4"

_SDK = {"create": "create", "open": "open", "Database": "Database"}


def __getattr__(name: str):
    if name in ("Map", "side_by_side"):
        try:
            from . import widget
        except ImportError as e:
            raise ImportError(
                f"mosaica.{name} needs the widget extra: pip install 'mosaica[widget]'"
            ) from e
        return getattr(widget, name)
    if name in _SDK:
        try:
            from . import _database
        except ImportError as e:
            raise ImportError("making a database needs pyarrow: pip install 'pyarrow>=14.0.1'") from e
        return getattr(_database, _SDK[name])
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


def __dir__() -> list[str]:
    # The names loaded on first use are listed too, so a notebook completes them.
    return sorted(set(globals()) | set(__all__))
