# hypercast for C and C++

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![C/C++](https://img.shields.io/github/v/release/SkunkWerkx/HyperCast?label=c%2Fc%2B%2B&sort=semver)](https://github.com/SkunkWerkx/HyperCast/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

Allocation-free casts from untrusted text to typed values: booleans, characters, integers,
reals, exact decimals, UUIDs and temporals. Every cast returns the value or a fault, a closed
reason (empty, malformed, out of range) with the byte span of the input that caused it. It is
the same core every HyperCast binding uses, here through the C ABI in `include/hypercast.h`.
C++17 and later have their own header over it, `include/hypercast.hpp`, linked against the
same library.

## Install

Each GitHub Release carries one archive per platform, `hypercast-<version>-<rid>.tar.gz`
(`.zip` on Windows), for linux-x64, linux-arm64, linux-musl-x64, linux-musl-arm64, osx-x64,
osx-arm64, win-x64, win-arm64, android-arm64, android-x64, ios-arm64, iossimulator-arm64,
maccatalyst-arm64, maccatalyst-x64 and wasi-wasm32. Verify one before use; the release
attestation lists every file the release shipped:

```sh
gh release download v<version> --repo SkunkWerkx/HyperCast --pattern 'hypercast-*-linux-x64.tar.gz'
gh attestation verify hypercast-<version>-linux-x64.tar.gz --repo SkunkWerkx/HyperCast
```

Unpack it anywhere:

```
include/hypercast.h, include/hypercast.hpp
lib/libhypercast.a                       static (Windows: lib/hypercast.lib)
lib/libhypercast.so | lib/libhypercast.dylib
                                         shared (Windows: bin/hypercast.dll and its
                                         import library lib/hypercast.dll.lib)
lib/pkgconfig/hypercast.pc
lib/cmake/hypercast/
LICENSE, README.md                       this file
```

The iOS, iOS simulator and Mac Catalyst archives have the static library only, linked into
the app, and so does wasi-wasm32, for wasi-sdk.

## Build against it

CMake, with the unpacked directory on `CMAKE_PREFIX_PATH`:

```cmake
find_package(hypercast 1.0 REQUIRED)
target_link_libraries(app PRIVATE hypercast::hypercast)          # shared
target_link_libraries(app PRIVATE hypercast::hypercast_static)   # or static
```

Or let CMake fetch the archive at configure time. Name your platform's archive and its
SHA-256, which the release's `inventory.json` lists for every file:

```cmake
include(FetchContent)
FetchContent_Declare(hypercast
  URL https://github.com/SkunkWerkx/HyperCast/releases/download/v<version>/hypercast-<version>-linux-x64.tar.gz
  URL_HASH SHA256=<sha256>)
FetchContent_MakeAvailable(hypercast)
find_package(hypercast CONFIG REQUIRED PATHS "${hypercast_SOURCE_DIR}" NO_DEFAULT_PATH NO_CMAKE_FIND_ROOT_PATH)
target_link_libraries(app PRIVATE hypercast::hypercast_static)
```

The archive holds no `CMakeLists.txt`, so `FetchContent_MakeAvailable` only downloads,
checks and unpacks it; `find_package` then reads its CMake files from where it landed.
`NO_CMAKE_FIND_ROOT_PATH` keeps a cross build (iOS, Android) from looking for them inside its
SDK instead. Without FetchContent, a cross build names the archive with
`-Dhypercast_DIR=<dir>/lib/cmake/hypercast`, for the same reason.

pkg-config, with `<dir>/lib/pkgconfig` on `PKG_CONFIG_PATH`:

```sh
cc app.c $(pkg-config --cflags --libs hypercast)
```

The static library needs nothing past the C runtime. To link it with pkg-config, name it
directly: `cc app.c $(pkg-config --cflags hypercast) <dir>/lib/libhypercast.a`.

The shared library is found at run time by its own name: `@rpath/libhypercast.dylib` on
macOS and the soname `libhypercast.so` on Linux and Android, so a program needs an rpath
(`-Wl,-rpath,<dir>/lib`) or the library on the loader's path. On Windows the DLL goes beside
the program.

## C

```c
#include <stdio.h>
#include "hypercast.h"

int main(void) {
  const char text[] = "(1,234)";
  int32_t value;
  HyperCastRawFault fault;
  switch (cast_i32((const uint8_t *)text, sizeof text - 1, NULL, &value, &fault)) {
  case 0:
    printf("got %d\n", value); /* -1234, accounting negative */
    break;
  case -1:
    return 2; /* a contract violation: a caller bug, not data */
  default:    /* 1 empty, 2 malformed, 3 out of range */
    printf("fault at byte %u, %u long\n", fault.offset, fault.len);
  }
  return 0;
}
```

Every door reads `len` bytes of UTF-8 and returns its verdict as the return code: 0 with the
value written to `out`, or 1 empty, 2 malformed, 3 out of range with the offending span written
to `fault` (when it is not null), which indexes the caller's own bytes. -1 is a contract
violation: an undefined precision, epoch or date-order code, or a numeric format or day-period
layout the core refuses. A null format is the invariant one (`.` and `,`, every lenience); a
declared one is a `HyperCastRawNumFormat` the caller fills in, `size` first, and the core reads
in place. The header documents every function, type and layout. `hypercast_version()` is the
one way to read the version at run time. A minor release may add functions; only a major
release changes or removes one.

## C++

`hypercast.hpp` is header-only and needs C++17. Every function is `noexcept`, nothing throws or
allocates, and it builds with exceptions off (`-fno-exceptions`, or `/EHs-c-` with
`_HAS_EXCEPTIONS=0`). CI builds it with GCC on Linux, Apple Clang on macOS, iOS and Mac
Catalyst, and MSVC on Windows, as C++17, C++20 and C++23, with exceptions on and off. It links
against the same targets.

```cmake
target_link_libraries(app PRIVATE hypercast::hypercast_static)
target_compile_features(app PRIVATE cxx_std_17)
```

```cpp
#include <cstdio>
#include "hypercast.hpp"

int main() {
  auto verdict = hypercast::cast_i32("(1,234)", hypercast::NumFormat::invariant());
  if (const auto *value = verdict.value_if()) {
    std::printf("got %d\n", *value); // -1234, accounting negative
  } else {
    const hypercast::Fault &fault = verdict.fault();
    std::printf("fault at byte %zu, %zu long\n", fault.offset, fault.length);
  }

  hypercast::NumFormat::Fields euro;
  euro.decimal_sep = ",";
  euro.group_sep = ".";
  euro.currency = "€";
  auto format = hypercast::NumFormat::declare(euro); // std::nullopt if the core refuses it
  auto price = hypercast::cast_decimal("1.234,50 €", *format);
  return price.match([](const hypercast::Decimal &) { return 0; },
                     [](const hypercast::Fault &) { return 1; });
}
```

Every door returns a `hypercast::Verdict<T>`: the value, or a `hypercast::Fault` (`reason`,
`offset`, `length`, the span in bytes of the input's UTF-8). There is no third state and no
default constructor. Take it apart with `ok()`, `value_if()` and `fault_if()` (a pointer to the
case, or null), `value()` and `fault()` (unchecked: check `ok()` first; the wrong one is a
value-initialized value, never undefined behaviour), `value_or`, `match(on_value, on_fault)`,
`visit` with an overload set, or `to_variant()` for `std::visit`. Under C++23 with `<expected>`
a verdict converts to `std::expected<T, hypercast::Fault>`, and `to_expected()` says so
explicitly. `hypercast::optional(verdict)` reads an empty input as `std::nullopt`.

The doors are named as in the C ABI: `cast_bool`, `cast_char` (`char32_t`), `cast_i8` through
`cast_u64`, `cast_f32`, `cast_f64` and `cast_decimal` under a `NumFormat`, `cast_uuid`,
`cast_timestamp`, `cast_unix` (a `UnixPrecision`), `cast_excel_serial` (an `ExcelEpoch`),
`cast_date` (strict, or with a `DateOrder`), `cast_datetime` (a `DateOrder` or a `DateFormat`
with a culture's day periods), `cast_time` (`std::chrono::nanoseconds`), `cast_duration`, and the
typed doors `cast_decimal_from_double`, `cast_excel_serial_from_double`, `cast_excel_time` and
`cast_excel_duration`. Text is anything that converts to `std::string_view`, a
`std::u8string_view` under C++20, or a pointer and a size. The values are the core's own
types: `Decimal` (with `to_chars`), `Uuid`, `Timestamp`, `Date`, `CivilDateTime` and
`Duration`, each its fields with nothing hidden.

`NumFormat::declare` and `DateFormat::declare` ask the core about a declared format once and
return `std::nullopt` for one it refuses, so a door never meets one. The one contract violation
left is an enum value outside its enumerators (`static_cast<hypercast::DateOrder>(9)`), a
caller bug that ends the program with `std::abort()`. `native_version()` is the linked library's
version, and `is_available()` is always true: the core is linked in.
