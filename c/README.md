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
layout the core refuses. A null format is the invariant one (`.` and `,`, every lenience, legacy
grouping); a declared one is a `HyperCastRawNumFormat` the caller fills in, `size` first, and the core reads
in place. The header documents every function, type and layout. `hypercast_version()` is the
one way to read the version at run time. A minor release may add functions; only a major
release changes or removes one. The library exports 36 functions.

White space is Unicode's `White_Space`, the 25 code points (ASCII, U+0085, U+00A0, U+2000 to
U+200A, U+202F, U+3000 among them), and every door ignores it at both ends (`cast_char` given
exactly one character keeps it); the numeric doors also ignore it around a sign, a percent or
currency symbol and inside accounting parentheses, but not between digits, where it is
malformed unless the format declares a space-like group separator (an ASCII space, U+00A0 or
U+202F, read as each other). Nothing but white space is empty. A layout the core refuses (-1)
includes a sign or percent symbol, or a decimal separator (number or currency), with white
space in it, a currency symbol with white space at either edge, and a day-period designator
with white space at either edge.

Five of them carry validation constraints, the OpenAPI Schema Object's keywords judged after a
door parses. `cast_constraints_check` judges a declaration once, a `HyperCastRawConstraints`
(72 bytes, `size` first) and an `enum` list beside it, read only when `flags` has
`HyperCastRawConstraints_HAS_ENUM` (256) beside the `HAS_*` bits of the other keywords and
`TRIM`, and returns 0 or the refusal code of the rule it breaks: 1 a keyword the door does not
take, 2 minimum with exclusiveMinimum, 3 maximum with exclusiveMaximum, 4 a lower bound not below
the upper, 5 a multipleOf not positive, 6 a rounding without multipleOf, 7 minLength above
maxLength, 8 a bound, step or entry the door cannot hold, 9 an enum entry that is not UTF-8, 10 a
duplicate entry, 11 a malformed layout (among its causes a non-empty list without `HAS_ENUM`, or
a decimal sign byte other than 0 or 1), 12 an enum with no entries (`HAS_ENUM` with an empty
list), 13 a text enum entry no input can match (an empty one, or under `TRIM` one with leading or
trailing white space).
`cast_checked` (text) and `cast_checked_from_f64` (a double a source already decoded) cast
through a numeric door or the text door and judge the value: they add return code 4, a
violation, with the value's span in `fault` and the keyword broken in a `HyperCastRawCheck`
(1 minimum, 2 exclusiveMinimum, 3 maximum, 4 exclusiveMaximum, 5 multipleOf, 6 minLength,
7 maxLength, 8 enum; 9 and 10 are reserved for `format` and `pattern`, so a switch over them
needs a default). The rounding codes are 0 strict, 1 midpoint (half to even), 2 floor and
3 ceiling. `cast_text` is the text door, the span of valid UTF-8 input, and
`cast_declared_decimal` converts a bound, step or integer entry written in invariant notation
into a `HyperCastDecimal`, exactly or not at all (8).

Four more exports, 1.1's, reach the doors above by their door code (`cast_detailed`, with `param`
for the Unix precision, date order or Excel epoch a door declares, and `0` elsewhere) and report
what failed. A failure writes a `HyperCastRawFaultDetail` (16 bytes, `size` first and set by the
caller; a larger one from a later layout is accepted and its tail never written; null is allowed)
whose `detail` is the code of the vocabulary: 1 `blank`, 2 `unexpected_character`, 3
`digit_expected` and so on to 32 `not_a_scalar`, `0` only on a checked violation. The codes keep
their numbers; a later minor release may add more, which a reader treats as a failure with a usable
reason and span. `cast_detailed_from_f64` is the four typed doors (19 decimal, 22 Excel serial with
its epoch, 16 time, 17 duration), and `cast_checked_detailed` and `cast_checked_detailed_from_f64`
the checked pair, with the same return codes as `cast_checked`'s. Every argument is validated
before anything is written, and -1 writes nothing: an unknown door, a door outside the export's
set, a `param` the door does not take, a `fault` under 16 bytes or not a multiple of 4, or a layout
the core refuses.

They also take what the 1.0 exports cannot. A format's grouping policy sits in bits 8 and 9 of its
`flags`: `HyperCastNumFormat_GROUPING_LEGACY` (0, a group separator anywhere between two digits),
`_GROUPING_WESTERN` (groups of three counted from the right, `12,34` malformed) and
`_GROUPING_INDIAN` (the last group three digits, the others two). Policy 3 and any flag bit the core
does not define are -1 there; a null format is the invariant one with legacy grouping, and every
other export ignores bits 8 and up and always reads legacy. `cast_detailed` also takes a
`HyperCastRawDateOptions` (`size` 8 or 12, then `flags`: `1` compact `yyyyMMdd` at door 15, `2` a
space for the `T` and `4` `+hh` offsets at door 13; and `pivot`, 0 or 100 to 9999, the last year of
the window a two-digit year is read in at doors 20 and 21) and a month vocabulary at 20 and 21,
packed as each entry's month byte, a little-endian `uint32_t` length, and the UTF-8, which the core
judges in full on every call.

**A 1.0 core has none of this**: no detailed export, and a policy it reads as legacy. Check
`hypercast_version()` before declaring a strict policy or calling the exports, and refuse a core
that cannot drive the headers you built against: another major version, or a minor older than
theirs (1.1.0 is `0x010100`; the patch never matters). `c/smoke/main.c` is the example:

```c
#define BUILT_AGAINST 0x010100u /* the headers' version, major << 16 | minor << 8 | patch */

uint32_t version = hypercast_version();
if ((version >> 16) != (BUILT_AGAINST >> 16) ||
    ((version >> 8) & 0xff) < ((BUILT_AGAINST >> 8) & 0xff)) {
  fprintf(stderr, "hypercast 1.1 is needed, found %u.%u.%u\n", version >> 16,
          (version >> 8) & 0xff, version & 0xff);
  return 1;
}
```

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
  // At startup: false means the linked library cannot drive this header (see below).
  if (!hypercast::core_is_compatible()) {
    return 3;
  }
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
with a culture's day periods), `cast_time` (`std::chrono::nanoseconds`), `cast_duration`,
`cast_text` (a `std::string_view` of valid UTF-8 input), and the typed doors
`cast_decimal_from_double`, `cast_excel_serial_from_double`, `cast_excel_time` and
`cast_excel_duration`. Text is anything that converts to `std::string_view`, a
`std::u8string_view` under C++20, or a pointer and a size. The values are the core's own types:
`Decimal` (with `to_chars`), `Uuid`, `Timestamp`, `Date`, `CivilDateTime` and `Duration`, each
its fields with nothing hidden.

`NumFormat::declare` and `DateFormat::declare` ask the core about a declared format once and
return `std::nullopt` for one it refuses, so a door never meets one. The contract violations
left are an enum value outside its enumerators (`static_cast<hypercast::DateOrder>(9)`) and a
refused declaration's unbuilt `Constraints` handed to a door (below), caller bugs that end the
program with `std::abort()`. `native_version()` is the linked library's version, and
`is_available()` is always true: the core is linked in.

### Grouping, detail and date shapes

A `NumFormat` declares how its group separators are judged, beside its styles: `Grouping::western`
by default (`NumFormat::invariant()` is strict Western, and so is `detect()` and a `Fields` that
says nothing), `Grouping::indian`, or `Grouping::legacy`, a separator anywhere between two digits.
`12,34` is `Malformed` under the first two, spanning `,34`, and 1234 under legacy
(`invariant().with_grouping(Grouping::legacy)`); an ungrouped numeral stays valid, and with
`NumStyles::grouping` off a group separator stays forbidden under every policy. `with_grouping`
replaces the policy and keeps the styles; `styles()` never carries one, and `declare` refuses styles
that do, a policy that is not one of the three, and a style bit that is not `NumStyles`'s. The
plain doors honour the policy.

```cpp
const auto invariant = hypercast::NumFormat::invariant();
auto bad = hypercast::detailed::cast_i32("12,34", invariant);
const hypercast::Failure &failure = bad.failure();
// failure.reason == CastFailure::malformed, failure.offset == 2, failure.length == 3,
// failure.detail == Detail::group_size, hypercast::name(failure.detail) == "group_size"
```

`hypercast::detailed` has every door above under the same name and arguments, returning a
`DetailedVerdict<T>` (or, with constraints, a `DetailedChecked<T>`): the value, or a `Failure`, a
`Fault`'s `reason`, `offset` and `length` with a `Detail` beside them. `Fault` itself is as it was,
its equality blind to any detail, and `failure.fault()` is the plain door's answer; a
`DetailedVerdict` has the accessors of a `Verdict` (`failure_if()`, `failure()`, `to_verdict()`, ...),
a `DetailedChecked` those of a `Checked` over a `DetailedRejection` (`is_failure()`,
`failure_if()`, `violation_if()`, `to_checked()`, ...; a `Violation` has no detail).
`Detail` is the vocabulary (`blank`, `unexpected_character`, `digit_expected`, ... `not_a_scalar`,
numbered 1 to 32 as in the C section) and is open: any `std::uint32_t` is a value, `is_known` says
whether this header names it, `name(detail)` spells it (`"group_size"`, or `"detail 999"` for a
code a later core adds), and `failure.detail_code()` is the number. `none` is 0, a checked violation's.

The date doors of `hypercast::detailed` take, after the text, the shapes the plain ones never read.
A `DateOptions` holds `DateStyles` (`compact_date`, `space_separator`, `hour_offset`, or
`common_forms` for the three) and a pivot; `DateOptions::declare(styles, pivot)` returns
`std::nullopt` for a style bit it does not define or a pivot that is neither 0 nor 100 to 9999, and
a default `DateOptions` is none. A `MonthNames` is a vocabulary, Excel style (`07-Jan-26`), packed
into storage the caller owns as an `EnumStorage` is (`MonthNames::storage_size(entries)` bytes),
declared from `MonthName{month, name}` entries or an already packed buffer, and judged once by the
core: every month needs a name, none empty, past 64 bytes, or holding a digit, `/`, `-` or white
space, and two that fold alike are refused. Punctuation is part of a name: with `janv.` declared
and `.` the separator, `7.janv..2026` reads and `7.janv.2026` is `Malformed`.

```cpp
std::uint8_t storage[512];
const hypercast::MonthName months[] = {{1, "Jan"}, {2, "Feb"}, {3, "Mar"},  {4, "Apr"},
                                       {5, "May"}, {6, "Jun"}, {7, "Jul"},  {8, "Aug"},
                                       {9, "Sep"}, {10, "Oct"}, {11, "Nov"}, {12, "Dec"}};
const auto names = hypercast::MonthNames::declare(months, storage);
const auto pivot = hypercast::DateOptions::declare(hypercast::DateStyles::none, 2029);
auto date = hypercast::detailed::cast_date("07-Jan-26", hypercast::DateOrder::day_month_year,
                                           *pivot, *names);
// date.value() == hypercast::Date{2026, 1, 7}
```

**The core must be able to drive the header.** The header's own version is `HYPERCAST_VERSION_MAJOR`,
`_MINOR` and `_PATCH` (and `HYPERCAST_VERSION`, packed as `hypercast_version()` packs the
library's): the crate's version, which a release moves with everything else. A library drives it
when its major version is the header's and its minor at least the header's, whatever the patch (a
patch release adds no export); `core_is_compatible()` asks, once. **Call it at startup** and read
`false` as a deployment error: a library older than the header (1.0 under the 1.1 header) has no
detailed export and ignores a format's grouping policy, so nothing strict may reach it. A strict
`declare`, `DateOptions::declare` and `MonthNames::declare` return `std::nullopt` against one, and
a door handed a strict format, or a detailed door, writes one line naming both versions to stderr
and ends the program with `std::abort()` instead of parsing it as legacy. A legacy format and the
plain doors keep working against any core, as far as the library's own exports go. A program
built from this header also names the detailed exports, so how a library without them is met
depends on the platform: with lazy binding (Linux) the program loads and the guard is what
fires, while on macOS every symbol is bound at load and the dynamic loader refuses the program
first.

### Validation constraints

```cpp
#include <cstdio>
#include "hypercast.hpp"

namespace hc = hypercast;

int main() {
  const auto invariant = hc::NumFormat::invariant();

  // Latitude: -90 to 90, rounded half to even to six places.
  hc::Constraints<double>::Fields degrees;
  degrees.minimum = -90;
  degrees.maximum = 90;
  degrees.multiple_of = "0.000001"; // text or a hypercast::Decimal, never a double
  degrees.rounding = hc::Rounding::midpoint;
  const auto latitude = hc::Constraints<double>::declare(degrees);
  if (const hc::Refusal *refusal = latitude.refusal_if()) {
    const hc::RefusalMessage message = refusal->message(); // a caller bug: the rule it broke
    std::printf("%s\n", message.c_str());
    return 2;
  }
  auto north = hc::cast_f64("45.1234567", invariant, latitude.value());
  std::printf("%.6f adjusted=%d\n", north.value().value, north.value().adjusted); // 45.123457 1
  auto south = hc::cast_f64("-90.5", invariant, latitude.value());
  const hc::Violation &broken = south.rejection().violation();
  std::printf("%s at %zu+%zu\n", hc::name(broken.keyword).c_str(), broken.offset,
              broken.length); // minimum at 0+5

  // Euro cash: whole multiples of five cents, strict (the default), no rounding.
  hc::Constraints<hc::Decimal>::Fields cents;
  cents.multiple_of = "0.05";
  const auto cash = hc::Constraints<hc::Decimal>::declare(cents).value();
  std::printf("%d\n", hc::cast_decimal("12.35", invariant, cash).ok()); // 1
  std::printf("%d\n", hc::cast_decimal("12.37", invariant, cash).ok()); // 0: multipleOf

  // A sex code: trimmed, matched under case folding, answering with the declared entry.
  std::uint8_t storage[32]; // the packed list lives here, and outlives sex and its answers
  hc::Constraints<std::string_view>::Fields code;
  code.trim = true;
  const auto sex =
      hc::Constraints<std::string_view>::declare(code, {"Female", "Male", "Other"}, storage)
          .value();
  auto answer = hc::cast_text(" MALE ", sex);
  answer.match(
      [](const hc::CheckedValue<std::string_view> &checked) {
        std::printf("%.*s, entry %u\n", static_cast<int>(checked.value.size()),
                    checked.value.data(), *checked.enum_index); // Male, entry 1
      },
      [](const hc::Rejection &rejection) {
        if (const hc::Violation *violation = rejection.violation_if()) {
          switch (violation->keyword) {
          case hc::Keyword::enum_:
            std::printf("not a sex code\n");
            break;
          default: // keywords are open: format and pattern may join
            std::printf("breaks %s\n", hc::name(violation->keyword).c_str());
          }
        }
      });
  return 0;
}
```

A `Constraints<T>` is bound to the door its type names (`std::int8_t` through `std::uint64_t`,
`float`, `double`, `hypercast::Decimal`, and `std::string_view` for the text door) and judged
once, by the core, when it is declared. `declare` is the only way to build one: it returns a
`Declared<Constraints<T>>`, the declaration or a `Refusal` with the core's code (1 to 13, as in
the C section) and one fixed message per code (`Constraints refused (code 2): minimum together
with exclusiveMinimum.`; a code this header does not name reads `Constraints refused (code 14):
a rule this binding does not name.`). The message is a `RefusalMessage` held by value, as a
`KeywordName` is, so it outlives the `Declared` it came from: compare it with a string, print
`c_str()`, or take `view()` from a named one (on a temporary, `view()` does not compile, nor
does a `NumFormat`'s or `DateFormat`'s symbol accessor, each a view into its object). A refused declaration is a caller bug, not data, and its
`value()` is an unbuilt declaration that no door reads as "no rules": a door handed it ends the
program, as any contract violation does. Under C++20 a designated initializer names just the
keywords wanted (`declare({.minimum = -90, .maximum = 90})`).

Every keyword is in every door's `Fields`, so the core alone refuses one a door does not take:
the integer doors take `minimum`, `exclusive_minimum`, `maximum`, `exclusive_maximum`,
`multiple_of` with a `Rounding` (`strict`, the default, `midpoint`, half to even, `floor`,
`ceiling`) and an `enum`; `float`, `double` and `Decimal` the same without `enum`; the text door
`min_length`, `max_length` (in code points), `trim` (Unicode `White_Space`) and an `enum`,
matched under Unicode simple case folding. Bounds, steps and integer entries are
`ConstraintValue`s: an integer, a `Decimal`, or text in invariant notation, each converted
exactly by the core (`"1e-29"`, past the 28 places a decimal holds, is refused rather than
rounded). There is no constructor from a `double`, so `0.1` cannot be declared inexactly. A
float door also judges the float it delivers against each bound read as the door reads it, so
`1e-400`, which `double` reads as 0, breaks `exclusive_minimum = 0`:

```cpp
hc::Constraints<double>::Fields positive;
positive.exclusive_minimum = 0;
const auto above_zero = hc::Constraints<double>::declare(positive);
const auto tiny = hc::cast_f64("1e-400", hc::NumFormat::invariant(), above_zero.value());
// tiny.rejection().violation() == hc::Violation{hc::Keyword::exclusive_minimum, 0, 6}
```

Nothing allocates, so an `enum` list is packed into storage the caller owns:
`Constraints<T>::storage_size(entries)` bytes (16 a numeric entry, 4 plus its UTF-8 a text
entry), an array or a pointer and a size. Too little, or a text entry past 4,294,967,295 bytes,
is `Refusal` code 0, this header's own, with its own message rather than a core refusal's.
`declare(fields)` declares no `enum`; every overload that takes entries declares one and sets
`HAS_ENUM`, so an empty list (`declare(fields, {}, storage)`) is one no value could match, which
the core refuses (12). The storage must outlive the declaration and every value read through it,
and must not change after it; a copy of a `Constraints` refers to the same storage. The entries
themselves are only read while `declare` runs.

A checked door returns a `Checked<T>`: a `CheckedValue<T>` (`value`, `adjusted`, true only when
rounding changed the value, and `enum_index`, set only on a text `enum` match) or a `Rejection`,
which is the plain door's own `Fault` or a `Violation` (its `keyword` and the value's span, in
bytes like a fault's). It is taken apart as a `Verdict` is (`ok()`, `value_if()`,
`rejection_if()`, `match`, `visit`, `to_variant()`, `to_expected()` under C++23), and so is a
`Rejection` (`is_fault()`, `fault_if()`, `violation_if()`, `match`, `visit`, `to_variant()`).
`hypercast::optional` reads an empty input as `std::nullopt` here too. `Keyword` is an enum
class over the core's codes (`minimum` through `enum_`, as `enum` is reserved), and open:
`format` and `pattern` may join in a minor release, so a switch over keywords needs its
default arm, and `name(keyword)` spells a code this header does not know as `keyword 9`.

The checked doors are overloads of the plain ones with a `Constraints` last (`cast_i32(text,
format, constraints)`, through `cast_decimal`), `cast_i8_from_double` through
`cast_f64_from_double` and `cast_decimal_from_double(value, constraints)` for a number a source
already decoded, judged on the shortest decimal that names it with an empty span, and
`cast_text(text, constraints)`, whose value is a view of the input (trimmed, when declared) or,
on an `enum` match, of the declared entry in the caller's storage. `cast_text(text)` alone is
the plain text door, a `Verdict<std::string_view>` of the untrimmed input. The plain names are
overload sets, so taking a door's address needs a cast to its signature or a lambda.
`raw()`, `enum_data()` and `enum_size()` hand a declaration to a C ABI of its own.
