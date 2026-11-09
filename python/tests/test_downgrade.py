"""The downgrade guard: a native core below 1.1 reads the grouping-policy bits of a format as
nothing and parses ``12,34`` as 1234, so a request against it would silently be a 1.0
parse. The package refuses to load against one.

A real 1.0 core is the extension of the 1.0.0 wheel; the test that uses it runs when
``HYPERCAST_CORE_1_0`` names the unpacked 1.0 ``hypercast`` package directory (``pip download
hypercast==1.0.0 --no-deps``, then unzip the wheel), and is skipped without it. The checks on a
stand-in for a 1.0 module always run.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from importlib.machinery import EXTENSION_SUFFIXES
from pathlib import Path
from types import SimpleNamespace

import pytest

import hypercast

PACKAGE = Path(hypercast.__file__).resolve().parent
CORE_1_0 = os.environ.get("HYPERCAST_CORE_1_0")


def _a_1_0_core() -> SimpleNamespace:
    """What a 1.0 ``hypercast._native`` has: the 1.0 format, with no policy and no date options."""
    return SimpleNamespace(
        native_version=lambda: "1.0.0",
        NumFormat=SimpleNamespace(GROUPING=1, ALL=95, SEPARATOR_DETECT=32),
    )


def test_a_1_0_core_is_refused_before_anything_binds_to_it():
    """The guard names the version found and says what to do."""
    with pytest.raises(ImportError, match=r"version 1\.1 or later.*is 1\.0\.0") as refused:
        hypercast._require_core_1_1(_a_1_0_core())
    assert "Reinstall" in str(refused.value)
    # An extension too old to say its version is refused, and by the same rule.
    with pytest.raises(ImportError, match="of unknown version"):
        hypercast._require_core_1_1(SimpleNamespace(NumFormat=SimpleNamespace()))
    # Either half of the 1.1 surface alone is not enough.
    with pytest.raises(ImportError):
        hypercast._require_core_1_1(
            SimpleNamespace(
                native_version=lambda: "1.0.9", DateOptions=object, NumFormat=SimpleNamespace()
            )
        )
    with pytest.raises(ImportError):
        hypercast._require_core_1_1(
            SimpleNamespace(
                native_version=lambda: "1.0.9",
                NumFormat=SimpleNamespace(GROUPING_WESTERN=256),
            )
        )


def test_the_loaded_core_is_accepted():
    """The negative control: the extension this package ships with passes the guard."""
    hypercast._require_core_1_1(hypercast._native)


def _copy_package(into: Path) -> Path:
    """This checkout's package sources, without the extension it was built with."""
    target = into / "hypercast"
    target.mkdir()
    for source in PACKAGE.iterdir():
        if source.suffix in {".py", ".pyi"} or source.name == "py.typed":
            shutil.copy(source, target / source.name)
    return target


def _extension_of(package: Path) -> Path:
    """The extension in ``package`` this interpreter would load."""
    found = [
        p
        for p in package.iterdir()
        if p.name.startswith("_native") and p.name[len("_native") :] in EXTENSION_SUFFIXES
    ]
    if not found:
        pytest.skip(f"{package} has no extension this interpreter can load")
    assert len(found) == 1, found
    return found[0]


def _run(path: Path, code: str) -> subprocess.CompletedProcess[str]:
    env = {**os.environ, "PYTHONPATH": str(path)}
    return subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, env=env, check=False
    )


@pytest.mark.skipif(CORE_1_0 is None, reason="HYPERCAST_CORE_1_0 names no 1.0 package")
def test_a_real_1_0_core_is_rejected_at_import_never_parsed_as_1_0(tmp_path):
    """A 1.0 core really does ignore the policy (the hazard); this package, over that same
    extension, refuses to import; over its own extension it imports and refuses ``12,34``."""
    old_package = Path(CORE_1_0 or "")
    old_extension = _extension_of(old_package)
    # 1. The hazard: 1.0's own package reads this by 1.0's rule, the retired bit ignored.
    hazard = _run(
        old_package.parent,
        "import hypercast as h; f = h.NumFormat('.', ',', h.NumFormat.ALL | (1 << 8)); "
        "print(h.native_version(), h.cast_i32('12,34', f))",
    )
    assert hazard.returncode == 0, hazard.stderr
    assert hazard.stdout.split(maxsplit=1)[0] == "1.0.0"
    assert "Success(value=1234)" in hazard.stdout, "a 1.0 core parses 12,34 as 1234"
    # 2. This package against that extension: refused at import, never a 1.0 parse.
    mismatched = tmp_path / "mismatched"
    mismatched.mkdir()
    shutil.copy(old_extension, _copy_package(mismatched) / old_extension.name)
    refused = _run(mismatched, "import hypercast")
    assert refused.returncode != 0
    assert "ImportError" in refused.stderr
    assert "version 1.1 or later" in refused.stderr and "is 1.0.0" in refused.stderr
    # 3. The control: the same sources over this checkout's extension import and refuse 12,34.
    control = tmp_path / "control"
    control.mkdir()
    loaded = Path(hypercast._native.__file__)
    shutil.copy(loaded, _copy_package(control) / loaded.name)
    accepted = _run(
        control,
        "import hypercast as h; print(h.cast_i32('12,34', h.NumFormat.INVARIANT))",
    )
    assert accepted.returncode == 0, accepted.stderr
    assert "Fault(" in accepted.stdout and "Success" not in accepted.stdout
