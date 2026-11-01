// The C++ face of the hypercast core (https://github.com/SkunkWerkx/HyperCast): header-only,
// over the C ABI in hypercast.h, and linked against the same shared or static library. C++17
// or later; under C++23 with <expected>, a verdict also converts to std::expected.
//
// Every door returns a hypercast::Verdict<T>: the value, or a Fault (a closed reason, Empty,
// Malformed or OutOfRange, and the offending byte span of the caller's input). Nothing here
// throws, allocates or reads a culture: every function is noexcept, the header builds with
// exceptions off (-fno-exceptions, or /EHs-c- with _HAS_EXCEPTIONS=0), and the date order,
// the Unix precision, the Excel epoch and the numeric format are declared by the caller.
//
//   auto verdict = hypercast::cast_i32("(1,234)", hypercast::NumFormat::invariant());
//   if (const auto *value = verdict.value_if()) {
//     use(*value);                                    // -1234, accounting negative
//   } else {
//     const auto &fault = verdict.fault();            // reason, offset, length
//   }
//
// A format a caller declares is checked once, by the core, when it is declared
// (NumFormat::declare and DateFormat::declare return std::nullopt for one the core refuses),
// so a door never meets a layout it refuses. The one contract violation left is an enum
// value outside its enumerators (static_cast<DateOrder>(9)): a caller bug, not data, which
// ends the program with std::abort(), as Swift's precondition and C#'s exception do.

#ifndef HYPERCAST_HPP
#define HYPERCAST_HPP

#if !defined(__cplusplus) ||                                                                       \
    (__cplusplus < 201703L && (!defined(_MSVC_LANG) || _MSVC_LANG < 201703L))
#error "hypercast.hpp needs C++17 or later; C programs include hypercast.h"
#endif

#include <array>
#include <charconv>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <optional>
#include <string_view>
#include <system_error>
#include <type_traits>
#include <utility>
#include <variant>

#if __has_include(<version>)
#include <version>
#endif
#if defined(__cpp_lib_expected) && __cpp_lib_expected >= 202202L
#include <expected>
#define HYPERCAST_HAS_EXPECTED 1
#endif

#include "hypercast.h"

namespace hypercast {

// The closed set of reasons a cast can fail: the core's verdict codes, verbatim. Adding a
// member is a breaking change in every binding at once.
enum class CastFailure : std::int32_t {
  empty = 1,       // required input was empty or whitespace; optional() reads it as absent
  malformed = 2,   // present, but not recognizable as the target type
  out_of_range = 3 // well formed, but outside the target's range: "256" for a u8
};

// The failure case of a Verdict: why, and the offending span of the caller's own input, in
// bytes of its UTF-8, so text.substr(fault.offset, fault.length) is the offending text.
// offset + length never exceeds the input's size. A typed door (a double in, no text) has an
// empty span at 0. Nothing is captured: rendering a diagnostic is the caller's choice.
struct Fault {
  CastFailure reason;
  std::size_t offset;
  std::size_t length;

  friend constexpr bool operator==(const Fault &a, const Fault &b) noexcept {
    return a.reason == b.reason && a.offset == b.offset && a.length == b.length;
  }
  friend constexpr bool operator!=(const Fault &a, const Fault &b) noexcept { return !(a == b); }
};

// The outcome of a cast: exactly one of a value or a Fault, and nothing else. There is no
// default constructor, so a Verdict always holds one of the two.
//
//   ok()                      whether it is the value
//   value_if(), fault_if()    a pointer to the case, or nullptr: `if (auto *v = r.value_if())`
//   value(), fault()          the case, unchecked: check ok() first. Neither traps; the wrong
//                             one is a value-initialized T or Fault, never undefined behaviour
//   value_or(fallback)        the value, or the fallback
//   match(on_value, on_fault) calls the one for its case and returns what it returns
//   visit(visitor)            calls visitor(value) or visitor(fault): an overload set works
//   to_variant()              std::variant<T, Fault>, for std::visit
//   to_expected()             std::expected<T, Fault>, which a Verdict also converts to
//                             implicitly (C++23 with <expected> only)
//
// Deliberately no operator bool: std::expected<bool, Fault> would take it as the value.
template <class T> class [[nodiscard]] Verdict {
  static_assert(std::is_trivially_copyable_v<T> && std::is_default_constructible_v<T>,
                "a verdict holds a plain value");

public:
  using value_type = T;

  // The success case. Implicit, so a door, or a test, can return the value itself.
  constexpr Verdict(T success) noexcept : value_(success), fault_{}, ok_(true) {}
  // The failure case.
  constexpr Verdict(Fault failure) noexcept : value_{}, fault_(failure), ok_(false) {}

  constexpr bool ok() const noexcept { return ok_; }

  constexpr const T *value_if() const noexcept { return ok_ ? &value_ : nullptr; }
  constexpr const Fault *fault_if() const noexcept { return ok_ ? nullptr : &fault_; }

  // Precondition: ok(). Otherwise a value-initialized T.
  constexpr const T &value() const noexcept { return value_; }
  // Precondition: !ok(). Otherwise a value-initialized Fault, whose reason is no enumerator.
  constexpr const Fault &fault() const noexcept { return fault_; }

  constexpr T value_or(T fallback) const noexcept { return ok_ ? value_ : fallback; }

  // on_value(const T&) or on_fault(const Fault&), whichever is this verdict's case; both must
  // return the same type. Both are required, so a missing case does not compile.
  template <class OnValue, class OnFault>
  constexpr std::invoke_result_t<OnValue &&, const T &> match(OnValue &&on_value,
                                                              OnFault &&on_fault) const
      noexcept(std::is_nothrow_invocable_v<OnValue &&, const T &> &&
               std::is_nothrow_invocable_v<OnFault &&, const Fault &>) {
    static_assert(std::is_same_v<std::invoke_result_t<OnValue &&, const T &>,
                                 std::invoke_result_t<OnFault &&, const Fault &>>,
                  "match's two cases must return the same type");
    if (ok_) {
      return std::forward<OnValue>(on_value)(value_);
    }
    return std::forward<OnFault>(on_fault)(fault_);
  }

  // visitor(const T&) or visitor(const Fault&): match with one callable, the way std::visit
  // takes an overload set.
  template <class Visitor>
  constexpr std::invoke_result_t<Visitor &&, const T &> visit(Visitor &&visitor) const
      noexcept(std::is_nothrow_invocable_v<Visitor &&, const T &> &&
               std::is_nothrow_invocable_v<Visitor &&, const Fault &>) {
    static_assert(std::is_same_v<std::invoke_result_t<Visitor &&, const T &>,
                                 std::invoke_result_t<Visitor &&, const Fault &>>,
                  "visit's two cases must return the same type");
    if (ok_) {
      return std::forward<Visitor>(visitor)(value_);
    }
    return std::forward<Visitor>(visitor)(fault_);
  }

  // The verdict as a std::variant: index 0 the value, index 1 the fault.
  constexpr std::variant<T, Fault> to_variant() const noexcept {
    if (ok_) {
      return std::variant<T, Fault>(std::in_place_index<0>, value_);
    }
    return std::variant<T, Fault>(std::in_place_index<1>, fault_);
  }

#ifdef HYPERCAST_HAS_EXPECTED
  constexpr std::expected<T, Fault> to_expected() const noexcept {
    if (ok_) {
      return std::expected<T, Fault>(std::in_place, value_);
    }
    return std::expected<T, Fault>(std::unexpect, fault_);
  }
  constexpr operator std::expected<T, Fault>() const noexcept { return to_expected(); }
#endif

  friend constexpr bool operator==(const Verdict &a, const Verdict &b) noexcept {
    return a.ok_ == b.ok_ && (a.ok_ ? a.value_ == b.value_ : a.fault_ == b.fault_);
  }
  friend constexpr bool operator!=(const Verdict &a, const Verdict &b) noexcept {
    return !(a == b);
  }

private:
  T value_;
  Fault fault_;
  bool ok_;
};

// A verdict presented optionally: an Empty fault becomes std::nullopt (absent input, for a
// column where blank means null), and every other verdict comes through as it is.
template <class T>
constexpr std::optional<Verdict<T>> optional(const Verdict<T> &verdict) noexcept {
  if (const Fault *fault = verdict.fault_if();
      fault != nullptr && fault->reason == CastFailure::empty) {
    return std::nullopt;
  }
  return verdict;
}

// The input every text door takes: UTF-8 bytes, borrowed for the call. Anything that
// converts to std::string_view (a literal, std::string, a string_view), a std::u8string_view
// under C++20, or a pointer and a size.
class Text {
public:
  template <class S, std::enable_if_t<std::is_convertible_v<const S &, std::string_view>, int> = 0>
  Text(const S &text) noexcept : Text(std::string_view(text)) {}
  Text(std::string_view text) noexcept
      : data_(reinterpret_cast<const std::uint8_t *>(text.data())), size_(text.size()) {}
#ifdef __cpp_char8_t
  template <class S, std::enable_if_t<std::is_convertible_v<const S &, std::u8string_view> &&
                                          !std::is_convertible_v<const S &, std::string_view>,
                                      int> = 0>
  Text(const S &text) noexcept : Text(std::u8string_view(text)) {}
  Text(std::u8string_view text) noexcept
      : data_(reinterpret_cast<const std::uint8_t *>(text.data())), size_(text.size()) {}
#endif
  constexpr Text(const std::uint8_t *data, std::size_t size) noexcept : data_(data), size_(size) {}

  constexpr const std::uint8_t *data() const noexcept { return data_; }
  constexpr std::size_t size() const noexcept { return size_; }

private:
  const std::uint8_t *data_;
  std::size_t size_;
};

// ---------------------------------------------------------------------------------------------
// Values. Each is the core's type of the same name, field for field.

// An exact decimal: (-1)^negative × (hi·2^64 + lo) × 10^-scale. A cast's is canonical:
// trailing fraction zeros trimmed, the scale minimal (0 to 28), and zero never negative.
struct Decimal {
  std::uint64_t lo;   // the magnitude's low 64 bits
  std::uint32_t hi;   // its high 32 bits: the magnitude is at most 2^96 - 1
  std::uint8_t scale; // how many places the magnitude is shifted right
  bool negative;

  // The longest text to_chars writes: a sign, "0.", and a scale of 255 places.
  static constexpr std::size_t max_chars = 258;

  // Writes the value as the core displays it (-1234.5, 0.0000000000000000000000000001, no
  // trailing fraction zeros a canonical value lacks) into [first, last), std::to_chars
  // style: ptr is one past the last character written; on a buffer too small, ptr is last
  // and ec std::errc::value_too_large. Nothing is allocated.
  constexpr std::to_chars_result to_chars(char *first, char *last) const noexcept {
    char digits[29] = {};
    std::size_t count = 0;
    std::uint32_t limbs[3] = {hi, static_cast<std::uint32_t>(lo >> 32),
                              static_cast<std::uint32_t>(lo)};
    do {
      std::uint64_t remainder = 0;
      for (std::uint32_t &limb : limbs) {
        const std::uint64_t current = remainder << 32 | limb;
        limb = static_cast<std::uint32_t>(current / 10);
        remainder = current % 10;
      }
      digits[count++] = static_cast<char>('0' + remainder);
    } while (limbs[0] != 0 || limbs[1] != 0 || limbs[2] != 0);
    const std::size_t places = scale;
    const std::size_t length =
        (negative ? 1 : 0) + (places >= count ? 2 + places : count + (places > 0 ? 1 : 0));
    if (static_cast<std::size_t>(last - first) < length) {
      return {last, std::errc::value_too_large};
    }
    char *out = first;
    if (negative) {
      *out++ = '-';
    }
    if (places >= count) {
      *out++ = '0';
      *out++ = '.';
      for (std::size_t i = count; i < places; ++i) {
        *out++ = '0';
      }
      while (count > 0) {
        *out++ = digits[--count];
      }
    } else {
      while (count > places) {
        *out++ = digits[--count];
      }
      if (places > 0) {
        *out++ = '.';
        while (count > 0) {
          *out++ = digits[--count];
        }
      }
    }
    return {out, std::errc{}};
  }

  friend constexpr bool operator==(const Decimal &a, const Decimal &b) noexcept {
    return a.lo == b.lo && a.hi == b.hi && a.scale == b.scale && a.negative == b.negative;
  }
  friend constexpr bool operator!=(const Decimal &a, const Decimal &b) noexcept {
    return !(a == b);
  }
};

// Sixteen bytes in RFC 9562 order.
struct Uuid {
  std::array<std::uint8_t, 16> bytes;

  friend constexpr bool operator==(const Uuid &a, const Uuid &b) noexcept {
    for (std::size_t i = 0; i < 16; ++i) {
      if (a.bytes[i] != b.bytes[i]) {
        return false;
      }
    }
    return true;
  }
  friend constexpr bool operator!=(const Uuid &a, const Uuid &b) noexcept { return !(a == b); }
};

// An instant, protobuf's google.protobuf.Timestamp: seconds since 1970-01-01T00:00:00Z and
// the nanoseconds after them, 0 to 999,999,999 even before the epoch. 0001-01-01 through
// 9999-12-31T23:59:59.999999999Z, wider than a std::chrono::nanoseconds time point reaches.
struct Timestamp {
  std::int64_t seconds;
  std::int32_t nanos;

  friend constexpr bool operator==(const Timestamp &a, const Timestamp &b) noexcept {
    return a.seconds == b.seconds && a.nanos == b.nanos;
  }
  friend constexpr bool operator!=(const Timestamp &a, const Timestamp &b) noexcept {
    return !(a == b);
  }
};

// A calendar date with no time or zone, proleptic Gregorian, years 1 to 9999.
struct Date {
  std::uint16_t year;
  std::uint8_t month;
  std::uint8_t day;

  friend constexpr bool operator==(const Date &a, const Date &b) noexcept {
    return a.year == b.year && a.month == b.month && a.day == b.day;
  }
  friend constexpr bool operator!=(const Date &a, const Date &b) noexcept { return !(a == b); }
};

// A wall-clock date and time with no zone: what `1/7/2026 3:04 PM` names. Not an instant;
// fusing a zone is the caller's job.
struct CivilDateTime {
  Date date;
  std::uint64_t nanos_of_day; // since the date's midnight, under 86,400,000,000,000

  friend constexpr bool operator==(const CivilDateTime &a, const CivilDateTime &b) noexcept {
    return a.date == b.date && a.nanos_of_day == b.nanos_of_day;
  }
  friend constexpr bool operator!=(const CivilDateTime &a, const CivilDateTime &b) noexcept {
    return !(a == b);
  }
};

// A signed span, protobuf's google.protobuf.Duration: up to ±10,000 years (wider than
// std::chrono::nanoseconds holds), nanos carrying the sign of seconds.
struct Duration {
  std::int64_t seconds;
  std::int32_t nanos;

  friend constexpr bool operator==(const Duration &a, const Duration &b) noexcept {
    return a.seconds == b.seconds && a.nanos == b.nanos;
  }
  friend constexpr bool operator!=(const Duration &a, const Duration &b) noexcept {
    return !(a == b);
  }
};

// ---------------------------------------------------------------------------------------------
// What a caller declares. The values match the core's discriminants.

// The field order of a separated date: `1/7/2026` is January 7th or July 1st only because
// the caller said which.
enum class DateOrder : std::uint32_t {
  year_month_day = 1, // ISO's order, with any accepted separator
  month_day_year = 2, // the en-US short date
  day_month_year = 3  // the en-GB, most-of-the-world order
};

// The unit of a Unix-epoch value: no magnitude guessing.
enum class UnixPrecision : std::uint32_t {
  seconds = 1,
  milliseconds = 2,
  microseconds = 3,
  nanoseconds = 4
};

// The date system of an Excel serial, a workbook setting the cell does not carry.
enum class ExcelEpoch : std::uint32_t {
  y1900 = 1, // the Windows default: serial 1 is 1900-01-01, and 60 a February 29th that never was
  y1904 = 2  // the legacy Macintosh system: serial 0 is 1904-01-01
};

// The leniences a numeric door permits, and the separator policy. Combine with | and &.
enum class NumStyles : std::uint32_t {
  none = 0,
  grouping = HyperCastNumFormat_GROUPING,                 // the group separator between digits
  parentheses = HyperCastNumFormat_PARENS,                // (1,234) is -1234
  exponent = HyperCastNumFormat_EXPONENT,                 // 1e3, 2.5E-3
  radix_prefixes = HyperCastNumFormat_RADIX_PREFIX,       // 0x, &H and 0b, integer doors only
  percent = HyperCastNumFormat_PERCENT,                   // a trailing % divides by 100, real doors
  separator_detect = HyperCastNumFormat_SEPARATOR_DETECT, // the ./, roles read from structure
  currency = HyperCastNumFormat_CURRENCY,                 // the declared currency symbol at an edge
  trailing_sign = HyperCastNumFormat_TRAILING_SIGN, // 5- as a culture's `n-` pattern writes it
  all = HyperCastNumFormat_ALL                      // every lenience but trailing_sign
};

constexpr NumStyles operator|(NumStyles a, NumStyles b) noexcept {
  return static_cast<NumStyles>(static_cast<std::uint32_t>(a) | static_cast<std::uint32_t>(b));
}
constexpr NumStyles operator&(NumStyles a, NumStyles b) noexcept {
  return static_cast<NumStyles>(static_cast<std::uint32_t>(a) & static_cast<std::uint32_t>(b));
}
constexpr NumStyles operator^(NumStyles a, NumStyles b) noexcept {
  return static_cast<NumStyles>(static_cast<std::uint32_t>(a) ^ static_cast<std::uint32_t>(b));
}
constexpr NumStyles operator~(NumStyles a) noexcept {
  return static_cast<NumStyles>(~static_cast<std::uint32_t>(a) & static_cast<std::uint32_t>(0xFF));
}
constexpr NumStyles &operator|=(NumStyles &a, NumStyles b) noexcept { return a = a | b; }
constexpr NumStyles &operator&=(NumStyles &a, NumStyles b) noexcept { return a = a & b; }

namespace detail {
[[noreturn]] inline void contract_violation() noexcept {
  // The core refused an argument the types should have ruled out: an enum value outside its
  // enumerators. A caller bug, never data.
  std::abort();
}

template <class Symbol> constexpr bool store(Symbol &symbol, std::string_view text) noexcept {
  if (text.size() > sizeof symbol.bytes) {
    return false;
  }
  for (std::size_t i = 0; i < text.size(); ++i) {
    symbol.bytes[i] = static_cast<std::uint8_t>(text[i]);
  }
  symbol.len = static_cast<std::uint8_t>(text.size());
  return true;
}

template <class Symbol> inline std::string_view view(const Symbol &symbol) noexcept {
  return std::string_view(reinterpret_cast<const char *>(symbol.bytes), symbol.len);
}

template <class Raw> constexpr bool same_bytes(const Raw &a, const Raw &b) noexcept;
} // namespace detail

// Caller-declared numeric notation for the integer, real and decimal doors: the separators,
// the leniences, and whatever the text's culture spells differently from ASCII. The core
// carries no culture data; a caller maps its own to these fields. 92 bytes, the core's
// RawNumFormat, which it reads in place.
class NumFormat {
public:
  // What NumFormat::declare reads. Every field after styles declares nothing when empty,
  // which reads exactly as ASCII does; under C++20 a designated initializer names just the
  // fields a culture needs: {.decimal_sep = ",", .group_sep = "."}.
  struct Fields {
    std::string_view decimal_sep;          // 1 to 8 bytes of UTF-8, no ASCII digit
    std::string_view group_sep;            // likewise, and not the decimal separator
    NumStyles styles = NumStyles::all;     // the leniences; all, as the invariant format's
    std::string_view currency;             // up to 16 bytes, read only under NumStyles::currency
    std::string_view currency_decimal_sep; // read in place of decimal_sep beside the symbol
    std::string_view currency_group_sep;   // likewise for group_sep
    std::string_view minus_sign;           // a further minus beside ASCII - (U+2212)
    std::string_view plus_sign;            // a further plus beside ASCII +
    std::string_view percent;              // a further percent beside ASCII % (٪, ％)
    char32_t digit_zero = 0;               // the zero of native digits read beside 0-9 (U+0660)
  };

  // The format these fields declare, or std::nullopt for one the core refuses (a symbol that
  // is too long, holds an ASCII digit or is not UTF-8, equal separators, a digit zero that
  // starts no digit block, ...). The core itself is the judge, asked once here, so a door
  // never meets a format it refuses.
  static std::optional<NumFormat> declare(const Fields &fields) noexcept {
    NumFormat format;
    HyperCastNumFormat &f = format.raw_.format;
    f.flags = static_cast<std::uint32_t>(fields.styles);
    f.digits = static_cast<HyperCastNativeDigits>(fields.digit_zero);
    if (!detail::store(f.decimal_sep, fields.decimal_sep) ||
        !detail::store(f.group_sep, fields.group_sep) ||
        !detail::store(f.currency_decimal_sep, fields.currency_decimal_sep) ||
        !detail::store(f.currency_group_sep, fields.currency_group_sep) ||
        !detail::store(f.minus_sign, fields.minus_sign) ||
        !detail::store(f.plus_sign, fields.plus_sign) ||
        !detail::store(f.percent, fields.percent) || !detail::store(f.currency, fields.currency)) {
      return std::nullopt;
    }
    // Empty text with a format the core reads is Empty; one it refuses is -1.
    std::int32_t ignored = 0;
    if (::cast_i32(nullptr, 0, &format.raw_, &ignored, nullptr) < 0) {
      return std::nullopt;
    }
    return format;
  }

  // `.` decimal and `,` group separators and NumStyles::all: what the core reads by default.
  static constexpr NumFormat invariant() noexcept { return NumFormat(".", ",", NumStyles::all); }

  // The ./, roles resolved per input from structure (NumStyles::separator_detect, beside
  // all), where only unambiguous text resolves: `1,000` is Malformed, never guessed.
  static constexpr NumFormat detect() noexcept {
    return NumFormat(".", ",", NumStyles::all | NumStyles::separator_detect);
  }

  NumStyles styles() const noexcept { return static_cast<NumStyles>(raw_.format.flags); }
  std::string_view decimal_sep() const noexcept { return detail::view(raw_.format.decimal_sep); }
  std::string_view group_sep() const noexcept { return detail::view(raw_.format.group_sep); }
  std::string_view currency() const noexcept { return detail::view(raw_.format.currency); }
  std::string_view currency_decimal_sep() const noexcept {
    return detail::view(raw_.format.currency_decimal_sep);
  }
  std::string_view currency_group_sep() const noexcept {
    return detail::view(raw_.format.currency_group_sep);
  }
  std::string_view minus_sign() const noexcept { return detail::view(raw_.format.minus_sign); }
  std::string_view plus_sign() const noexcept { return detail::view(raw_.format.plus_sign); }
  std::string_view percent() const noexcept { return detail::view(raw_.format.percent); }
  char32_t digit_zero() const noexcept { return static_cast<char32_t>(raw_.format.digits); }

  // The layout the core reads, for a library that hands it to a C ABI of its own.
  constexpr const HyperCastRawNumFormat &raw() const noexcept { return raw_; }

  // Two formats are equal when they declare the same thing: the same bytes.
  friend constexpr bool operator==(const NumFormat &a, const NumFormat &b) noexcept {
    return detail::same_bytes(a.raw_, b.raw_);
  }
  friend constexpr bool operator!=(const NumFormat &a, const NumFormat &b) noexcept {
    return !(a == b);
  }

private:
  constexpr NumFormat() noexcept : raw_{} { raw_.size = HyperCastRawNumFormat_SIZE; }
  constexpr NumFormat(std::string_view decimal, std::string_view group,
                      NumStyles leniences) noexcept
      : NumFormat() {
    raw_.format.flags = static_cast<std::uint32_t>(leniences);
    detail::store(raw_.format.decimal_sep, decimal);
    detail::store(raw_.format.group_sep, group);
  }

  HyperCastRawNumFormat raw_;
};

// Caller-declared civil date-time notation for cast_datetime: the date's field order, and
// the day-period designators a culture writes beside a 12-hour clock (오전/오후, vorm./nachm.),
// read beside the English AM/PM, which are always read. 40 bytes, the core's RawDayPeriods.
class DateFormat {
public:
  // The order alone: the English AM/PM only.
  constexpr explicit DateFormat(DateOrder date_order) noexcept : order_(date_order), raw_{} {
    raw_.size = HyperCastRawDayPeriods_SIZE;
  }

  // The order and the designators, or std::nullopt for a designator the core refuses: over
  // 16 bytes, an ASCII digit, ASCII whitespace at either end, not UTF-8. Empty declares none.
  static std::optional<DateFormat> declare(DateOrder date_order, std::string_view am_symbol,
                                           std::string_view pm_symbol) noexcept {
    DateFormat format(date_order);
    if (!detail::store(format.raw_.periods.am, am_symbol) ||
        !detail::store(format.raw_.periods.pm, pm_symbol)) {
      return std::nullopt;
    }
    HyperCastCivilDateTime ignored{};
    if (::cast_datetime(nullptr, 0, static_cast<std::uint32_t>(DateOrder::year_month_day),
                        &format.raw_, &ignored, nullptr) < 0) {
      return std::nullopt;
    }
    return format;
  }

  constexpr DateOrder order() const noexcept { return order_; }
  std::string_view am() const noexcept { return detail::view(raw_.periods.am); }
  std::string_view pm() const noexcept { return detail::view(raw_.periods.pm); }

  // The layout the core reads, for a library that hands it to a C ABI of its own.
  constexpr const HyperCastRawDayPeriods &raw() const noexcept { return raw_; }

  friend constexpr bool operator==(const DateFormat &a, const DateFormat &b) noexcept {
    return a.order_ == b.order_ && detail::same_bytes(a.raw_, b.raw_);
  }
  friend constexpr bool operator!=(const DateFormat &a, const DateFormat &b) noexcept {
    return !(a == b);
  }

private:
  DateOrder order_;
  HyperCastRawDayPeriods raw_;
};

namespace detail {
template <class Raw> constexpr bool same_bytes(const Raw &a, const Raw &b) noexcept {
  // Field by field, so padding never counts; constexpr, so no memcmp.
  if constexpr (std::is_same_v<Raw, HyperCastRawNumFormat>) {
    const HyperCastNumFormat &x = a.format;
    const HyperCastNumFormat &y = b.format;
    auto same = [](const auto &s, const auto &t) {
      if (s.len != t.len) {
        return false;
      }
      for (std::size_t i = 0; i < s.len; ++i) {
        if (s.bytes[i] != t.bytes[i]) {
          return false;
        }
      }
      return true;
    };
    return a.size == b.size && x.flags == y.flags && x.digits == y.digits &&
           same(x.decimal_sep, y.decimal_sep) && same(x.group_sep, y.group_sep) &&
           same(x.currency_decimal_sep, y.currency_decimal_sep) &&
           same(x.currency_group_sep, y.currency_group_sep) && same(x.minus_sign, y.minus_sign) &&
           same(x.plus_sign, y.plus_sign) && same(x.percent, y.percent) &&
           same(x.currency, y.currency);
  } else {
    auto same = [](const auto &s, const auto &t) {
      if (s.len != t.len) {
        return false;
      }
      for (std::size_t i = 0; i < s.len; ++i) {
        if (s.bytes[i] != t.bytes[i]) {
          return false;
        }
      }
      return true;
    };
    return a.size == b.size && same(a.periods.am, b.periods.am) && same(a.periods.pm, b.periods.pm);
  }
}

// The verdict a door's return code names: 0 the value, 1 to 3 a fault, -1 a contract
// violation (unreachable from well-typed arguments).
template <class T>
inline Verdict<T> finish(std::int32_t code, const T &value,
                         const HyperCastRawFault &fault) noexcept {
  if (code == 0) {
    return Verdict<T>(value);
  }
  if (code < static_cast<std::int32_t>(CastFailure::empty) ||
      code > static_cast<std::int32_t>(CastFailure::out_of_range)) {
    contract_violation();
  }
  return Verdict<T>(Fault{static_cast<CastFailure>(code), fault.offset, fault.len});
}

template <class T, class Door>
inline Verdict<T> numeric(Door door, Text text, const NumFormat &format) noexcept {
  T out{};
  HyperCastRawFault fault{};
  const std::int32_t code = door(text.data(), text.size(), &format.raw(), &out, &fault);
  return finish(code, out, fault);
}

inline Decimal decimal(const HyperCastDecimal &raw) noexcept {
  return Decimal{raw.lo, raw.hi, raw.scale, raw.negative};
}
inline Timestamp timestamp(const HyperCastTimestamp &raw) noexcept {
  return Timestamp{raw.seconds, raw.nanos};
}
inline Date date(const HyperCastDate &raw) noexcept { return Date{raw.year, raw.month, raw.day}; }
inline CivilDateTime civil(const HyperCastCivilDateTime &raw) noexcept {
  return CivilDateTime{date(raw.date), raw.nanos_of_day};
}
inline Duration duration(const HyperCastDuration &raw) noexcept {
  return Duration{raw.seconds, raw.nanos};
}

template <class T, class Raw, class Convert>
inline Verdict<T> converted(std::int32_t code, const Raw &out, const HyperCastRawFault &fault,
                            Convert convert) noexcept {
  if (code == 0) {
    return Verdict<T>(convert(out));
  }
  return finish(code, T{}, fault);
}
} // namespace detail

// ---------------------------------------------------------------------------------------------
// The doors. Each reads the caller's UTF-8 (or a double), never allocates, and returns the
// value or a Fault whose span indexes that UTF-8. Names mirror the C ABI's.

// true/false, yes/no, on/off, 1/0 and the rest of the core's boolean grammar.
inline Verdict<bool> cast_bool(Text text) noexcept {
  std::uint8_t out = 0;
  HyperCastRawFault fault{};
  return detail::converted<bool>(::cast_bool(text.data(), text.size(), &out, &fault), out, fault,
                                 [](std::uint8_t v) { return v != 0; });
}

// One Unicode scalar value: never a surrogate, never past U+10FFFF.
inline Verdict<char32_t> cast_char(Text text) noexcept {
  std::uint32_t out = 0;
  HyperCastRawFault fault{};
  return detail::converted<char32_t>(::cast_char(text.data(), text.size(), &out, &fault), out,
                                     fault,
                                     [](std::uint32_t v) { return static_cast<char32_t>(v); });
}

// Integers under a declared format; a value outside the type is OutOfRange.
inline Verdict<std::int8_t> cast_i8(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::int8_t>(::cast_i8, text, format);
}
inline Verdict<std::int16_t> cast_i16(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::int16_t>(::cast_i16, text, format);
}
inline Verdict<std::int32_t> cast_i32(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::int32_t>(::cast_i32, text, format);
}
inline Verdict<std::int64_t> cast_i64(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::int64_t>(::cast_i64, text, format);
}
inline Verdict<std::uint8_t> cast_u8(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::uint8_t>(::cast_u8, text, format);
}
inline Verdict<std::uint16_t> cast_u16(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::uint16_t>(::cast_u16, text, format);
}
inline Verdict<std::uint32_t> cast_u32(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::uint32_t>(::cast_u32, text, format);
}
inline Verdict<std::uint64_t> cast_u64(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::uint64_t>(::cast_u64, text, format);
}

// Reals under a declared format; a magnitude beyond the type's finite range is OutOfRange,
// and NaN and the infinities are never read.
inline Verdict<float> cast_f32(Text text, const NumFormat &format) noexcept {
  return detail::numeric<float>(::cast_f32, text, format);
}
inline Verdict<double> cast_f64(Text text, const NumFormat &format) noexcept {
  return detail::numeric<double>(::cast_f64, text, format);
}

// An exact, canonical decimal under a declared format. Never rounds: a magnitude past 96 bits
// or more than 28 significant places is OutOfRange.
inline Verdict<Decimal> cast_decimal(Text text, const NumFormat &format) noexcept {
  HyperCastDecimal out{};
  HyperCastRawFault fault{};
  return detail::converted<Decimal>(
      ::cast_decimal(text.data(), text.size(), &format.raw(), &out, &fault), out, fault,
      detail::decimal);
}

// A UUID in any of the forms the core reads, as its RFC 9562 bytes.
inline Verdict<Uuid> cast_uuid(Text text) noexcept {
  Uuid out{};
  HyperCastRawFault fault{};
  return detail::finish(::cast_uuid(text.data(), text.size(), out.bytes.data(), &fault), out,
                        fault);
}

// An RFC 3339 instant.
inline Verdict<Timestamp> cast_timestamp(Text text) noexcept {
  HyperCastTimestamp out{};
  HyperCastRawFault fault{};
  return detail::converted<Timestamp>(::cast_timestamp(text.data(), text.size(), &out, &fault), out,
                                      fault, detail::timestamp);
}

// An integer Unix-epoch value in the declared unit.
inline Verdict<Timestamp> cast_unix(Text text, UnixPrecision precision) noexcept {
  HyperCastTimestamp out{};
  HyperCastRawFault fault{};
  return detail::converted<Timestamp>(
      ::cast_unix(text.data(), text.size(), static_cast<std::uint32_t>(precision), &out, &fault),
      out, fault, detail::timestamp);
}

// An Excel date serial, as text, in the declared date system.
inline Verdict<Timestamp> cast_excel_serial(Text text, ExcelEpoch epoch) noexcept {
  HyperCastTimestamp out{};
  HyperCastRawFault fault{};
  return detail::converted<Timestamp>(::cast_excel_serial(text.data(), text.size(),
                                                          static_cast<std::uint32_t>(epoch), &out,
                                                          &fault),
                                      out, fault, detail::timestamp);
}

// A strict yyyy-MM-dd date.
inline Verdict<Date> cast_date(Text text) noexcept {
  HyperCastDate out{};
  HyperCastRawFault fault{};
  return detail::converted<Date>(::cast_date(text.data(), text.size(), &out, &fault), out, fault,
                                 detail::date);
}

// A separated date (1/7/2026, 07.01.2026, 2026-1-7) in the declared field order.
inline Verdict<Date> cast_date(Text text, DateOrder order) noexcept {
  HyperCastDate out{};
  HyperCastRawFault fault{};
  return detail::converted<Date>(::cast_date_ordered(text.data(), text.size(),
                                                     static_cast<std::uint32_t>(order), &out,
                                                     &fault),
                                 out, fault, detail::date);
}

// A zone-less date and time (`1/7/2026 3:04 PM`, `2026-01-07 15:04:05`) in the declared order:
// cast_date(text, order)'s date, then optionally a 24-hour or 12-hour clock. No zone is read
// and none is invented.
inline Verdict<CivilDateTime> cast_datetime(Text text, DateOrder order) noexcept {
  HyperCastCivilDateTime out{};
  HyperCastRawFault fault{};
  return detail::converted<CivilDateTime>(::cast_datetime(text.data(), text.size(),
                                                          static_cast<std::uint32_t>(order),
                                                          nullptr, &out, &fault),
                                          out, fault, detail::civil);
}

// The same under a declared DateFormat: its order, and its culture's day-period designators
// beside the English AM/PM.
inline Verdict<CivilDateTime> cast_datetime(Text text, const DateFormat &format) noexcept {
  HyperCastCivilDateTime out{};
  HyperCastRawFault fault{};
  return detail::converted<CivilDateTime>(
      ::cast_datetime(text.data(), text.size(), static_cast<std::uint32_t>(format.order()),
                      &format.raw(), &out, &fault),
      out, fault, detail::civil);
}

// An ISO 24-hour time of day, as the nanoseconds since midnight.
inline Verdict<std::chrono::nanoseconds> cast_time(Text text) noexcept {
  std::uint64_t out = 0;
  HyperCastRawFault fault{};
  return detail::converted<std::chrono::nanoseconds>(
      ::cast_time(text.data(), text.size(), &out, &fault), out, fault,
      [](std::uint64_t v) { return std::chrono::nanoseconds(static_cast<std::int64_t>(v)); });
}

// A duration: ISO 8601 (PT1H30M), the invariant colon form (01:30:00) or protobuf JSON
// seconds (5400s).
inline Verdict<Duration> cast_duration(Text text) noexcept {
  HyperCastDuration out{};
  HyperCastRawFault fault{};
  return detail::converted<Duration>(::cast_duration(text.data(), text.size(), &out, &fault), out,
                                     fault, detail::duration);
}

// The typed doors read a double the caller already holds, the number a workbook stores for a
// cell. Their fault has no text to index: its span is always empty, at 0.

// The shortest decimal that names `value`, canonical: 0.1 is one tenth. NaN is Malformed; an
// infinity, a magnitude past 2^96 - 1 or more than 28 places is OutOfRange.
inline Verdict<Decimal> cast_decimal_from_double(double value) noexcept {
  HyperCastDecimal out{};
  HyperCastRawFault fault{};
  return detail::converted<Decimal>(::cast_decimal_from_f64(value, &out, &fault), out, fault,
                                    detail::decimal);
}

// The wall clock an Excel serial names in the declared date system, its fraction snapped to
// the fewest fractional-second digits that store as the same double.
inline Verdict<CivilDateTime> cast_excel_serial_from_double(double value,
                                                            ExcelEpoch epoch) noexcept {
  HyperCastCivilDateTime out{};
  HyperCastRawFault fault{};
  return detail::converted<CivilDateTime>(
      ::cast_excel_serial_from_f64(value, static_cast<std::uint32_t>(epoch), &out, &fault), out,
      fault, detail::civil);
}

// The time of day of an Excel serial (0.75 and 45292.75 are both 18:00), snapped likewise.
inline Verdict<std::chrono::nanoseconds> cast_excel_time(double value) noexcept {
  std::uint64_t out = 0;
  HyperCastRawFault fault{};
  return detail::converted<std::chrono::nanoseconds>(
      ::cast_excel_time(value, &out, &fault), out, fault,
      [](std::uint64_t v) { return std::chrono::nanoseconds(static_cast<std::int64_t>(v)); });
}

// A number of days as a duration (1.5 is a day and twelve hours), snapped likewise.
inline Verdict<Duration> cast_excel_duration(double value) noexcept {
  HyperCastDuration out{};
  HyperCastRawFault fault{};
  return detail::converted<Duration>(::cast_excel_duration(value, &out, &fault), out, fault,
                                     detail::duration);
}

// Always true: the core is linked into the program, so there is nothing to fail to load. Kept
// for parity with the bindings that load a library at run time.
constexpr bool is_available() noexcept { return true; }

// The linked library's own version, "major.minor.patch", from ::hypercast_version().
inline std::string_view native_version() noexcept {
  struct Rendered {
    char text[16] = {};
    std::size_t size = 0;
    explicit Rendered(std::uint32_t packed) noexcept {
      const std::uint32_t parts[3] = {packed >> 16, (packed >> 8) & 0xFF, packed & 0xFF};
      for (std::size_t i = 0; i < 3; ++i) {
        if (i > 0) {
          text[size++] = '.';
        }
        char digits[5];
        std::size_t count = 0;
        std::uint32_t part = parts[i];
        do {
          digits[count++] = static_cast<char>('0' + part % 10);
          part /= 10;
        } while (part != 0);
        while (count > 0) {
          text[size++] = digits[--count];
        }
      }
    }
  };
  static const Rendered rendered(::hypercast_version());
  return std::string_view(rendered.text, rendered.size);
}

} // namespace hypercast

#endif // HYPERCAST_HPP
