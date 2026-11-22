# The registry of systems: name -> adapter class (see base.py for the interface). To add a system, write a class in
# this package and add it here; then name it in a suite's `systems` (suites.py) under the types it supports.
from .base import System, Unavailable  # noqa: F401
from .vexact import Vexact
from .faiss_sys import FaissFlat, FaissPanorama, FaissHNSW, FaissFlatMmap, FaissIVFDisk, FaissIVFPanorama, FaissIVFPQfs
from .ann_libs import HnswLib, Glass, PyNNDescent
from .diskann import DiskANN
from .pdx import PdxBond
from .acorn import Acorn
from .pgvector import PgVector

REGISTRY = {c.name: c for c in (Vexact, FaissFlat, FaissPanorama, FaissHNSW, FaissFlatMmap, FaissIVFDisk, DiskANN,
                                PdxBond, Acorn, PgVector, FaissIVFPanorama, FaissIVFPQfs, HnswLib, Glass, PyNNDescent)}
