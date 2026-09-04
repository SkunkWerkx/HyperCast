"""The WebAssembly backend: the same Rust core, compiled to ``wasm32-wasip1`` and run inside
CPython by `wasmtime-py <https://github.com/bytecodealliance/wasmtime-py>`_ instead of linked
in as a compiled extension. Nothing here is a second implementation — every door below lands
in the identical ``cast_*`` C-ABI export the PyO3 extension and every other binding in this
repo call, just across a guest/host memory boundary rather than a direct function call.

Selected by ``hypercast`` itself (see ``__init__``): ``HYPERCAST_WASM=1`` forces it, and it is
the automatic fallback when no ``_native`` extension matches the running interpreter and
``wasmtime`` is importable. It exposes exactly the surface ``__init__`` consumes from
``_native`` — the same ``Success``/``Fault``/``NumFormat`` types (``__match_args__``,
equality and ``repr`` included), the same doors with the same argument shapes, the same
exception types and messages — so the package above it never knows which one it got,
and the whole test suite runs against both.

Three things about the crossing are load-bearing:

* **Buffers come from the guest.** A wasm module only sees its own linear memory, so the host
  cannot hand it a pointer the way every native binding does. The module exports wasi-libc's
  ``malloc``/``free`` (see ``rust/.cargo/config.toml``), and every buffer this backend touches
  — the input text, the 16-byte out-value, the fault span, the 32-byte ``NumFormat`` — is one
  the guest's own allocator handed out. Picking a host-side offset past the data segments instead
  corrupts memory: dlmalloc claims the tail of the initial memory on its first allocation.
* **Calls are serialized.** A wasmtime ``Store`` is not thread-safe, so one process-wide lock
  guards every call. Under the GIL that lock is uncontended; on a free-threaded build it is
  what keeps two threads out of one store.
* **The call path avoids wasmtime-py's per-call type lookup.** ``Func.__call__`` re-fetches
  the function's type from the engine and builds and frees a ``FuncType`` plus one ``ValType``
  wrapper per parameter and result on every call, and almost none of that time is in the
  guest. The argument and result arrays are built once here and handed straight to the same
  ``wasmtime_func_call`` C entry point the library uses after that bookkeeping. That path
  touches ``wasmtime._ffi``, which is not public API, so it is bound inside a ``try`` at load
  time and degrades to the public call — slow, never broken — if a wasmtime release moves it.
"""

from __future__ import annotations

import ctypes
import datetime
import struct
import threading
import uuid as _uuid
from decimal import Decimal
from pathlib import Path

_PACKAGE_DIR = Path(__file__).resolve().parent
_MODULE_PATH = _PACKAGE_DIR / "native" / "wasm32-wasip1" / "hypercast.wasm"
# Development loop: the in-repo cargo build, exactly what the Ruby binding's runtime falls
# back to for the native library and what the Java build stages for itself.
_REPO_BUILD_PATH = (
    _PACKAGE_DIR.parent.parent.parent / "rust" / "target" / "wasm32-wasip1" / "release" / "hypercast.wasm"
)

# Signatures of the exports this backend calls, in wasm value-type terms — the C ABI in
# rust/src/ffi.rs with every pointer and size_t an i32 offset into the guest's memory. Kept
# explicit rather than read back from the engine so the direct call path below never has to
# ask wasmtime for a type at call time.
_I32 = "i32"
_PLAIN = ((_I32, _I32, _I32, _I32), _I32)
_NUMERIC = ((_I32, _I32, _I32, _I32, _I32), _I32)
_DECLARED = ((_I32, _I32, _I32, _I32, _I32), _I32)
_SIGNATURES = {
    "cast_bool": _PLAIN,
    "cast_i8": _NUMERIC, "cast_i16": _NUMERIC, "cast_i32": _NUMERIC, "cast_i64": _NUMERIC,
    "cast_u8": _NUMERIC, "cast_u16": _NUMERIC, "cast_u32": _NUMERIC, "cast_u64": _NUMERIC,
    "cast_f32": _NUMERIC, "cast_f64": _NUMERIC, "cast_decimal": _NUMERIC,
    "cast_uuid": _PLAIN,
    "cast_timestamp": _PLAIN, "cast_unix": _DECLARED, "cast_excel_serial": _DECLARED,
    "cast_date": _PLAIN, "cast_date_ordered": _DECLARED, "cast_datetime": _DECLARED,
    "cast_time": _PLAIN, "cast_duration": _PLAIN,
    "hypercast_version": ((), _I32),
    "malloc": ((_I32,), _I32),
    "free": ((_I32,), None),
}

_UUID_NEW = _uuid.UUID.__new__
_OBJECT_SETATTR = object.__setattr__
_IS_SAFE_UNKNOWN = _uuid.SafeUUID.unknown
_UTC = datetime.timezone.utc

# The out-value layouts the core writes — rust/src/verdict.rs's repr(C) structs, little
# endian as wasm memory always is.
_TIMESTAMP = struct.Struct("<qi")
_DATE = struct.Struct("<HBB")
_CIVIL = struct.Struct("<HBB4xQ")
_FAULT = struct.Struct("<II")
_DECIMAL = struct.Struct("<QIBB2x")
# RawNumFormat: the separators as code points, the flags, then the currency symbol as
# `currency_len` UTF-8 bytes held inline (zero-padded to 16) — 32 bytes, 4-byte alignment.
_FORMAT = struct.Struct("<IIII16s")


# --- the verdict types, shaped exactly as the PyO3 extension's --------------------------


class Success:
    """The success case of a verdict: a cast value."""

    __slots__ = ("value",)
    __match_args__ = ("value",)

    def __init__(self, value):  # noqa: ANN001
        _OBJECT_SETATTR(self, "value", value)

    def __setattr__(self, name, value):  # noqa: ANN001
        raise AttributeError(f"attribute '{name}' of 'hypercast.Success' objects is not writable")

    def __eq__(self, other):  # noqa: ANN001
        return isinstance(other, Success) and self.value == other.value

    __hash__ = None

    def __repr__(self) -> str:
        return f"Success(value={self.value!r})"


class Fault:
    """The failure case: a closed reason plus the offending span, in the caller's own units —
    byte offsets for ``bytes`` input, code-point offsets for ``str`` input — so slicing the
    offending text back out of what you passed (``text[offset:offset + length]``) needs no
    mapping."""

    __slots__ = ("reason", "offset", "length")
    __match_args__ = ("reason", "offset", "length")

    def __init__(self, reason, offset: int, length: int):  # noqa: ANN001
        _OBJECT_SETATTR(self, "reason", reason)
        _OBJECT_SETATTR(self, "offset", offset)
        _OBJECT_SETATTR(self, "length", length)

    def __setattr__(self, name, value):  # noqa: ANN001
        raise AttributeError(f"attribute '{name}' of 'hypercast.Fault' objects is not writable")

    def __eq__(self, other):  # noqa: ANN001
        return (
            isinstance(other, Fault)
            and self.offset == other.offset
            and self.length == other.length
            and self.reason == other.reason
        )

    __hash__ = None

    def __repr__(self) -> str:
        return f"Fault(reason={self.reason!r}, offset={self.offset}, length={self.length})"


def _single_char(text: str) -> str:
    if not isinstance(text, str) or len(text) != 1:
        raise ValueError("Separators must be single characters")
    return text


# The bytes the core's ``CurrencySymbol::new`` rejects: ASCII digits and Rust's
# ``is_ascii_whitespace`` set (space, tab, line feed, form feed, carriage return — not
# vertical tab).
_CURRENCY_FORBIDDEN = frozenset(b"0123456789 \t\n\x0c\r")
_CURRENCY_MAX_BYTES = 16


def _currency_bytes(currency: str) -> bytes:
    """The declared symbol as the UTF-8 bytes the core reads — ``b""`` declares none.
    Anything else must be a valid ``CurrencySymbol`` (1 to 16 UTF-8 bytes, no ASCII digit
    or whitespace), or it is a caller bug raised here, at construction, the way equal
    separators are — the same text the PyO3 extension raises."""
    if not isinstance(currency, str):
        raise TypeError("currency must be str")
    if not currency:
        return b""
    encoded = currency.encode("utf-8")
    if len(encoded) > _CURRENCY_MAX_BYTES or any(byte in _CURRENCY_FORBIDDEN for byte in encoded):
        raise ValueError(
            "Currency symbol must be 1 to 16 UTF-8 bytes with no ASCII digit or whitespace; "
            f'got "{currency}"'
        )
    return encoded


class NumFormat:
    """Caller-declared numeric notation, held pre-packed in the 32-byte form the core reads
    (separators, flags, and the currency symbol's UTF-8 bytes inline) so the hot path pays
    zero conversion."""

    __slots__ = ("_decimal_sep", "_group_sep", "_flags", "_currency", "_packed")

    GROUPING = 1 << 0
    PARENTHESES = 1 << 1
    EXPONENT = 1 << 2
    RADIX_PREFIXES = 1 << 3
    PERCENT = 1 << 4
    SEPARATOR_DETECT = 1 << 5
    CURRENCY = 1 << 6
    ALL = GROUPING | PARENTHESES | EXPONENT | RADIX_PREFIXES | PERCENT | CURRENCY

    def __init__(self, decimal_sep: str, group_sep: str, flags: int, currency: str = ""):
        decimal, group = _single_char(decimal_sep), _single_char(group_sep)
        if decimal == group:
            # The same text the PyO3 extension raises, Rust's {:?} quoting included.
            raise ValueError(f'Decimal and group separators must differ; both are "{decimal_sep}"')
        symbol = _currency_bytes(currency)
        _OBJECT_SETATTR(self, "_decimal_sep", decimal)
        _OBJECT_SETATTR(self, "_group_sep", group)
        _OBJECT_SETATTR(self, "_flags", int(flags))
        _OBJECT_SETATTR(self, "_currency", currency)
        _OBJECT_SETATTR(
            self, "_packed", _FORMAT.pack(ord(decimal), ord(group), int(flags), len(symbol), symbol)
        )

    def __setattr__(self, name, value):  # noqa: ANN001
        raise AttributeError(f"attribute '{name}' of 'hypercast.NumFormat' objects is not writable")

    @property
    def decimal_sep(self) -> str:
        """The declared decimal separator."""
        return self._decimal_sep

    @property
    def group_sep(self) -> str:
        """The declared digit-group separator."""
        return self._group_sep

    @property
    def flags(self) -> int:
        """The bitwise OR of the lenience flags."""
        return self._flags

    @property
    def currency(self) -> str:
        """The declared currency symbol — ``""`` when none is declared."""
        return self._currency

    @staticmethod
    def from_localeconv(conv: dict | None = None) -> NumFormat:
        """Bridges ``locale.localeconv()`` (or a dict shaped like it) to a declared format —
        ``decimal_point``, ``thousands_sep``, and ``currency_symbol``."""
        if conv is None:
            import locale

            conv = locale.localeconv()

        def field(name: str, fallback: str) -> str:
            value = conv.get(name)
            if value is None:
                return fallback
            text = str(value)
            return text[0] if text else fallback

        return NumFormat(
            field("decimal_point", "."),
            field("thousands_sep", ","),
            NumFormat.ALL,
            conv.get("currency_symbol", ""),
        )


NumFormat.INVARIANT = NumFormat(".", ",", NumFormat.ALL)
NumFormat.DETECT = NumFormat(".", ",", NumFormat.ALL | NumFormat.SEPARATOR_DETECT)


# --- the guest -----------------------------------------------------------------------------


class _Guest:
    """One instantiated module: its store, its memory, a callable per export, and the guest
    buffers every door crosses through."""

    def __init__(self) -> None:
        import wasmtime

        path = _MODULE_PATH if _MODULE_PATH.is_file() else _REPO_BUILD_PATH
        if not path.is_file():
            raise ImportError(
                f"hypercast: {_MODULE_PATH} not found (this install was built without the "
                "wasm32-wasip1 module)"
            )
        engine = wasmtime.Engine()
        module = wasmtime.Module.from_file(engine, str(path))
        linker = wasmtime.Linker(engine)
        linker.define_wasi()
        self._store = wasmtime.Store(engine)
        # No stdio, no args, no env, no preopens: the module's four WASI imports are
        # wasi-libc's startup and panic plumbing, and the core itself needs nothing.
        self._store.set_wasi(wasmtime.WasiConfig())
        exports = linker.instantiate(self._store, module).exports(self._store)
        self._memory = exports["memory"]
        self._call = {name: self._bind(exports[name], name) for name in _SIGNATURES}

        # Guest-allocated for the life of the process: the 16-byte out-value (every door
        # reads its own prefix), the 8-byte fault span, the 32-byte NumFormat — the same
        # trio the Java binding keeps per thread, held once here because every call is
        # already serialized under the lock.
        self._out = self._malloc(16)
        self._fault = self._malloc(8)
        self._format = self._malloc(_FORMAT.size)
        # Grow-only buffer for the input text, so a steady stream of ordinary scalars never
        # touches the guest allocator again.
        self._in_ptr, self._in_cap = 0, 0
        # The format currently written at _format, memoized by identity: formats are reused
        # objects in practice (INVARIANT, DETECT, a per-locale instance), and NumFormat is
        # immutable, so one `is` check skips the 32-byte write on the overwhelming majority
        # of numeric calls.
        self._last_format: NumFormat | None = None

    # --- guest memory ---------------------------------------------------------------

    def _malloc(self, size: int) -> int:
        ptr = self._call["malloc"](size)
        if ptr == 0:
            raise MemoryError(f"hypercast: guest malloc({size}) failed")
        # Growing the guest memory can relocate it on the host side, and malloc is the only
        # call here that can grow it (no cast_* export allocates), so the host address of
        # offset 0 is re-read exactly here and nowhere else — one fewer wasmtime call on
        # every read and write below.
        self._base = ctypes.addressof(self._memory.data_ptr(self._store).contents)
        return ptr

    def _read(self, ptr: int, length: int) -> bytes:
        return ctypes.string_at(self._base + ptr, length)

    def _write(self, ptr: int, data: bytes) -> None:
        ctypes.memmove(self._base + ptr, data, len(data))

    def _stage_input(self, data: bytes) -> int:
        """Copies the input into the guest and returns its address — 0 (the ABI's NULL) for
        empty input, which the core never dereferences."""
        size = len(data)
        if size == 0:
            return 0
        if size > self._in_cap:
            if self._in_ptr:
                self._call["free"](self._in_ptr)
                self._in_ptr, self._in_cap = 0, 0
            self._in_ptr, self._in_cap = self._malloc(size), size
        self._write(self._in_ptr, data)
        return self._in_ptr

    def _stage_format(self, fmt: NumFormat) -> int:
        if fmt is not self._last_format:
            self._write(self._format, fmt._packed)
            self._last_format = fmt
        return self._format

    # --- calls ----------------------------------------------------------------------

    def _bind(self, func, name):  # noqa: ANN001
        """Return a plain callable for one export. Tries the direct ``wasmtime_func_call``
        path (see the module docstring for why); falls back to ``Func.__call__`` if the
        private surface it needs is not where this was written against.
        """
        param_kinds, result_kind = _SIGNATURES[name]
        try:
            return self._bind_direct(func, param_kinds, result_kind, name)
        except Exception:  # noqa: BLE001 — any drift in wasmtime's internals lands here
            store = self._store
            return lambda *args: func(store, *args)

    def _bind_direct(self, func, param_kinds, result_kind, name):  # noqa: ANN001
        from ctypes import POINTER, byref

        from wasmtime import _ffi as ffi

        n_params, n_results = len(param_kinds), 0 if result_kind is None else 1
        params = (ffi.wasmtime_val_t * max(n_params, 1))()
        results = (ffi.wasmtime_val_t * max(n_results, 1))()
        i32_kind = int(ffi.WASMTIME_I32.value)
        for slot in params[:n_params]:
            slot.kind = i32_kind
        slots = [slot.of for slot in params[:n_params]]
        context = self._store._context()
        func_ref = byref(func._func)
        trap = POINTER(ffi.wasm_trap_t)()
        trap_ref = byref(trap)
        trap_size = ctypes.sizeof(trap)
        wasmtime_func_call = ffi.wasmtime_func_call
        wasmtime_error_delete = ffi.wasmtime_error_delete
        wasm_trap_delete = ffi.wasm_trap_delete

        def call(*args):  # noqa: ANN002, ANN202
            # Every parameter is an i32: a guest offset, a length, a discriminant.
            for slot, value in zip(slots, args):
                slot.i32 = value
            error = wasmtime_func_call(context, func_ref, params, n_params, results, n_results, trap_ref)
            if error:
                wasmtime_error_delete(error)
                raise RuntimeError(f"hypercast: {name} failed inside the wasm guest")
            if trap:
                wasm_trap_delete(trap)
                ctypes.memset(trap_ref, 0, trap_size)
                raise RuntimeError(f"hypercast: {name} trapped inside the wasm guest")
            if result_kind is None:
                return None
            return results[0].of.i32

        return call


_lock = threading.Lock()
_guest: _Guest | None = None


def _get() -> _Guest:
    global _guest
    if _guest is None:
        with _lock:
            if _guest is None:
                _guest = _Guest()
    return _guest


# --- the surface __init__ consumes --------------------------------------------------

_EMPTY = _MALFORMED = _OUT_OF_RANGE = None


def _bind(cast_failure) -> None:  # noqa: ANN001
    """Hands the package's own ``CastFailure`` IntEnum to this backend so faults carry the
    exact members callers compare with ``is`` — the same line ``__init__`` calls on the
    extension — and instantiates the module now rather than on the first call, so a
    missing ``wasmtime`` or a missing ``.wasm`` surfaces at import time exactly where the
    native extension's own import failure would.
    """
    global _EMPTY, _MALFORMED, _OUT_OF_RANGE
    _EMPTY = cast_failure.EMPTY
    _MALFORMED = cast_failure.MALFORMED
    _OUT_OF_RANGE = cast_failure.OUT_OF_RANGE
    _get()


_REASONS = {1: lambda: _EMPTY, 2: lambda: _MALFORMED, 3: lambda: _OUT_OF_RANGE}


def _text(text) -> bytes:  # noqa: ANN001
    if isinstance(text, str):
        return text.encode("utf-8")
    if isinstance(text, bytes):
        return text
    raise TypeError("text must be str or bytes")


def _code_points(chunk: bytes) -> int:
    # Every code point starts with exactly one non-continuation byte.
    return sum(1 for byte in chunk if byte & 0xC0 != 0x80)


def _verdict(guest: _Guest, rc: int, read, text, data: bytes):  # noqa: ANN001
    if rc == 0:
        return Success(read(guest._read(guest._out, 16)))
    if rc == -1:
        raise RuntimeError(
            "hypercast: libhypercast reported a contract violation — a binding bug, please report it"
        )
    offset, length = _FAULT.unpack(guest._read(guest._fault, 8))
    # The core's span is bytes into `data`; a `str` caller's unit is the code point. ASCII
    # needs no mapping (code points equal bytes) and pays one length comparison — the same
    # presentation the extension makes.
    if isinstance(text, str) and len(data) != len(text):
        offset = min(offset, len(data))
        end = min(offset + length, len(data))
        offset, length = _code_points(data[:offset]), _code_points(data[offset:end])
    return Fault(_REASONS[rc](), offset, length)


def _plain(name: str, text, read):  # noqa: ANN001
    data = _text(text)
    guest = _get()
    with _lock:
        in_ptr = guest._stage_input(data)
        rc = guest._call[name](in_ptr, len(data), guest._out, guest._fault)
        return _verdict(guest, rc, read, text, data)


def _numeric(name: str, text, fmt, read):  # noqa: ANN001
    if not isinstance(fmt, NumFormat):
        raise TypeError("format must be a NumFormat")
    data = _text(text)
    guest = _get()
    with _lock:
        in_ptr = guest._stage_input(data)
        format_ptr = guest._stage_format(fmt)
        rc = guest._call[name](in_ptr, len(data), format_ptr, guest._out, guest._fault)
        return _verdict(guest, rc, read, text, data)


def _declared(name: str, text, discriminant: int, read):  # noqa: ANN001
    data = _text(text)
    guest = _get()
    with _lock:
        in_ptr = guest._stage_input(data)
        rc = guest._call[name](in_ptr, len(data), discriminant, guest._out, guest._fault)
        return _verdict(guest, rc, read, text, data)


def _reader(fmt: str):  # noqa: ANN202
    unpack = struct.Struct(fmt).unpack_from
    return lambda out: unpack(out)[0]


_READ_BOOL = lambda out: out[0] != 0  # noqa: E731
_READ_I8, _READ_I16, _READ_I32, _READ_I64 = (_reader(f) for f in ("<b", "<h", "<i", "<q"))
_READ_U8, _READ_U16, _READ_U32, _READ_U64 = (_reader(f) for f in ("<B", "<H", "<I", "<Q"))
_READ_F32, _READ_F64 = (_reader(f) for f in ("<f", "<d"))


def _read_decimal(out: bytes) -> Decimal:
    lo, hi, scale, negative = _DECIMAL.unpack_from(out)
    magnitude = (hi << 64) | lo
    # The (sign, digits, exponent) triple is Decimal's own storage, so the core's canonical
    # scale ("1.10" is 11 at scale 1, trailing zeros trimmed) lands verbatim — no text round trip.
    return Decimal((negative, tuple(map(int, str(magnitude))), -scale))


def _read_uuid(out: bytes) -> _uuid.UUID:
    # The same fastuuid-style constructor the extension uses — ``UUID.__new__`` plus direct
    # slot assignment, because ``UUID.__init__`` would only re-validate a value the core
    # already validated. The result is indistinguishable from ``UUID(bytes=...)``.
    instance = _UUID_NEW(_uuid.UUID)
    _OBJECT_SETATTR(instance, "int", int.from_bytes(out[:16], "big"))
    _OBJECT_SETATTR(instance, "is_safe", _IS_SAFE_UNKNOWN)
    return instance


def _civil_from_days(days: int) -> tuple[int, int, int]:
    """Hinnant's civil_from_days — the same arithmetic the extension runs."""
    shifted = days + 719_468
    era = shifted // 146_097
    day_of_era = shifted - era * 146_097
    year_of_era = (day_of_era - day_of_era // 1_460 + day_of_era // 36_524 - day_of_era // 146_096) // 365
    year = year_of_era + era * 400
    day_of_year = day_of_era - (365 * year_of_era + year_of_era // 4 - year_of_era // 100)
    month_shifted = (5 * day_of_year + 2) // 153
    day = day_of_year - (153 * month_shifted + 2) // 5 + 1
    month = month_shifted + 3 if month_shifted < 10 else month_shifted - 9
    return year + (month <= 2), month, day


def _read_instant(out: bytes) -> datetime.datetime:
    seconds, nanos = _TIMESTAMP.unpack_from(out)
    days, second_of_day = divmod(seconds, 86_400)
    year, month, day = _civil_from_days(days)
    hour, rest = divmod(second_of_day, 3_600)
    minute, second = divmod(rest, 60)
    # Sub-microsecond digits truncate — datetime's ceiling, not the parser's.
    return datetime.datetime(year, month, day, hour, minute, second, nanos // 1_000, _UTC)


def _read_date(out: bytes) -> datetime.date:
    year, month, day = _DATE.unpack_from(out)
    return datetime.date(year, month, day)


def _read_civil(out: bytes) -> datetime.datetime:
    year, month, day, nanos_of_day = _CIVIL.unpack_from(out)
    second_of_day, nano = divmod(nanos_of_day, 1_000_000_000)
    hour, rest = divmod(second_of_day, 3_600)
    minute, second = divmod(rest, 60)
    # Naive: the text named no zone, so the value carries none.
    return datetime.datetime(year, month, day, hour, minute, second, nano // 1_000)


def _read_time(out: bytes) -> datetime.time:
    second_of_day, nano = divmod(_READ_U64(out), 1_000_000_000)
    hour, rest = divmod(second_of_day, 3_600)
    minute, second = divmod(rest, 60)
    return datetime.time(hour, minute, second, nano // 1_000)


def _read_duration(out: bytes) -> datetime.timedelta:
    seconds, nanos = _TIMESTAMP.unpack_from(out)
    # Truncate sub-microsecond digits toward zero on both signs, matching every other
    # binding's truncation; timedelta normalizes the mixed-sign pieces.
    micros = nanos // 1_000 if nanos >= 0 else -((-nanos) // 1_000)
    days, second_of_day = divmod(seconds, 86_400)
    return datetime.timedelta(days=days, seconds=second_of_day, microseconds=micros)


def cast_bool(text):  # noqa: ANN001
    """Casts boolean text under the natural-language lexicon."""
    return _plain("cast_bool", text, _READ_BOOL)


def cast_i8(text, fmt):  # noqa: ANN001
    """Casts integer text to a signed 8-bit value under the declared format."""
    return _numeric("cast_i8", text, fmt, _READ_I8)


def cast_i16(text, fmt):  # noqa: ANN001
    """Casts integer text to a signed 16-bit value under the declared format."""
    return _numeric("cast_i16", text, fmt, _READ_I16)


def cast_i32(text, fmt):  # noqa: ANN001
    """Casts integer text to a signed 32-bit value under the declared format."""
    return _numeric("cast_i32", text, fmt, _READ_I32)


def cast_i64(text, fmt):  # noqa: ANN001
    """Casts integer text to a signed 64-bit value under the declared format."""
    return _numeric("cast_i64", text, fmt, _READ_I64)


def cast_u8(text, fmt):  # noqa: ANN001
    """Casts integer text to an unsigned 8-bit value under the declared format."""
    return _numeric("cast_u8", text, fmt, _READ_U8)


def cast_u16(text, fmt):  # noqa: ANN001
    """Casts integer text to an unsigned 16-bit value under the declared format."""
    return _numeric("cast_u16", text, fmt, _READ_U16)


def cast_u32(text, fmt):  # noqa: ANN001
    """Casts integer text to an unsigned 32-bit value under the declared format."""
    return _numeric("cast_u32", text, fmt, _READ_U32)


def cast_u64(text, fmt):  # noqa: ANN001
    """Casts integer text to an unsigned 64-bit value — the true unsigned value, ``int``
    being unbounded — under the declared format."""
    return _numeric("cast_u64", text, fmt, _READ_U64)


def cast_f32(text, fmt):  # noqa: ANN001
    """Casts real text to an IEEE single (widened losslessly) under the declared format."""
    return _numeric("cast_f32", text, fmt, _READ_F32)


def cast_f64(text, fmt):  # noqa: ANN001
    """Casts real text to an IEEE double under the declared format."""
    return _numeric("cast_f64", text, fmt, _READ_F64)


def cast_decimal(text, fmt):  # noqa: ANN001
    """Casts decimal text under the declared format to an exact, canonical
    ``decimal.Decimal`` — trailing fraction zeros trimmed, so ``"1.10"`` is ``Decimal('1.1')``;
    never rounded."""
    return _numeric("cast_decimal", text, fmt, _read_decimal)


def cast_uuid(text):  # noqa: ANN001
    """Casts UUID text — every .NET ``Guid`` form plus ``urn:uuid:``-style prefixes — to a
    ``uuid.UUID``."""
    return _plain("cast_uuid", text, _read_uuid)


def cast_timestamp(text):  # noqa: ANN001
    """Casts an RFC 3339 instant to an aware UTC ``datetime`` (microsecond truncation)."""
    return _plain("cast_timestamp", text, _read_instant)


def cast_unix(text, precision: int):  # noqa: ANN001
    """Casts an integer Unix-epoch value under the declared ``UnixPrecision``."""
    if precision not in (1, 2, 3, 4):
        raise ValueError("precision must be a UnixPrecision")
    return _declared("cast_unix", text, int(precision), _read_instant)


def cast_excel_serial(text, epoch: int):  # noqa: ANN001
    """Casts an Excel date serial under the declared ``ExcelEpoch``."""
    if epoch not in (1, 2):
        raise ValueError("epoch must be an ExcelEpoch")
    return _declared("cast_excel_serial", text, int(epoch), _read_instant)


def cast_date(text, order: int | None = None):  # noqa: ANN001
    """Casts a calendar date: strict ISO ``yyyy-MM-dd`` with no order, the separated forms
    under a declared ``DateOrder``."""
    if order is None:
        return _plain("cast_date", text, _read_date)
    if order not in (1, 2, 3):
        raise ValueError("order must be a DateOrder")
    return _declared("cast_date_ordered", text, int(order), _read_date)


def cast_datetime(text, order: int):  # noqa: ANN001
    """Casts a zone-less civil date-time under a declared ``DateOrder`` to a naive
    ``datetime``."""
    if order not in (1, 2, 3):
        raise ValueError("order must be a DateOrder")
    return _declared("cast_datetime", text, int(order), _read_civil)


def cast_time(text):  # noqa: ANN001
    """Casts an ISO 24-hour time-of-day to a ``time`` (microsecond truncation)."""
    return _plain("cast_time", text, _read_time)


def cast_duration(text):  # noqa: ANN001
    """Casts a duration (ISO 8601, invariant colon form, or protobuf JSON seconds) to a
    ``timedelta`` (microsecond truncation toward zero)."""
    return _plain("cast_duration", text, _read_duration)


def native_version() -> str:
    """This library's version as ``"major.minor.patch"``, decoded from the packed
    ``hypercast_version`` export of the wasm module actually loaded."""
    guest = _get()
    with _lock:
        packed = guest._call["hypercast_version"]()
    return f"{packed >> 16}.{(packed >> 8) & 0xFF}.{packed & 0xFF}"
