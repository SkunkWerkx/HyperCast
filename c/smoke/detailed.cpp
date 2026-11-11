// The 1.1 surface of the C++ header (what main.cpp predates) against an unpacked release archive:
// the version macros and the run-time guard, detail codes and Failure, grouping policies, the
// detailed doors (and the plain ones that route through them), date options and month names, and
// what a Fault still is to code written before any of it. main.cpp is that older code, unchanged:
// the same file compiled against this header is the compatibility test, and this one holds the
// shapes it cannot reach. CMakeLists.txt builds it as C++17, C++20 and, where the compiler has it,
// C++23, warnings as errors, and once more with exceptions off.

#include <cstdio>
#include <limits>
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

// The compile-time version, packed as the library packs its own.
static_assert(HYPERCAST_VERSION == ((HYPERCAST_VERSION_MAJOR << 16) |
                                    (HYPERCAST_VERSION_MINOR << 8) | HYPERCAST_VERSION_PATCH),
              "the version macros pack as hypercast_version() does");

// The detailed doors, and everything they return, are noexcept; a Detail is a plain number.
static_assert(noexcept(hc::detailed::cast_i32("1", hc::NumFormat::invariant())), "noexcept");
static_assert(noexcept(hc::detailed::cast_date("2026-01-07", hc::DateOptions::common_forms())),
              "noexcept");
static_assert(noexcept(hc::core_is_compatible()) && noexcept(hc::name(hc::Detail::blank)),
              "noexcept");
static_assert(std::is_same_v<std::underlying_type_t<hc::Detail>, std::uint32_t> &&
                  static_cast<std::uint32_t>(hc::Detail::group_size) == 11 &&
                  static_cast<std::uint32_t>(hc::Detail::not_a_scalar) == 32 &&
                  static_cast<std::uint32_t>(hc::Detail::none) == 0,
              "a Detail is the core's code");
static_assert(!std::is_default_constructible_v<hc::DetailedVerdict<int>> &&
                  !std::is_default_constructible_v<hc::DetailedChecked<double>> &&
                  !std::is_constructible_v<bool, hc::DetailedVerdict<bool>>,
              "a detailed verdict is one of its cases, and no bool");
static_assert(hc::name(hc::Detail::date_field_out_of_range) == "date_field_out_of_range" &&
                  hc::name(static_cast<hc::Detail>(999)) == "detail 999" &&
                  hc::name(hc::Detail::none) == "detail 0" &&
                  hc::name(static_cast<hc::Detail>(4294967295U)) == "detail 4294967295",
              "a detail is named, and an unknown one keeps its number");
static_assert(hc::is_known(hc::Detail::blank) && hc::is_known(hc::Detail::not_a_scalar) &&
                  !hc::is_known(hc::Detail::none) && !hc::is_known(static_cast<hc::Detail>(33)),
              "the published codes are 1 to 32");
static_assert(hc::DateOptions::common_forms().styles() == hc::DateStyles::common_forms &&
                  hc::DateOptions::none().pivot() == 0 &&
                  hc::DateOptions::common_forms().raw().size == HyperCastRawDateOptions_SIZE &&
                  hc::DateOptions() == hc::DateOptions::none(),
              "the date options, as the layout carries them");
static_assert(hc::DateStyles::common_forms ==
                  (hc::DateStyles::compact_date | hc::DateStyles::space_separator |
                   hc::DateStyles::hour_offset),
              "common forms is the three shapes");

// The overload set Verdict::visit takes.
template <class... F> struct overloaded : F... {
  using F::operator()...;
};
template <class... F> overloaded(F...) -> overloaded<F...>;

// What a decimal displays as.
static std::string text(const hc::Decimal &decimal) {
  char buffer[hc::Decimal::max_chars];
  return std::string(buffer, decimal.to_chars(buffer, buffer + sizeof buffer).ptr);
}

// The French names of the corpus's `janv.` rows: `mar` and `mar.` both name March, and the dots
// belong to the names.
static const hc::MonthName french[] = {{1, "janv."},
                                       {2, "f\xc3\xa9vr."},
                                       {3, "mar"},
                                       {3, "mar."},
                                       {4, "avr."},
                                       {5, "mai"},
                                       {6, "juin"},
                                       {7, "juil."},
                                       {8, "ao\xc3\xbbt"},
                                       {9, "sept."},
                                       {10, "oct."},
                                       {11, "nov."},
                                       {12, "d\xc3\xa9"
                                            "c."}};
static const hc::MonthName english[] = {
    {1, "Jan"}, {2, "Feb"}, {3, "Mar"},  {4, "Apr"},  {5, "May"},  {6, "Jun"}, {7, "Jul"},
    {8, "Aug"}, {9, "Sep"}, {9, "Sept"}, {10, "Oct"}, {11, "Nov"}, {12, "Dec"}};

int main() {
  // The version: the header's macros, the library's own, and the guard between them.
  CHECK(hc::core_is_compatible());
  CHECK(hc::detail::version_accepts(hc::native_version_number(), HYPERCAST_VERSION));
  CHECK((hc::native_version_number() >> 16) == HYPERCAST_VERSION_MAJOR);
  CHECK(hc::native_version_number() == hypercast_version());
  // The rule: the core's major is the header's, its minor at least the header's, its patch
  // nothing. A 1.1.1 header drives a 1.1.0 core; a 1.0.x or a 2.x core is refused by a 1.1 header.
  static_assert(hc::detail::version_accepts(0x010100, 0x010101) &&
                    hc::detail::version_accepts(0x010100, 0x0101ff) &&
                    hc::detail::version_accepts(0x010100, 0x010100) &&
                    hc::detail::version_accepts(0x010203, 0x010100) &&
                    hc::detail::version_accepts(0x010000, 0x010000),
                "the patch never matters, and a newer minor drives an older header");
  static_assert(!hc::detail::version_accepts(0x010000, 0x010100) &&
                    !hc::detail::version_accepts(0x0100ff, 0x010100) &&
                    !hc::detail::version_accepts(0x020000, 0x010100) &&
                    !hc::detail::version_accepts(0x020100, 0x010100) &&
                    !hc::detail::version_accepts(0x010100, 0x020100) &&
                    !hc::detail::version_accepts(0x010000, 0x010101),
                "an older minor or another major version is refused");

  // Every published detail has its name, and a code nobody published is a name and a number.
  static const char *const names[33] = {"detail 0",
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
  for (std::uint32_t code = 0; code <= 32; ++code) {
    const hc::DetailName named = hc::name(static_cast<hc::Detail>(code));
    CHECK(named == names[code]);
    CHECK(std::string_view(named.c_str()) == names[code] && named.size() == named.view().size());
  }
  CHECK(hc::name(static_cast<hc::Detail>(33)) == "detail 33");

  // An unknown detail decodes: the reason and span are usable, the code is kept. Here a core
  // that reports 999, which no header names, for a malformed span of two bytes at 3.
  HyperCastRawFaultDetail future = {HyperCastRawFaultDetail_SIZE, 3, 2, 999};
  const auto decoded = hc::detail::detailed_finish<int>(
      static_cast<std::int32_t>(hc::CastFailure::malformed), 0, future, hc::detail::Identity{});
  CHECK(!decoded.ok() && decoded.failure().reason == hc::CastFailure::malformed);
  CHECK(decoded.failure().offset == 3 && decoded.failure().length == 2);
  CHECK(decoded.failure().detail_code() == 999 && !hc::is_known(decoded.failure().detail));
  CHECK(hc::name(decoded.failure().detail) == "detail 999");
  CHECK((decoded.to_verdict().fault() == hc::Fault{hc::CastFailure::malformed, 3, 2}));

  // A Failure is a Fault and a detail; the Fault is untouched. A 1.0 Fault still takes aggregate
  // initialization, equality and structured bindings, and ignores the detail it never had.
  const hc::Failure failure{hc::CastFailure::out_of_range, hc::Detail::too_large, 4, 5};
  CHECK((failure.fault() == hc::Fault{hc::CastFailure::out_of_range, 4, 5}));
  CHECK((failure == hc::Failure{hc::CastFailure::out_of_range, hc::Detail::too_large, 4, 5}));
  CHECK((failure != hc::Failure{hc::CastFailure::out_of_range, hc::Detail::too_small, 4, 5}));
  const hc::Fault old_fault{hc::CastFailure::malformed, 2, 1};
  const auto [reason, offset, length] = old_fault;
  CHECK(reason == hc::CastFailure::malformed && offset == 2 && length == 1);
  CHECK(hc::cast_i32("12,34", hc::NumFormat::invariant()).fault() ==
        hc::detailed::cast_i32("12,34", hc::NumFormat::invariant()).failure().fault());

  // Grouping. A format declares it beside its styles, Western by default: `12,34` is
  // Malformed, spanning the group with its separator, and the detail says why.
  const auto invariant = hc::NumFormat::invariant();
  CHECK(invariant.grouping() == hc::Grouping::western && invariant.styles() == hc::NumStyles::all);
  CHECK(invariant.raw().format.flags == HyperCastNumFormat_ALL);
  const auto grouped = hc::detailed::cast_i32("12,34", invariant);
  CHECK(
      (grouped.failure() == hc::Failure{hc::CastFailure::malformed, hc::Detail::group_size, 2, 3}));
  CHECK((hc::cast_i32("12,34", invariant).fault() == hc::Fault{hc::CastFailure::malformed, 2, 3}));
  CHECK((hc::detailed::cast_i32("1234,567", invariant).failure() ==
         hc::Failure{hc::CastFailure::malformed, hc::Detail::group_size, 0, 4}));
  CHECK(hc::cast_i32("1,234,567", invariant).value() == 1234567);
  CHECK(hc::cast_i32("1234567", invariant).value() == 1234567);

  // Western reads groups of three alone: every shape 1.0 let through is refused.
  CHECK((hc::detailed::cast_i32("1,23,456", invariant).failure() ==
         hc::Failure{hc::CastFailure::malformed, hc::Detail::group_size, 1, 3}));
  CHECK(
      (hc::cast_i32("1,23,456", invariant).fault() == hc::Fault{hc::CastFailure::malformed, 1, 3}));
  CHECK((hc::detailed::cast_i32("0,125", invariant).failure() ==
         hc::Failure{hc::CastFailure::malformed, hc::Detail::group_size, 0, 1}));
  CHECK(hc::cast_i32("1234,567,890", invariant).fault().reason == hc::CastFailure::malformed);

  // Indian: the last group three, every group before it two. Setting a policy replaces the one
  // there, never combining two, and keeps the styles.
  const auto indian = invariant.with_grouping(hc::Grouping::indian);
  CHECK(hc::cast_i32("12,34,567", indian).value() == 1234567);
  CHECK(hc::detailed::cast_i32("12,34,567", indian).value() == 1234567);
  CHECK((hc::detailed::cast_i32("12,34", indian).failure().detail == hc::Detail::group_size));
  CHECK((hc::detailed::cast_i32("12,34,567", invariant).failure() ==
         hc::Failure{hc::CastFailure::malformed, hc::Detail::group_size, 2, 3}));
  const auto replaced =
      indian.with_grouping(hc::Grouping::western).with_grouping(hc::Grouping::indian);
  CHECK(replaced == indian && replaced.styles() == hc::NumStyles::all);
  CHECK(replaced.raw().format.flags ==
        (HyperCastNumFormat_ALL | static_cast<std::uint32_t>(HyperCastNumFormat_GROUPING_INDIAN)));
  // Both directions, the plain door and the detailed one alike.
  CHECK(hc::cast_i32("1,23,456", indian).value() == 123456);
  CHECK(hc::cast_i32("1,234,567", indian).fault().reason == hc::CastFailure::malformed);
  const auto back = indian.with_grouping(hc::Grouping::western);
  CHECK(back == invariant);
  CHECK(hc::cast_i32("1,234,567", back).value() == 1234567);
  CHECK(hc::cast_i32("1,23,456", back).fault().reason == hc::CastFailure::malformed);

  // A policy only judges groups: with the grouping style off a separator stays forbidden.
  hc::NumFormat::Fields plain;
  plain.decimal_sep = ".";
  plain.group_sep = ",";
  plain.styles = hc::NumStyles::all & ~hc::NumStyles::grouping;
  const auto ungrouped = hc::NumFormat::declare(plain);
  CHECK(ungrouped.has_value() && ungrouped->grouping() == hc::Grouping::western);
  CHECK((hc::detailed::cast_i32("1,234", *ungrouped).failure() ==
         hc::Failure{hc::CastFailure::malformed, hc::Detail::unexpected_character, 1, 1}));
  CHECK(hc::cast_i32("1,234", *ungrouped).fault().reason == hc::CastFailure::malformed);

  // Declaring: the default is Western, and the core judges the rest. A policy that is
  // none of Grouping's, a style bit nobody defined and styles carrying a policy are refused.
  hc::NumFormat::Fields fields;
  fields.decimal_sep = ",";
  fields.group_sep = ".";
  CHECK(hc::NumFormat::declare(fields).has_value() &&
        hc::NumFormat::declare(fields)->grouping() == hc::Grouping::western);
  fields.grouping = hc::Grouping::indian;
  CHECK(hc::NumFormat::declare(fields).has_value());
  fields.grouping = static_cast<hc::Grouping>(3 << 8);
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.grouping = static_cast<hc::Grouping>(1 << 8);
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.grouping = static_cast<hc::Grouping>(1);
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.grouping = hc::Grouping::western;
  fields.styles = static_cast<hc::NumStyles>(1 << 10);
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.styles = static_cast<hc::NumStyles>(HyperCastNumFormat_ALL | (1 << 8)); // 351
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.styles = static_cast<hc::NumStyles>(HyperCastNumFormat_ALL | (3 << 8)); // 863
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.styles =
      static_cast<hc::NumStyles>(HyperCastNumFormat_ALL | HyperCastNumFormat_GROUPING_INDIAN);
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.styles = hc::NumStyles::all;
  CHECK(hc::NumFormat::declare(fields).has_value());
  // The positional form a 1.0 caller wrote still declares: Western, now.
  CHECK(hc::NumFormat::declare({".", ",", hc::NumStyles::all, {}, {}, {}, {}, {}, {}, 0}) ==
        invariant);
  CHECK(hc::NumFormat::detect().grouping() == hc::Grouping::western);

  // The plain doors honour the policy too: they go through the detailed export for Indian, and
  // the 1.0 export, Western on any core that drives this header, for Western.
  CHECK(hc::cast_f64("1,234.5", invariant).value() == 1234.5);
  CHECK(hc::cast_f64("12,34.5", invariant).fault().reason == hc::CastFailure::malformed);
  CHECK(hc::cast_f64("1,23,456.5", indian).value() == 123456.5);
  CHECK(hc::cast_f64("1,23,456.5", invariant).fault().reason == hc::CastFailure::malformed);
  CHECK(hc::cast_f64("12,34.5", indian).fault().reason == hc::CastFailure::malformed);
  CHECK(text(hc::cast_decimal("1,234.50", invariant).value()) == "1234.5");
  CHECK(hc::cast_decimal("12,34.50", invariant).fault().reason == hc::CastFailure::malformed);
  CHECK(hc::cast_u8("1,000", invariant).fault().reason == hc::CastFailure::out_of_range);
  hc::Constraints<std::int32_t>::Fields below;
  below.maximum = 1000;
  const auto cap = hc::Constraints<std::int32_t>::declare(below);
  CHECK((hc::cast_i32("1,234", invariant, cap.value()).rejection().violation() ==
         hc::Violation<std::int32_t>{hc::Keyword::maximum, 1234, 0, 5}));
  CHECK((hc::cast_i32("12,34", invariant, cap.value()).rejection().fault() ==
         hc::Fault{hc::CastFailure::malformed, 2, 3}));
  CHECK(hc::cast_i32("1,23,456", indian, cap.value()).rejection().violation().keyword ==
        hc::Keyword::maximum);
  CHECK((hc::cast_i32("1,23,456", invariant, cap.value()).rejection().fault() ==
         hc::Fault{hc::CastFailure::malformed, 1, 3}));

  // The detailed doors: the value, or a Failure; to_verdict() is the plain door's answer.
  const auto fine = hc::detailed::cast_i32("1,234", invariant);
  CHECK(fine.ok() && fine.value() == 1234 && fine.failure_if() == nullptr);
  CHECK(fine.to_verdict() == hc::Verdict<std::int32_t>(1234));
  CHECK(fine.match([](std::int32_t v) { return v; }, [](const hc::Failure &) { return 0; }) ==
        1234);
  CHECK(std::holds_alternative<hc::Failure>(grouped.to_variant()));
  CHECK(grouped.visit(overloaded{
            [](std::int32_t) { return std::string_view("value"); },
            [](const hc::Failure &) { return std::string_view("failure"); }}) == "failure");
  CHECK(grouped.value_if() == nullptr && grouped.value_or(7) == 7 && grouped.value() == 0);
  CHECK(!hc::optional(hc::detailed::cast_i32(" ", invariant)).has_value());
  CHECK(hc::optional(grouped).has_value() && hc::optional(fine)->value() == 1234);
  CHECK(hc::detailed::cast_i32("", invariant).failure().detail == hc::Detail::blank);
#ifdef HYPERCAST_HAS_EXPECTED
  std::expected<std::int32_t, hc::Failure> expected = hc::detailed::cast_i32("42", invariant);
  CHECK(expected.has_value() && *expected == 42);
  CHECK(grouped.to_expected().error().detail == hc::Detail::group_size);
#endif

  CHECK(hc::detailed::cast_bool("yes").value());
  CHECK(hc::detailed::cast_bool("maybe").failure().detail == hc::Detail::not_recognized);
  CHECK(hc::detailed::cast_char("\xe2\x82\xac").value() == U'\u20AC');
  CHECK(hc::detailed::cast_char("U+D800").failure().detail == hc::Detail::not_a_scalar);
  CHECK(hc::detailed::cast_i8("128", invariant).failure().detail == hc::Detail::too_large);
  CHECK(hc::detailed::cast_i8("-129", invariant).failure().detail == hc::Detail::too_small);
  CHECK(hc::detailed::cast_u8("-1", invariant).failure().detail ==
        hc::Detail::negative_not_allowed);
  CHECK(hc::detailed::cast_i32("12a", invariant).failure().detail ==
        hc::Detail::unexpected_character);
  CHECK(hc::detailed::cast_i32("0xG", invariant).failure().detail == hc::Detail::digit_expected);
  CHECK(hc::detailed::cast_i32("1.5", invariant).failure().detail ==
        hc::Detail::fraction_not_allowed);
  CHECK(hc::detailed::cast_i32("NaN", invariant).failure().detail == hc::Detail::not_finite);
  CHECK(hc::detailed::cast_f32("1e39", invariant).failure() ==
        (hc::Failure{hc::CastFailure::out_of_range, hc::Detail::too_large, 0, 4}));
  CHECK(hc::detailed::cast_f64("1e400", invariant).failure().detail == hc::Detail::too_large);
  CHECK(hc::detailed::cast_i16("-32768", invariant).value() == -32768);
  CHECK(hc::detailed::cast_i64("9223372036854775807", invariant).value() == INT64_MAX);
  CHECK(hc::detailed::cast_u16("65535", invariant).value() == 65535);
  CHECK(hc::detailed::cast_u32("4294967295", invariant).value() == UINT32_MAX);
  CHECK(hc::detailed::cast_u64("18446744073709551615", invariant).value() == UINT64_MAX);
  CHECK(hc::detailed::cast_f32("1.5", invariant).value() == 1.5f);
  CHECK(text(hc::detailed::cast_decimal("-1,234.50", invariant).value()) == "-1234.5");
  CHECK(hc::detailed::cast_decimal("1e-29", invariant).failure().detail == hc::Detail::too_precise);
  const auto uuid = hc::detailed::cast_uuid("01020304-0506-0708-090a-0b0c0d0e0f10");
  CHECK(uuid.ok() && uuid.value().bytes[0] == 0x01 && uuid.value().bytes[15] == 0x10);
  CHECK(hc::detailed::cast_uuid("01020304-0506-0708-090a-0b0c0d0e0f1").failure().detail ==
        hc::Detail::wrong_length);
  CHECK((hc::detailed::cast_timestamp("2022-02-22T19:22:22Z").value() ==
         hc::Timestamp{1645557742, 0}));
  CHECK((hc::detailed::cast_unix("1700000000123", hc::UnixPrecision::milliseconds).value() ==
         hc::Timestamp{1700000000, 123000000}));
  CHECK(hc::detailed::cast_unix("253402300800", hc::UnixPrecision::seconds).failure().detail ==
        hc::Detail::outside_window);
  CHECK((hc::detailed::cast_excel_serial("25569", hc::ExcelEpoch::y1900).value() ==
         hc::Timestamp{0, 0}));
  CHECK(hc::detailed::cast_excel_serial("60", hc::ExcelEpoch::y1900).failure().detail ==
        hc::Detail::nonexistent_date);
  CHECK(hc::detailed::cast_time("15:04:05").value() == std::chrono::nanoseconds(54245000000000LL));
  CHECK(hc::detailed::cast_time("25:00:00").failure().detail ==
        hc::Detail::time_field_out_of_range);
  CHECK((hc::detailed::cast_duration("PT1H30M").value() == hc::Duration{5400, 0}));
  CHECK(hc::detailed::cast_duration("P").failure().detail == hc::Detail::component_missing);
  CHECK(hc::detailed::cast_text("ab").value() == "ab");
  CHECK(hc::detailed::cast_text("ab\xff").failure() ==
        (hc::Failure{hc::CastFailure::malformed, hc::Detail::invalid_utf8, 2, 1}));
  CHECK((hc::detailed::cast_date("2026-01-02").value() == hc::Date{2026, 1, 2}));
  CHECK(hc::detailed::cast_date("2026-1-07").failure().detail == hc::Detail::wrong_length);
  CHECK((hc::detailed::cast_date("1/7/2026", hc::DateOrder::day_month_year).value() ==
         hc::Date{2026, 7, 1}));
  CHECK(hc::detailed::cast_date("13/1/2026", hc::DateOrder::month_day_year).failure() ==
        (hc::Failure{hc::CastFailure::out_of_range, hc::Detail::date_field_out_of_range, 0, 2}));
  constexpr std::uint64_t three_oh_four_pm = 54240000000000ULL;
  CHECK((hc::detailed::cast_datetime("1/7/2026 3:04 PM", hc::DateOrder::month_day_year).value() ==
         hc::CivilDateTime{{2026, 1, 7}, three_oh_four_pm}));
  const auto german = hc::DateFormat::declare(hc::DateOrder::day_month_year, "vorm.", "nachm.");
  CHECK(german.has_value());
  CHECK((hc::detailed::cast_datetime("7.1.2026 3:04 nachm.", *german).value() ==
         hc::CivilDateTime{{2026, 1, 7}, three_oh_four_pm}));

  // The typed doors: a double in, an empty span on a failure.
  CHECK(text(hc::detailed::cast_decimal_from_double(0.1).value()) == "0.1");
  CHECK((hc::detailed::cast_decimal_from_double(1e300).failure() ==
         hc::Failure{hc::CastFailure::out_of_range, hc::Detail::too_large, 0, 0}));
  CHECK(
      (hc::detailed::cast_decimal_from_double(std::numeric_limits<double>::quiet_NaN()).failure() ==
       hc::Failure{hc::CastFailure::malformed, hc::Detail::not_finite, 0, 0}));
  CHECK((hc::detailed::cast_excel_serial_from_double(45292.75, hc::ExcelEpoch::y1900).value() ==
         hc::CivilDateTime{{2024, 1, 1}, 64800000000000ULL}));
  CHECK(hc::detailed::cast_excel_time(0.75).value() == std::chrono::hours(18));
  CHECK(hc::detailed::cast_excel_time(-1).failure().detail == hc::Detail::negative_not_allowed);
  CHECK((hc::detailed::cast_excel_duration(1.5).value() == hc::Duration{129600, 0}));

  // The checked doors: a Failure carries its detail, a Violation has none.
  hc::Constraints<double>::Fields degrees;
  degrees.minimum = -90;
  degrees.maximum = 90;
  const auto latitude = hc::Constraints<double>::declare(degrees);
  const auto north = hc::detailed::cast_f64("45.5", invariant, latitude.value());
  CHECK(north.ok() && north.value().value == 45.5 && !north.value().adjusted);
  const auto beyond = hc::detailed::cast_f64("91", invariant, latitude.value());
  CHECK(!beyond.ok() && !beyond.rejection().is_failure());
  CHECK((beyond.rejection().violation() == hc::Violation<double>{hc::Keyword::maximum, 91, 0, 2}));
  CHECK(beyond.to_checked() == hc::cast_f64("91", invariant, latitude.value()));
  const auto unreadable = hc::detailed::cast_f64("9,1", invariant, latitude.value());
  CHECK(unreadable.rejection().is_failure() &&
        unreadable.rejection().failure().detail == hc::Detail::group_size);
  CHECK(unreadable.rejection().visit(
            overloaded{[](const hc::Failure &f) { return f.detail_code(); },
                       [](const hc::Violation<double> &) { return 0u; }}) == 11);
  CHECK(unreadable.to_checked() == hc::cast_f64("9,1", invariant, latitude.value()));
  CHECK((hc::detailed::cast_f64_from_double(-90.5, latitude.value()).rejection().violation() ==
         hc::Violation<double>{hc::Keyword::minimum, -90.5, 0, 0}));
  CHECK((
      hc::detailed::cast_f64_from_double(std::numeric_limits<double>::quiet_NaN(), latitude.value())
          .rejection()
          .failure() == hc::Failure{hc::CastFailure::malformed, hc::Detail::not_finite, 0, 0}));
  CHECK(!hc::optional(hc::detailed::cast_f64("  ", invariant, latitude.value())).has_value());
  std::uint8_t storage[32];
  hc::Constraints<std::string_view>::Fields code;
  code.trim = true;
  const auto sex =
      hc::Constraints<std::string_view>::declare(code, {"Female", "Male", "Other"}, storage);
  const auto male = hc::detailed::cast_text(" MALE ", sex.value());
  CHECK(male.ok() && male.value().value == "Male" && male.value().enum_index == 1u);
  CHECK(hc::detailed::cast_text("Mail", sex.value()).rejection().violation().keyword ==
        hc::Keyword::enum_);
  CHECK(hc::detailed::cast_text("ab\xff", sex.value()).rejection().failure().detail ==
        hc::Detail::invalid_utf8);
  hc::Constraints<hc::Decimal>::Fields cash;
  cash.multiple_of = "0.05";
  const auto euros = hc::Constraints<hc::Decimal>::declare(cash);
  CHECK(text(hc::detailed::cast_decimal("12.35", invariant, euros.value()).value().value) ==
        "12.35");
  CHECK(hc::detailed::cast_decimal_from_double(12.37, euros.value())
            .rejection()
            .violation()
            .keyword == hc::Keyword::multiple_of);

  // Date options. Each shape is opt-in: the same text without them keeps its old answer.
  CHECK(hc::detailed::cast_date("20260107").failure().detail == hc::Detail::wrong_length);
  CHECK((hc::detailed::cast_date("20260107", hc::DateOptions::common_forms()).value() ==
         hc::Date{2026, 1, 7}));
  const auto compact = hc::DateOptions::declare(hc::DateStyles::compact_date);
  CHECK(compact.has_value() && compact->styles() == hc::DateStyles::compact_date);
  CHECK((hc::detailed::cast_date("20260107", *compact).value() == hc::Date{2026, 1, 7}));
  CHECK((hc::detailed::cast_date("2026-01-07", *compact).value() == hc::Date{2026, 1, 7}));
  CHECK((hc::detailed::cast_timestamp("2026-01-07 15:04:05Z").failure() ==
         hc::Failure{hc::CastFailure::malformed, hc::Detail::delimiter_expected, 10, 1}));
  CHECK((hc::detailed::cast_timestamp("2026-01-07 15:04:05Z", hc::DateOptions::common_forms())
             .value() == hc::Timestamp{1767798245, 0}));
  const auto spaced = hc::DateOptions::declare(hc::DateStyles::space_separator);
  CHECK(spaced.has_value() && hc::detailed::cast_timestamp("2026-01-07 15:04:05Z", *spaced).ok());
  CHECK(!hc::detailed::cast_timestamp("2026-01-07T15:04:05+00", *spaced).ok());
  const auto hours = hc::DateOptions::declare(hc::DateStyles::hour_offset);
  CHECK(hours.has_value() && hc::detailed::cast_timestamp("2026-01-07T15:04:05+00", *hours).ok());
  CHECK((hc::detailed::cast_timestamp("2026-01-07T15:04:05-05", hc::DateOptions::common_forms())
             .value() == hc::Timestamp{1767816245, 0}));
  // The date options never reach a door that reads none: the plain one keeps its shapes.
  CHECK(!hc::cast_date("20260107").ok() && !hc::cast_timestamp("2026-01-07 15:04:05Z").ok());

  // A pivot is the last year of the window a two-digit year is read in; none, no two-digit year.
  const auto pivot = hc::DateOptions::declare(hc::DateStyles::none, 2029);
  CHECK(pivot.has_value() && pivot->pivot() == 2029 && pivot->styles() == hc::DateStyles::none);
  CHECK((hc::detailed::cast_date("26/01/07", hc::DateOrder::year_month_day, *pivot).value() ==
         hc::Date{2026, 1, 7}));
  CHECK((hc::detailed::cast_date("26/01/07", hc::DateOrder::day_month_year, *pivot).value() ==
         hc::Date{2007, 1, 26}));
  CHECK((hc::detailed::cast_date("26/01/07", hc::DateOrder::year_month_day).failure() ==
         hc::Failure{hc::CastFailure::malformed, hc::Detail::field_width, 0, 2}));
  CHECK((hc::detailed::cast_datetime("26/01/07 3:04 PM", hc::DateOrder::year_month_day, *pivot)
             .value() == hc::CivilDateTime{{2026, 1, 7}, three_oh_four_pm}));
  CHECK((hc::detailed::cast_datetime("26/01/07 3:04 nachm.", *german, *pivot).value() ==
         hc::CivilDateTime{{2007, 1, 26}, three_oh_four_pm}));
  // The edges of the window: 100 and 9999 are pivots, 99 and 10000 are not.
  CHECK(hc::DateOptions::declare(hc::DateStyles::none, 100).has_value());
  CHECK(hc::DateOptions::declare(hc::DateStyles::none, 9999).has_value());
  CHECK(!hc::DateOptions::declare(hc::DateStyles::none, 99).has_value());
  CHECK(!hc::DateOptions::declare(hc::DateStyles::none, 1).has_value());
  CHECK(!hc::DateOptions::declare(hc::DateStyles::none, 10000).has_value());
  CHECK(!hc::DateOptions::declare(hc::DateStyles::none, 65535).has_value());
  CHECK(!hc::DateOptions::declare(hc::DateStyles::none, 4294967295U).has_value());
  CHECK(!hc::DateOptions::declare(static_cast<hc::DateStyles>(8)).has_value());
  CHECK(!hc::DateOptions::declare(static_cast<hc::DateStyles>(0x80000000U)).has_value());
  CHECK(!hc::DateOptions::declare(hc::DateStyles::common_forms, 99).has_value());
  CHECK(hc::DateOptions::declare(hc::DateStyles::common_forms) == hc::DateOptions::common_forms());
  CHECK(hc::DateOptions::declare(hc::DateStyles::none) == hc::DateOptions::none());

  // Month names, Excel style: a declared name takes the place of the numeric month.
  std::uint8_t months_storage[512];
  const auto months = hc::MonthNames::declare(english, months_storage);
  CHECK(months.has_value() && !months->empty() &&
        months->size() == hc::MonthNames::storage_size(english));
  CHECK(months->data() == months_storage);
  CHECK(hc::MonthNames().empty() && hc::MonthNames().data() == nullptr);
  CHECK((hc::detailed::cast_date("07-Jan-26", hc::DateOrder::day_month_year, *pivot, *months)
             .value() == hc::Date{2026, 1, 7}));
  CHECK((hc::detailed::cast_date("7-Jan-2026", hc::DateOrder::day_month_year,
                                 hc::DateOptions::none(), *months)
             .value() == hc::Date{2026, 1, 7}));
  CHECK((hc::detailed::cast_date("7-sept-2026", hc::DateOrder::day_month_year,
                                 hc::DateOptions::none(), *months)
             .value() == hc::Date{2026, 9, 7})); // folded, and the alias: Sept
  CHECK((hc::detailed::cast_date("Jan/7/2026", hc::DateOrder::month_day_year,
                                 hc::DateOptions::none(), *months)
             .value() == hc::Date{2026, 1, 7}));
  CHECK((hc::detailed::cast_datetime("7-Jan-2026T15:04:05", hc::DateOrder::day_month_year,
                                     hc::DateOptions::none(), *months)
             .ok()));
  // A two-digit year with names still needs a pivot, and an undeclared name is not a month.
  CHECK((hc::detailed::cast_date("07-Jan-26", hc::DateOrder::day_month_year,
                                 hc::DateOptions::none(), *months)
             .failure() == hc::Failure{hc::CastFailure::malformed, hc::Detail::field_width, 7, 2}));
  CHECK(!hc::detailed::cast_date("7-Foo-2026", hc::DateOrder::day_month_year,
                                 hc::DateOptions::none(), *months)
             .ok());
  CHECK(!hc::detailed::cast_date("7-Jan-2026", hc::DateOrder::day_month_year).ok());

  // Punctuation is part of a name: with `janv.` declared and `.` the separator, `7.janv..2026`
  // reads, and `7.janv.2026` is Malformed.
  std::uint8_t french_storage[512];
  const auto janv = hc::MonthNames::declare(french, french_storage);
  CHECK(janv.has_value());
  CHECK((hc::detailed::cast_date("7.janv..2026", hc::DateOrder::day_month_year,
                                 hc::DateOptions::none(), *janv)
             .value() == hc::Date{2026, 1, 7}));
  CHECK((
      hc::detailed::cast_date("7.janv..26", hc::DateOrder::day_month_year, *pivot, *janv).value() ==
      hc::Date{2026, 1, 7}));
  CHECK((hc::detailed::cast_date("7.janv.2026", hc::DateOrder::day_month_year,
                                 hc::DateOptions::none(), *janv)
             .failure() ==
         hc::Failure{hc::CastFailure::malformed, hc::Detail::not_recognized, 2, 4}));
  CHECK(hc::detailed::cast_date("7.janv.26", hc::DateOrder::day_month_year, *pivot, *janv)
            .failure()
            .reason == hc::CastFailure::malformed);
  CHECK((hc::detailed::cast_date("7/mar/2026", hc::DateOrder::day_month_year,
                                 hc::DateOptions::none(), *janv)
             .value() == hc::Date{2026, 3, 7}));
  CHECK((hc::detailed::cast_date("7/mar./2026", hc::DateOrder::day_month_year,
                                 hc::DateOptions::none(), *janv)
             .value() == hc::Date{2026, 3, 7})); // the longest of `mar` and `mar.`
  CHECK((hc::detailed::cast_date("7/D\xc3\x89"
                                 "C./2026",
                                 hc::DateOrder::day_month_year, hc::DateOptions::none(), *janv)
             .value() == hc::Date{2026, 12, 7})); // simple case folding reaches é
  CHECK((hc::detailed::cast_datetime("7.janv..2026 9:05", hc::DateOrder::day_month_year,
                                     hc::DateOptions::none(), *janv)
             .value() == hc::CivilDateTime{{2026, 1, 7}, 32700000000000ULL}));
  // The vocabulary is independent of the options: a door that reads neither ignores both.
  CHECK((hc::detailed::cast_date("2026-01-07", hc::DateOptions::none()).value() ==
         hc::Date{2026, 1, 7}));

  // A vocabulary the core refuses is no MonthNames: a month with no entry, a digit, `/`, `-` or
  // white space in a name, an empty or over-long name, a month outside 1 to 12, a duplicate
  // under folding (even for one month), storage too small, and an empty list.
  std::uint8_t scratch[1024];
  hc::MonthName broken[13];
  for (std::size_t i = 0; i < 12; ++i) {
    broken[i] = english[i < 9 ? i : i + 1];
  }
  CHECK(hc::MonthNames::declare(broken, 12, scratch).has_value());
  broken[12] = {1, "JAN"};
  CHECK(!hc::MonthNames::declare(broken, 13, scratch).has_value());
  broken[12] = {1, "Ja1"};
  CHECK(!hc::MonthNames::declare(broken, 13, scratch).has_value());
  broken[12] = {1, "Ja/n"};
  CHECK(!hc::MonthNames::declare(broken, 13, scratch).has_value());
  broken[12] = {1, "Ja-n"};
  CHECK(!hc::MonthNames::declare(broken, 13, scratch).has_value());
  broken[12] = {1, "Ja n"};
  CHECK(!hc::MonthNames::declare(broken, 13, scratch).has_value());
  broken[12] = {1, ""};
  CHECK(!hc::MonthNames::declare(broken, 13, scratch).has_value());
  broken[12] = {1, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"};
  CHECK(!hc::MonthNames::declare(broken, 13, scratch).has_value());
  broken[12] = {13, "x"};
  CHECK(!hc::MonthNames::declare(broken, 13, scratch).has_value());
  broken[12] = {0, "x"};
  CHECK(!hc::MonthNames::declare(broken, 13, scratch).has_value());
  broken[12] = {1, "Janv."};
  CHECK(hc::MonthNames::declare(broken, 13, scratch).has_value()); // an alias is fine
  std::uint8_t tiny[8];
  CHECK(!hc::MonthNames::declare(english, tiny).has_value());
  CHECK(!hc::MonthNames::declare(english, 0, scratch).has_value());
  CHECK(!hc::MonthNames::declare({}, scratch).has_value());
  CHECK(hc::MonthNames::storage_size({{1, "ab"}, {2, "c"}}) == 5 + 2 + 5 + 1);
  // The packed layout directly: each entry a month byte, a little-endian length, the UTF-8.
  const std::size_t packed_length = hc::MonthNames::storage_size(english);
  CHECK(hc::MonthNames::from_packed(months_storage, packed_length).has_value());
  CHECK(hc::MonthNames::from_packed(months_storage, 0) == std::nullopt);
  CHECK(hc::MonthNames::from_packed(nullptr, 5) == std::nullopt);
  std::uint8_t packed[512];
  for (std::size_t i = 0; i < packed_length; ++i) {
    packed[i] = months_storage[i];
  }
  packed[packed_length] = 3; // a cut-short entry past the last
  CHECK(!hc::MonthNames::from_packed(packed, packed_length + 1).has_value());
  CHECK(!hc::MonthNames::from_packed(packed, packed_length - 1).has_value());

  const std::string_view version = hc::native_version();
  std::printf("%.*s\n", static_cast<int>(version.size()), version.data());
  return 0;
}
