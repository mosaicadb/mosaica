"""Datasets to try, each fetched from its publisher, prepared, built and served with one call.

    db = mosaica.datasets.load("places")
    db.map()

`mosaica demo NAME...` does the same from a shell and opens the map in a browser, with a picker
for whom to view it as, and `prepared(name)` stops before the build and returns the declaration. Each step is
skipped when what it made is still current, so a second `load` opens the database at once.
`names()` lists the datasets, and `describe(name)` says what one holds and what it takes to make.
The datasets are kept under `root()`, which `MOSAICA_DATASETS` moves.
"""

from __future__ import annotations

import dataclasses
import importlib.util
import os
import sys
import tomllib
from pathlib import Path
from typing import Callable, Optional

from .. import _instance, _progress
from .._database import Database, _reason_first
from .._database import open as _open
from .._refusal import Refusal
from . import _steps, gbif, geonames

__all__ = ["Dataset", "Persona", "describe", "load", "names", "prepared", "root"]


@dataclasses.dataclass(frozen=True)
class Persona:
    """Someone to view a dataset as: `mosaica demo` lists them in its "Viewing as" picker, and
    `db.viewer(persona.terms)` reads as one in Python.

    - `id`: the persona's name in the page's address, as in `?as=gb`.
    - `label`, `description`: what the picker shows.
    - `terms`: the access terms the persona holds, or `None` for one who reads everything.
    """

    id: str
    label: str
    description: str
    terms: Optional[tuple[str, ...]] = None


@dataclasses.dataclass(frozen=True)
class Dataset:
    """One dataset of the catalogue, and what it takes to make.

    - `name`, `title`, `items`, `licence`, `publisher`: what it is.
    - `download`: the bytes fetched where the files are not on disk; a checkout of the
      repository holds those of the two samples. GBIF's whole-snapshot figures are modelled from
      a 64-part sample and from the ladder's build of an earlier snapshot; the rest were measured.
    - `personas`: who `mosaica demo` offers to view the dataset as, the first by default.
    - `disk`: the bytes on disk once built, the fetched files and the bundle included.
    - `bundle`: the bytes of the built bundle.
    - `needs`: the modules making it imports besides this package's own, which the `demo` extra
      installs.
    - `sample`: what `sample=N` keeps, or `None` where the dataset takes no sample.
    - `per_sample`: the bytes downloaded and on disk for each unit of `sample=N`, as
      `(download, disk)`, where a sample downloads less than the whole, or `None`.
    - `fetch`, `prepare`: how `load` makes it. `fetch` puts the files a sample needs in a
      directory, or finds them in a checkout, and returns their paths, the declaration first
      where there is nothing to prepare. `prepare` writes `schema.toml` and the files it names
      from them, and its module's source and `DECLARATION` are the recipe, so a change to either
      prepares the dataset again. It is `None` where the fetched files are those already.
    """

    name: str
    title: str
    items: int
    licence: str
    publisher: str
    download: int
    disk: int
    bundle: int
    needs: tuple[str, ...]
    sample: Optional[str]
    per_sample: Optional[tuple[int, int]]
    personas: tuple[Persona, ...]
    fetch: Callable[[Path, bool, Optional[int]], list[Path]] = dataclasses.field(repr=False)
    prepare: Optional[Callable[[Path, Path, Optional[int]], None]] = dataclasses.field(
        default=None, repr=False
    )


def root() -> Path:
    """Where datasets are kept: `MOSAICA_DATASETS` when set, else `mosaica/datasets` in the user's
    cache directory."""
    named = os.environ.get("MOSAICA_DATASETS")
    if named:
        return Path(named).expanduser()
    cache = os.environ.get("XDG_CACHE_HOME")
    return (Path(cache) if cache else Path.home() / ".cache") / "mosaica" / "datasets"


#: The repository's root, when this package runs from a checkout of it.
_REPOSITORY = Path(__file__).resolve().parents[4]


def _example(directory: str) -> Callable[[Path, bool, Optional[int]], list[Path]]:
    """One of the repository's example datasets: a checkout's copy, or the files this package's
    release tagged, fetched."""

    def fetch(raw: Path, progress: bool, sample: Optional[int] = None) -> list[Path]:
        from .. import __version__

        checkout = _REPOSITORY / "examples" / "data" / directory / "schema.toml"
        if checkout.exists():
            return [checkout]
        base = (
            f"https://raw.githubusercontent.com/mosaicadb/mosaica/v{__version__}/"
            f"examples/data/{directory}/"
        )
        schema = _steps.fetch(raw, {"schema.toml": base + "schema.toml"}, progress)
        sources = tomllib.loads(schema[0].read_text(encoding="utf-8"))["sources"]
        files = {name: base + name for name in ["NOTICE.md", *sources.values()]}
        return schema + _steps.fetch(raw, files, progress)

    return fetch


_EVERYTHING = Persona("everything", "Everything", "Reads as the operator, so sees every item.")
_EU = tuple(
    "AT BE BG CY CZ DE DK EE ES FI FR GR HR HU IE IT LT LU LV MT NL PL PT RO SE SI SK".split()
)
#: Readers who each hold some countries' codes, for the datasets whose terms are country codes.
_BY_COUNTRY = (
    _EVERYTHING,
    Persona("eu", "European Union", "Holds the 27 EU member states' codes.", _EU),
    Persona("us", "United States", "Holds one term, US.", ("US",)),
    Persona("gb", "United Kingdom", "Holds one term, GB.", ("GB",)),
    Persona("jp", "Japan", "Holds one term, JP.", ("JP",)),
)

_CS = tuple(
    "cs.AI cs.AR cs.CC cs.CE cs.CG cs.CL cs.CR cs.CV cs.CY cs.DB cs.DC cs.DL cs.DM cs.DS "
    "cs.ET cs.FL cs.GL cs.GR cs.GT cs.HC cs.IR cs.IT cs.LG cs.LO cs.MA cs.MM cs.MS cs.NA "
    "cs.NE cs.NI cs.OH cs.OS cs.PF cs.PL cs.RO cs.SC cs.SD cs.SE cs.SI cs.SY cmp-lg"
    .split()
)
_MATH = tuple(
    "math.AC math.AG math.AP math.AT math.CA math.CO math.CT math.CV math.DG math.DS "
    "math.FA math.GM math.GN math.GR math.GT math.HO math.IT math.KT math.LO math.MG "
    "math.MP math.NA math.NT math.OA math.OC math.PR math.QA math.RA math.RT math.SG "
    "math.SP math.ST alg-geom dg-ga funct-an q-alg"
    .split()
)
_PHYSICS = tuple(
    "adap-org ao-sci astro-ph astro-ph.CO astro-ph.EP astro-ph.GA astro-ph.HE astro-ph.IM "
    "astro-ph.SR atom-ph chao-dyn chem-ph comp-gas cond-mat cond-mat.dis-nn "
    "cond-mat.mes-hall cond-mat.mtrl-sci cond-mat.other cond-mat.quant-gas cond-mat.soft "
    "cond-mat.stat-mech cond-mat.str-el cond-mat.supr-con gr-qc hep-ex hep-lat hep-ph "
    "hep-th math-ph mtrl-th nlin.AO nlin.CD nlin.CG nlin.PS nlin.SI nucl-ex nucl-th "
    "patt-sol physics.acc-ph physics.ao-ph physics.app-ph physics.atm-clus physics.atom-ph "
    "physics.bio-ph physics.chem-ph physics.class-ph physics.comp-ph physics.data-an "
    "physics.ed-ph physics.flu-dyn physics.gen-ph physics.geo-ph physics.hist-ph "
    "physics.ins-det physics.med-ph physics.optics physics.plasm-ph physics.pop-ph "
    "physics.soc-ph physics.space-ph plasm-ph quant-ph solv-int supr-con"
    .split()
)
_STATS = tuple("stat.AP stat.CO stat.ME stat.ML stat.OT stat.TH".split())
#: Readers who each hold one field's arXiv categories.
_BY_ARCHIVE = (
    _EVERYTHING,
    Persona("cs", "Computer science", "Holds the cs.* categories and cmp-lg.", _CS),
    Persona(
        "math",
        "Mathematics",
        "Holds the math.* categories and the retired alg-geom, dg-ga, funct-an and q-alg.",
        _MATH,
    ),
    Persona(
        "physics",
        "Physics",
        "Holds the physics archives: astro-ph, cond-mat, hep, quant-ph and the rest.",
        _PHYSICS,
    ),
    Persona("stats", "Statistics", "Holds the stat.* categories.", _STATS),
)


_CATALOGUE = (
    Dataset(
        name="places",
        title="150,000 GeoNames places on a world map",
        items=150_000,
        licence="CC BY 4.0, GeoNames",
        publisher="GeoNames, cut by this repository",
        download=3_327_240,
        disk=23_037_347,
        bundle=19_710_107,
        needs=(),
        sample=None,
        per_sample=None,
        personas=_BY_COUNTRY,
        fetch=_example("geonames"),
    ),
    Dataset(
        name="arxiv-sample",
        title="50,000 arXiv papers on a topic map",
        items=50_000,
        licence="CC0 1.0 (arXiv metadata); layouts and clusters Apache-2.0",
        publisher="arXiv, cut by this repository",
        download=4_198_016,
        disk=20_205_379,
        bundle=16_007_363,
        needs=(),
        sample=None,
        per_sample=None,
        personas=_BY_ARCHIVE,
        fetch=_example("arxiv"),
    ),
    Dataset(
        name="geonames",
        title="Every GeoNames feature on a world map",
        items=13_483_763,
        licence="CC BY 4.0, GeoNames",
        publisher="GeoNames, download.geonames.org",
        download=425_146_247,
        disk=2_786_487_687,
        bundle=1_962_892_389,
        needs=("duckdb",),
        sample="the first N features by modification date",
        per_sample=None,
        personas=_BY_COUNTRY,
        fetch=geonames.fetch,
        prepare=geonames.prepare,
    ),
    Dataset(
        name="gbif",
        title="Every GBIF occurrence with a coordinate, on a world map",
        items=3_800_000_000,
        licence="each record its publisher's: CC0 1.0, CC BY 4.0 or CC BY-NC 4.0",
        publisher="GBIF, its monthly snapshot on Amazon S3",
        download=86_000_000_000,
        disk=406_000_000_000,
        bundle=210_000_000_000,
        needs=("numpy",),
        sample="N of the snapshot's parts, evenly spaced, each about 370,000 records",
        per_sample=(8_312_077, 55_600_000),
        personas=_BY_COUNTRY,
        fetch=gbif.fetch,
        prepare=gbif.prepare,
    ),
)


def names() -> list[str]:
    """The names of the datasets, smallest first."""
    return [dataset.name for dataset in _CATALOGUE]


def describe(name: str) -> Dataset:
    """The catalogue's entry for one dataset."""
    for dataset in _CATALOGUE:
        if dataset.name == name:
            return dataset
    raise Refusal(f"there is no dataset {name!r}; the datasets are {', '.join(names())}")


def prepared(
    name: str,
    dir: str | os.PathLike | None = None,  # noqa: A002
    sample: int | None = None,
    progress: bool = True,
) -> Path:
    """Fetch and prepare a dataset where that has not been done, and return the path of its
    declaration, `schema.toml`, beside the files it names.

    Use it to build and serve the dataset with a `mosaica.toml` of your own. `dir`, `sample` and
    `progress` are `load`'s.
    """
    dataset = _checked(name, sample)
    raw = _home(name, dir) / "raw"
    fetched = dataset.fetch(raw, progress, sample)
    if dataset.prepare is None:
        return fetched[0]
    out = raw.parent / ("prepared" if sample is None else f"prepared-{sample}")
    lock = _steps.locked(raw)
    module = sys.modules[dataset.prepare.__module__]
    inputs = {
        "fetched": {str(p.relative_to(raw)): lock.get(str(p.relative_to(raw))) for p in fetched},
        "recipe": _steps.digest_of((Path(module.__file__), module.DECLARATION)),
        "sample": sample,
    }
    _steps.made(out, inputs, lambda: dataset.prepare(raw, out, sample), f"preparing {name}")
    return out / "schema.toml"


def load(
    name: str,
    dir: str | os.PathLike | None = None,  # noqa: A002
    sample: int | None = None,
    port: int | None = None,
    progress: bool = True,
) -> Database:
    """Fetch, prepare and build a dataset where that has not been done, and open it.

    - `name`: one of `names()`.
    - `dir`: the directory datasets are kept under, each in a directory of its own name;
      `root()` by default.
    - `sample`: build from part of the dataset, as `describe(name).sample` says. The sample is
      prepared and built beside the whole, from the same fetched files.
    - `port`: the port its server listens on, as `mosaica.create()` takes it.
    - `progress`: `False` draws no progress bars.

    A step whose inputs changed since it ran, such as a newer recipe or another `mosaica`
    binary, runs again from the start, and the database it makes replaces the one there, writes
    made to it included.

        db = mosaica.datasets.load("geonames", sample=1_000_000)
        db.view("world").count()
    """
    schema = prepared(name, dir, sample, progress)
    database = _home(name, dir) / ("db" if sample is None else f"db-{sample}")
    binary, _ = _instance.find_binary()
    found = os.stat(binary)
    inputs = {
        "schema": str(schema),
        "sources": _steps.fingerprint(schema.parent),
        "binary": [binary, found.st_size, found.st_mtime_ns],
    }
    _steps.made(
        database, inputs, lambda: _build(database, schema, port, progress), f"building {name}"
    )
    return _open(database, port)


def _checked(name: str, sample: int | None) -> Dataset:
    """The dataset `name`, refused where it takes no `sample` or its extras are not installed."""
    dataset = describe(name)
    if sample is not None and dataset.sample is None:
        raise Refusal(f"{name} takes no sample; leave sample out")
    if sample is not None and sample < 1:
        raise Refusal(f"sample={sample} keeps nothing; give a number above 0")
    missing = [module for module in dataset.needs if importlib.util.find_spec(module) is None]
    if missing:
        raise Refusal(
            f"{name} is prepared with {', '.join(missing)}, which is not installed; install "
            "the demo extra, `pip install 'mosaica[demo]'`"
        )
    return dataset


def _home(name: str, dir: str | os.PathLike | None) -> Path:  # noqa: A002
    return (Path(dir).expanduser() if dir is not None else root()) / name


def _build(directory: Path, schema: Path, port: int | None, progress: bool) -> None:
    """Write a deployment in `directory` naming `schema`, and build its bundle."""
    (directory / ".mosaica" / "cache").mkdir(parents=True)
    _instance.write_deployment(directory, port, schema=str(schema))
    _instance.secrets_for(directory)
    build = _open(directory, port)._build(_progress.shown(progress), progress)
    if build.returncode != 0:
        log = build.stdout + build.stderr
        raise Refusal(_reason_first(f"load: the build of {directory} failed", log, log))
    for line in build.stdout.splitlines():
        if line.startswith("built "):
            print(f"mosaica: {line}")
