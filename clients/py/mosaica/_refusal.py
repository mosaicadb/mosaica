"""The one exception the package raises for a request it or the server will not carry out, and
how an argument that takes several values reads one."""

from __future__ import annotations

from typing import Any


def listed(values: Any) -> list:
    """An argument that takes several values, as a list: a string or an integer is one value."""
    if isinstance(values, (str, bytes, int)):
        return [values]
    return list(values)


class Refusal(ValueError):
    """A request that was refused, with a message saying what was wrong and what to do instead.

    The package raises it for a call it will not make, and for a request the server refused, in
    which case the message carries the server's status and answer. A `commit()` that did nothing
    carries its report as `report`, which is `None` otherwise. `authorise` and `revoke` raise
    `PermissionError` for a refused request instead.

        try:
            db.commit()
        except mosaica.Refusal as refused:
            print(refused.report)
    """

    def __init__(self, message: str = "", report=None) -> None:
        super().__init__(message)
        self.report = report
