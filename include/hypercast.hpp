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
// so a door never meets a layout it refuses. Validation constraints are declared the same way
// (Constraints<T>::declare returns the core's Refusal for one it refuses), and a checked door
// returns a Checked<T>: the value, or a Rejection, the plain door's Fault or a Violation of a
// declared rule. The contract violations left are an enum value outside its enumerators
// (static_cast<DateOrder>(9)) and a refused declaration's unbuilt Constraints handed to a
// door: caller bugs, not data, which end the program with std::abort(), as Swift's
// precondition and C#'s exception do.

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
#include <cstring>
#include <initializer_list>
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

// ---------------------------------------------------------------------------------------------
// Validation constraints: the OpenAPI Schema Object's keywords, judged by the core after a door
// parses. A Constraints is declared once for its door and judged then, by the core; a checked
// door returns a Checked<T>, the value (and whether rounding changed it) or a Rejection, which
// is the plain door's own Fault or a Violation of a declared rule.

// The keyword a Violation broke: the core's code, open to additions (9 format and 10 pattern
// are reserved for a minor release), so a switch over keywords needs a default arm. Any
// std::uint32_t is a value; name() spells an unknown one "keyword N".
enum class Keyword : std::uint32_t {
  minimum = 1,
  exclusive_minimum = 2,
  maximum = 3,
  exclusive_maximum = 4,
  multiple_of = 5,
  min_length = 6,
  max_length = 7,
  enum_ = 8 // `enum`, which C++ reserves
};

// A keyword's name, held by value: the OpenAPI spelling (exclusiveMinimum, multipleOf, enum),
// or "keyword N" for a code this header does not name. Compare it with a string, print
// c_str(), or take view() from a named KeywordName (not a temporary, which would dangle).
class KeywordName {
public:
  constexpr std::string_view view() const & noexcept { return std::string_view(text_, size_); }
  std::string_view view() const && = delete;
  constexpr const char *c_str() const noexcept { return text_; }
  constexpr std::size_t size() const noexcept { return size_; }

  friend constexpr bool operator==(const KeywordName &a, std::string_view b) noexcept {
    return a.view() == b;
  }
  friend constexpr bool operator==(std::string_view a, const KeywordName &b) noexcept {
    return a == b.view();
  }
  friend constexpr bool operator!=(const KeywordName &a, std::string_view b) noexcept {
    return !(a == b);
  }
  friend constexpr bool operator!=(std::string_view a, const KeywordName &b) noexcept {
    return !(a == b);
  }

private:
  friend constexpr KeywordName name(Keyword keyword) noexcept;
  constexpr KeywordName() noexcept = default;
  constexpr void append(std::string_view text) noexcept {
    for (char c : text) {
      text_[size_++] = c;
    }
    text_[size_] = '\0';
  }

  char text_[24] = {}; // "keyword 4294967295" and its NUL
  std::size_t size_ = 0;
};

constexpr KeywordName name(Keyword keyword) noexcept {
  KeywordName out;
  switch (keyword) {
  case Keyword::minimum:
    out.append("minimum");
    break;
  case Keyword::exclusive_minimum:
    out.append("exclusiveMinimum");
    break;
  case Keyword::maximum:
    out.append("maximum");
    break;
  case Keyword::exclusive_maximum:
    out.append("exclusiveMaximum");
    break;
  case Keyword::multiple_of:
    out.append("multipleOf");
    break;
  case Keyword::min_length:
    out.append("minLength");
    break;
  case Keyword::max_length:
    out.append("maxLength");
    break;
  case Keyword::enum_:
    out.append("enum");
    break;
  default: {
    out.append("keyword ");
    char digits[10] = {};
    std::size_t count = 0;
    std::uint32_t code = static_cast<std::uint32_t>(keyword);
    do {
      digits[count++] = static_cast<char>('0' + code % 10);
      code /= 10;
    } while (code != 0);
    while (count > 0) {
      const char digit[1] = {digits[--count]};
      out.append(std::string_view(digit, 1));
    }
  }
  }
  return out;
}

// What a value that is not a multiple of multipleOf becomes. Anything but strict needs
// multipleOf; the core refuses the declaration otherwise.
enum class Rounding : std::uint32_t {
  strict = 0,   // it is a Violation of multipleOf
  midpoint = 1, // the nearest multiple, a tie to the even one
  floor = 2,    // the multiple below
  ceiling = 3   // the multiple above
};

// A broken rule: the keyword, and the span of the caller's input the judged value came from,
// in bytes of its UTF-8, as a Fault's span is (after trim, the trimmed text). A typed door's
// span is empty, at 0.
struct Violation {
  Keyword keyword;
  std::size_t offset;
  std::size_t length;

  friend constexpr bool operator==(const Violation &a, const Violation &b) noexcept {
    return a.keyword == b.keyword && a.offset == b.offset && a.length == b.length;
  }
  friend constexpr bool operator!=(const Violation &a, const Violation &b) noexcept {
    return !(a == b);
  }
};

// The failure case of a Checked: the door's own Fault (it did not parse), or a Violation (it
// parsed and broke a declared rule). Exactly one; no default constructor.
//
//   is_fault()                         whether it is the Fault
//   fault_if(), violation_if()         a pointer to the case, or nullptr
//   fault(), violation()               the case, unchecked: the wrong one is value-initialized
//   match(on_fault, on_violation)      calls the one for its case
//   visit(visitor)                     visitor(fault) or visitor(violation)
//   to_variant()                       std::variant<Fault, Violation>
class Rejection {
public:
  constexpr Rejection(Fault fault) noexcept : fault_(fault), violation_{}, is_fault_(true) {}
  constexpr Rejection(Violation violation) noexcept
      : fault_{}, violation_(violation), is_fault_(false) {}

  constexpr bool is_fault() const noexcept { return is_fault_; }

  constexpr const Fault *fault_if() const noexcept { return is_fault_ ? &fault_ : nullptr; }
  constexpr const Violation *violation_if() const noexcept {
    return is_fault_ ? nullptr : &violation_;
  }

  // Precondition: is_fault(). Otherwise a value-initialized Fault.
  constexpr const Fault &fault() const noexcept { return fault_; }
  // Precondition: !is_fault(). Otherwise a value-initialized Violation.
  constexpr const Violation &violation() const noexcept { return violation_; }

  template <class OnFault, class OnViolation>
  constexpr std::invoke_result_t<OnFault &&, const Fault &> match(OnFault &&on_fault,
                                                                  OnViolation &&on_violation) const
      noexcept(std::is_nothrow_invocable_v<OnFault &&, const Fault &> &&
               std::is_nothrow_invocable_v<OnViolation &&, const Violation &>) {
    static_assert(std::is_same_v<std::invoke_result_t<OnFault &&, const Fault &>,
                                 std::invoke_result_t<OnViolation &&, const Violation &>>,
                  "match's two cases must return the same type");
    if (is_fault_) {
      return std::forward<OnFault>(on_fault)(fault_);
    }
    return std::forward<OnViolation>(on_violation)(violation_);
  }

  template <class Visitor>
  constexpr std::invoke_result_t<Visitor &&, const Fault &> visit(Visitor &&visitor) const
      noexcept(std::is_nothrow_invocable_v<Visitor &&, const Fault &> &&
               std::is_nothrow_invocable_v<Visitor &&, const Violation &>) {
    static_assert(std::is_same_v<std::invoke_result_t<Visitor &&, const Fault &>,
                                 std::invoke_result_t<Visitor &&, const Violation &>>,
                  "visit's two cases must return the same type");
    if (is_fault_) {
      return std::forward<Visitor>(visitor)(fault_);
    }
    return std::forward<Visitor>(visitor)(violation_);
  }

  constexpr std::variant<Fault, Violation> to_variant() const noexcept {
    if (is_fault_) {
      return std::variant<Fault, Violation>(std::in_place_index<0>, fault_);
    }
    return std::variant<Fault, Violation>(std::in_place_index<1>, violation_);
  }

  friend constexpr bool operator==(const Rejection &a, const Rejection &b) noexcept {
    return a.is_fault_ == b.is_fault_ &&
           (a.is_fault_ ? a.fault_ == b.fault_ : a.violation_ == b.violation_);
  }
  friend constexpr bool operator!=(const Rejection &a, const Rejection &b) noexcept {
    return !(a == b);
  }

private:
  Fault fault_;
  Violation violation_;
  bool is_fault_;
};

// The success case of a Checked: the value, whether rounding to multipleOf changed it, and at
// the text door, when an enum entry matched, that entry's index (the value is then the entry
// as declared). An integer enum has no index: the value is the answer.
template <class T> struct CheckedValue {
  T value;
  bool adjusted;
  std::optional<std::uint32_t> enum_index;

  friend constexpr bool operator==(const CheckedValue &a, const CheckedValue &b) noexcept {
    return a.value == b.value && a.adjusted == b.adjusted && a.enum_index == b.enum_index;
  }
  friend constexpr bool operator!=(const CheckedValue &a, const CheckedValue &b) noexcept {
    return !(a == b);
  }
};

// The outcome of a checked cast: a CheckedValue or a Rejection, two cases and nothing else.
// The same accessors as Verdict, over those cases; under C++23 with <expected> it converts to
// std::expected<CheckedValue<T>, Rejection>. Deliberately no operator bool, as Verdict has
// none.
template <class T> class [[nodiscard]] Checked {
  static_assert(std::is_trivially_copyable_v<T> && std::is_default_constructible_v<T>,
                "a checked verdict holds a plain value");

public:
  using value_type = T;

  constexpr Checked(CheckedValue<T> success) noexcept
      : value_(success), rejection_(Fault{}), ok_(true) {}
  constexpr Checked(Rejection failure) noexcept : value_{}, rejection_(failure), ok_(false) {}

  constexpr bool ok() const noexcept { return ok_; }

  constexpr const CheckedValue<T> *value_if() const noexcept { return ok_ ? &value_ : nullptr; }
  constexpr const Rejection *rejection_if() const noexcept { return ok_ ? nullptr : &rejection_; }

  // Precondition: ok(). Otherwise a value-initialized CheckedValue.
  constexpr const CheckedValue<T> &value() const noexcept { return value_; }
  // Precondition: !ok(). Otherwise a Rejection holding a value-initialized Fault.
  constexpr const Rejection &rejection() const noexcept { return rejection_; }

  // The value alone, or the fallback.
  constexpr T value_or(T fallback) const noexcept { return ok_ ? value_.value : fallback; }

  template <class OnValue, class OnRejection>
  constexpr std::invoke_result_t<OnValue &&, const CheckedValue<T> &>
  match(OnValue &&on_value, OnRejection &&on_rejection) const
      noexcept(std::is_nothrow_invocable_v<OnValue &&, const CheckedValue<T> &> &&
               std::is_nothrow_invocable_v<OnRejection &&, const Rejection &>) {
    static_assert(std::is_same_v<std::invoke_result_t<OnValue &&, const CheckedValue<T> &>,
                                 std::invoke_result_t<OnRejection &&, const Rejection &>>,
                  "match's two cases must return the same type");
    if (ok_) {
      return std::forward<OnValue>(on_value)(value_);
    }
    return std::forward<OnRejection>(on_rejection)(rejection_);
  }

  template <class Visitor>
  constexpr std::invoke_result_t<Visitor &&, const CheckedValue<T> &> visit(Visitor &&visitor) const
      noexcept(std::is_nothrow_invocable_v<Visitor &&, const CheckedValue<T> &> &&
               std::is_nothrow_invocable_v<Visitor &&, const Rejection &>) {
    static_assert(std::is_same_v<std::invoke_result_t<Visitor &&, const CheckedValue<T> &>,
                                 std::invoke_result_t<Visitor &&, const Rejection &>>,
                  "visit's two cases must return the same type");
    if (ok_) {
      return std::forward<Visitor>(visitor)(value_);
    }
    return std::forward<Visitor>(visitor)(rejection_);
  }

  constexpr std::variant<CheckedValue<T>, Rejection> to_variant() const noexcept {
    if (ok_) {
      return std::variant<CheckedValue<T>, Rejection>(std::in_place_index<0>, value_);
    }
    return std::variant<CheckedValue<T>, Rejection>(std::in_place_index<1>, rejection_);
  }

#ifdef HYPERCAST_HAS_EXPECTED
  constexpr std::expected<CheckedValue<T>, Rejection> to_expected() const noexcept {
    if (ok_) {
      return std::expected<CheckedValue<T>, Rejection>(std::in_place, value_);
    }
    return std::expected<CheckedValue<T>, Rejection>(std::unexpect, rejection_);
  }
  constexpr operator std::expected<CheckedValue<T>, Rejection>() const noexcept {
    return to_expected();
  }
#endif

  friend constexpr bool operator==(const Checked &a, const Checked &b) noexcept {
    return a.ok_ == b.ok_ && (a.ok_ ? a.value_ == b.value_ : a.rejection_ == b.rejection_);
  }
  friend constexpr bool operator!=(const Checked &a, const Checked &b) noexcept {
    return !(a == b);
  }

private:
  CheckedValue<T> value_;
  Rejection rejection_;
  bool ok_;
};

// A checked verdict presented optionally: an Empty fault becomes std::nullopt; a Violation,
// every other fault and every value come through as they are.
template <class T>
constexpr std::optional<Checked<T>> optional(const Checked<T> &checked) noexcept {
  if (const Rejection *rejection = checked.rejection_if()) {
    if (const Fault *fault = rejection->fault_if();
        fault != nullptr && fault->reason == CastFailure::empty) {
      return std::nullopt;
    }
  }
  return checked;
}

// Why a declaration was refused: the core's refusal code, 1 to 13, with one fixed message per
// code worded as every binding words it, or 0, this header's own refusal of an enum list that
// does not fit (EnumStorage smaller than storage_size(entries), or a text entry past
// 4,294,967,295 bytes), which the core never sees. A code this header does not name (one a
// later core adds) keeps its number in its message. A refused declaration is a caller bug, not
// data.
struct Refusal {
  std::int32_t code;

  // Explicit, so a bare integer is never taken for a Refusal (refusal == 14 does not compile).
  constexpr explicit Refusal(std::int32_t refused) noexcept : code(refused) {
    if (refused >= 0 && refused <= 13) {
      return; // a fixed message: nothing to render
    }
    // The message of a code this header does not name, spelled once, here, with its number.
    constexpr std::string_view head = "Constraints refused (code ";
    constexpr std::string_view tail = "): a rule this binding does not name.";
    std::size_t at = 0;
    for (const char c : head) {
      unnamed_[at++] = c;
    }
    std::uint32_t magnitude = static_cast<std::uint32_t>(refused);
    if (refused < 0) {
      unnamed_[at++] = '-';
      magnitude = 0u - magnitude;
    }
    char digits[10] = {};
    std::size_t count = 0;
    do {
      digits[count++] = static_cast<char>('0' + magnitude % 10);
      magnitude /= 10;
    } while (magnitude != 0);
    while (count > 0) {
      unnamed_[at++] = digits[--count];
    }
    for (const char c : tail) {
      unnamed_[at++] = c;
    }
    unnamed_size_ = at;
  }

  // The fixed message for codes 0 to 13; for any other code (a core newer than this header),
  // a view into this Refusal, valid only while it lives: for such a code
  // `auto m = C::declare(f).refusal().message();` dangles once the Declared temporary is
  // gone, so keep the Declared (or a copy of the Refusal) while the message is read.
  constexpr std::string_view message() const noexcept {
    switch (code) {
    case 0:
      return "The enum list does not fit: the EnumStorage is smaller than storage_size(entries), "
             "or a text entry is longer than 4294967295 UTF-8 bytes.";
    case 1:
      return "Constraints refused (code 1): a constraint the door does not take.";
    case 2:
      return "Constraints refused (code 2): minimum together with exclusiveMinimum.";
    case 3:
      return "Constraints refused (code 3): maximum together with exclusiveMaximum.";
    case 4:
      return "Constraints refused (code 4): the lower bound is not below the upper bound.";
    case 5:
      return "Constraints refused (code 5): multipleOf is not positive.";
    case 6:
      return "Constraints refused (code 6): a rounding without multipleOf.";
    case 7:
      return "Constraints refused (code 7): minLength is above maxLength.";
    case 8:
      return "Constraints refused (code 8): a bound, step or enum entry the door cannot hold.";
    case 9:
      return "Constraints refused (code 9): an enum entry that is not UTF-8.";
    case 10:
      return "Constraints refused (code 10): a duplicate enum entry.";
    case 11:
      return "Constraints refused (code 11): a malformed constraints layout.";
    case 12:
      return "Constraints refused (code 12): an enum with no entries.";
    case 13:
      return "Constraints refused (code 13): a text enum entry no input can match.";
    default:
      return std::string_view(unnamed_, unnamed_size_);
    }
  }

  friend constexpr bool operator==(const Refusal &a, const Refusal &b) noexcept {
    return a.code == b.code;
  }
  friend constexpr bool operator!=(const Refusal &a, const Refusal &b) noexcept {
    return !(a == b);
  }

private:
  char unnamed_[80] = {};
  std::size_t unnamed_size_ = 0;
};

// The outcome of declaring constraints: the Constraints, or the Refusal. value() on a refused
// one is an unbuilt Constraints, which no door accepts as "no rules": a door handed it ends
// the program, as any contract violation does.
template <class C> class [[nodiscard]] Declared {
public:
  constexpr Declared(const C &declared) noexcept : value_(declared), refusal_{-1}, ok_(true) {}
  constexpr Declared(Refusal refusal) noexcept : value_(), refusal_(refusal), ok_(false) {}

  constexpr bool ok() const noexcept { return ok_; }
  constexpr const C *value_if() const noexcept { return ok_ ? &value_ : nullptr; }
  constexpr const Refusal *refusal_if() const noexcept { return ok_ ? nullptr : &refusal_; }
  // Precondition: ok(). Otherwise the unbuilt Constraints.
  constexpr const C &value() const noexcept { return value_; }
  // Precondition: !ok(). Otherwise Refusal{-1}, which names no refusal.
  constexpr const Refusal &refusal() const noexcept { return refusal_; }

  template <class OnValue, class OnRefusal>
  constexpr std::invoke_result_t<OnValue &&, const C &> match(OnValue &&on_value,
                                                              OnRefusal &&on_refusal) const
      noexcept(std::is_nothrow_invocable_v<OnValue &&, const C &> &&
               std::is_nothrow_invocable_v<OnRefusal &&, const Refusal &>) {
    static_assert(std::is_same_v<std::invoke_result_t<OnValue &&, const C &>,
                                 std::invoke_result_t<OnRefusal &&, const Refusal &>>,
                  "match's two cases must return the same type");
    if (ok_) {
      return std::forward<OnValue>(on_value)(value_);
    }
    return std::forward<OnRefusal>(on_refusal)(refusal_);
  }

private:
  C value_;
  Refusal refusal_;
  bool ok_;
};

namespace detail {
// An integer type that is a number: not bool, and not a character type.
template <class I>
constexpr bool is_whole_number_v =
    std::is_integral_v<I> && !std::is_same_v<I, bool> && !std::is_same_v<I, char> &&
    !std::is_same_v<I, wchar_t> && !std::is_same_v<I, char16_t> && !std::is_same_v<I, char32_t>
#ifdef __cpp_char8_t
    && !std::is_same_v<I, char8_t>
#endif
    ;

// The core's conversion of a declaration's decimal text, the one conversion every binding
// calls: 0 with `out` written, or 8 for text no door can hold.
inline std::int32_t declared_decimal(std::string_view text, HyperCastDecimal &out) noexcept {
  const std::int32_t code = ::cast_declared_decimal(
      reinterpret_cast<const std::uint8_t *>(text.data()), text.size(), &out);
  if (code != 0 && code != 8) {
    contract_violation();
  }
  return code;
}
} // namespace detail

// A bound, multipleOf or numeric enum entry as the caller wrote it: an exact Decimal, an
// integer, or decimal text in invariant notation ("0.000001", "-90", "1e-6"), borrowed until
// the declaration is made. Never a binary float: there is no constructor from one, so 0.1
// cannot be declared inexactly by accident; write "0.1". Every form reaches the core as text
// through its own conversion (cast_declared_decimal), which refuses rather than rounds a value
// past 96 bits or 28 places.
class ConstraintValue {
public:
  constexpr ConstraintValue(const Decimal &number) noexcept : number_(number), is_text_(false) {}
  template <class I, std::enable_if_t<detail::is_whole_number_v<I>, int> = 0>
  constexpr ConstraintValue(I number) noexcept
      : number_{magnitude(number), 0, 0, number < 0}, is_text_(false) {}
  template <class F, std::enable_if_t<std::is_floating_point_v<F>, int> = 0>
  ConstraintValue(F) = delete;
  constexpr ConstraintValue(std::string_view text) noexcept
      : number_{}, text_(text), is_text_(true) {}
  constexpr ConstraintValue(const char *text) noexcept : ConstraintValue(std::string_view(text)) {}

  // Whether it was written as text.
  constexpr bool is_text() const noexcept { return is_text_; }
  // The text as written; empty for a number.
  constexpr std::string_view text() const noexcept { return text_; }

  // The exact decimal the core reads this value as, or std::nullopt for one no door can hold
  // (the core's refusal 8): canonical, trailing zeros dropped, zero never negative.
  std::optional<Decimal> decimal() const noexcept {
    HyperCastDecimal out{};
    if (convert(out) != 0) {
      return std::nullopt;
    }
    return Decimal{out.lo, out.hi, out.scale, out.negative};
  }

  // The same in the core's layout: 0, or the refusal code 8.
  std::int32_t convert(HyperCastDecimal &out) const noexcept {
    if (is_text_) {
      return detail::declared_decimal(text_, out);
    }
    char buffer[Decimal::max_chars];
    const std::to_chars_result written = number_.to_chars(buffer, buffer + sizeof buffer);
    return detail::declared_decimal(
        std::string_view(buffer, static_cast<std::size_t>(written.ptr - buffer)), out);
  }

private:
  template <class I> static constexpr std::uint64_t magnitude(I number) noexcept {
    if constexpr (std::is_signed_v<I>) {
      return number < 0 ? 0 - static_cast<std::uint64_t>(number)
                        : static_cast<std::uint64_t>(number);
    } else {
      return static_cast<std::uint64_t>(number);
    }
  }

  Decimal number_;
  std::string_view text_;
  bool is_text_;
};

namespace detail {
// A borrowed run of enum entries.
template <class E> struct List {
  const E *data;
  std::size_t size;

  constexpr const E *begin() const noexcept { return data; }
  constexpr const E *end() const noexcept { return data + size; }
};
} // namespace detail

// The caller's bytes a declaration packs its enum list into, as the core reads it: 16 bytes a
// numeric entry, 4 plus its UTF-8 a text entry (Constraints<T>::storage_size says how many).
// Nothing is allocated, so the storage is the caller's: it must outlive the Constraints and
// every value read through it (a text enum match is a view into it), and must not change
// after the declaration. A copy of a Constraints refers to the same storage.
class EnumStorage {
public:
  template <std::size_t N>
  constexpr EnumStorage(std::uint8_t (&bytes)[N]) noexcept : data_(bytes), size_(N) {}
  template <std::size_t N>
  constexpr EnumStorage(std::array<std::uint8_t, N> &bytes) noexcept
      : data_(bytes.data()), size_(N) {}
  constexpr EnumStorage(std::uint8_t *data, std::size_t size) noexcept : data_(data), size_(size) {}

  constexpr std::uint8_t *data() const noexcept { return data_; }
  constexpr std::size_t size() const noexcept { return size_; }

private:
  std::uint8_t *data_;
  std::size_t size_;
};

namespace detail {
// The core's door code for a checked door's type.
template <class T> constexpr std::uint32_t door_of() noexcept {
  if constexpr (std::is_same_v<T, std::int8_t>) {
    return 2;
  } else if constexpr (std::is_same_v<T, std::int16_t>) {
    return 3;
  } else if constexpr (std::is_same_v<T, std::int32_t>) {
    return 4;
  } else if constexpr (std::is_same_v<T, std::int64_t>) {
    return 5;
  } else if constexpr (std::is_same_v<T, std::uint8_t>) {
    return 6;
  } else if constexpr (std::is_same_v<T, std::uint16_t>) {
    return 7;
  } else if constexpr (std::is_same_v<T, std::uint32_t>) {
    return 8;
  } else if constexpr (std::is_same_v<T, std::uint64_t>) {
    return 9;
  } else if constexpr (std::is_same_v<T, float>) {
    return 10;
  } else if constexpr (std::is_same_v<T, double>) {
    return 11;
  } else if constexpr (std::is_same_v<T, std::string_view>) {
    return 18;
  } else {
    static_assert(std::is_same_v<T, Decimal>,
                  "a Constraints is bound to a checked door: std::int8_t to std::uint64_t, "
                  "float, double, hypercast::Decimal, or std::string_view for the text door");
    return 19;
  }
}
} // namespace detail

// A declaration of validation constraints bound to the door T names: the integers std::int8_t
// to std::uint64_t, float, double, hypercast::Decimal, and std::string_view for the text door.
// Immutable, and judged once, by the core, when it is declared: declare() returns a
// Declared<Constraints<T>>, which holds the Refusal of a declaration that cannot mean anything
// at its door (a keyword the door does not take, minimum with exclusiveMinimum, a bound the
// door cannot hold, a duplicate entry, ...). There is no other way to build one, so every
// checked cast after that only judges data.
//
// The integer doors take minimum, exclusiveMinimum, maximum, exclusiveMaximum, multipleOf with
// a Rounding, and enum (whole numbers); float, double and Decimal the same but enum; the text
// door minLength, maxLength (in code points), trim (Unicode White_Space) and enum (strings,
// matched under simple case folding, a match delivering the entry as declared). Every field
// is in every door's Fields, so the core alone refuses one the door does not take.
//
//   auto latitude = hypercast::Constraints<double>::declare(
//       {.minimum = -90, .maximum = 90, .multiple_of = "0.000001",
//        .rounding = hypercast::Rounding::midpoint});                // C++20; Fields in C++17
template <class T> class Constraints {
public:
  // The door this declaration is bound to, the core's code.
  static constexpr std::uint32_t door = detail::door_of<T>();
  static constexpr bool is_text = std::is_same_v<T, std::string_view>;

  // An enum entry: a ConstraintValue at a numeric door, the text itself at the text door.
  using Entry = std::conditional_t<is_text, std::string_view, ConstraintValue>;

  // What declare reads; under C++20 a designated initializer names just the keywords wanted.
  struct Fields {
    std::optional<ConstraintValue> minimum;
    std::optional<ConstraintValue> exclusive_minimum;
    std::optional<ConstraintValue> maximum;
    std::optional<ConstraintValue> exclusive_maximum;
    std::optional<ConstraintValue> multiple_of;
    Rounding rounding = Rounding::strict;
    std::optional<std::uint32_t> min_length;
    std::optional<std::uint32_t> max_length;
    bool trim = false;
  };

  // The declaration without an enum: no enum is declared.
  static Declared<Constraints> declare(const Fields &fields) noexcept {
    return build(fields, nullptr, EnumStorage(nullptr, 0));
  }

  // The declaration with an enum list (a braced list, {"Female", "Male"} or {10, 20}; an
  // array; or a pointer and a count), packed into the caller's storage (see EnumStorage for
  // its lifetime). Storage smaller than storage_size(entries), or a text entry past
  // 4,294,967,295 bytes, is Refusal{0}. Every one of these declares an enum, so an empty list
  // ({}, or a count of 0) is one no value could match, which the core refuses (12); the
  // overload without entries is the way to declare none.
  static Declared<Constraints> declare(const Fields &fields, std::initializer_list<Entry> entries,
                                       EnumStorage storage) noexcept {
    const List list{entries.begin(), entries.size()};
    return build(fields, &list, storage);
  }
  template <std::size_t N>
  static Declared<Constraints> declare(const Fields &fields, const Entry (&entries)[N],
                                       EnumStorage storage) noexcept {
    const List list{entries, N};
    return build(fields, &list, storage);
  }
  template <std::size_t N>
  static Declared<Constraints> declare(const Fields &fields, const std::array<Entry, N> &entries,
                                       EnumStorage storage) noexcept {
    const List list{entries.data(), N};
    return build(fields, &list, storage);
  }
  static Declared<Constraints> declare(const Fields &fields, const Entry *entries,
                                       std::size_t count, EnumStorage storage) noexcept {
    const List list{entries, count};
    return build(fields, &list, storage);
  }

  // The bytes an enum list packs into: 16 a numeric entry, 4 plus its UTF-8 a text entry.
  static constexpr std::size_t storage_size(std::initializer_list<Entry> entries) noexcept {
    return packed_size(List{entries.begin(), entries.size()});
  }
  template <std::size_t N>
  static constexpr std::size_t storage_size(const Entry (&entries)[N]) noexcept {
    return packed_size(List{entries, N});
  }
  template <std::size_t N>
  static constexpr std::size_t storage_size(const std::array<Entry, N> &entries) noexcept {
    return packed_size(List{entries.data(), N});
  }
  static constexpr std::size_t storage_size(const Entry *entries, std::size_t count) noexcept {
    return packed_size(List{entries, count});
  }

  // The layout and the packed enum list the core reads, for a library that hands them to a C
  // ABI of its own.
  constexpr const HyperCastRawConstraints &raw() const noexcept { return raw_; }
  constexpr const std::uint8_t *enum_data() const noexcept { return enum_; }
  constexpr std::size_t enum_size() const noexcept { return enum_size_; }

  // At the text door, the declared entry at `index`, a view into the storage holding it.
  std::string_view entry(std::uint32_t index) const noexcept {
    static_assert(is_text, "only the text door's entries are strings");
    std::size_t at = 0;
    for (std::uint32_t i = 0;; ++i) {
      if (at + 4 > enum_size_) {
        detail::contract_violation(); // the core matched an entry never declared: a header bug
      }
      const std::size_t length = static_cast<std::size_t>(enum_[at]) |
                                 static_cast<std::size_t>(enum_[at + 1]) << 8 |
                                 static_cast<std::size_t>(enum_[at + 2]) << 16 |
                                 static_cast<std::size_t>(enum_[at + 3]) << 24;
      at += 4;
      if (at + length > enum_size_) {
        detail::contract_violation(); // a packed entry past the list: a header bug
      }
      if (i == index) {
        return std::string_view(reinterpret_cast<const char *>(enum_ + at), length);
      }
      at += length;
    }
  }

private:
  template <class> friend class Declared;

  // Unbuilt: a size of 0, which the core refuses at every door, so it never reads as "no
  // rules".
  constexpr Constraints() noexcept : raw_{} {}

  using List = detail::List<Entry>;

  static constexpr std::size_t packed_size(List entries) noexcept {
    if constexpr (is_text) {
      std::size_t size = 0;
      for (const std::string_view &entry : entries) {
        size += 4 + entry.size();
      }
      return size;
    } else {
      return 16 * entries.size;
    }
  }

  // `listed` is the enum list, or null for none declared.
  static Declared<Constraints> build(const Fields &fields, const List *listed,
                                     EnumStorage storage) noexcept {
    const List entries = listed != nullptr ? *listed : List{nullptr, 0};
    Constraints declared;
    HyperCastRawConstraints &raw = declared.raw_;
    raw.size = HyperCastRawConstraints_SIZE;
    struct Slot {
      const std::optional<ConstraintValue> &value;
      std::uint32_t flag;
      HyperCastDecimal &out;
    };
    const Slot slots[] = {
        {fields.minimum, HyperCastRawConstraints_HAS_MINIMUM, raw.lower},
        {fields.exclusive_minimum, HyperCastRawConstraints_HAS_EXCLUSIVE_MINIMUM, raw.lower},
        {fields.maximum, HyperCastRawConstraints_HAS_MAXIMUM, raw.upper},
        {fields.exclusive_maximum, HyperCastRawConstraints_HAS_EXCLUSIVE_MAXIMUM, raw.upper},
        {fields.multiple_of, HyperCastRawConstraints_HAS_MULTIPLE_OF, raw.multiple_of},
    };
    for (const Slot &slot : slots) {
      if (slot.value.has_value()) {
        if (const std::int32_t code = slot.value->convert(slot.out); code != 0) {
          return Refusal{code};
        }
        raw.flags |= slot.flag;
      }
    }
    raw.rounding = static_cast<std::uint32_t>(fields.rounding);
    if (fields.min_length.has_value()) {
      raw.flags |= HyperCastRawConstraints_HAS_MIN_LENGTH;
      raw.min_length = *fields.min_length;
    }
    if (fields.max_length.has_value()) {
      raw.flags |= HyperCastRawConstraints_HAS_MAX_LENGTH;
      raw.max_length = *fields.max_length;
    }
    if (fields.trim) {
      raw.flags |= HyperCastRawConstraints_TRIM;
    }
    if (listed != nullptr) {
      raw.flags |= HyperCastRawConstraints_HAS_ENUM;
    }

    const std::size_t needed = packed_size(entries);
    if (needed > storage.size()) {
      return Refusal{0};
    }
    std::size_t at = 0;
    for (const Entry &entry : entries) {
      if constexpr (is_text) {
        if constexpr (sizeof(std::size_t) > sizeof(std::uint32_t)) {
          if (entry.size() > UINT32_MAX) {
            return Refusal{0};
          }
        }
        const std::uint32_t length = static_cast<std::uint32_t>(entry.size());
        for (int shift = 0; shift < 32; shift += 8) {
          storage.data()[at++] = static_cast<std::uint8_t>(length >> shift); // little-endian
        }
        for (char c : entry) {
          storage.data()[at++] = static_cast<std::uint8_t>(c);
        }
      } else {
        HyperCastDecimal decimal{};
        if (const std::int32_t code = entry.convert(decimal); code != 0) {
          return Refusal{code};
        }
        static_assert(sizeof decimal == 16, "an enum entry is the core's 16-byte Decimal");
        std::memcpy(storage.data() + at, &decimal, sizeof decimal); // native-endian
        at += sizeof decimal;
      }
    }
    if (needed > 0) {
      declared.enum_ = storage.data();
      declared.enum_size_ = needed;
    }
    if (const std::int32_t code =
            ::cast_constraints_check(door, 0, &raw, declared.enum_, declared.enum_size_);
        code != 0) {
      return Refusal{code};
    }
    return declared;
  }

  HyperCastRawConstraints raw_;
  const std::uint8_t *enum_ = nullptr;
  std::size_t enum_size_ = 0;
};

namespace detail {
// The checked verdict a return code names: 0 the value, 1 to 3 a fault, 4 a violation, -1 a
// contract violation (a Constraints the door does not take: unbuilt, or another door's).
template <class T, class Raw, class Convert>
inline Checked<T> judged(std::int32_t code, const Raw &out, const HyperCastRawFault &fault,
                         const HyperCastRawCheck &check, Convert convert) noexcept {
  switch (code) {
  case 0: {
    std::optional<std::uint32_t> index;
    if (check.enum_index != UINT32_MAX) {
      index = check.enum_index;
    }
    return Checked<T>(CheckedValue<T>{convert(out, index), check.adjusted != 0, index});
  }
  case 1:
  case 2:
  case 3:
    return Checked<T>(Rejection(Fault{static_cast<CastFailure>(code), fault.offset, fault.len}));
  case 4:
    return Checked<T>(
        Rejection(Violation{static_cast<Keyword>(check.keyword), fault.offset, fault.len}));
  default:
    contract_violation();
  }
}

template <class T, class Raw = T, class Convert>
inline Checked<T> checked(const Constraints<T> &constraints, const NumFormat *format, Text text,
                          Convert convert) noexcept {
  Raw out{};
  HyperCastRawFault fault{};
  HyperCastRawCheck check{};
  const std::int32_t code =
      ::cast_checked(Constraints<T>::door, 0, format != nullptr ? &format->raw() : nullptr,
                     &constraints.raw(), constraints.enum_data(), constraints.enum_size(),
                     text.data(), text.size(), &out, &fault, &check);
  return judged<T>(code, out, fault, check, convert);
}

template <class T, class Raw = T, class Convert>
inline Checked<T> checked_from_double(const Constraints<T> &constraints, double value,
                                      Convert convert) noexcept {
  Raw out{};
  HyperCastRawFault fault{};
  HyperCastRawCheck check{};
  const std::int32_t code = ::cast_checked_from_f64(Constraints<T>::door, value, &constraints.raw(),
                                                    constraints.enum_data(),
                                                    constraints.enum_size(), &out, &fault, &check);
  return judged<T>(code, out, fault, check, convert);
}

struct Same {
  template <class V> constexpr V operator()(V value, std::optional<std::uint32_t>) const noexcept {
    return value;
  }
};
struct AsDecimal {
  Decimal operator()(const HyperCastDecimal &raw, std::optional<std::uint32_t>) const noexcept {
    return decimal(raw);
  }
};

template <class T>
inline Checked<T> numeric_checked(Text text, const NumFormat &format,
                                  const Constraints<T> &constraints) noexcept {
  return checked<T>(constraints, &format, text, Same{});
}
} // namespace detail

// The checked doors: the plain door's parse under the declared format, then the value judged
// by the constraints. A Violation's span is the value's, in bytes of the input, as a Fault's.
inline Checked<std::int8_t> cast_i8(Text text, const NumFormat &format,
                                    const Constraints<std::int8_t> &constraints) noexcept {
  return detail::numeric_checked(text, format, constraints);
}
inline Checked<std::int16_t> cast_i16(Text text, const NumFormat &format,
                                      const Constraints<std::int16_t> &constraints) noexcept {
  return detail::numeric_checked(text, format, constraints);
}
inline Checked<std::int32_t> cast_i32(Text text, const NumFormat &format,
                                      const Constraints<std::int32_t> &constraints) noexcept {
  return detail::numeric_checked(text, format, constraints);
}
inline Checked<std::int64_t> cast_i64(Text text, const NumFormat &format,
                                      const Constraints<std::int64_t> &constraints) noexcept {
  return detail::numeric_checked(text, format, constraints);
}
inline Checked<std::uint8_t> cast_u8(Text text, const NumFormat &format,
                                     const Constraints<std::uint8_t> &constraints) noexcept {
  return detail::numeric_checked(text, format, constraints);
}
inline Checked<std::uint16_t> cast_u16(Text text, const NumFormat &format,
                                       const Constraints<std::uint16_t> &constraints) noexcept {
  return detail::numeric_checked(text, format, constraints);
}
inline Checked<std::uint32_t> cast_u32(Text text, const NumFormat &format,
                                       const Constraints<std::uint32_t> &constraints) noexcept {
  return detail::numeric_checked(text, format, constraints);
}
inline Checked<std::uint64_t> cast_u64(Text text, const NumFormat &format,
                                       const Constraints<std::uint64_t> &constraints) noexcept {
  return detail::numeric_checked(text, format, constraints);
}
inline Checked<float> cast_f32(Text text, const NumFormat &format,
                               const Constraints<float> &constraints) noexcept {
  return detail::numeric_checked(text, format, constraints);
}
inline Checked<double> cast_f64(Text text, const NumFormat &format,
                                const Constraints<double> &constraints) noexcept {
  return detail::numeric_checked(text, format, constraints);
}
inline Checked<Decimal> cast_decimal(Text text, const NumFormat &format,
                                     const Constraints<Decimal> &constraints) noexcept {
  return detail::checked<Decimal, HyperCastDecimal>(constraints, &format, text,
                                                    detail::AsDecimal{});
}

// The checked typed doors judge a double a source already decoded (a JSON parser's number, a
// spreadsheet's cell) on its sign and shortest round-trip decimal: the judgement the text
// door makes of that text. -0.0 is zero; NaN is Malformed and an infinity OutOfRange. With no
// text to index, every Fault and Violation has an empty span at 0.
namespace detail {
template <class T>
inline Checked<T> from_double(double value, const Constraints<T> &constraints) noexcept {
  return checked_from_double<T>(constraints, value, Same{});
}
} // namespace detail

inline Checked<std::int8_t>
cast_i8_from_double(double value, const Constraints<std::int8_t> &constraints) noexcept {
  return detail::from_double(value, constraints);
}
inline Checked<std::int16_t>
cast_i16_from_double(double value, const Constraints<std::int16_t> &constraints) noexcept {
  return detail::from_double(value, constraints);
}
inline Checked<std::int32_t>
cast_i32_from_double(double value, const Constraints<std::int32_t> &constraints) noexcept {
  return detail::from_double(value, constraints);
}
inline Checked<std::int64_t>
cast_i64_from_double(double value, const Constraints<std::int64_t> &constraints) noexcept {
  return detail::from_double(value, constraints);
}
inline Checked<std::uint8_t>
cast_u8_from_double(double value, const Constraints<std::uint8_t> &constraints) noexcept {
  return detail::from_double(value, constraints);
}
inline Checked<std::uint16_t>
cast_u16_from_double(double value, const Constraints<std::uint16_t> &constraints) noexcept {
  return detail::from_double(value, constraints);
}
inline Checked<std::uint32_t>
cast_u32_from_double(double value, const Constraints<std::uint32_t> &constraints) noexcept {
  return detail::from_double(value, constraints);
}
inline Checked<std::uint64_t>
cast_u64_from_double(double value, const Constraints<std::uint64_t> &constraints) noexcept {
  return detail::from_double(value, constraints);
}
inline Checked<float> cast_f32_from_double(double value,
                                           const Constraints<float> &constraints) noexcept {
  return detail::from_double(value, constraints);
}
inline Checked<double> cast_f64_from_double(double value,
                                            const Constraints<double> &constraints) noexcept {
  return detail::from_double(value, constraints);
}
inline Checked<Decimal> cast_decimal_from_double(double value,
                                                 const Constraints<Decimal> &constraints) noexcept {
  return detail::checked_from_double<Decimal, HyperCastDecimal>(constraints, value,
                                                                detail::AsDecimal{});
}

// The text door: the caller's input, once it is valid UTF-8, as a view of it. Invalid UTF-8
// is Malformed, spanning the first invalid sequence; empty input is Empty. Untrimmed.
inline Verdict<std::string_view> cast_text(Text text) noexcept {
  HyperCastRawSpan out{};
  HyperCastRawFault fault{};
  const std::int32_t code = ::cast_text(text.data(), text.size(), &out, &fault);
  return detail::finish(
      code,
      code == 0
          ? std::string_view(reinterpret_cast<const char *>(text.data()) + out.offset, out.len)
          : std::string_view(),
      fault);
}

// The checked text door: the input, trimmed when the constraints say so, judged by its
// lengths in code points and its enum. The value is a view of the caller's input, or on an
// enum match the entry as declared, a view into the declaration's EnumStorage, with its index.
inline Checked<std::string_view>
cast_text(Text text, const Constraints<std::string_view> &constraints) noexcept {
  return detail::checked<std::string_view, HyperCastRawSpan>(
      constraints, nullptr, text,
      [&](const HyperCastRawSpan &span, std::optional<std::uint32_t> index) {
        if (index.has_value()) {
          return constraints.entry(*index);
        }
        return std::string_view(reinterpret_cast<const char *>(text.data()) + span.offset,
                                span.len);
      });
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
