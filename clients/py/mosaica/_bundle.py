"""Where the components' JavaScript bundle is, which the notebook's `Map` and `mosaica demo`
load."""

from __future__ import annotations

import pathlib
from typing import Optional

_HERE = pathlib.Path(__file__).parent
_BUNDLE = "mosaica-components.js"


def bundle_path() -> Optional[pathlib.Path]:
    """The path of the components' JavaScript bundle, or `None` when there is none.

    The copy packaged in the wheel under `mosaica/static/` is used when it is there, and a
    checkout's own build in `clients/ts/components/dist/` otherwise.
    """
    packaged = _HERE / "static" / _BUNDLE
    if packaged.exists():
        return packaged
    checkout = _HERE.parents[1] / "ts" / "components" / "dist" / _BUNDLE
    if checkout.exists():
        return checkout
    return None
