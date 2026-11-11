// The C++ header against an unpacked release archive: every door through hypercast.hpp, each
// verdict taken apart the ways the header offers, and the version printed beside main.c's.
// This is also the header's compatibility test: it is the code as it was written before 1.1
// (constructing a Fault, a Verdict and a Rejection, reading them by equality and by variant,
// declaring formats by the positional Fields), and it still compiles and passes unchanged.
// detailed.cpp holds what 1.1 added.
// CMakeLists.txt builds it as C++17, C++20 and, where the compiler has it, C++23, warnings
// as errors, and once more with exceptions off.

#include <cstdio>
#include <string>
#include <string_view>
#include <type_traits>
#include <variant>

#include "hypercast.hpp"

#define CHECK(condition)                                                                           \
  do {                                                                                             \
    if (!(condition)) {                                                                            \
      std::fprintf(stderr, "smoke: %s:%d: failed: %s\n", __FILE__, __LINE__, #condition);          \
      return 1;                                                                                    \
    }                                                                                              \
  } while (0)

namespace hc = hypercast;

// Every door, and everything a verdict offers, is noexcept.
static_assert(noexcept(hc::cast_i32("1", hc::NumFormat::invariant())), "doors are noexcept");
static_assert(noexcept(hc::NumFormat::declare({})), "declaring a format is noexcept");
static_assert(noexcept(hc::cast_i32("1", hc::NumFormat::invariant()).value_if()),
              "access is noexcept");
static_assert(!std::is_default_constructible_v<hc::Verdict<int>>, "a verdict is one of its cases");
static_assert(!std::is_constructible_v<bool, hc::Verdict<bool>>, "a verdict is not a bool");
static_assert(hc::is_available(), "the core is linked in");
static_assert(noexcept(hc::Constraints<double>::declare({})), "declaring constraints is noexcept");
static_assert(noexcept(hc::cast_text("a")), "the text door is noexcept");
static_assert(!std::is_default_constructible_v<hc::Checked<double>>,
              "a checked verdict is one case");
static_assert(!std::is_constructible_v<bool, hc::Checked<bool>>, "a checked verdict is not a bool");
static_assert(!std::is_default_constructible_v<hc::Constraints<double>>,
              "constraints come from declare alone");
static_assert(!std::is_constructible_v<hc::ConstraintValue, double>, "never a binary float");
static_assert(hc::name(hc::Keyword::multiple_of) == "multipleOf" &&
                  hc::name(static_cast<hc::Keyword>(10)) == "keyword 10",
              "keywords are named the OpenAPI way, and stay open");
// A declaration equals one that declares the same thing, however it was built, as the core's ==
// holds. MonthNames has no ==: a view over the caller's storage, as the core's borrowed one.
static_assert(hc::NumFormat::invariant()
                          .with_grouping(hc::Grouping::indian)
                          .with_grouping(hc::Grouping::western) == hc::NumFormat::invariant() &&
                  hc::NumFormat::invariant().with_grouping(hc::Grouping::indian) !=
                      hc::NumFormat::invariant() &&
                  hc::NumFormat::detect() != hc::NumFormat::invariant() &&
                  hc::DateFormat(hc::DateOrder::day_month_year) ==
                      hc::DateFormat(hc::DateOrder::day_month_year) &&
                  hc::DateFormat(hc::DateOrder::day_month_year) !=
                      hc::DateFormat(hc::DateOrder::month_day_year) &&
                  hc::DateOptions::common_forms() != hc::DateOptions::none(),
              "declarations compare by what they declare");

// What a decimal displays as, through to_chars.
static std::string text(const hc::Decimal &decimal) {
  char buffer[hc::Decimal::max_chars];
  const auto written = decimal.to_chars(buffer, buffer + sizeof buffer);
  return std::string(buffer, written.ptr);
}

// The overload set std::visit and Verdict::visit take.
template <class... F> struct overloaded : F... {
  using F::operator()...;
};
template <class... F> overloaded(F...) -> overloaded<F...>;

int main() {
  const auto invariant = hc::NumFormat::invariant();

  // The verdict, every way in.
  auto i32 = hc::cast_i32("(1,234)", invariant);
  CHECK(i32.ok() && i32.value() == -1234 && i32.fault_if() == nullptr);
  if (const auto *value = i32.value_if()) {
    CHECK(*value == -1234);
  } else {
    CHECK(false);
  }
  CHECK(i32 == hc::Verdict<std::int32_t>(-1234));
  CHECK(i32.match([](std::int32_t v) { return v; }, [](const hc::Fault &) { return 0; }) == -1234);

  const std::string_view input = "12.5";
  auto bad = hc::cast_i32(input, invariant);
  CHECK(!bad.ok() && bad.value_if() == nullptr && bad.value() == 0 && bad.value_or(7) == 7);
  CHECK((bad.fault() == hc::Fault{hc::CastFailure::malformed, 2, 1}));
  CHECK(input.substr(bad.fault().offset, bad.fault().length) == ".");
  CHECK(bad.visit(overloaded{[](std::int32_t) { return std::string_view("value"); },
                             [](const hc::Fault &) { return std::string_view("fault"); }}) ==
        "fault");
  CHECK(std::holds_alternative<hc::Fault>(bad.to_variant()));
  CHECK(
      std::visit(overloaded{[](std::int32_t v) { return v; }, [](const hc::Fault &) { return -1; }},
                 i32.to_variant()) == -1234);

  // Empty is the one reason optional() reads as absent.
  CHECK(!hc::optional(hc::cast_i32("  ", invariant)).has_value());
  CHECK(hc::optional(bad).has_value() && hc::optional(i32)->value() == -1234);

#ifdef HYPERCAST_HAS_EXPECTED
  std::expected<std::int32_t, hc::Fault> expected = hc::cast_i32("42", invariant);
  CHECK(expected.has_value() && *expected == 42);
  CHECK(!bad.to_expected().has_value() && bad.to_expected().error().offset == 2);
  std::expected<bool, hc::Fault> no = hc::cast_bool("no");
  CHECK(no.has_value() && !*no);
#endif

  CHECK(hc::cast_bool(" yes ").value_or(false));
  CHECK((hc::cast_bool("\tyess\n").fault() == hc::Fault{hc::CastFailure::malformed, 1, 4}));
  CHECK(hc::cast_char("\xe2\x82\xac").value() == U'\u20AC');
#ifdef __cpp_char8_t
  CHECK(hc::cast_char(u8"\u20AC").value() == U'\u20AC');
#endif

  CHECK(hc::cast_i8("0xFF", invariant).value() == -1);
  CHECK(hc::cast_i16("-32768", invariant).value() == -32768);
  CHECK(hc::cast_i64(std::string("9223372036854775807"), invariant).value() == INT64_MAX);
  CHECK((hc::cast_u8("256", invariant).fault() == hc::Fault{hc::CastFailure::out_of_range, 0, 3}));
  CHECK(hc::cast_u16("65535", invariant).value() == 65535);
  CHECK(hc::cast_u32("4294967295", invariant).value() == UINT32_MAX);
  CHECK(hc::cast_u64("18446744073709551615", invariant).value() == UINT64_MAX);
  CHECK(hc::cast_f32("1.5", invariant).value() == 1.5f);
  CHECK(hc::cast_f64("1e400", invariant).fault().reason == hc::CastFailure::out_of_range);
  CHECK(hc::cast_i32("1,000", hc::NumFormat::detect()).fault().reason ==
        hc::CastFailure::malformed);

  // A declared format, checked by the core when it is declared.
  hc::NumFormat::Fields fields;
  fields.decimal_sep = ",";
  fields.group_sep = ".";
  fields.currency = "\xe2\x82\xac";
  const auto euro = hc::NumFormat::declare(fields);
  CHECK(euro.has_value() && euro->decimal_sep() == "," && euro->currency() == "\xe2\x82\xac");
  CHECK(hc::cast_f64("1.234,5 \xe2\x82\xac", *euro).value() == 1234.5);
  const auto decimal = hc::cast_decimal("-1.234,50", *euro);
  CHECK(decimal.ok() && text(decimal.value()) == "-1234.5" && decimal.value().scale == 1);
  fields.group_sep = ",";
  CHECK(!hc::NumFormat::declare(fields).has_value()); // equal separators
  fields.group_sep = ".";
  fields.decimal_sep = "\xc2\xa0";
  CHECK(!hc::NumFormat::declare(fields).has_value()); // a decimal separator that is white space
  fields.decimal_sep = ",";
  fields.currency = "\xc2\xa0\xe2\x82\xac";
  CHECK(!hc::NumFormat::declare(fields).has_value()); // a currency symbol starting with white space
  // White space inside one is allowed (CLDR's `F CFA`; in octal, so `CFA` isn't read as hex).
  fields.currency = "F\342\200\257CFA";
  const auto cfa = hc::NumFormat::declare(fields);
  CHECK(cfa.has_value() && hc::cast_i32("1.000 F\302\240CFA", *cfa).value() == 1000);
  fields.currency = "\xe2\x82\xac";
  fields.group_sep = "123456789";
  CHECK(!hc::NumFormat::declare(fields).has_value()); // too long for the layout
  CHECK(hc::NumFormat::declare({".", ",", hc::NumStyles::all, {}, {}, {}, {}, {}, {}, 0}) ==
        invariant);
  CHECK(invariant.raw().size == HyperCastRawNumFormat_SIZE &&
        invariant.styles() == hc::NumStyles::all);

  const auto uuid = hc::cast_uuid("01020304-0506-0708-090a-0b0c0d0e0f10");
  CHECK(uuid.ok() && uuid.value().bytes[0] == 0x01 && uuid.value().bytes[15] == 0x10);

  CHECK((hc::cast_timestamp("2022-02-22T19:22:22Z").value() == hc::Timestamp{1645557742, 0}));
  CHECK((hc::cast_unix("1700000000123", hc::UnixPrecision::milliseconds).value() ==
         hc::Timestamp{1700000000, 123000000}));
  CHECK((hc::cast_excel_serial("25569", hc::ExcelEpoch::y1900).value() == hc::Timestamp{0, 0}));

  CHECK((hc::cast_date("2026-01-02").value() == hc::Date{2026, 1, 2}));
  CHECK((hc::cast_date("1/7/2026", hc::DateOrder::day_month_year).value() == hc::Date{2026, 7, 1}));
  CHECK((hc::cast_date("13/1/2026", hc::DateOrder::month_day_year).fault() ==
         hc::Fault{hc::CastFailure::out_of_range, 0, 2}));

  constexpr std::uint64_t three_oh_four_pm = 54240000000000ULL;
  CHECK((hc::cast_datetime("1/7/2026 3:04 PM", hc::DateOrder::month_day_year).value() ==
         hc::CivilDateTime{{2026, 1, 7}, three_oh_four_pm}));
  const auto german = hc::DateFormat::declare(hc::DateOrder::day_month_year, "vorm.", "nachm.");
  CHECK(german.has_value() && german->pm() == "nachm.");
  CHECK((hc::cast_datetime("7.1.2026 3:04 nachm.", *german).value() ==
         hc::CivilDateTime{{2026, 1, 7}, three_oh_four_pm}));
  CHECK(!hc::DateFormat::declare(hc::DateOrder::day_month_year, " am", "pm").has_value());
  CHECK(
      !hc::DateFormat::declare(hc::DateOrder::day_month_year, "am", "\xe2\x80\xafPM").has_value());
  CHECK(hc::DateFormat::declare(hc::DateOrder::year_month_day, "", "") ==
        hc::DateFormat(hc::DateOrder::year_month_day));

  CHECK(hc::cast_time("15:04:05").value() == std::chrono::nanoseconds(54245000000000LL));
  CHECK((hc::cast_duration("PT1H30M").value() == hc::Duration{5400, 0}));

  // The typed doors: a double in, and an empty span on a fault.
  CHECK(text(hc::cast_decimal_from_double(0.1).value()) == "0.1");
  CHECK((hc::cast_decimal_from_double(1e300).fault() ==
         hc::Fault{hc::CastFailure::out_of_range, 0, 0}));
  CHECK((hc::cast_excel_serial_from_double(45292.75, hc::ExcelEpoch::y1900).value() ==
         hc::CivilDateTime{{2024, 1, 1}, 64800000000000ULL}));
  CHECK(hc::cast_excel_time(0.75).value() == std::chrono::hours(18));
  CHECK((hc::cast_excel_duration(1.5).value() == hc::Duration{129600, 0}));

  // Validation constraints, declared once and judged by the core.
  hc::Constraints<double>::Fields degrees;
  degrees.minimum = -90;
  degrees.maximum = 90;
  degrees.multiple_of = "0.000001";
  degrees.rounding = hc::Rounding::midpoint;
  const auto latitude = hc::Constraints<double>::declare(degrees);
  CHECK(latitude.ok());
  const auto north = hc::cast_f64("45.1234567", invariant, latitude.value());
  CHECK(north.ok() && north.value().value == 45.123457 && north.value().adjusted &&
        !north.value().enum_index.has_value());
  const auto beyond = hc::cast_f64("91", invariant, latitude.value());
  // A violation carries the value as cast, never rounded.
  CHECK((beyond.rejection().violation() == hc::Violation<double>{hc::Keyword::maximum, 91, 0, 2}));
  CHECK(beyond.rejection().visit(overloaded{[](const hc::Fault &) { return 0; },
                                            [](const hc::Violation<double> &) { return 1; }}) == 1);
  CHECK(hc::cast_f64("90.0000006", invariant, latitude.value()).rejection().violation().value ==
        90.0000006);
  CHECK((hc::cast_f64_from_double(-90.5, latitude.value()).rejection().violation() ==
         hc::Violation<double>{hc::Keyword::minimum, -90.5, 0, 0}));
  CHECK(beyond.rejection().map([](double v) { return v > 90; }).violation().value);
  CHECK(hc::cast_f64("x", invariant, latitude.value()).rejection().fault().reason ==
        hc::CastFailure::malformed);

  hc::Constraints<hc::Decimal>::Fields cash;
  cash.multiple_of = "0.05";
  const auto euros = hc::Constraints<hc::Decimal>::declare(cash);
  CHECK(text(hc::cast_decimal("12.35", invariant, euros.value()).value().value) == "12.35");
  const auto odd = hc::cast_decimal("12.37", invariant, euros.value()).rejection().violation();
  CHECK(odd.keyword == hc::Keyword::multiple_of && text(odd.value) == "12.37");

  std::uint8_t storage[32];
  hc::Constraints<std::string_view>::Fields code;
  code.trim = true;
  const auto sex =
      hc::Constraints<std::string_view>::declare(code, {"Female", "Male", "Other"}, storage);
  const auto male = hc::cast_text(" MALE ", sex.value());
  CHECK(male.ok() && male.value().value == "Male" && male.value().enum_index == 1u);
  const auto mail = hc::cast_text(" Mail ", sex.value()).rejection().violation();
  CHECK(mail.keyword == hc::Keyword::enum_ && mail.value == "Mail" && mail.offset == 1);
  // case_sensitive matches the entries byte for byte instead of under case folding.
  code.case_sensitive = true;
  std::uint8_t exact_storage[32];
  const auto exact =
      hc::Constraints<std::string_view>::declare(code, {"Female", "Male", "Other"}, exact_storage);
  CHECK(hc::cast_text(" Male ", exact.value()).value().enum_index == 1u);
  CHECK(hc::cast_text(" MALE ", exact.value()).rejection().violation().value == "MALE");
  code.case_sensitive = false;
  CHECK(hc::cast_text("ab").value() == "ab");
  CHECK(!hc::optional(hc::cast_text("  ", sex.value())).has_value());

  // All four bounds may be declared together, each judged on its own.
  degrees.exclusive_minimum = -89;
  const auto both = hc::Constraints<double>::declare(degrees);
  CHECK((both.ok() && hc::cast_f64("-89.5", invariant, both.value()).rejection().violation() ==
                          hc::Violation<double>{hc::Keyword::exclusive_minimum, -89.5, 0, 5}));
  // A refusal, worded from the core's code.
  degrees.trim = true;
  const auto trimmed_number = hc::Constraints<double>::declare(degrees);
  CHECK(!trimmed_number.ok() && trimmed_number.refusal().code == 1 &&
        trimmed_number.refusal().message() ==
            "Constraints refused (code 1): a constraint the door does not take.");
  std::uint8_t small_storage[4];
  CHECK(hc::Constraints<std::string_view>::declare({}, {"Male"}, small_storage).refusal().code ==
        0);
  // An enum is declared by passing entries, an empty list included, which every value violates;
  // declare(fields) declares none.
  CHECK((sex.value().raw().flags & HyperCastRawConstraints_HAS_ENUM) != 0);
  CHECK((euros.value().raw().flags & HyperCastRawConstraints_HAS_ENUM) == 0);
  const auto none = hc::Constraints<std::string_view>::declare(code, {}, small_storage);
  CHECK(none.ok() &&
        hc::cast_text("Male", none.value()).rejection().violation().keyword == hc::Keyword::enum_);
  const auto no_number = hc::Constraints<std::int32_t>::declare({}, nullptr, 0, small_storage);
  CHECK((hc::cast_i32("7", invariant, no_number.value()).rejection().violation() ==
         hc::Violation<std::int32_t>{hc::Keyword::enum_, 7, 0, 1}));
  // Invalid UTF-8 (here a lone surrogate's bytes) reaches the core unchanged at a plain door, as
  // at a checked one: the same fault, span included.
  const auto loose = hc::Constraints<std::int32_t>::declare({});
  const auto plain_bad = hc::cast_i32("12\xed\xa0\x80"
                                      "3",
                                      invariant);
  const auto checked_bad = hc::cast_i32("12\xed\xa0\x80"
                                        "3",
                                        invariant, loose.value());
  CHECK(!plain_bad.ok() && plain_bad.fault().reason == hc::CastFailure::malformed &&
        plain_bad.fault().offset == 2 && plain_bad.fault().length == 3 && !checked_bad.ok() &&
        checked_bad.rejection().is_fault() && checked_bad.rejection().fault() == plain_bad.fault());
  // The message is a value: kept past the Declared temporary it came from, it is still whole.
  const auto kept = hc::Constraints<double>::declare(degrees).refusal().message();
  CHECK(kept == "Constraints refused (code 1): a constraint the door does not take.");
  CHECK(hc::Refusal{14}.message() ==
        "Constraints refused (code 14): a rule this binding does not name.");
  CHECK(hc::Refusal{-7}.message() ==
        "Constraints refused (code -7): a rule this binding does not name.");

#ifdef HYPERCAST_HAS_EXPECTED
  std::expected<hc::CheckedValue<double>, hc::Rejection<double>> checked = north;
  CHECK(checked.has_value() && checked->adjusted);
#endif

  // to_chars refuses a buffer too small, and writes the longest a decimal can be.
  char small[3];
  CHECK((hc::Decimal{12345, 0, 1, true}.to_chars(small, small + sizeof small).ec ==
         std::errc::value_too_large));
  char wide[hc::Decimal::max_chars];
  CHECK(
      (hc::Decimal{1, 0, 255, true}.to_chars(wide, wide + sizeof wide).ptr == wide + sizeof wide));
  CHECK((text(hc::Decimal{~0ULL, ~0U, 0, false}) == "79228162514264337593543950335"));

  const std::string_view version = hc::native_version();
  std::printf("%.*s\n", static_cast<int>(version.size()), version.data());
  return 0;
}
