// The downgrade test: the C++ header against a core that cannot drive it, which lacks the detailed
// exports or reads no grouping policy. A core drives a header when its major version is the
// header's and its minor at least the header's, whatever the patch. A strict request against one
// that cannot must be refused, never parsed as legacy, and a core that can must accept it.
//
// run.sh builds it with the archive's header rewritten to other versions (downgrade_leg there): a
// patch ahead of the archive's library (accept), a major version ahead (refuse) and, when
// SMOKE_CORE_10 names a directory holding a real 1.0 core (a build of the v1.0.0 tag), the header
// as the release after it has it, 1.1.0 (refuse), with the export that core lacks, cast_detailed,
// stood in for by a function that ends the run with status 99: the header may reach it only behind
// its version guard. Nothing here may call a symbol such a core lacks except through the guard.
//
// usage: smoke_downgrade accept|refuse     the declarations, and the plain doors
//        smoke_downgrade door              a strict plain door: parses nothing as legacy
//
// `door` prints `malformed` for `12,34` under strict grouping on a core that drives the header, and
// on one that cannot the header writes a line naming both versions to stderr and ends the program
// (std::abort) before it parses anything: it prints `parsed` only for the failure the guard exists
// to prevent, the legacy reading, 1234.

#include <cstdio>
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
  if (argc != 2) {
    std::fprintf(stderr, "usage: %s accept|refuse|door\n", argv[0]);
    return 2;
  }
  const std::string_view mode = argv[1];
  if (mode == "door") {
    // The strict default, at a plain door. On a core that cannot drive the header this aborts.
    const auto verdict = hc::cast_i32("12,34", hc::NumFormat::invariant());
    if (verdict.ok()) {
      std::printf("parsed %d\n", verdict.value());
    } else {
      std::printf("malformed\n");
    }
    return 0;
  }
  const bool current = mode == "accept";
  CHECK(current || mode == "refuse");

  const std::uint32_t version = hc::native_version_number();
  CHECK(version == hypercast_version());
  CHECK(hc::core_is_compatible() == current);
  CHECK(hc::detail::version_accepts(version, HYPERCAST_VERSION) == current);

  // A strict grouping is declared, or refused: against a core that would ignore it, it never is.
  hc::NumFormat::Fields fields;
  fields.decimal_sep = ".";
  fields.group_sep = ",";
  CHECK(hc::NumFormat::declare(fields).has_value() == current); // strict Western, the default
  fields.grouping = hc::Grouping::western;
  CHECK(hc::NumFormat::declare(fields).has_value() == current);
  fields.grouping = hc::Grouping::indian;
  CHECK(hc::NumFormat::declare(fields).has_value() == current);
  // A policy bit or a style bit only the detailed check knows is refused by any core: the
  // plain check never saw them, so the header asks the detailed judge, which an older core lacks.
  fields.grouping = static_cast<hc::Grouping>(3 << 8);
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.grouping = hc::Grouping::western;
  fields.styles = static_cast<hc::NumStyles>(1 << 10);
  CHECK(!hc::NumFormat::declare(fields).has_value());
  fields.styles = hc::NumStyles::all;

  // Legacy grouping is the 1.0 behaviour, and any core serves it through the 1.0 exports.
  fields.grouping = hc::Grouping::legacy;
  const auto legacy = hc::NumFormat::declare(fields);
  CHECK(legacy.has_value());
  CHECK(hc::cast_i32("12,34", *legacy).value() == 1234);
  CHECK(hc::cast_f64("1,2,3.5", *legacy).value() == 123.5);
  CHECK(hc::cast_decimal("12,34", *legacy).ok());
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
