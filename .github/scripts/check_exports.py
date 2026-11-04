"""Checks that every binding declares exactly the C ABI the core exports.

The export list is read from the core's source: every `pub extern "C" fn` in rust/src/ffi.rs,
the one file that exports (cbindgen generates include/hypercast.h from it). Each binding's declarations are then read from its own source by the patterns in
SITES, and any difference fails the check: a missing export means a door nobody added to that
binding, and an extra one means the binding still declares a door the core dropped or renamed.
Both of those otherwise surface late, in one binding's suite on whichever leg calls the door.
The one allowance is SUPERSEDES: a site that calls a 1.1 detailed export may omit the 1.0
exports that export reaches by door code.

The Python and Ruby extensions are not C ABI consumers: they link the core as a Rust crate
and register the doors under their language's spelling, so their sites map each export to
that spelling (a None folds the export into another entry point: the four detailed exports
fold into the doors, which are the core's detailed doors). The PHP extension
(rust/src/php_ext/) is not a C ABI consumer either, and calls the core's Rust API, so it has no
site; php_guard holds every PHP-callable function in it to `guard(`, and php_link its extern blocks to the php8/php8ts pair.

No dependencies and no build: CI runs it in the lint jobs, and so can a dev loop.

usage: python .github/scripts/check_exports.py
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

CORE = ["rust/src/ffi.rs"]
VERSION = "hypercast_version"


def core_exports():
    names = set()
    for path in CORE:
        text = (ROOT / path).read_text(encoding="utf-8")
        names |= set(re.findall(r'pub extern "C" fn (\w+)', text))
    return names


def python_spelling(export):
    """python_ext.rs's name for an export: the typed doors say float, and cast_date takes
    the order cast_date_ordered would."""
    if export == VERSION:
        return "native_version"
    if export == "cast_date_ordered":
        return None
    return export.replace("_from_f64", "_from_float")


# The 1.1 detailed exports and the 1.0 exports each one reaches by door code, as
# include/hypercast.h documents them: cast_detailed reads every text door (Door's codes 1 to
# 23, cast_text's span at 18), cast_detailed_from_f64 the four typed doors, and the checked
# pair the two checked exports. A site that calls a detailed export may omit the 1.0 exports
# it supersedes (a binding that routes its doors through it no longer calls them); a site may
# still declare them (C, C++ and C# declare every export). The exports no detailed one reaches
# (hypercast_version, cast_constraints_check, cast_declared_decimal) and the four detailed
# exports themselves are required at every site, in the site's own spelling.
SUPERSEDES = {
    "cast_detailed": (
        "cast_bool",
        "cast_i8",
        "cast_i16",
        "cast_i32",
        "cast_i64",
        "cast_u8",
        "cast_u16",
        "cast_u32",
        "cast_u64",
        "cast_f32",
        "cast_f64",
        "cast_uuid",
        "cast_timestamp",
        "cast_unix",
        "cast_date",
        "cast_time",
        "cast_duration",
        "cast_text",
        "cast_decimal",
        "cast_date_ordered",
        "cast_datetime",
        "cast_excel_serial",
        "cast_char",
    ),
    "cast_detailed_from_f64": (
        "cast_decimal_from_f64",
        "cast_excel_serial_from_f64",
        "cast_excel_time",
        "cast_excel_duration",
    ),
    "cast_checked_detailed": ("cast_checked",),
    "cast_checked_detailed_from_f64": ("cast_checked_from_f64",),
}

DETAILED = set(SUPERSEDES)


def detail_folded(spelling):
    """`spelling`, with the four detailed exports folded (None) into the existing doors: for
    an extension that links the core as a Rust crate, each door it registers is the core's
    own `detailed` door (a fault carries its detail), so there is no second door to declare,
    and its 1.0 doors are required."""

    def spelled(export):
        return None if export in DETAILED else spelling(export)

    spelled.__doc__ = f"{spelling.__name__}, with the detailed exports folded into the doors."
    return spelled


def superseded(wants, got):
    """The names a site may omit: each 1.0 export, in the site's spelling, whose detailed
    export the site calls. A detailed export the site folds (None) supersedes nothing."""
    optional = set()
    for detailed, olds in SUPERSEDES.items():
        spelled = wants(detailed)
        if spelled is not None and spelled in got:
            optional |= {wants(old) for old in olds}
    return optional - {None}


pyo3_spelling = detail_folded(python_spelling)


def ts_spelling(export):
    """The TypeScript addon's name for an export, before napi-rs camel-cases it: the typed
    doors say number (a JavaScript number is an f64), cast_date takes the order
    cast_date_ordered would, and the version probe is nativeVersion."""
    if export == VERSION:
        return "native_version"
    if export == "cast_date_ordered":
        return None
    return export.replace("_from_f64", "_from_number")


def ts_camel(export):
    """binding.d.ts's name for an export, or None for one folded into another door."""
    name = ts_spelling(export)
    return camel(name) if name else None


def napi_catch_unwind(path):
    """Every napi function, getter and factory must carry `catch_unwind`: without it a panic
    unwinds into Node-API and aborts the process instead of throwing. The attribute may span
    lines and may sit in a macro_rules! template; attributes above consts, structs, enums and
    impls are not functions and are skipped. The one exception is a `#[napi(module_exports)]`
    hook, which takes no `catch_unwind` option (napi runs it once, at load, outside any call):
    its body must call `catch_unwind(` itself."""
    path = Path(path)
    if not path.is_absolute():
        path = ROOT / path
    text = path.read_text(encoding="utf-8")
    bad, functions = [], 0
    for match in re.finditer(r"#\[napi(\((?:[^()]|\([^()]*\))*\))?\]", text):
        rest = text[match.end() :]
        item = re.match(r"(?:\s*#\[[^\]]*\])*\s*(?:pub(?:\([^)]*\))? )?(?:const |unsafe |async )*fn ([\w$]+)", rest)
        if not item:
            continue
        functions += 1
        options = match.group(1) or ""
        if re.fullmatch(r"\(\s*module_exports\s*\)", options):
            body = _fn_body(_mask_rust(text), match.end() + item.end())
            if body is not None and "catch_unwind(" in body:
                continue
        if "catch_unwind" not in options:
            line = text.count("\n", 0, match.start()) + 1
            shown = path.relative_to(ROOT) if path.is_relative_to(ROOT) else path
            bad.append(f"{shown}:{line} {item.group(1)}")
    if not functions:
        sys.exit(f"check_exports: no #[napi] function found in {path}")
    if bad:
        sys.exit("check_exports: napi functions without catch_unwind:\n  " + "\n  ".join(bad))


def camel(name):
    """The JNI natives' Java spelling of an export: `cast_excel_serial_from_f64` is
    `castExcelSerialFromF64`."""
    head, *rest = name.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in rest)


def jni_no_panic(path):
    """Every JNI entry point must carry the no-panic attribute: the example's link-time proof
    covers only the fns that have it, so one dropped attribute would pass every site."""
    path = Path(path)
    if not path.is_absolute():
        path = ROOT / path
    lines = path.read_text(encoding="utf-8").splitlines()
    bad = []
    for number, line in enumerate(lines):
        match = re.match(r'pub unsafe extern "system" fn (\w+)', line)
        if not match:
            continue
        attrs, above = [], number - 1
        while above >= 0 and lines[above].startswith("#["):
            attrs.append(lines[above])
            above -= 1
        if not any("no_panic::no_panic" in attr for attr in attrs):
            bad.append(f"{match.group(1)} (line {number + 1})")
    if bad:
        sys.exit(f"check_exports: {path.name} has JNI functions without no_panic: " + ", ".join(bad))


_RUST_NOISE = re.compile(
    r"""//[^\n]*|/\*.*?\*/|b?"(?:\\.|[^"\\])*"|'(?:\\.[^']*|[^\\'])'""", re.DOTALL
)


def _mask_rust(text):
    """Blanks comments, string literals and char literals (newlines kept) so that braces and
    parentheses inside them don't confuse the matching below."""
    return _RUST_NOISE.sub(lambda m: re.sub(r"[^\n]", " ", m.group(0)), text)


def _match_close(text, open_at, open_char, close_char):
    """Index of the bracket closing the one at `open_at`, or -1."""
    depth = 0
    for index in range(open_at, len(text)):
        if text[index] == open_char:
            depth += 1
        elif text[index] == close_char:
            depth -= 1
            if depth == 0:
                return index
    return -1


def _fn_body(masked, name_end):
    """The body text (between the braces) of the fn whose name ends at `name_end`, or None
    for a declaration without one."""
    paren = masked.find("(", name_end)
    if paren < 0:
        return None
    close = _match_close(masked, paren, "(", ")")
    if close < 0:
        return None
    # The body's `{` or a `;` ends the signature only outside every bracket: `[u8; 16]` and
    # `Foo<[u8; 4]>` in a return type carry a `;`. `->` is an arrow, not a closing angle.
    depth, index, start = 0, close + 1, -1
    while index < len(masked):
        char = masked[index]
        if char in "[<(":
            depth += 1
        elif char in "])" or (char == ">" and masked[index - 1] != "-"):
            depth -= 1
        elif depth <= 0 and char in "{;":
            start = index if char == "{" else -1
            break
        index += 1
    if start < 0:
        return None
    end = _match_close(masked, start, "{", "}")
    return masked[start + 1 : end] if end >= 0 else None


def php_guard(directory):
    """Every PHP-callable Rust function in the extension must start with `guard(`: a panic
    that unwinds through the Zend engine's C frames aborts the PHP process instead of
    throwing. PHP-callable means every fn in a `#[php_impl]` block, every `#[php_function]`,
    and every handler registered with `FunctionBuilder::new("name", handler)` (the value
    classes' methods, a hand-written `extern "C" fn`, which must call `guard(` somewhere
    inside, since `run_handler(` surrounds it).

    Every other `extern "C" fn` the engine calls (the lifecycle callbacks: MINIT, RINIT,
    RSHUTDOWN, post-deactivate, post-startup) has no PHP caller to throw to, so it must contain
    `catch_unwind(` or `run_handler(` instead."""
    directory = Path(directory)
    if not directory.is_absolute():
        directory = ROOT / directory
    files = sorted(directory.glob("*.rs"))
    if not files:
        sys.exit(f"check_exports: {directory} matches no file")
    bad, unwinding, checked = [], [], 0
    fn_head = re.compile(r"\bfn\s+(\w+)")
    for path in files:
        raw = path.read_text(encoding="utf-8")
        masked = _mask_rust(raw)  # same length as `raw`, so offsets carry across
        for match in re.finditer(r"#\[php_impl\b[^\]]*\]", masked):
            brace = masked.find("{", match.end())
            end = _match_close(masked, brace, "{", "}")
            block = masked[brace + 1 : end]
            for fn in fn_head.finditer(block):
                if block.count("{", 0, fn.start()) != block.count("}", 0, fn.start()):
                    continue  # a fn nested in a method body
                body = _fn_body(block, fn.end())
                checked += 1
                if body is None or not body.lstrip().startswith("guard("):
                    bad.append(fn.group(1))
        for match in re.finditer(r"#\[php_function\b[^\]]*\]", masked):
            fn = fn_head.search(masked, match.end())
            body = _fn_body(masked, fn.end()) if fn else None
            checked += fn is not None
            if fn and (body is None or not body.lstrip().startswith("guard(")):
                bad.append(fn.group(1))
        # The handler names survive masking: only the method's name is a string literal.
        for handler in re.findall(r"FunctionBuilder::new\(\s*\"[^\"]*\"\s*,\s*(\w+)\s*\)", raw):
            found = re.search(rf'extern "C" fn {handler}\b', raw)
            inner = _fn_body(masked, found.end()) if found else None
            checked += 1
            if inner is None or "guard(" not in inner:
                bad.append(handler)
        for found in re.finditer(r'\bextern\s+"C"\s+fn\s+(\w+)', raw):
            body = _fn_body(masked, found.end())
            if body is None or not ("catch_unwind(" in body or "run_handler(" in body):
                unwinding.append(found.group(1))
    if not checked:
        sys.exit("check_exports: php_ext has no PHP-callable functions to check")
    if bad:
        sys.exit("check_exports: php_ext has PHP-callable functions without guard: " + ", ".join(bad))
    if unwinding:
        sys.exit(
            'check_exports: php_ext has extern "C" functions without catch_unwind or run_handler: '
            + ", ".join(unwinding)
        )


PHP_LINK_NTS = '#[cfg_attr(all(windows, not(php_zts)), link(name = "php8"))]'
PHP_LINK_TS = '#[cfg_attr(all(windows, php_zts), link(name = "php8ts"))]'


def php_link(directory):
    """Every extern block in the extension that names the engine's Windows import library names
    it through the pair PHP_LINK_NTS, PHP_LINK_TS, on adjacent lines and opening the block (other
    attributes may follow). A thread-safe Windows PHP is php8ts.dll, with php8ts.lib and no
    php8.lib: a bare `link(name = "php8")` fails to link there, or links a DLL PHP cannot load.
    build.rs sets `php_zts` from the PHP ext-php-rs builds against."""
    directory = Path(directory)
    if not directory.is_absolute():
        directory = ROOT / directory
    files = sorted(directory.glob("*.rs"))
    if not files:
        sys.exit(f"check_exports: {directory} matches no file")
    bad = []
    for path in files:
        raw = path.read_text(encoding="utf-8").split("\n")
        masked = _mask_rust("\n".join(raw)).split("\n")
        paired = set()
        for index, line in enumerate(raw):
            if not masked[index].lstrip().startswith("#[") or not re.search(r'\blink\(\s*name\s*=\s*"php8', line):
                continue
            if index in paired:
                continue
            rest = masked[index + 2 :]
            opens = next((code.strip() for code in rest if not code.strip().startswith("#[")), "")
            if (
                line.strip() == PHP_LINK_NTS
                and index + 1 < len(raw)
                and raw[index + 1].strip() == PHP_LINK_TS
                and re.match(r"(unsafe\s+)?extern\b", opens)
            ):
                paired.add(index + 1)
                continue
            bad.append(f"{path.name}:{index + 1}")
    if bad:
        sys.exit(
            "check_exports: php_ext names the engine's library without the php8/php8ts pair "
            "(build.rs's php_zts): " + ", ".join(bad)
        )


def ruby_spelling(export):
    """ruby_ext.rs's name: the Python extension's (detailed exports folded), without the
    cast_ prefix (the module is the namespace)."""
    spelled = pyo3_spelling(export)
    return spelled and spelled.removeprefix("cast_")


C_PROTOTYPE = r"(?m)^(?:u?int\d+_t|void) (\w+)\("

# Swift's comments (`//` and `/* */`) and string literals (plain, multi-line and raw `#"…"#`).
_SWIFT_NOISE = re.compile(
    r'//[^\n]*|/\*.*?\*/|(#+)"""(?:.)*?"""\1|"""(?:\\.|[^\\])*?"""|(#+)"[^\n]*?"\2|"(?:\\.|[^"\\\n])*"',
    re.DOTALL,
)


# C, C++ and Go comments and string, char and raw-string literals.
_C_NOISE = re.compile(r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\\n])*"|\'(?:\\.|[^\'\\\n])\'|`[^`]*`', re.DOTALL)


def code_only(pattern):
    """A site pattern that reads only code: comments and literals are blanked first, so a
    comment naming an export never stands in for a call."""

    def calls(text):
        masked = _C_NOISE.sub(lambda m: re.sub(r"[^\n]", " ", m.group(0)), text)
        return [g for m in re.finditer(pattern, masked, re.MULTILINE) for g in m.groups() if g]

    calls.pattern = pattern
    return calls


def swift_calls(text):
    """Every export a Swift source calls (followed by its argument list) or passes as a
    function (followed by the closing parenthesis), in its code: comments and string literals
    are blanked first, so neither ever stands in for a call, and every call on a line counts."""
    masked = _SWIFT_NOISE.sub(lambda m: re.sub(r"[^\n]", " ", m.group(0)), text)
    return re.findall(r"\b((?:cast|hypercast)_\w+)(?=[()])", masked)


# (binding, [(file glob, pattern)], what it must declare, names it may declare besides).
# A pattern is a regex whose groups are the names, or a function from the file's text to them.
# `all` is every export, `doors` every export but the version probe, and a function maps
# each export to the binding's own name. At every site, SUPERSEDES lets a binding that calls a
# detailed export omit the 1.0 exports it supersedes (see superseded()). `code_only` sites
# blank comments and literals first, so a comment naming an export is not a call.
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
        [
            (
                "java/src/main/java25/io/github/skunkwerkx/hypercast/NativeCore.java",
                r'export\("(\w+)"\)',
            )
        ],
        "all",
        (),
    ),
    # The JDK 21 base: the Java declarations, the JNI library's registration table, its
    # no-panic proof (snake case, so not `camel`) and its own C imports of the core.
    (
        "Java (JNI natives)",
        [
            (
                "java/src/main/java/io/github/skunkwerkx/hypercast/NativeCore.java",
                r"static native \w+ (\w+)\(",
            )
        ],
        camel,
        ("jniNatives",),
    ),
    (
        "Java (JNI table)",
        [("java/jni/src/lib.rs", r'(?m)^\s*native!\(\s*"(\w+)",')],
        camel,
        ("jniNatives",),
    ),
    (
        "Java (JNI no-panic)",
        [("java/jni/examples/no_panic.rs", r"hypercast_jni::(?:native_(\w+)|(JNI_OnLoad))\(")],
        "all",
        ("jni_natives", "JNI_OnLoad"),
    ),
    (
        "Java (JNI imports)",
        [("java/jni/src/lib.rs", r"(?m)^\s*fn ((?:cast|hypercast)_\w+)\(")],
        "all",
        (),
    ),
    (
        # The core translated to Go by wasm2go (go/internal/core/core.go, regenerated by `go
        # generate` in go/): every export is a method on the generated Module. The prefix
        # matters: wasm2go also emits Xmemory, which a bare X(\w+) would call a stale export.
        "Go (translated core)",
        [("go/internal/core/core.go", r"func \(m \*Module\) X((?:cast|hypercast)_\w+)\(")],
        "all",
        (),
    ),
    (
        # The binding's use of the translated core: every export is named in go/core.go, as a
        # method expression (the doors cast.go passes around) or a call, in code.
        "Go (binding calls)",
        [("go/core.go", code_only(r"\.X((?:cast|hypercast)_\w+)\b"))],
        "all",
        (),
    ),
    # Swift reaches the C exports through the HyperCastCore module (the bundle's copy of the
    # header): every export is called, or passed as a function (the version probe), by name.
    # Comments and strings are skipped, so a doc comment naming an export never stands in for a call.
    (
        "Swift (HyperCastCore calls)",
        [("swift/Sources/HyperCast/*.swift", swift_calls)],
        "all",
        (),
    ),
    # Generated by cbindgen (rust/tests/header.rs), which also holds the Swift bundle's copy
    # byte-identical to it.
    ("C (include/hypercast.h)", [("include/hypercast.h", C_PROTOTYPE)], "all", ()),
    # The header-only C++ binding: every export is named qualified, as a call or (the numeric
    # doors) passed as a function, hence the word boundary rather than a parenthesis.
    ("C++ (include/hypercast.hpp)", [("include/hypercast.hpp", code_only(r"::((?:cast|hypercast)_\w+)\b"))], "all", ()),
    # napi-rs generates binding.d.ts from typescript/src/lib.rs (`npm run build` rewrites it),
    # so it is the one list of what TypeScript callers see, the macro-made doors included.
    (
        "TypeScript (binding.d.ts)",
        [("typescript/binding.d.ts", r"(?m)^export declare function (\w+)\(")],
        ts_camel,
        (),
    ),
    # The R package's C shim calls the core through the header vendor.sh stages from
    # include/, so the calls it makes (in code) are its declarations.
    ("R (C shim)", [("r/src/hypercast.c", code_only(r"\b(cast_[a-z0-9_]+|hypercast_version)\("))], "all", ()),
    ("C (smoke)", [("c/smoke/main.c", code_only(r"\b((?:cast|hypercast)_\w+)\("))], "all", ()),
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
        pyo3_spelling,
        ("_bind", "_detail_name"),
    ),
    (
        "Python (stubs)",
        [("python/src/hypercast/_native.pyi", r"(?m)^def (\w+)\(")],
        pyo3_spelling,
        ("_bind", "_detail_name"),
    ),
    (
        "cdylib smoke",
        [(".github/scripts/cdylib_smoke.py", r'"(cast_\w+)": |lib\.(\w+)\.restype')],
        "all",
        (),
    ),
]


# Sites whose names must each appear once: a table that registers a name twice is a bug a
# set comparison cannot see.
UNIQUE_SITES = ("Java (JNI table)",)


def declared_list(sources):
    names = []
    for glob, pattern in sources:
        files = sorted(ROOT.glob(glob))
        if not files:
            sys.exit(f"check_exports: {glob} matches no file")
        for path in files:
            text = path.read_text(encoding="utf-8")
            if callable(pattern):
                names.extend(pattern(text))
                continue
            for match in re.finditer(pattern, text, re.MULTILINE):
                names.extend(group for group in match.groups() if group)
    return names


def declared(sources):
    return set(declared_list(sources))


def main():
    exports = core_exports()
    if VERSION not in exports or len(exports) < 2:
        sys.exit(f"check_exports: read {sorted(exports)} from {CORE}, which is not the core's ABI")
    print(f"core: {len(exports)} exports")
    jni_no_panic("java/jni/src/lib.rs")
    php_guard("rust/src/php_ext")
    php_link("rust/src/php_ext")
    napi_catch_unwind("typescript/src/lib.rs")
    unknown = sorted((DETAILED | {name for olds in SUPERSEDES.values() for name in olds}) - exports)
    if unknown:
        sys.exit(f"check_exports: SUPERSEDES names {unknown}, which the core does not export")
    failed = False
    for binding, sources, wants, extra in SITES:
        if wants == "all":
            wants = str
        elif wants == "doors":
            wants = lambda name: None if name == VERSION else name
        expected = {name for name in map(wants, exports) if name} | set(extra)
        found = declared_list(sources)
        got = set(found)
        repeats = sorted({name for name in found if found.count(name) > 1}) if binding in UNIQUE_SITES else []
        missing, stale = sorted(expected - got - superseded(wants, got)), sorted(got - expected)
        if missing or stale or repeats:
            failed = True
            print(f"FAIL {binding}")
            for name in repeats:
                print(f"     repeats {name}")
            for name in missing:
                print(f"     missing {name}")
            for name in stale:
                print(f"     declares {name}, which the core does not export")
        else:
            omitted = len(expected - got)
            note = f" ({omitted} 1.0 exports superseded by the detailed ones)" if omitted else ""
            print(f"ok   {binding}: {len(got)}{note}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
