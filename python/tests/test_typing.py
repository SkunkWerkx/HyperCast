"""Pins the typed surface: ``py.typed`` ships, ``_native.pyi`` describes what the loaded
extension actually has, and a consumer's ``match`` over a verdict type-checks as exhaustive
against the package. The last one needs mypy and is skipped without it."""

from __future__ import annotations

import ast
import inspect
import os
from pathlib import Path

import pytest

import hypercast
from hypercast import _native

PACKAGE = Path(hypercast.__file__).resolve().parent
SAMPLES = Path(__file__).resolve().parent / "typecheck"
STUB = ast.parse((PACKAGE / "_native.pyi").read_text(encoding="utf-8"))


def _functions(body: list[ast.stmt]) -> dict[str, ast.FunctionDef]:
    return {node.name: node for node in body if isinstance(node, ast.FunctionDef)}


def _classes() -> dict[str, ast.ClassDef]:
    return {node.name: node for node in STUB.body if isinstance(node, ast.ClassDef)}


def test_the_package_is_marked_typed():
    assert (PACKAGE / "py.typed").is_file()


def test_the_stub_and_the_loaded_backend_name_the_same_surface():
    stubbed = set(_functions(STUB.body)) | set(_classes())
    missing = {name for name in stubbed if not hasattr(_native, name)}
    assert not missing, f"in _native.pyi but not on the extension: {missing}"
    # And the other way: everything the package re-exports from the extension is stubbed.
    reexported = {name for name in hypercast.__all__ if getattr(_native, name, None) is getattr(hypercast, name)}
    assert reexported <= stubbed, f"re-exported but not in _native.pyi: {reexported - stubbed}"
    assert {"Success", "Fault", "NumFormat", "cast_i32", "native_version"} <= reexported


def test_the_stub_and_the_loaded_backend_agree_on_parameter_names():
    # Parameter names are part of the surface — a keyword call must work.
    for name, node in _functions(STUB.body).items():
        expected = [arg.arg for arg in node.args.args]
        actual = list(inspect.signature(getattr(_native, name)).parameters)
        assert actual == expected, f"{name} on the extension"


def test_the_stub_and_the_loaded_backend_agree_on_class_members():
    for name, node in _classes().items():
        cls = getattr(_native, name)
        declared = set(_functions(node.body)) - {"__new__", "__eq__"}
        declared |= {
            stmt.target.id
            for stmt in node.body
            if isinstance(stmt, ast.AnnAssign) and isinstance(stmt.target, ast.Name)
        }
        missing = {member for member in declared if not hasattr(cls, member)}
        assert not missing, f"{name} on the extension lacks {missing}"


def test_a_consumers_match_type_checks_as_exhaustive(monkeypatch):
    api = pytest.importorskip("mypy.api")
    # The package as this checkout has it, the way conftest.py puts it on sys.path. --strict
    # follows the import, so hypercast's own annotations are checked against the stub too.
    monkeypatch.setenv("MYPYPATH", str(PACKAGE.parent))
    out, err, status = api.run(["--strict", "--cache-dir", os.devnull, str(SAMPLES / "consumer.py")])
    assert status == 0, out + err


def test_the_checker_catches_a_missing_case(monkeypatch, tmp_path):
    # The other half of the promise: drop the Fault arm and the same assert_never is an
    # error. Without this, "it type-checks" could just mean the checker saw Any.
    api = pytest.importorskip("mypy.api")
    monkeypatch.setenv("MYPYPATH", str(PACKAGE.parent))
    sample = tmp_path / "missing_case.py"
    sample.write_text(
        "from typing import assert_never\n"
        "from hypercast import Success, Verdict\n"
        "def describe(verdict: Verdict[int]) -> str:\n"
        "    match verdict:\n"
        "        case Success(value):\n"
        "            return str(value)\n"
        "        case _:\n"
        "            assert_never(verdict)\n",
        encoding="utf-8",
    )
    out, err, status = api.run(["--strict", "--cache-dir", os.devnull, str(sample)])
    assert status != 0, "a match with no Fault arm type-checked as exhaustive"
    assert "Fault" in out and "assert_never" in out, out + err
