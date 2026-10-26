"""The one exception the package raises for a request it or the server will not carry out, how
long a refusal asks the caller to wait, and how an argument that takes several values reads one."""

from __future__ import annotations

import math
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
    return seconds if math.isfinite(seconds) and seconds > RETRY_AFTER_MIN_S else RETRY_AFTER_MIN_S


class Refusal(ValueError):
    """The exception the package raises when it will not make a call, or when the server refuses
    a request, with a message that says what was wrong and what to write instead.

    Catching `Refusal` handles every refusal from a `Database` or a reader in one place, and since
    it is a `ValueError`, code that catches `ValueError` catches it too. The token functions
    `login`, `logout`, `authorise` and `revoke` raise `PermissionError` for a request the server
    refuses, and `Control` returns each refusal in its `Answer` without raising.

    - `status`: the HTTP status of a read the server refused, and `None` for every other refusal.
    - `retry_after_s`: the wait in seconds the server asked for before the request is sent again,
      which it names when it answers `429` because it is busy, and `None` when it named none.
    - `report`: the report of a `commit`, `remove`, `suppress`, `unsuppress` or `leave` that
      applied nothing, which says what was refused and why, and `None` otherwise.

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
