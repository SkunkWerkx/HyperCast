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
//
// What failed, and the date shapes (hypercast::detailed). The doors above report a Fault; the
// same doors under hypercast::detailed report a Failure, a Fault's reason and span with a
// Detail beside them (name(detail) spells it: group_size, too_large, ...; any number a later
// core adds is a Detail too, never an error). Only they take date options: a DateOptions
// (compact dates, a space for the `T`, +hh offsets, a two-digit-year pivot) and a MonthNames
// vocabulary, each checked once by the core when it is declared.
//
// A numeric format also declares its grouping: strict Western by default (`12,34` is
// Malformed), strict Indian, or Grouping::legacy, a group separator anywhere between two
// digits. Strict grouping and everything detailed needs a core of this header's major version
// and a minor version at least as new (the patch never matters: a patch release adds no export),
// which the header checks at run time before it is used: core_is_compatible() asks, and a
// program should ask it once at startup. A core that fails it lacks the detailed exports or
// ignores the grouping policy, which would silently read as legacy: a strict declaration against
// one is refused (std::nullopt), and a door handed a strict format by other means writes one
// line naming both versions to stderr and ends the program with std::abort(), never parsing it
// as legacy.

#ifndef HYPERCAST_HPP
#define HYPERCAST_HPP

// The version of this header, packed as hypercast_version() packs the library's own:
// major << 16 | minor << 8 | patch. It is the crate's version, which a release moves with
// everything else (the smoke test holds the two equal). A core drives the header when its major
// version is the header's and its minor is at least the header's: a library older than the
// header it is called from lacks what that header added, and a patch release adds nothing, so
// core_is_compatible() ignores the patch.
#define HYPERCAST_VERSION_MAJOR 1
#define HYPERCAST_VERSION_MINOR 0
#define HYPERCAST_VERSION_PATCH 0
#define HYPERCAST_VERSION                                                                          \
  ((HYPERCAST_VERSION_MAJOR << 16) | (HYPERCAST_VERSION_MINOR << 8) | HYPERCAST_VERSION_PATCH)

#if !defined(__cplusplus) ||                                                                       \
    (__cplusplus < 201703L && (!defined(_MSVC_LANG) || _MSVC_LANG < 201703L))
#error "hypercast.hpp needs C++17 or later; C programs include hypercast.h"
#endif

#include <array>
#include <charconv>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdio>
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

// The linked library's own version, packed as HYPERCAST_VERSION packs the header's.
inline std::uint32_t native_version_number() noexcept { return ::hypercast_version(); }

namespace detail {
// Whether a core of this packed version drives a header of that one: the same major version, a
// minor at least as new, and any patch (a patch release adds no export). The rule hypercast.h's
// direct C callers apply to the headers they were built against.
constexpr bool version_accepts(std::uint32_t core, std::uint32_t header) noexcept {
  return (core >> 16) == (header >> 16) && ((core >> 8) & 0xFF) >= ((header >> 8) & 0xFF);
}
} // namespace detail

// Whether the linked library can drive this header (HYPERCAST_VERSION): its major version is
// the header's and its minor at least as new, whatever the patch. A core that fails has no
// detailed exports, or reads no grouping policy in a format's flags, so strict grouping against
// it would silently read as legacy; every path that uses either asks this first. A program that
// uses them should ask it once at startup, and read an answer of false as a deployment error.
// Read once, then remembered.
inline bool core_is_compatible() noexcept {
  static const bool compatible =
      detail::version_accepts(::hypercast_version(), static_cast<std::uint32_t>(HYPERCAST_VERSION));
  return compatible;
}

// The closed set of reasons a cast can fail: the core's verdict codes, verbatim. Adding a
// member is a breaking change in every binding at once.
enum class CastFailure : std::int32_t {
  empty = 1,       // required input was empty or white space; optional() reads it as absent
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

// What exactly a cast found wrong, beside its reason: the core's detail codes, which keep their
// numbers and meanings. New codes may join in a minor release, so a Detail is open: any
// std::uint32_t is a value, an unknown one keeps its number (the reason and span of its Failure
// still stand), and name() spells it "detail N". `none` (0) is a checked door's Violation, which
// breaks a rule rather than fails to parse; it is never a failure's detail. A detail says what
// failed, never how to retry.
enum class Detail : std::uint32_t {
  none = 0,
  blank = 1,                    // empty, or only white space
  unexpected_character = 2,     // a character the position doesn't allow
  digit_expected = 3,           // a digit was required: `0xG`, `1e`
  delimiter_expected = 4,       // a required mark is missing or different
  trailing_text = 5,            // the value is complete, but more text follows
  no_digits = 6,                // no numeral at all: only a sign, a symbol or a bare prefix
  sign_misplaced = 7,           // a sign where none may stand
  parentheses_unbalanced = 8,   // `(` without `)`
  mark_repeated = 9,            // a once-only mark appears twice
  group_misplaced = 10,         // a group separator not between digits, or an empty group
  group_size = 11,              // a group of the wrong digit count for the grouping
  group_in_fraction = 12,       // grouping after the decimal separator or in an exponent
  ambiguous_separator = 13,     // separator detection can't tell `.` from `,`
  fraction_not_allowed = 14,    // a fraction where the type is whole
  field_width = 15,             // a delimited field with the wrong number of digits
  wrong_length = 16,            // a fixed-shape token of the wrong length
  component_missing = 17,       // a required part of the value is absent
  not_recognized = 18,          // not one of the door's accepted words or values
  unknown_unit = 19,            // an unknown or misplaced duration unit
  invalid_utf8 = 20,            // the bytes where the cast failed are not UTF-8
  too_long = 21,                // past an implementation limit
  not_finite = 22,              // a NaN or infinity
  not_whole = 23,               // a non-integral number at a whole-number door
  negative_not_allowed = 24,    // negative where the type has no negatives
  too_large = 25,               // beyond what the type represents
  too_small = 26,               // below a signed type's minimum
  too_precise = 27,             // more fractional digits than the type holds
  date_field_out_of_range = 28, // year 0, month 13, a day past the month's end
  time_field_out_of_range = 29, // hour, minute, second or offset out of range
  outside_window = 30,          // an instant or duration past the supported window
  nonexistent_date = 31,        // Excel's phantom 1900-02-29
  not_a_scalar = 32             // a code point above U+10FFFF, or a surrogate
};

// Whether this header names the detail: 1 to 32. A core newer than the header may report
// others, which are no error.
constexpr bool is_known(Detail detail) noexcept {
  return static_cast<std::uint32_t>(detail) >= 1 && static_cast<std::uint32_t>(detail) <= 32;
}

// A detail's name, held by value: the snake_case spelling above (group_size, too_large), or
// "detail N" for a code this header does not name, `none` included. Compare it with a string,
// print c_str(), or take view() from a named DetailName (not a temporary, which would dangle).
class DetailName {
public:
  constexpr std::string_view view() const & noexcept { return std::string_view(text_, size_); }
  std::string_view view() const && = delete;
  constexpr const char *c_str() const noexcept { return text_; }
  constexpr std::size_t size() const noexcept { return size_; }

  friend constexpr bool operator==(const DetailName &a, std::string_view b) noexcept {
    return a.view() == b;
  }
  friend constexpr bool operator==(std::string_view a, const DetailName &b) noexcept {
    return a == b.view();
  }
  friend constexpr bool operator!=(const DetailName &a, std::string_view b) noexcept {
    return !(a == b);
  }
  friend constexpr bool operator!=(std::string_view a, const DetailName &b) noexcept {
    return !(a == b);
  }

private:
  friend constexpr DetailName name(Detail detail) noexcept;
  constexpr DetailName() noexcept = default;
  constexpr void append(std::string_view text) noexcept {
    for (char c : text) {
      text_[size_++] = c;
    }
    text_[size_] = '\0';
  }

  char text_[32] = {}; // "date_field_out_of_range" and "detail 4294967295", and a NUL
  std::size_t size_ = 0;
};

constexpr DetailName name(Detail detail) noexcept {
  constexpr std::string_view names[] = {"",
                                        "blank",
                                        "unexpected_character",
                                        "digit_expected",
                                        "delimiter_expected",
                                        "trailing_text",
                                        "no_digits",
                                        "sign_misplaced",
                                        "parentheses_unbalanced",
                                        "mark_repeated",
                                        "group_misplaced",
                                        "group_size",
                                        "group_in_fraction",
                                        "ambiguous_separator",
                                        "fraction_not_allowed",
                                        "field_width",
                                        "wrong_length",
                                        "component_missing",
                                        "not_recognized",
                                        "unknown_unit",
                                        "invalid_utf8",
                                        "too_long",
                                        "not_finite",
                                        "not_whole",
                                        "negative_not_allowed",
                                        "too_large",
                                        "too_small",
                                        "too_precise",
                                        "date_field_out_of_range",
                                        "time_field_out_of_range",
                                        "outside_window",
                                        "nonexistent_date",
                                        "not_a_scalar"};
  DetailName out;
  if (is_known(detail)) {
    out.append(names[static_cast<std::uint32_t>(detail)]);
    return out;
  }
  out.append("detail ");
  char digits[10] = {};
  std::size_t count = 0;
  std::uint32_t code = static_cast<std::uint32_t>(detail);
  do {
    digits[count++] = static_cast<char>('0' + code % 10);
    code /= 10;
  } while (code != 0);
  while (count > 0) {
    const char digit[1] = {digits[--count]};
    out.append(std::string_view(digit, 1));
  }
  return out;
}

// The failure case of a detailed door: a Fault's reason and span, and what failed in it. The
// Fault is exactly what the plain door reports for the same text (fault()); Fault itself is
// unchanged, and its equality never sees the detail.
struct Failure {
  CastFailure reason;
  Detail detail;
  std::size_t offset;
  std::size_t length;

  // The plain door's Fault: the reason and the span.
  constexpr Fault fault() const noexcept { return Fault{reason, offset, length}; }
  // The raw detail code, which an unknown detail keeps.
  constexpr std::uint32_t detail_code() const noexcept {
    return static_cast<std::uint32_t>(detail);
  }

  friend constexpr bool operator==(const Failure &a, const Failure &b) noexcept {
    return a.reason == b.reason && a.detail == b.detail && a.offset == b.offset &&
           a.length == b.length;
  }
  friend constexpr bool operator!=(const Failure &a, const Failure &b) noexcept {
    return !(a == b);
  }
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

// The outcome of a detailed cast: exactly one of a value or a Failure, which is a Fault with
// its Detail. The same accessors as Verdict, over those cases (failure_if(), failure()), and
// to_verdict() for the plain Verdict the same text gives, detail dropped. Under C++23 with
// <expected> it converts to std::expected<T, Failure>.
template <class T> class [[nodiscard]] DetailedVerdict {
  static_assert(std::is_trivially_copyable_v<T> && std::is_default_constructible_v<T>,
                "a verdict holds a plain value");

public:
  using value_type = T;

  constexpr DetailedVerdict(T success) noexcept : value_(success), failure_{}, ok_(true) {}
  constexpr DetailedVerdict(Failure failure) noexcept : value_{}, failure_(failure), ok_(false) {}

  constexpr bool ok() const noexcept { return ok_; }

  constexpr const T *value_if() const noexcept { return ok_ ? &value_ : nullptr; }
  constexpr const Failure *failure_if() const noexcept { return ok_ ? nullptr : &failure_; }

  // Precondition: ok(). Otherwise a value-initialized T.
  constexpr const T &value() const noexcept { return value_; }
  // Precondition: !ok(). Otherwise a value-initialized Failure, whose reason is no enumerator.
  constexpr const Failure &failure() const noexcept { return failure_; }

  constexpr T value_or(T fallback) const noexcept { return ok_ ? value_ : fallback; }

  // The plain Verdict: the value, or the Fault of this Failure.
  constexpr Verdict<T> to_verdict() const noexcept {
    if (ok_) {
      return Verdict<T>(value_);
    }
    return Verdict<T>(failure_.fault());
  }

  template <class OnValue, class OnFailure>
  constexpr std::invoke_result_t<OnValue &&, const T &> match(OnValue &&on_value,
                                                              OnFailure &&on_failure) const
      noexcept(std::is_nothrow_invocable_v<OnValue &&, const T &> &&
               std::is_nothrow_invocable_v<OnFailure &&, const Failure &>) {
    static_assert(std::is_same_v<std::invoke_result_t<OnValue &&, const T &>,
                                 std::invoke_result_t<OnFailure &&, const Failure &>>,
                  "match's two cases must return the same type");
    if (ok_) {
      return std::forward<OnValue>(on_value)(value_);
    }
    return std::forward<OnFailure>(on_failure)(failure_);
  }

  template <class Visitor>
  constexpr std::invoke_result_t<Visitor &&, const T &> visit(Visitor &&visitor) const
      noexcept(std::is_nothrow_invocable_v<Visitor &&, const T &> &&
               std::is_nothrow_invocable_v<Visitor &&, const Failure &>) {
    static_assert(std::is_same_v<std::invoke_result_t<Visitor &&, const T &>,
                                 std::invoke_result_t<Visitor &&, const Failure &>>,
                  "visit's two cases must return the same type");
    if (ok_) {
      return std::forward<Visitor>(visitor)(value_);
    }
    return std::forward<Visitor>(visitor)(failure_);
  }

  // The verdict as a std::variant: index 0 the value, index 1 the failure.
  constexpr std::variant<T, Failure> to_variant() const noexcept {
    if (ok_) {
      return std::variant<T, Failure>(std::in_place_index<0>, value_);
    }
    return std::variant<T, Failure>(std::in_place_index<1>, failure_);
  }

#ifdef HYPERCAST_HAS_EXPECTED
  constexpr std::expected<T, Failure> to_expected() const noexcept {
    if (ok_) {
      return std::expected<T, Failure>(std::in_place, value_);
    }
    return std::expected<T, Failure>(std::unexpect, failure_);
  }
  constexpr operator std::expected<T, Failure>() const noexcept { return to_expected(); }
#endif

  friend constexpr bool operator==(const DetailedVerdict &a, const DetailedVerdict &b) noexcept {
    return a.ok_ == b.ok_ && (a.ok_ ? a.value_ == b.value_ : a.failure_ == b.failure_);
  }
  friend constexpr bool operator!=(const DetailedVerdict &a, const DetailedVerdict &b) noexcept {
    return !(a == b);
  }

private:
  T value_;
  Failure failure_;
  bool ok_;
};

// A detailed verdict presented optionally: a blank failure becomes std::nullopt.
template <class T>
constexpr std::optional<DetailedVerdict<T>> optional(const DetailedVerdict<T> &verdict) noexcept {
  if (const Failure *failure = verdict.failure_if();
      failure != nullptr && failure->reason == CastFailure::empty) {
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

// How a numeral's group separators are judged: the policy a NumFormat declares beside its
// styles. A policy only judges groups: with NumStyles::grouping off, a group separator stays
// forbidden under every one. Policy bits 8 and 9 of the layout's flags, which never share the
// styles; a value other than these three is a refused declaration.
enum class Grouping : std::uint32_t {
  legacy =
      HyperCastNumFormat_GROUPING_LEGACY, // a separator anywhere between two digits: 12,34 is 1234
  western =
      HyperCastNumFormat_GROUPING_WESTERN,    // groups of three counted from the right: 1,234,567
  indian = HyperCastNumFormat_GROUPING_INDIAN // lakh and crore: the last group three, the rest two
};

namespace detail {
[[noreturn]] inline void contract_violation() noexcept {
  // The core refused an argument the types should have ruled out: an enum value outside its
  // enumerators. A caller bug, never data.
  std::abort();
}

// A strict or detailed cast reached a core that cannot drive this header (core_is_compatible()
// is false): one line on stderr naming both versions, then the end of the program, rather than
// parse a strict request as legacy. A deployment error, not a caller bug, so it says so.
[[noreturn]] inline void stale_core() noexcept {
  const std::uint32_t core = ::hypercast_version();
  std::fprintf(stderr,
               "hypercast: the linked library is %u.%u.%u, which cannot drive this header "
               "(%d.%d.%d: the same major version and at least its minor are needed); "
               "refusing a strict or detailed cast\n",
               core >> 16, (core >> 8) & 0xFFu, core & 0xFFu, HYPERCAST_VERSION_MAJOR,
               HYPERCAST_VERSION_MINOR, HYPERCAST_VERSION_PATCH);
  std::abort();
}

// The core's Door codes, as the detailed exports number them (and the checked ones: 2 to 11,
// 18 and 19).
enum : std::uint32_t {
  door_bool = 1,
  door_i8 = 2,
  door_i16 = 3,
  door_i32 = 4,
  door_i64 = 5,
  door_u8 = 6,
  door_u16 = 7,
  door_u32 = 8,
  door_u64 = 9,
  door_f32 = 10,
  door_f64 = 11,
  door_uuid = 12,
  door_timestamp = 13,
  door_unix = 14,
  door_date = 15,
  door_time = 16,
  door_duration = 17,
  door_text = 18,
  door_decimal = 19,
  door_date_ordered = 20,
  door_datetime = 21,
  door_excel_serial = 22,
  door_char = 23
};

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
// RawNumFormat, which it reads in place. Grouping defaults to strict Western everywhere a format
// is declared: `12,34` is Malformed unless the caller declares Grouping::legacy.
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
    Grouping grouping = Grouping::western; // how group separators are judged; styles holds none
  };

  // The format these fields declare, or std::nullopt for one the core refuses (a symbol that
  // is too long, holds an ASCII digit or is not UTF-8, equal separators, a sign, percent
  // symbol or decimal separator with white space in it, a currency symbol with white space
  // at either edge, a digit zero that starts no digit block, a policy that is not one of
  // Grouping's, a style bit that is not NumStyles's, or styles carrying a grouping policy). The
  // core itself is the judge, asked once here, so a door never meets a format it refuses. A
  // strict grouping needs a core as new as this header (core_is_compatible()): against an older one
  // the declaration is refused, since that core would read the policy as legacy.
  static std::optional<NumFormat> declare(const Fields &fields) noexcept {
    NumFormat format;
    HyperCastNumFormat &f = format.raw_.format;
    const std::uint32_t styles = static_cast<std::uint32_t>(fields.styles);
    const std::uint32_t policy = static_cast<std::uint32_t>(fields.grouping);
    if ((styles & HyperCastNumFormat_GROUPING_POLICY) != 0 ||
        (policy & ~static_cast<std::uint32_t>(HyperCastNumFormat_GROUPING_POLICY)) != 0) {
      return std::nullopt;
    }
    f.flags = styles | policy;
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
    // Empty text with a format the core reads is Empty; one it refuses is -1. A format with no
    // bits past the 1.0 flags is judged by the 1.0 rules, which any core has; a policy or a bit
    // only a newer core knows needs the detailed judge, and so a core of the header's version.
    std::int32_t ignored = 0;
    if (f.flags >> 8 == 0) {
      if (::cast_i32(nullptr, 0, &format.raw_, &ignored, nullptr) < 0) {
        return std::nullopt;
      }
      return format;
    }
    if (!core_is_compatible() ||
        ::cast_detailed(detail::door_i32, 0, &format.raw_, nullptr, nullptr, nullptr, 0, nullptr, 0,
                        &ignored, nullptr) < 0) {
      return std::nullopt;
    }
    return format;
  }

  // `.` decimal and `,` group separators, NumStyles::all and strict Western grouping: what a
  // caller of this header reads by default. `12,34` is Malformed; Grouping::legacy
  // (invariant().with_grouping(Grouping::legacy)) is the 1.0 reading, 1234.
  static constexpr NumFormat invariant() noexcept {
    return NumFormat(".", ",", NumStyles::all, Grouping::western);
  }

  // The ./, roles resolved per input from structure (NumStyles::separator_detect, beside
  // all), where only unambiguous text resolves: `1,000` is Malformed, never guessed. Strict
  // Western grouping, as invariant(); the policy never settles an ambiguous separator.
  static constexpr NumFormat detect() noexcept {
    return NumFormat(".", ",", NumStyles::all | NumStyles::separator_detect, Grouping::western);
  }

  // The leniences, which never carry the grouping policy (grouping() has it).
  NumStyles styles() const noexcept {
    return static_cast<NumStyles>(raw_.format.flags &
                                  ~static_cast<std::uint32_t>(HyperCastNumFormat_GROUPING_POLICY));
  }
  Grouping grouping() const noexcept {
    return static_cast<Grouping>(raw_.format.flags & HyperCastNumFormat_GROUPING_POLICY);
  }
  // This format with another grouping policy, replacing the current one (never combined with
  // it), the styles and every symbol kept. One of Grouping's three values; any other is a
  // contract violation and ends the program with std::abort(), as an undefined date order does.
  constexpr NumFormat with_grouping(Grouping policy) const noexcept {
    const std::uint32_t bits = static_cast<std::uint32_t>(policy);
    if (bits != HyperCastNumFormat_GROUPING_LEGACY && bits != HyperCastNumFormat_GROUPING_WESTERN &&
        bits != HyperCastNumFormat_GROUPING_INDIAN) {
      detail::contract_violation();
    }
    NumFormat copy = *this;
    copy.raw_.format.flags =
        (copy.raw_.format.flags & ~static_cast<std::uint32_t>(HyperCastNumFormat_GROUPING_POLICY)) |
        bits;
    return copy;
  }
  // Each symbol is a view into this NumFormat, so it is taken from a named one: on a temporary
  // it would dangle, and does not compile.
  std::string_view decimal_sep() const & noexcept { return detail::view(raw_.format.decimal_sep); }
  std::string_view decimal_sep() const && = delete;
  std::string_view group_sep() const & noexcept { return detail::view(raw_.format.group_sep); }
  std::string_view group_sep() const && = delete;
  std::string_view currency() const & noexcept { return detail::view(raw_.format.currency); }
  std::string_view currency() const && = delete;
  std::string_view currency_decimal_sep() const & noexcept {
    return detail::view(raw_.format.currency_decimal_sep);
  }
  std::string_view currency_decimal_sep() const && = delete;
  std::string_view currency_group_sep() const & noexcept {
    return detail::view(raw_.format.currency_group_sep);
  }
  std::string_view currency_group_sep() const && = delete;
  std::string_view minus_sign() const & noexcept { return detail::view(raw_.format.minus_sign); }
  std::string_view minus_sign() const && = delete;
  std::string_view plus_sign() const & noexcept { return detail::view(raw_.format.plus_sign); }
  std::string_view plus_sign() const && = delete;
  std::string_view percent() const & noexcept { return detail::view(raw_.format.percent); }
  std::string_view percent() const && = delete;
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
  constexpr NumFormat(std::string_view decimal, std::string_view group, NumStyles leniences,
                      Grouping policy) noexcept
      : NumFormat() {
    raw_.format.flags = static_cast<std::uint32_t>(leniences) | static_cast<std::uint32_t>(policy);
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
  // 16 bytes, an ASCII digit, white space at either end, not UTF-8. Empty declares none.
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
  // Views into this DateFormat, taken from a named one, as NumFormat's symbols are.
  std::string_view am() const & noexcept { return detail::view(raw_.periods.am); }
  std::string_view am() const && = delete;
  std::string_view pm() const & noexcept { return detail::view(raw_.periods.pm); }
  std::string_view pm() const && = delete;

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

// The caller's bytes a declaration packs a list into, as the core reads it: an enum list (16
// bytes a numeric entry, 4 plus its UTF-8 a text entry; Constraints<T>::storage_size says how
// many) or a month vocabulary (5 plus its UTF-8 an entry; MonthNames::storage_size). Nothing is
// allocated, so the storage is the caller's: it must outlive the declaration and every value
// read through it (a text enum match is a view into it), and must not change after the
// declaration. A copy of a Constraints or MonthNames refers to the same storage.
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

// The extra shapes the detailed date doors read, each opt-in: the doors without options keep
// the shapes they always have.
enum class DateStyles : std::uint32_t {
  none = 0,
  compact_date = 1,    // cast_date also reads `yyyyMMdd`, eight ASCII digits
  space_separator = 2, // cast_timestamp also reads a single space where the `T` goes
  hour_offset = 4,     // cast_timestamp also reads an offset of `+hh`, with no minutes
  common_forms = 7     // all three together; never a pivot or a vocabulary
};

constexpr DateStyles operator|(DateStyles a, DateStyles b) noexcept {
  return static_cast<DateStyles>(static_cast<std::uint32_t>(a) | static_cast<std::uint32_t>(b));
}
constexpr DateStyles operator&(DateStyles a, DateStyles b) noexcept {
  return static_cast<DateStyles>(static_cast<std::uint32_t>(a) & static_cast<std::uint32_t>(b));
}

// The options of the detailed date doors: the styles above and a two-digit-year pivot, the
// last year of the 100-year window a two-digit `yy` is read in (the one year in [pivot-99,
// pivot] ending in `yy`; 0 is none, and then a two-digit year is a width fault; 100 to 9999
// otherwise). A default DateOptions is no option. A known option a door does not read is
// ignored; a style bit this header does not name is refused. 12 bytes, the core's
// RawDateOptions. Needs a core as new as this header.
class DateOptions {
public:
  // No option: every door reads exactly the shapes it always has.
  constexpr DateOptions() noexcept : raw_{HyperCastRawDateOptions_SIZE, 0, 0} {}

  static constexpr DateOptions none() noexcept { return DateOptions(); }
  // compact_date, space_separator and hour_offset together, with no pivot.
  static constexpr DateOptions common_forms() noexcept {
    return DateOptions(DateStyles::common_forms, 0);
  }

  // The options these styles and this pivot declare, or std::nullopt for ones the core refuses
  // (a style bit it does not define, a pivot that is neither 0 nor 100 to 9999), or when the
  // core is older than this header. The core is the judge, asked once here.
  static std::optional<DateOptions> declare(DateStyles styles, std::uint32_t pivot = 0) noexcept {
    const DateOptions options(styles, pivot);
    HyperCastDate ignored{};
    if (!core_is_compatible() ||
        ::cast_detailed(detail::door_date, 0, nullptr, nullptr, &options.raw_, nullptr, 0, nullptr,
                        0, &ignored, nullptr) < 0) {
      return std::nullopt;
    }
    return options;
  }

  constexpr DateStyles styles() const noexcept { return static_cast<DateStyles>(raw_.flags); }
  constexpr std::uint32_t pivot() const noexcept { return raw_.pivot; }

  // The layout the core reads, for a library that hands it to a C ABI of its own.
  constexpr const HyperCastRawDateOptions &raw() const noexcept { return raw_; }

  friend constexpr bool operator==(const DateOptions &a, const DateOptions &b) noexcept {
    return a.raw_.size == b.raw_.size && a.raw_.flags == b.raw_.flags &&
           a.raw_.pivot == b.raw_.pivot;
  }
  friend constexpr bool operator!=(const DateOptions &a, const DateOptions &b) noexcept {
    return !(a == b);
  }

private:
  constexpr DateOptions(DateStyles styles, std::uint32_t pivot) noexcept
      : raw_{HyperCastRawDateOptions_SIZE, static_cast<std::uint32_t>(styles), pivot} {}

  HyperCastRawDateOptions raw_;
};

// One entry of a month vocabulary: a month (1 to 12) and a name for it.
struct MonthName {
  std::uint8_t month;
  std::string_view name;
};

// The month names a detailed cast_date(text, order) or cast_datetime reads in the month field,
// Excel style (`07-Jan-26`), in the position the declared order selects, with the repeated
// `/`, `-` or `.` separator. Caller-built: a name is a spelling the caller chose
// (`Sep` and `Sept` are two entries), matched under simple case folding, the longest first
// among those the separator follows. Punctuation is part of a name: with `janv.` declared and
// `.` as the separator, `7.janv..2026` reads and `7.janv.2026` is Malformed.
//
// Each month needs an entry; no name is empty, over 64 bytes, or holds an ASCII digit, `/`, `-`
// or white space (`.` is fine); two names that fold alike are refused. The core judges all of
// it, once, when a vocabulary is declared, and again on every call that uses it. A default
// MonthNames is no vocabulary. Packed into the caller's storage in the core's layout (a month
// byte, a little-endian 32-bit length, the UTF-8, no padding), which must outlive the
// MonthNames and every cast that reads it. Needs a core as new as this header.
class MonthNames {
public:
  // No vocabulary: the month field stays numeric.
  constexpr MonthNames() noexcept : data_(nullptr), size_(0) {}

  // The vocabulary these entries declare, packed into `storage`, or std::nullopt for one the
  // core refuses, for storage smaller than storage_size(entries) or a name past 4,294,967,295
  // bytes, or when the core is older than this header. An empty list is refused: every month needs
  // a name (a default MonthNames is the way to declare none).
  static std::optional<MonthNames> declare(std::initializer_list<MonthName> entries,
                                           EnumStorage storage) noexcept {
    return pack(entries.begin(), entries.size(), storage);
  }
  template <std::size_t N>
  static std::optional<MonthNames> declare(const MonthName (&entries)[N],
                                           EnumStorage storage) noexcept {
    return pack(entries, N, storage);
  }
  static std::optional<MonthNames> declare(const MonthName *entries, std::size_t count,
                                           EnumStorage storage) noexcept {
    return pack(entries, count, storage);
  }

  // A vocabulary already in the packed layout, which the core judges as it does any other; the
  // bytes are borrowed, not copied. std::nullopt when it refuses them, or the list is empty
  // (that is no vocabulary), or the core is older than this header.
  static std::optional<MonthNames> from_packed(const std::uint8_t *bytes,
                                               std::size_t size) noexcept {
    if (bytes == nullptr || size == 0 || !core_is_compatible()) {
      return std::nullopt;
    }
    HyperCastDate ignored{};
    if (::cast_detailed(detail::door_date_ordered, 1, nullptr, nullptr, nullptr, bytes, size,
                        nullptr, 0, &ignored, nullptr) < 0) {
      return std::nullopt;
    }
    MonthNames names;
    names.data_ = bytes;
    names.size_ = size;
    return names;
  }

  // The bytes the entries pack into: 5 plus its UTF-8 for each. Saturates at the largest size_t
  // for a list too big to hold, which no storage fits.
  static constexpr std::size_t storage_size(std::initializer_list<MonthName> entries) noexcept {
    return packed_size(entries.begin(), entries.size());
  }
  template <std::size_t N>
  static constexpr std::size_t storage_size(const MonthName (&entries)[N]) noexcept {
    return packed_size(entries, N);
  }
  static constexpr std::size_t storage_size(const MonthName *entries, std::size_t count) noexcept {
    return packed_size(entries, count);
  }

  // The packed layout, for a library that hands it to a C ABI of its own; empty for none.
  constexpr const std::uint8_t *data() const noexcept { return data_; }
  constexpr std::size_t size() const noexcept { return size_; }
  constexpr bool empty() const noexcept { return size_ == 0; }

private:
  static constexpr std::size_t packed_size(const MonthName *entries, std::size_t count) noexcept {
    std::size_t size = 0;
    for (std::size_t i = 0; i < count; ++i) {
      const std::size_t entry = 5 + entries[i].name.size();
      if (entry < entries[i].name.size() || size + entry < size) {
        return static_cast<std::size_t>(-1);
      }
      size += entry;
    }
    return size;
  }

  static std::optional<MonthNames> pack(const MonthName *entries, std::size_t count,
                                        EnumStorage storage) noexcept {
    if (count == 0 || packed_size(entries, count) > storage.size()) {
      return std::nullopt;
    }
    std::size_t at = 0;
    for (std::size_t i = 0; i < count; ++i) {
      if (static_cast<std::uint64_t>(entries[i].name.size()) > UINT32_MAX) {
        return std::nullopt;
      }
      const std::uint32_t length = static_cast<std::uint32_t>(entries[i].name.size());
      storage.data()[at++] = entries[i].month;
      for (int shift = 0; shift < 32; shift += 8) {
        storage.data()[at++] = static_cast<std::uint8_t>(length >> shift); // little-endian
      }
      for (char c : entries[i].name) {
        storage.data()[at++] = static_cast<std::uint8_t>(c);
      }
    }
    return from_packed(storage.data(), at);
  }

  const std::uint8_t *data_;
  std::size_t size_;
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

// A detailed export's fault buffer: its size word set, the rest for the core to write.
inline HyperCastRawFaultDetail new_fault() noexcept {
  return HyperCastRawFaultDetail{HyperCastRawFaultDetail_SIZE, 0, 0, 0};
}

// What a detailed export reports for a failure: the plain fault's span.
inline HyperCastRawFault plain(const HyperCastRawFaultDetail &fault) noexcept {
  return HyperCastRawFault{fault.offset, fault.len};
}

// cast_detailed, once the core is known to have it: a core older than this header may have no such
// export and reads no grouping policy, so nothing past this point may run against one, and the
// program ends rather than parse a strict request as legacy. -1, a contract violation, cannot come
// from a format or option the types declared; it ends the program too.
template <class Out>
inline std::int32_t detailed_text(std::uint32_t door, std::uint32_t param,
                                  const HyperCastRawNumFormat *format,
                                  const HyperCastRawDayPeriods *periods,
                                  const HyperCastRawDateOptions *options, const MonthNames &months,
                                  Text text, Out *out, HyperCastRawFaultDetail *fault) noexcept {
  if (!core_is_compatible()) {
    stale_core();
  }
  const std::int32_t code = ::cast_detailed(door, param, format, periods, options, months.data(),
                                            months.size(), text.data(), text.size(), out, fault);
  if (code < 0) {
    contract_violation();
  }
  return code;
}

template <class T, class Door>
inline Verdict<T> numeric(Door door, std::uint32_t code, Text text,
                          const NumFormat &format) noexcept {
  T out{};
  if (format.grouping() == Grouping::legacy) {
    HyperCastRawFault fault{};
    return finish(door(text.data(), text.size(), &format.raw(), &out, &fault), out, fault);
  }
  // The 1.0 exports read a format as legacy whatever its policy says: strict goes through the
  // detailed export, and its detail is dropped.
  HyperCastRawFaultDetail fault = new_fault();
  const std::int32_t result =
      detailed_text(code, 0, &format.raw(), nullptr, nullptr, MonthNames(), text, &out, &fault);
  return finish(result, out, plain(fault));
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
  return detail::numeric<std::int8_t>(::cast_i8, detail::door_i8, text, format);
}
inline Verdict<std::int16_t> cast_i16(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::int16_t>(::cast_i16, detail::door_i16, text, format);
}
inline Verdict<std::int32_t> cast_i32(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::int32_t>(::cast_i32, detail::door_i32, text, format);
}
inline Verdict<std::int64_t> cast_i64(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::int64_t>(::cast_i64, detail::door_i64, text, format);
}
inline Verdict<std::uint8_t> cast_u8(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::uint8_t>(::cast_u8, detail::door_u8, text, format);
}
inline Verdict<std::uint16_t> cast_u16(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::uint16_t>(::cast_u16, detail::door_u16, text, format);
}
inline Verdict<std::uint32_t> cast_u32(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::uint32_t>(::cast_u32, detail::door_u32, text, format);
}
inline Verdict<std::uint64_t> cast_u64(Text text, const NumFormat &format) noexcept {
  return detail::numeric<std::uint64_t>(::cast_u64, detail::door_u64, text, format);
}

// Reals under a declared format; a magnitude beyond the type's finite range is OutOfRange,
// and NaN and the infinities are never read.
inline Verdict<float> cast_f32(Text text, const NumFormat &format) noexcept {
  return detail::numeric<float>(::cast_f32, detail::door_f32, text, format);
}
inline Verdict<double> cast_f64(Text text, const NumFormat &format) noexcept {
  return detail::numeric<double>(::cast_f64, detail::door_f64, text, format);
}

// An exact, canonical decimal under a declared format. Never rounds: a magnitude past 96 bits
// or more than 28 significant places is OutOfRange.
inline Verdict<Decimal> cast_decimal(Text text, const NumFormat &format) noexcept {
  HyperCastDecimal out{};
  if (format.grouping() == Grouping::legacy) {
    HyperCastRawFault fault{};
    return detail::converted<Decimal>(
        ::cast_decimal(text.data(), text.size(), &format.raw(), &out, &fault), out, fault,
        detail::decimal);
  }
  HyperCastRawFaultDetail fault = detail::new_fault();
  const std::int32_t code = detail::detailed_text(detail::door_decimal, 0, &format.raw(), nullptr,
                                                  nullptr, MonthNames(), text, &out, &fault);
  return detail::converted<Decimal>(code, out, detail::plain(fault), detail::decimal);
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

// A refusal's message, held by value, as a KeywordName is: one fixed sentence per code, worded as
// every binding words it. Compare it with a string, print c_str(), or take view() from a named
// RefusalMessage (not a temporary, which would dangle; view() on one does not compile).
class RefusalMessage {
public:
  constexpr std::string_view view() const & noexcept { return std::string_view(text_, size_); }
  std::string_view view() const && = delete;
  constexpr const char *c_str() const noexcept { return text_; }
  constexpr std::size_t size() const noexcept { return size_; }

  friend constexpr bool operator==(const RefusalMessage &a, std::string_view b) noexcept {
    return a.view() == b;
  }
  friend constexpr bool operator==(std::string_view a, const RefusalMessage &b) noexcept {
    return a == b.view();
  }
  friend constexpr bool operator!=(const RefusalMessage &a, std::string_view b) noexcept {
    return !(a == b);
  }
  friend constexpr bool operator!=(std::string_view a, const RefusalMessage &b) noexcept {
    return !(a == b);
  }

private:
  friend struct Refusal;
  constexpr RefusalMessage() noexcept = default;
  constexpr void append(std::string_view text) noexcept {
    for (char c : text) {
      text_[size_++] = c;
    }
    text_[size_] = '\0';
  }
  // The longest message, code 0's, is 137 bytes; the C++ smoke test pins every code's, where
  // one that did not fit would not compile.
  char text_[160] = {};
  std::size_t size_ = 0;
};

// Why a declaration was refused: the core's refusal code, 1 to 13, with one fixed message per
// code worded as every binding words it, or 0, this header's own refusal of an enum list that
// does not fit (EnumStorage smaller than storage_size(entries), or a text entry past
// 4,294,967,295 bytes), which the core never sees. A code this header does not name (one a
// later core adds) keeps its number in its message. A refused declaration is a caller bug, not
// data.
struct Refusal {
  std::int32_t code;

  // Explicit, so a bare integer is never taken for a Refusal (refusal == 14 does not compile).
  constexpr explicit Refusal(std::int32_t refused) noexcept : code(refused) {}

  // The message, by value, so it outlives this Refusal and the Declared that held it:
  // `auto m = C::declare(f).refusal().message();` is safe to keep.
  constexpr RefusalMessage message() const noexcept {
    RefusalMessage out;
    switch (code) {
    case 0:
      out.append(
          "The enum list does not fit: the EnumStorage is smaller than storage_size(entries), "
          "or a text entry is longer than 4294967295 UTF-8 bytes.");
      return out;
    case 1:
      out.append("Constraints refused (code 1): a constraint the door does not take.");
      return out;
    case 2:
      out.append("Constraints refused (code 2): minimum together with exclusiveMinimum.");
      return out;
    case 3:
      out.append("Constraints refused (code 3): maximum together with exclusiveMaximum.");
      return out;
    case 4:
      out.append("Constraints refused (code 4): the lower bound is not below the upper bound.");
      return out;
    case 5:
      out.append("Constraints refused (code 5): multipleOf is not positive.");
      return out;
    case 6:
      out.append("Constraints refused (code 6): a rounding without multipleOf.");
      return out;
    case 7:
      out.append("Constraints refused (code 7): minLength is above maxLength.");
      return out;
    case 8:
      out.append("Constraints refused (code 8): a bound, step or enum entry the door cannot hold.");
      return out;
    case 9:
      out.append("Constraints refused (code 9): an enum entry that is not UTF-8.");
      return out;
    case 10:
      out.append("Constraints refused (code 10): a duplicate enum entry.");
      return out;
    case 11:
      out.append("Constraints refused (code 11): a malformed constraints layout.");
      return out;
    case 12:
      out.append("Constraints refused (code 12): an enum with no entries.");
      return out;
    case 13:
      out.append("Constraints refused (code 13): a text enum entry no input can match.");
      return out;
    default:
      break;
    }
    // A code this header does not name (a core newer than this header), with its number.
    out.append("Constraints refused (code ");
    std::uint32_t magnitude = static_cast<std::uint32_t>(code);
    if (code < 0) {
      out.append("-");
      magnitude = 0u - magnitude;
    }
    char digits[10] = {};
    std::size_t count = 0;
    do {
      digits[count++] = static_cast<char>('0' + magnitude % 10);
      magnitude /= 10;
    } while (magnitude != 0);
    while (count > 0) {
      const char digit[1] = {digits[--count]};
      out.append(std::string_view(digit, 1));
    }
    out.append("): a rule this binding does not name.");
    return out;
  }

  friend constexpr bool operator==(const Refusal &a, const Refusal &b) noexcept {
    return a.code == b.code;
  }
  friend constexpr bool operator!=(const Refusal &a, const Refusal &b) noexcept {
    return !(a == b);
  }
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
  HyperCastRawCheck check{};
  if (format != nullptr && format->grouping() != Grouping::legacy) {
    // The 1.0 export reads a format as legacy whatever its policy says: strict goes through the
    // detailed export, and its detail is dropped.
    HyperCastRawFaultDetail fault = new_fault();
    if (!core_is_compatible()) {
      stale_core();
    }
    const std::int32_t code = ::cast_checked_detailed(
        Constraints<T>::door, 0, &format->raw(), &constraints.raw(), constraints.enum_data(),
        constraints.enum_size(), text.data(), text.size(), &out, &fault, &check);
    return judged<T>(code, out, plain(fault), check, convert);
  }
  HyperCastRawFault fault{};
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
// door makes of that text. -0.0 is zero. NaN is Malformed; an infinity is OutOfRange at a float
// or decimal door and Malformed at an integer door. With no text to index, every Fault and
// Violation has an empty span at 0.
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

// ---------------------------------------------------------------------------------------------
// Detailed doors: every door above, reporting a Failure (the Fault, and what failed in it)
// instead of a Fault, through the core's detailed exports. They honour a format's grouping
// policy, and the date doors take a DateOptions and a MonthNames; a door without a
// declaration reads exactly the shapes its plain twin does, and a Failure's fault() is the
// plain door's Fault. All of it needs a core as new as this header (core_is_compatible()): against
// an older one a detailed door ends the program, as a strict format does at a plain door.

// The failure case of a detailed checked door: the door's own Failure (it did not parse), or a
// Violation (it parsed and broke a declared rule; a Violation has no detail). Exactly one; no
// default constructor. Accessors as Rejection's, over those cases.
class DetailedRejection {
public:
  constexpr DetailedRejection(Failure failure) noexcept
      : failure_(failure), violation_{}, is_failure_(true) {}
  constexpr DetailedRejection(Violation violation) noexcept
      : failure_{}, violation_(violation), is_failure_(false) {}

  constexpr bool is_failure() const noexcept { return is_failure_; }

  constexpr const Failure *failure_if() const noexcept { return is_failure_ ? &failure_ : nullptr; }
  constexpr const Violation *violation_if() const noexcept {
    return is_failure_ ? nullptr : &violation_;
  }

  // Precondition: is_failure(). Otherwise a value-initialized Failure.
  constexpr const Failure &failure() const noexcept { return failure_; }
  // Precondition: !is_failure(). Otherwise a value-initialized Violation.
  constexpr const Violation &violation() const noexcept { return violation_; }

  // The plain Rejection: the Fault of the Failure, or the Violation.
  constexpr Rejection to_rejection() const noexcept {
    if (is_failure_) {
      return Rejection(failure_.fault());
    }
    return Rejection(violation_);
  }

  template <class OnFailure, class OnViolation>
  constexpr std::invoke_result_t<OnFailure &&, const Failure &>
  match(OnFailure &&on_failure, OnViolation &&on_violation) const
      noexcept(std::is_nothrow_invocable_v<OnFailure &&, const Failure &> &&
               std::is_nothrow_invocable_v<OnViolation &&, const Violation &>) {
    static_assert(std::is_same_v<std::invoke_result_t<OnFailure &&, const Failure &>,
                                 std::invoke_result_t<OnViolation &&, const Violation &>>,
                  "match's two cases must return the same type");
    if (is_failure_) {
      return std::forward<OnFailure>(on_failure)(failure_);
    }
    return std::forward<OnViolation>(on_violation)(violation_);
  }

  template <class Visitor>
  constexpr std::invoke_result_t<Visitor &&, const Failure &> visit(Visitor &&visitor) const
      noexcept(std::is_nothrow_invocable_v<Visitor &&, const Failure &> &&
               std::is_nothrow_invocable_v<Visitor &&, const Violation &>) {
    static_assert(std::is_same_v<std::invoke_result_t<Visitor &&, const Failure &>,
                                 std::invoke_result_t<Visitor &&, const Violation &>>,
                  "visit's two cases must return the same type");
    if (is_failure_) {
      return std::forward<Visitor>(visitor)(failure_);
    }
    return std::forward<Visitor>(visitor)(violation_);
  }

  constexpr std::variant<Failure, Violation> to_variant() const noexcept {
    if (is_failure_) {
      return std::variant<Failure, Violation>(std::in_place_index<0>, failure_);
    }
    return std::variant<Failure, Violation>(std::in_place_index<1>, violation_);
  }

  friend constexpr bool operator==(const DetailedRejection &a,
                                   const DetailedRejection &b) noexcept {
    return a.is_failure_ == b.is_failure_ &&
           (a.is_failure_ ? a.failure_ == b.failure_ : a.violation_ == b.violation_);
  }
  friend constexpr bool operator!=(const DetailedRejection &a,
                                   const DetailedRejection &b) noexcept {
    return !(a == b);
  }

private:
  Failure failure_;
  Violation violation_;
  bool is_failure_;
};

// The outcome of a detailed checked cast: a CheckedValue or a DetailedRejection, two cases and
// nothing else. The same accessors as Checked, over those cases, and to_checked() for the plain
// Checked the same text gives. Under C++23 with <expected> it converts to
// std::expected<CheckedValue<T>, DetailedRejection>. Deliberately no operator bool.
template <class T> class [[nodiscard]] DetailedChecked {
  static_assert(std::is_trivially_copyable_v<T> && std::is_default_constructible_v<T>,
                "a checked verdict holds a plain value");

public:
  using value_type = T;

  constexpr DetailedChecked(CheckedValue<T> success) noexcept
      : value_(success), rejection_(Failure{}), ok_(true) {}
  constexpr DetailedChecked(DetailedRejection failure) noexcept
      : value_{}, rejection_(failure), ok_(false) {}

  constexpr bool ok() const noexcept { return ok_; }

  constexpr const CheckedValue<T> *value_if() const noexcept { return ok_ ? &value_ : nullptr; }
  constexpr const DetailedRejection *rejection_if() const noexcept {
    return ok_ ? nullptr : &rejection_;
  }

  // Precondition: ok(). Otherwise a value-initialized CheckedValue.
  constexpr const CheckedValue<T> &value() const noexcept { return value_; }
  // Precondition: !ok(). Otherwise a rejection holding a value-initialized Failure.
  constexpr const DetailedRejection &rejection() const noexcept { return rejection_; }

  constexpr T value_or(T fallback) const noexcept { return ok_ ? value_.value : fallback; }

  // The plain Checked: the Fault of a Failure, the Violation as it is.
  constexpr Checked<T> to_checked() const noexcept {
    if (ok_) {
      return Checked<T>(value_);
    }
    return Checked<T>(rejection_.to_rejection());
  }

  template <class OnValue, class OnRejection>
  constexpr std::invoke_result_t<OnValue &&, const CheckedValue<T> &>
  match(OnValue &&on_value, OnRejection &&on_rejection) const
      noexcept(std::is_nothrow_invocable_v<OnValue &&, const CheckedValue<T> &> &&
               std::is_nothrow_invocable_v<OnRejection &&, const DetailedRejection &>) {
    static_assert(std::is_same_v<std::invoke_result_t<OnValue &&, const CheckedValue<T> &>,
                                 std::invoke_result_t<OnRejection &&, const DetailedRejection &>>,
                  "match's two cases must return the same type");
    if (ok_) {
      return std::forward<OnValue>(on_value)(value_);
    }
    return std::forward<OnRejection>(on_rejection)(rejection_);
  }

  template <class Visitor>
  constexpr std::invoke_result_t<Visitor &&, const CheckedValue<T> &> visit(Visitor &&visitor) const
      noexcept(std::is_nothrow_invocable_v<Visitor &&, const CheckedValue<T> &> &&
               std::is_nothrow_invocable_v<Visitor &&, const DetailedRejection &>) {
    static_assert(std::is_same_v<std::invoke_result_t<Visitor &&, const CheckedValue<T> &>,
                                 std::invoke_result_t<Visitor &&, const DetailedRejection &>>,
                  "visit's two cases must return the same type");
    if (ok_) {
      return std::forward<Visitor>(visitor)(value_);
    }
    return std::forward<Visitor>(visitor)(rejection_);
  }

  constexpr std::variant<CheckedValue<T>, DetailedRejection> to_variant() const noexcept {
    if (ok_) {
      return std::variant<CheckedValue<T>, DetailedRejection>(std::in_place_index<0>, value_);
    }
    return std::variant<CheckedValue<T>, DetailedRejection>(std::in_place_index<1>, rejection_);
  }

#ifdef HYPERCAST_HAS_EXPECTED
  constexpr std::expected<CheckedValue<T>, DetailedRejection> to_expected() const noexcept {
    if (ok_) {
      return std::expected<CheckedValue<T>, DetailedRejection>(std::in_place, value_);
    }
    return std::expected<CheckedValue<T>, DetailedRejection>(std::unexpect, rejection_);
  }
  constexpr operator std::expected<CheckedValue<T>, DetailedRejection>() const noexcept {
    return to_expected();
  }
#endif

  friend constexpr bool operator==(const DetailedChecked &a, const DetailedChecked &b) noexcept {
    return a.ok_ == b.ok_ && (a.ok_ ? a.value_ == b.value_ : a.rejection_ == b.rejection_);
  }
  friend constexpr bool operator!=(const DetailedChecked &a, const DetailedChecked &b) noexcept {
    return !(a == b);
  }

private:
  CheckedValue<T> value_;
  DetailedRejection rejection_;
  bool ok_;
};

// A detailed checked verdict presented optionally: a blank failure becomes std::nullopt; a
// Violation, every other failure and every value come through as they are.
template <class T>
constexpr std::optional<DetailedChecked<T>> optional(const DetailedChecked<T> &checked) noexcept {
  if (const DetailedRejection *rejection = checked.rejection_if()) {
    if (const Failure *failure = rejection->failure_if();
        failure != nullptr && failure->reason == CastFailure::empty) {
      return std::nullopt;
    }
  }
  return checked;
}

namespace detail {
// The verdict a detailed export's return code names: 0 the value, 1 to 3 a failure. A -1 never
// reaches here (detailed_text ends the program on one first).
template <class T, class Raw, class Convert>
inline DetailedVerdict<T> detailed_finish(std::int32_t code, const Raw &out,
                                          const HyperCastRawFaultDetail &fault,
                                          Convert convert) noexcept {
  if (code == 0) {
    return DetailedVerdict<T>(convert(out));
  }
  if (code < static_cast<std::int32_t>(CastFailure::empty) ||
      code > static_cast<std::int32_t>(CastFailure::out_of_range)) {
    contract_violation();
  }
  return DetailedVerdict<T>(Failure{static_cast<CastFailure>(code),
                                    static_cast<Detail>(fault.detail), fault.offset, fault.len});
}

struct Identity {
  template <class V> constexpr V operator()(const V &value) const noexcept { return value; }
};

// A text door by its code: `Raw` is what the export writes, `convert` makes the value of it.
template <class T, class Raw = T, class Convert = Identity>
inline DetailedVerdict<T>
detailed_door(std::uint32_t door, std::uint32_t param, const HyperCastRawNumFormat *format,
              const HyperCastRawDayPeriods *periods, const HyperCastRawDateOptions *options,
              const MonthNames &months, Text text, Convert convert = Convert()) noexcept {
  Raw out{};
  HyperCastRawFaultDetail fault = new_fault();
  const std::int32_t code =
      detailed_text(door, param, format, periods, options, months, text, &out, &fault);
  return detailed_finish<T>(code, out, fault, convert);
}

// A typed door by its code: a double in.
template <class T, class Raw = T, class Convert = Identity>
inline DetailedVerdict<T> detailed_typed(std::uint32_t door, std::uint32_t param, double value,
                                         Convert convert = Convert()) noexcept {
  if (!core_is_compatible()) {
    stale_core();
  }
  Raw out{};
  HyperCastRawFaultDetail fault = new_fault();
  const std::int32_t code = ::cast_detailed_from_f64(door, param, value, &out, &fault);
  if (code < 0) {
    contract_violation();
  }
  return detailed_finish<T>(code, out, fault, convert);
}

inline std::chrono::nanoseconds time_of(std::uint64_t nanos) noexcept {
  return std::chrono::nanoseconds(static_cast<std::int64_t>(nanos));
}

// The checked verdict a detailed checked export's return code names: 0 the value, 1 to 3 a
// failure, 4 a violation.
template <class T, class Raw, class Convert>
inline DetailedChecked<T>
detailed_judged(std::int32_t code, const Raw &out, const HyperCastRawFaultDetail &fault,
                const HyperCastRawCheck &check, Convert convert) noexcept {
  switch (code) {
  case 0: {
    std::optional<std::uint32_t> index;
    if (check.enum_index != UINT32_MAX) {
      index = check.enum_index;
    }
    return DetailedChecked<T>(CheckedValue<T>{convert(out, index), check.adjusted != 0, index});
  }
  case 1:
  case 2:
  case 3:
    return DetailedChecked<T>(
        DetailedRejection(Failure{static_cast<CastFailure>(code), static_cast<Detail>(fault.detail),
                                  fault.offset, fault.len}));
  case 4:
    return DetailedChecked<T>(
        DetailedRejection(Violation{static_cast<Keyword>(check.keyword), fault.offset, fault.len}));
  default:
    contract_violation();
  }
}

template <class T, class Raw = T, class Convert>
inline DetailedChecked<T> detailed_checked(const Constraints<T> &constraints,
                                           const NumFormat *format, Text text,
                                           Convert convert) noexcept {
  if (!core_is_compatible()) {
    stale_core();
  }
  Raw out{};
  HyperCastRawFaultDetail fault = new_fault();
  HyperCastRawCheck check{};
  const std::int32_t code =
      ::cast_checked_detailed(Constraints<T>::door, 0, format != nullptr ? &format->raw() : nullptr,
                              &constraints.raw(), constraints.enum_data(), constraints.enum_size(),
                              text.data(), text.size(), &out, &fault, &check);
  return detailed_judged<T>(code, out, fault, check, convert);
}

template <class T, class Raw = T, class Convert>
inline DetailedChecked<T> detailed_checked_from_double(const Constraints<T> &constraints,
                                                       double value, Convert convert) noexcept {
  if (!core_is_compatible()) {
    stale_core();
  }
  Raw out{};
  HyperCastRawFaultDetail fault = new_fault();
  HyperCastRawCheck check{};
  const std::int32_t code = ::cast_checked_detailed_from_f64(
      Constraints<T>::door, 0, &constraints.raw(), constraints.enum_data(), constraints.enum_size(),
      value, &out, &fault, &check);
  return detailed_judged<T>(code, out, fault, check, convert);
}
} // namespace detail

namespace detailed {

// The plain doors, each returning a DetailedVerdict. Names and arguments mirror the plain
// doors'; the date doors also take a DateOptions and a MonthNames, none by default.

inline DetailedVerdict<bool> cast_bool(Text text) noexcept {
  return detail::detailed_door<bool, std::uint8_t>(detail::door_bool, 0, nullptr, nullptr, nullptr,
                                                   MonthNames(), text,
                                                   [](std::uint8_t v) { return v != 0; });
}

inline DetailedVerdict<char32_t> cast_char(Text text) noexcept {
  return detail::detailed_door<char32_t, std::uint32_t>(
      detail::door_char, 0, nullptr, nullptr, nullptr, MonthNames(), text,
      [](std::uint32_t v) { return static_cast<char32_t>(v); });
}

#define HYPERCAST_DETAILED_NUMERIC(Type, name, door)                                               \
  inline DetailedVerdict<Type> name(Text text, const NumFormat &format) noexcept {                 \
    return detail::detailed_door<Type>(door, 0, &format.raw(), nullptr, nullptr, MonthNames(),     \
                                       text);                                                      \
  }
HYPERCAST_DETAILED_NUMERIC(std::int8_t, cast_i8, detail::door_i8)
HYPERCAST_DETAILED_NUMERIC(std::int16_t, cast_i16, detail::door_i16)
HYPERCAST_DETAILED_NUMERIC(std::int32_t, cast_i32, detail::door_i32)
HYPERCAST_DETAILED_NUMERIC(std::int64_t, cast_i64, detail::door_i64)
HYPERCAST_DETAILED_NUMERIC(std::uint8_t, cast_u8, detail::door_u8)
HYPERCAST_DETAILED_NUMERIC(std::uint16_t, cast_u16, detail::door_u16)
HYPERCAST_DETAILED_NUMERIC(std::uint32_t, cast_u32, detail::door_u32)
HYPERCAST_DETAILED_NUMERIC(std::uint64_t, cast_u64, detail::door_u64)
HYPERCAST_DETAILED_NUMERIC(float, cast_f32, detail::door_f32)
HYPERCAST_DETAILED_NUMERIC(double, cast_f64, detail::door_f64)
#undef HYPERCAST_DETAILED_NUMERIC

inline DetailedVerdict<Decimal> cast_decimal(Text text, const NumFormat &format) noexcept {
  return detail::detailed_door<Decimal, HyperCastDecimal>(detail::door_decimal, 0, &format.raw(),
                                                          nullptr, nullptr, MonthNames(), text,
                                                          detail::decimal);
}

inline DetailedVerdict<Uuid> cast_uuid(Text text) noexcept {
  return detail::detailed_door<Uuid>(detail::door_uuid, 0, nullptr, nullptr, nullptr, MonthNames(),
                                     text);
}

// An RFC 3339 instant; `options` may open a space for the `T` and `+hh` offsets.
inline DetailedVerdict<Timestamp>
cast_timestamp(Text text, const DateOptions &options = DateOptions()) noexcept {
  return detail::detailed_door<Timestamp, HyperCastTimestamp>(detail::door_timestamp, 0, nullptr,
                                                              nullptr, &options.raw(), MonthNames(),
                                                              text, detail::timestamp);
}

inline DetailedVerdict<Timestamp> cast_unix(Text text, UnixPrecision precision) noexcept {
  return detail::detailed_door<Timestamp, HyperCastTimestamp>(
      detail::door_unix, static_cast<std::uint32_t>(precision), nullptr, nullptr, nullptr,
      MonthNames(), text, detail::timestamp);
}

inline DetailedVerdict<Timestamp> cast_excel_serial(Text text, ExcelEpoch epoch) noexcept {
  return detail::detailed_door<Timestamp, HyperCastTimestamp>(
      detail::door_excel_serial, static_cast<std::uint32_t>(epoch), nullptr, nullptr, nullptr,
      MonthNames(), text, detail::timestamp);
}

// A strict yyyy-MM-dd date; `options` may open `yyyyMMdd`.
inline DetailedVerdict<Date> cast_date(Text text,
                                       const DateOptions &options = DateOptions()) noexcept {
  return detail::detailed_door<Date, HyperCastDate>(
      detail::door_date, 0, nullptr, nullptr, &options.raw(), MonthNames(), text, detail::date);
}

// A separated date in the declared field order; `options` may add a pivot for a two-digit year,
// and `months` names for the month field.
inline DetailedVerdict<Date> cast_date(Text text, DateOrder order,
                                       const DateOptions &options = DateOptions(),
                                       const MonthNames &months = MonthNames()) noexcept {
  return detail::detailed_door<Date, HyperCastDate>(
      detail::door_date_ordered, static_cast<std::uint32_t>(order), nullptr, nullptr,
      &options.raw(), months, text, detail::date);
}

// A zone-less date and time in the declared order, with the options and names of the date.
inline DetailedVerdict<CivilDateTime>
cast_datetime(Text text, DateOrder order, const DateOptions &options = DateOptions(),
              const MonthNames &months = MonthNames()) noexcept {
  return detail::detailed_door<CivilDateTime, HyperCastCivilDateTime>(
      detail::door_datetime, static_cast<std::uint32_t>(order), nullptr, nullptr, &options.raw(),
      months, text, detail::civil);
}

// The same under a declared DateFormat's order and day-period designators.
inline DetailedVerdict<CivilDateTime>
cast_datetime(Text text, const DateFormat &format, const DateOptions &options = DateOptions(),
              const MonthNames &months = MonthNames()) noexcept {
  return detail::detailed_door<CivilDateTime, HyperCastCivilDateTime>(
      detail::door_datetime, static_cast<std::uint32_t>(format.order()), nullptr, &format.raw(),
      &options.raw(), months, text, detail::civil);
}

inline DetailedVerdict<std::chrono::nanoseconds> cast_time(Text text) noexcept {
  return detail::detailed_door<std::chrono::nanoseconds, std::uint64_t>(
      detail::door_time, 0, nullptr, nullptr, nullptr, MonthNames(), text, detail::time_of);
}

inline DetailedVerdict<Duration> cast_duration(Text text) noexcept {
  return detail::detailed_door<Duration, HyperCastDuration>(
      detail::door_duration, 0, nullptr, nullptr, nullptr, MonthNames(), text, detail::duration);
}

// The text door: the input as a view of it, once it is valid UTF-8.
inline DetailedVerdict<std::string_view> cast_text(Text text) noexcept {
  return detail::detailed_door<std::string_view, HyperCastRawSpan>(
      detail::door_text, 0, nullptr, nullptr, nullptr, MonthNames(), text,
      [&](const HyperCastRawSpan &span) {
        return std::string_view(reinterpret_cast<const char *>(text.data()) + span.offset,
                                span.len);
      });
}

// The typed doors: a double in, and an empty span on a failure.
inline DetailedVerdict<Decimal> cast_decimal_from_double(double value) noexcept {
  return detail::detailed_typed<Decimal, HyperCastDecimal>(detail::door_decimal, 0, value,
                                                           detail::decimal);
}

inline DetailedVerdict<CivilDateTime> cast_excel_serial_from_double(double value,
                                                                    ExcelEpoch epoch) noexcept {
  return detail::detailed_typed<CivilDateTime, HyperCastCivilDateTime>(
      detail::door_excel_serial, static_cast<std::uint32_t>(epoch), value, detail::civil);
}

inline DetailedVerdict<std::chrono::nanoseconds> cast_excel_time(double value) noexcept {
  return detail::detailed_typed<std::chrono::nanoseconds, std::uint64_t>(detail::door_time, 0,
                                                                         value, detail::time_of);
}

inline DetailedVerdict<Duration> cast_excel_duration(double value) noexcept {
  return detail::detailed_typed<Duration, HyperCastDuration>(detail::door_duration, 0, value,
                                                             detail::duration);
}

// The checked doors: the detailed parse under the declared format, then the value judged by the
// constraints. A DetailedRejection's Failure carries the detail; a Violation has none.
#define HYPERCAST_DETAILED_CHECKED(Type, name)                                                     \
  inline DetailedChecked<Type> name(Text text, const NumFormat &format,                            \
                                    const Constraints<Type> &constraints) noexcept {               \
    return detail::detailed_checked<Type>(constraints, &format, text, detail::Same{});             \
  }                                                                                                \
  inline DetailedChecked<Type> name##_from_double(double value,                                    \
                                                  const Constraints<Type> &constraints) noexcept { \
    return detail::detailed_checked_from_double<Type>(constraints, value, detail::Same{});         \
  }
HYPERCAST_DETAILED_CHECKED(std::int8_t, cast_i8)
HYPERCAST_DETAILED_CHECKED(std::int16_t, cast_i16)
HYPERCAST_DETAILED_CHECKED(std::int32_t, cast_i32)
HYPERCAST_DETAILED_CHECKED(std::int64_t, cast_i64)
HYPERCAST_DETAILED_CHECKED(std::uint8_t, cast_u8)
HYPERCAST_DETAILED_CHECKED(std::uint16_t, cast_u16)
HYPERCAST_DETAILED_CHECKED(std::uint32_t, cast_u32)
HYPERCAST_DETAILED_CHECKED(std::uint64_t, cast_u64)
HYPERCAST_DETAILED_CHECKED(float, cast_f32)
HYPERCAST_DETAILED_CHECKED(double, cast_f64)
#undef HYPERCAST_DETAILED_CHECKED

inline DetailedChecked<Decimal> cast_decimal(Text text, const NumFormat &format,
                                             const Constraints<Decimal> &constraints) noexcept {
  return detail::detailed_checked<Decimal, HyperCastDecimal>(constraints, &format, text,
                                                             detail::AsDecimal{});
}
inline DetailedChecked<Decimal>
cast_decimal_from_double(double value, const Constraints<Decimal> &constraints) noexcept {
  return detail::detailed_checked_from_double<Decimal, HyperCastDecimal>(constraints, value,
                                                                         detail::AsDecimal{});
}

// The checked text door: the input, trimmed when the constraints say so, judged by its lengths
// and its enum; the value a view of the input, or on an enum match the declared entry.
inline DetailedChecked<std::string_view>
cast_text(Text text, const Constraints<std::string_view> &constraints) noexcept {
  return detail::detailed_checked<std::string_view, HyperCastRawSpan>(
      constraints, nullptr, text,
      [&](const HyperCastRawSpan &span, std::optional<std::uint32_t> index) {
        if (index.has_value()) {
          return constraints.entry(*index);
        }
        return std::string_view(reinterpret_cast<const char *>(text.data()) + span.offset,
                                span.len);
      });
}

} // namespace detailed

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
