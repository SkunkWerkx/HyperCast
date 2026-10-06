"""Checks that every binding declares exactly the C ABI the core exports.

The export list is read from the core's source: every `pub extern "C" fn` in rust/src/ffi.rs
and rust/src/abi.rs, plus the rows of ffi.rs's numeric_exports! table, which generates eleven
of them. Each binding's declarations are then read from its own source by the patterns in
SITES, and any difference fails the check: a missing export means a door nobody added to that
binding, and an extra one means the binding still declares a door the core dropped or renamed.
Both of those otherwise surface late, in one binding's suite on whichever leg calls the door.

The Python and Ruby extensions are not C ABI consumers: they link the core as a Rust crate
and register the doors under their language's spelling, so their sites map each export to
that spelling (a None folds the export into another entry point). The PHP extension
(rust/src/php_ext.rs) is a benchmark spike that php/src never calls, so it is not checked.

No dependencies and no build: CI runs it in the lint jobs, and so can a dev loop.

usage: python .github/scripts/check_exports.py
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

CORE = ["rust/src/ffi.rs", "rust/src/abi.rs"]
VERSION = "hypercast_version"


def core_exports():
    names = set()
    for path in CORE:
        text = (ROOT / path).read_text(encoding="utf-8")
        names |= set(re.findall(r'pub extern "C" fn (\w+)', text))
        for table in re.findall(r"numeric_exports! \{(.*?)\n\}", text, re.DOTALL):
            names |= set(re.findall(r"(\w+) => \(", table))
    names.discard("$export")
    return names


def python_spelling(export):
    """python_ext.rs's name for an export: the typed doors say float, and cast_date takes
    the order cast_date_ordered would."""
    if export == VERSION:
        return "native_version"
    if export == "cast_date_ordered":
        return None
    return export.replace("_from_f64", "_from_float")


def ruby_spelling(export):
    """ruby_ext.rs's name: Python's, without the cast_ prefix (the module is the namespace)."""
    spelled = python_spelling(export)
    return spelled and spelled.removeprefix("cast_")


C_PROTOTYPE = r"(?m)^(?:u?int\d+_t|void) (\w+)\("

# (binding, [(file glob, pattern)], what it must declare, names it may declare besides).
# `all` is every export, `doors` every export but the version probe, and a function maps
# each export to the binding's own name.
SITES = [
    (
        "C# (P/Invoke)",
        [("csharp/HyperCast/*.cs", r'LibraryImport\("hypercast", EntryPoint = "(\w+)"\)')],
        "all",
        (),
    ),
    (
        "C# (Blazor WebAssembly)",
        [("csharp/HyperCast/*.cs", r'LibraryImport\("\*", EntryPoint = "(\w+)"\)')],
        "all",
        (),
    ),
    (
        "C# (iOS and Mac Catalyst)",
        [("csharp/HyperCast/*.cs", r'LibraryImport\("__Internal", EntryPoint = "(\w+)"\)')],
        "all",
        (),
    ),
    (
        "Java (FFM)",
        [("java/src/main/java/io/github/skunkwerkx/hypercast/Cast.java", r'export\("(\w+)"\)')],
        "all",
        (),
    ),
    (
        "Java (GraalWasm)",
        [
            ("java/src/main/java/io/github/skunkwerkx/hypercast/Door.java", r'^    \w+\("(\w+)"\)'),
            (
                "java/src/main/java/io/github/skunkwerkx/hypercast/WasmBackend.java",
                r'export\(exports, "(\w+)"\)',
            ),
        ],
        "all",
        ("malloc", "free"),
    ),
    ("Go (cgo)", [("go/backend_static.go", C_PROTOTYPE)], "all", ()),
    ("Go (TinyGo)", [("go/backend_tinygo.go", C_PROTOTYPE)], "all", ()),
    (
        "Swift (hypercast.h)",
        [("swift/HyperCastCore.artifactbundle/include/hypercast.h", C_PROTOTYPE)],
        "all",
        (),
    ),
    ("PHP (FFI cdef)", [("php/src/Cast.php", r"(?:int|uint32_t) (\w+)[{(]")], "all", ()),
    (
        "Ruby (Fiddle)",
        [("ruby/lib/hypercast/runtime.rb", r"(cast_\w+): :\w+|VERSION_PROBE = :(\w+)")],
        "all",
        (),
    ),
    (
        "Ruby (Magnus)",
        [("rust/src/ruby_ext.rs", r'define_singleton_method\(\s*"(\w+)"')],
        ruby_spelling,
        (),
    ),
    (
        "Python (PyO3)",
        [("rust/src/python_ext.rs", r"wrap_pyfunction!\((\w+), ")],
        python_spelling,
        ("_bind",),
    ),
    (
        "Python (stubs)",
        [("python/src/hypercast/_native.pyi", r"(?m)^def (\w+)\(")],
        python_spelling,
        ("_bind",),
    ),
    (
        "cdylib smoke",
        [(".github/scripts/cdylib_smoke.py", r'"(cast_\w+)": |lib\.(\w+)\.restype')],
        "all",
        (),
    ),
]


def declared(sources):
    names = set()
    for glob, pattern in sources:
        files = sorted(ROOT.glob(glob))
        if not files:
            sys.exit(f"check_exports: {glob} matches no file")
        for path in files:
            for match in re.finditer(pattern, path.read_text(encoding="utf-8"), re.MULTILINE):
                names.update(group for group in match.groups() if group)
    return names


def main():
    exports = core_exports()
    if VERSION not in exports or len(exports) < 2:
        sys.exit(f"check_exports: read {sorted(exports)} from {CORE}, which is not the core's ABI")
    print(f"core: {len(exports)} exports")
    failed = False
    for binding, sources, wants, extra in SITES:
        if wants == "all":
            expected = set(exports)
        elif wants == "doors":
            expected = exports - {VERSION}
        else:
            expected = {name for name in map(wants, exports) if name}
        expected |= set(extra)
        got = declared(sources)
        missing, stale = sorted(expected - got), sorted(got - expected)
        if missing or stale:
            failed = True
            print(f"FAIL {binding}")
            for name in missing:
                print(f"     missing {name}")
            for name in stale:
                print(f"     declares {name}, which the core does not export")
        else:
            print(f"ok   {binding}: {len(got)}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
