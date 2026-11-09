// The downgrade test: the C++ header against a core that cannot drive it, which lacks the detailed
// exports and reads grouped numerals by 1.0's rule. A core drives a header when its major version
// is the header's and its minor at least the header's, whatever the patch. A request against one
// that cannot must be refused, never read by that rule, and a core that can must accept it.
//
// run.sh builds it with the archive's header rewritten to other versions (downgrade_leg there): a
// patch ahead of the archive's library (accept), a major version ahead (refuse) and, when
// SMOKE_CORE_10 names a directory holding a real 1.0 core (a build of the v1.0.0 tag), the header
// as the release after it has it, 1.1.0 (refuse), with the export that core lacks, cast_detailed,
// stood in for by a function that ends the run with status 99: the header may reach it only behind
// its version guard. Nothing here may call a symbol such a core lacks except through the guard.
//
// usage: smoke_downgrade accept|refuse     the declarations, and the plain doors
//        smoke_downgrade door              a plain door, Western: reads nothing by 1.0's rule
//        smoke_downgrade indian            a plain door, Indian: likewise
//        smoke_downgrade retired <policy>  with_grouping with 256 or 768: ends the program
//
// `door` prints `malformed` for `1,23,456` on a core that drives the header, and `indian` prints
// `read 123456`. On one that cannot, the header writes a line naming both versions to stderr and
// ends the program (std::abort) before it parses anything: `door` prints `parsed` only for the
// failure the guard exists to prevent, 1.0's reading of 123456.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string_view>

#include "hypercast.hpp"

#define CHECK(condition)                                                                           \
  do {                                                                                             \
    if (!(condition)) {                                                                            \
      std::fprintf(stderr, "smoke: %s:%d: failed: %s\n", __FILE__, __LINE__, #condition);          \
      return 1;                                                                                    \
    }                                                                                              \
  } while (0)

namespace hc = hypercast;

int main(int argc, char **argv) {
  if (argc != 2 && !(argc == 3 && std::string_view(argv[1]) == "retired")) {
    std::fprintf(stderr, "usage: %s accept|refuse|door|indian|retired <policy>\n", argv[0]);
    return 2;
  }
  const std::string_view mode = argv[1];
  if (mode == "door") {
    // The default, at a plain door. On a core that cannot drive the header this aborts.
    const auto verdict = hc::cast_i32("1,23,456", hc::NumFormat::invariant());
    if (verdict.ok()) {
      std::printf("parsed %d\n", verdict.value());
    } else {
      std::printf("malformed\n");
    }
    return 0;
  }
  if (mode == "indian") {
    const auto indian = hc::NumFormat::invariant().with_grouping(hc::Grouping::indian);
    const auto verdict = hc::cast_i32("1,23,456", indian);
    if (verdict.ok()) {
      std::printf("read %d\n", verdict.value());
    } else {
      std::printf("malformed\n");
    }
    return 0;
  }
  if (mode == "retired") {
    // The contract violation with_grouping answers a bad policy with: the program ends here, on
    // any core, and nothing is printed.
    const auto policy = static_cast<std::uint32_t>(std::strtoul(argv[2], nullptr, 10));
    const auto stale = hc::NumFormat::invariant().with_grouping(static_cast<hc::Grouping>(policy));
    std::printf("accepted %u\n", static_cast<unsigned>(stale.raw().format.flags));
    return 0;
  }
  const bool current = mode == "accept";
  CHECK(current || mode == "refuse");

  const std::uint32_t version = hc::native_version_number();
  CHECK(version == hypercast_version());
  CHECK(hc::core_is_compatible() == current);
  CHECK(hc::detail::version_accepts(version, HYPERCAST_VERSION) == current);

  // A format is declared, or refused: against a core that reads by 1.0's rule, it never is.
  hc::NumFormat::Fields fields;
  fields.decimal_sep = ".";
  fields.group_sep = ",";
  CHECK(hc::NumFormat::declare(fields).has_value() == current); // Western, the default
  fields.grouping = hc::Grouping::western;
  CHECK(hc::NumFormat::declare(fields).has_value() == current);
  fields.grouping = hc::Grouping::indian;
  CHECK(hc::NumFormat::declare(fields).has_value() == current);
  // A policy bit or a style bit only the detailed check knows is refused by any core: the
  // plain check never saw them, so the header asks the detailed judge, which an older core lacks.
  fields.grouping = static_cast<hc::Grouping>(3 << 8);
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.grouping = static_cast<hc::Grouping>(1 << 8);
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.grouping = hc::Grouping::western;
  fields.styles = static_cast<hc::NumStyles>(1 << 10);
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.styles = static_cast<hc::NumStyles>(351); // 95 and the retired bit, at any core
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.styles = static_cast<hc::NumStyles>(863);
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.styles = hc::NumStyles::all;

  // A door that reads no numeral reaches any core through the 1.0 exports.
  CHECK(hc::cast_bool("yes").value());
  CHECK(hc::cast_date("2026-01-07").ok());

  // The date options and a month vocabulary need the export that reads them.
  CHECK(hc::DateOptions::declare(hc::DateStyles::common_forms).has_value() == current);
  CHECK(hc::DateOptions::declare(hc::DateStyles::none, 2029).has_value() == current);
  std::uint8_t storage[512];
  static const hc::MonthName months[] = {{1, "Jan"}, {2, "Feb"},  {3, "Mar"},  {4, "Apr"},
                                         {5, "May"}, {6, "Jun"},  {7, "Jul"},  {8, "Aug"},
                                         {9, "Sep"}, {10, "Oct"}, {11, "Nov"}, {12, "Dec"}};
  CHECK(hc::MonthNames::declare(months, storage).has_value() == current);

  if (current) {
    CHECK(!hc::cast_i32("12,34", hc::NumFormat::invariant()).ok());
    CHECK(hc::detailed::cast_i32("12,34", hc::NumFormat::invariant()).failure().detail ==
          hc::Detail::group_size);
  }
  std::printf("%u.%u.%u\n", version >> 16, (version >> 8) & 0xff, version & 0xff);
  return 0;
}
