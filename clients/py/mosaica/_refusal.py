"""The one exception the package raises for a request it or the server will not carry out, and
how an argument that takes several values reads one."""

from __future__ import annotations

from typing import Any, Optional

#: The shortest wait a `429` asks for, in seconds, which is also the server's own minimum.
RETRY_AFTER_MIN_S = 1.0


def listed(values: Any) -> list:
    """An argument that takes several values, as a list: a string or an integer is one value."""
    if isinstance(values, (str, bytes, int)):
        return [values]
    return list(values)


def retry_after(status: int, headers, body: Any) -> Optional[float]:
    """The wait a refusal asks for before the request is sent again, in seconds, or `None` where
    it is not a `429` and names no wait.

    The `Retry-After` header comes first, then the body's `retry_after_s`, then 1 second, and
    the wait is at least 1 second.
    """
    named = headers.get("retry-after") if headers is not None else None
    if named is None and isinstance(body, dict):
        named = body.get("retry_after_s")
    if named is None and status != 429:
        return None
    try:
        seconds = float(named)
    except (TypeError, ValueError):
        seconds = RETRY_AFTER_MIN_S
    return seconds if seconds >= RETRY_AFTER_MIN_S else RETRY_AFTER_MIN_S


class Refusal(ValueError):
    """A request that was refused, with a message saying what was wrong and what to do instead.

    The package raises it for a call it will not make, and for a request the server refused, in
    which case the message carries what the server said and `status` its HTTP status, which is
    `None` otherwise. `retry_after_s` is the wait in seconds the server asked for before the
    request is sent again, read by `retry_after`: on a `429`, and on any refusal that names a
    wait. It is `None` otherwise. A `commit()` that did nothing carries its report as `report`, which is
    `None` otherwise. `authorise` and `revoke` raise `PermissionError` for a refused request
    instead.

        try:
            db.commit()
        except mosaica.Refusal as refused:
            print(refused.report)
    """

    def __init__(
        self,
        message: str = "",
        report=None,
        status: int | None = None,
        retry_after_s: float | None = None,
    ) -> None:
        super().__init__(message)
        self.report = report
        self.status = status
        self.retry_after_s = retry_after_s
