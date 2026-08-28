"""ctypes plumbing for the native libhypercast shared library.

A native call shares this process's address space directly, so there's no alloc/dealloc
dance: inputs are the caller's own bytes, outputs are plain ctypes out-params passed by
reference. Loads the packaged library when this is an installed wheel, falling back to the
in-repo cargo build for the development loop.
"""

from __future__ import annotations

import ctypes
import importlib.resources
import threading
from pathlib import Path

_SO_RESOURCE = "libhypercast.so"

_lib: ctypes.CDLL | None = None
_lib_lock = threading.Lock()


class RawFault(ctypes.Structure):
    _fields_ = [("offset", ctypes.c_uint32), ("length", ctypes.c_uint32)]


class RawNumFormat(ctypes.Structure):
    _fields_ = [
        ("decimal_sep", ctypes.c_uint32),
        ("group_sep", ctypes.c_uint32),
        ("flags", ctypes.c_uint32),
    ]


class RawTimestamp(ctypes.Structure):
    _fields_ = [("seconds", ctypes.c_int64), ("nanos", ctypes.c_int32)]


class RawDate(ctypes.Structure):
    _fields_ = [("year", ctypes.c_uint16), ("month", ctypes.c_uint8), ("day", ctypes.c_uint8)]


class RawDuration(ctypes.Structure):
    _fields_ = [("seconds", ctypes.c_int64), ("nanos", ctypes.c_int32)]


def _library_path() -> Path:
    resource = importlib.resources.files(__package__).joinpath(_SO_RESOURCE)
    with importlib.resources.as_file(resource) as path:
        if path.exists():
            return path
    # Development loop: walk up to the repository root and use the release-profile cargo
    # build, exactly what the C# and Java bindings' local staging does.
    for parent in Path(__file__).resolve().parents:
        candidate = parent / "rust" / "target" / "release" / _SO_RESOURCE
        if candidate.exists():
            return candidate
    raise OSError(
        f"{_SO_RESOURCE} not found: neither packaged with hypercast nor built in-repo "
        "(run a release-profile cargo build in rust/)"
    )


def _load() -> ctypes.CDLL:
    lib = ctypes.CDLL(str(_library_path()))

    plain = [ctypes.c_char_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.POINTER(RawFault)]
    numeric = [
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.POINTER(RawNumFormat),
        ctypes.c_void_p,
        ctypes.POINTER(RawFault),
    ]
    unix = [
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.c_uint32,
        ctypes.c_void_p,
        ctypes.POINTER(RawFault),
    ]

    lib.cast_bool.argtypes = plain
    for door in ("cast_i8", "cast_i16", "cast_i32", "cast_i64",
                 "cast_u8", "cast_u16", "cast_u32", "cast_u64",
                 "cast_f32", "cast_f64"):
        getattr(lib, door).argtypes = numeric
    lib.cast_uuid.argtypes = plain
    lib.cast_timestamp.argtypes = plain
    lib.cast_unix.argtypes = unix
    lib.cast_date.argtypes = plain
    lib.cast_time.argtypes = plain
    lib.cast_duration.argtypes = plain
    for door in ("cast_bool", "cast_i8", "cast_i16", "cast_i32", "cast_i64",
                 "cast_u8", "cast_u16", "cast_u32", "cast_u64", "cast_f32", "cast_f64",
                 "cast_uuid", "cast_timestamp", "cast_unix", "cast_date", "cast_time",
                 "cast_duration"):
        getattr(lib, door).restype = ctypes.c_int32
    return lib


def get_lib() -> ctypes.CDLL:
    global _lib
    if _lib is None:
        with _lib_lock:
            if _lib is None:
                _lib = _load()
    return _lib
