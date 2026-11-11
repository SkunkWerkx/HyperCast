// Replays the conformance corpus (corpus/*.json at the repository root), the files every
// binding's suite replays, through hypercast.hpp: each vector's verdict, value and fault span,
// byte for byte, since a span here indexes the same UTF-8 the vector holds. A vector that
// fails is a break in the polyglot contract. Also holds every fault span inside its input.
// constraints.json goes through Constraints<T>::declare and the checked doors (refusals and
// the from_double doors included, floats compared by their bits, a violation's value held to
// the vector as a success's is), and declarations.json through ConstraintValue::decimal, the
// core's conversion of a declaration's text.
//
// Every row goes through the plain door and its detailed twin, which must agree on the reason
// and span, and every failure row's `detail` is held against the Failure's, by name and by
// code. A row's `grouping` declares its policy (absent is Western), and its `options` (flags,
// pivot, month names) go through the detailed date doors alone, which are the only ones to take
// them. grouping.json holds every shape to both policies and to detect(), and the retired value
// refused where it would first be declared.
// formats.json declares the formats, options and vocabularies and holds each to its verdict.
//
// usage: smoke_corpus <corpus directory>; prints the library's version when every vector
// passes, as the other smoke programs do. run.sh runs it against each build of the archive.

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <system_error>
#include <type_traits>
#include <utility>
#include <vector>

#include "hypercast.hpp"

namespace hc = hypercast;

namespace {

// Just enough JSON for the corpus: a number keeps its text, so a u64 or a double is read
// exactly by the one that needs it.
struct Json {
  enum class Kind { null, boolean, number, string, array, object } kind = Kind::null;
  bool boolean = false;
  std::string text;
  std::vector<Json> items;
  std::vector<std::pair<std::string, Json>> members;

  const Json *find(std::string_view key) const {
    for (const auto &member : members) {
      if (member.first == key) {
        return &member.second;
      }
    }
    return nullptr;
  }
  std::int64_t i64() const { return std::strtoll(text.c_str(), nullptr, 10); }
  std::uint64_t u64() const { return std::strtoull(text.c_str(), nullptr, 10); }
  double f64() const { return std::strtod(text.c_str(), nullptr); }
};

class Parser {
public:
  explicit Parser(std::string_view source) : source_(source) {}

  bool parse(Json &out) {
    if (!value(out)) {
      return false;
    }
    space();
    return at_ == source_.size();
  }

private:
  void space() {
    while (at_ < source_.size() && (source_[at_] == ' ' || source_[at_] == '\n' ||
                                    source_[at_] == '\r' || source_[at_] == '\t')) {
      ++at_;
    }
  }
  bool literal(std::string_view word) {
    if (source_.substr(at_, word.size()) != word) {
      return false;
    }
    at_ += word.size();
    return true;
  }
  static void utf8(std::string &out, std::uint32_t code) {
    if (code < 0x80) {
      out += static_cast<char>(code);
    } else if (code < 0x800) {
      out += static_cast<char>(0xC0 | code >> 6);
      out += static_cast<char>(0x80 | (code & 0x3F));
    } else if (code < 0x10000) {
      out += static_cast<char>(0xE0 | code >> 12);
      out += static_cast<char>(0x80 | (code >> 6 & 0x3F));
      out += static_cast<char>(0x80 | (code & 0x3F));
    } else {
      out += static_cast<char>(0xF0 | code >> 18);
      out += static_cast<char>(0x80 | (code >> 12 & 0x3F));
      out += static_cast<char>(0x80 | (code >> 6 & 0x3F));
      out += static_cast<char>(0x80 | (code & 0x3F));
    }
  }
  bool hex4(std::uint32_t &code) {
    if (at_ + 4 > source_.size()) {
      return false;
    }
    code = 0;
    for (int i = 0; i < 4; ++i) {
      const char c = source_[at_++];
      code <<= 4;
      if (c >= '0' && c <= '9') {
        code |= static_cast<std::uint32_t>(c - '0');
      } else if (c >= 'a' && c <= 'f') {
        code |= static_cast<std::uint32_t>(c - 'a' + 10);
      } else if (c >= 'A' && c <= 'F') {
        code |= static_cast<std::uint32_t>(c - 'A' + 10);
      } else {
        return false;
      }
    }
    return true;
  }
  bool string(std::string &out) {
    if (!literal("\"")) {
      return false;
    }
    while (at_ < source_.size()) {
      const char c = source_[at_++];
      if (c == '"') {
        return true;
      }
      if (c != '\\') {
        out += c;
        continue;
      }
      if (at_ >= source_.size()) {
        return false;
      }
      switch (source_[at_++]) {
      case '"':
        out += '"';
        break;
      case '\\':
        out += '\\';
        break;
      case '/':
        out += '/';
        break;
      case 'b':
        out += '\b';
        break;
      case 'f':
        out += '\f';
        break;
      case 'n':
        out += '\n';
        break;
      case 'r':
        out += '\r';
        break;
      case 't':
        out += '\t';
        break;
      case 'u': {
        std::uint32_t code = 0;
        if (!hex4(code)) {
          return false;
        }
        if (code >= 0xD800 && code < 0xDC00) {
          std::uint32_t low = 0;
          if (!literal("\\u") || !hex4(low) || low < 0xDC00 || low >= 0xE000) {
            return false;
          }
          code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
        }
        utf8(out, code);
        break;
      }
      default:
        return false;
      }
    }
    return false;
  }
  bool value(Json &out) {
    space();
    if (at_ >= source_.size()) {
      return false;
    }
    const char c = source_[at_];
    if (c == '{') {
      ++at_;
      out.kind = Json::Kind::object;
      space();
      if (literal("}")) {
        return true;
      }
      do {
        space();
        std::string key;
        Json member;
        if (!string(key)) {
          return false;
        }
        space();
        if (!literal(":") || !value(member)) {
          return false;
        }
        out.members.emplace_back(std::move(key), std::move(member));
        space();
      } while (literal(","));
      return literal("}");
    }
    if (c == '[') {
      ++at_;
      out.kind = Json::Kind::array;
      space();
      if (literal("]")) {
        return true;
      }
      do {
        Json item;
        if (!value(item)) {
          return false;
        }
        out.items.push_back(std::move(item));
        space();
      } while (literal(","));
      return literal("]");
    }
    if (c == '"') {
      out.kind = Json::Kind::string;
      return string(out.text);
    }
    if (literal("true")) {
      out.kind = Json::Kind::boolean;
      out.boolean = true;
      return true;
    }
    if (literal("false")) {
      out.kind = Json::Kind::boolean;
      return true;
    }
    if (literal("null")) {
      return true;
    }
    const std::size_t start = at_;
    while (at_ < source_.size() && std::strchr("+-0123456789.eE", source_[at_]) != nullptr) {
      ++at_;
    }
    out.kind = Json::Kind::number;
    out.text = std::string(source_.substr(start, at_ - start));
    return at_ > start;
  }

  std::string_view source_;
  std::size_t at_ = 0;
};

int failures = 0;
int vectors = 0;
// How many declaration rows of each formats.json kind were judged.
std::map<std::string, int> declarations;

void fail(const char *file, const Json &vector, const char *what) {
  ++failures;
  const Json *input = vector.find("input");
  std::fprintf(stderr, "corpus: %s: \"%s\": %s\n", file, input ? input->text.c_str() : "?", what);
}

const char *reason_name(hc::CastFailure reason) {
  switch (reason) {
  case hc::CastFailure::empty:
    return "empty";
  case hc::CastFailure::malformed:
    return "malformed";
  case hc::CastFailure::out_of_range:
    return "out_of_range";
  }
  return "?";
}

// The published details, by their code (the spec's table), restated so that a corpus row's name
// is held to the number the core reports as well as to the name the header gives it.
const char *const detail_names[] = {"",
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

// The code a corpus detail name stands for, or 0 for a name that is none.
std::uint32_t detail_code(const std::string &name) {
  for (std::uint32_t code = 1; code < sizeof detail_names / sizeof *detail_names; ++code) {
    if (name == detail_names[code]) {
      return code;
    }
  }
  return 0;
}

// A failure row's `detail`, held against the Failure's: the row names one, the header names the
// Failure's the same, and the raw code is the one the table gives that name.
void check_detail(const char *file, const Json &vector, const hc::Failure &failure) {
  const Json *named = vector.find("detail");
  if (named == nullptr) {
    fail(file, vector, "a failure row names no detail");
    return;
  }
  const std::uint32_t code = detail_code(named->text);
  if (code == 0) {
    fail(file, vector, "the row's detail is no published one");
  } else if (failure.detail_code() != code) {
    fail(file, vector, "wrong detail code");
  }
  if (hc::name(failure.detail) != named->text) {
    fail(file, vector, "wrong detail name");
  }
}

// The vector's "expect" and "fault" against a fault's reason and span.
void check_fault(const char *file, const Json &vector, const hc::Fault &fault,
                 std::size_t input_size) {
  const std::string &expect = vector.find("expect")->text;
  if (expect != reason_name(fault.reason)) {
    fail(file, vector, "wrong verdict");
  }
  if (fault.offset + fault.length > input_size) {
    fail(file, vector, "fault span outside the input");
  }
  if (const Json *span = vector.find("fault")) {
    if (span->items.size() != 2 || span->items[0].u64() != fault.offset ||
        span->items[1].u64() != fault.length) {
      fail(file, vector, "wrong fault span");
    }
  }
}

// The vector's "expect" and "fault" against a detailed verdict, then its `detail`; `same` holds
// the value. `plain`, when the row takes no options (the plain doors never see the new shapes),
// is the plain door's verdict, which must be this one's projection.
template <class T, class Same>
void check(const char *file, const Json &vector, const std::optional<hc::Verdict<T>> &plain,
           const hc::DetailedVerdict<T> &verdict, std::size_t input_size, Same same) {
  ++vectors;
  const std::string &expect = vector.find("expect")->text;
  if (plain.has_value() && !(*plain == verdict.to_verdict())) {
    fail(file, vector, "the plain door and the detailed door disagree");
  }
  if (const T *value = verdict.value_if()) {
    if (expect != "ok") {
      fail(file, vector, "parsed, expected a fault");
    } else if (!same(*value)) {
      fail(file, vector, "wrong value");
    }
    if (vector.find("detail") != nullptr) {
      fail(file, vector, "names a detail but parsed");
    }
    return;
  }
  const hc::Failure &failure = verdict.failure();
  check_fault(file, vector, failure.fault(), input_size);
  check_detail(file, vector, failure);
}

// A row with no options: the plain door's verdict beside its detailed twin's.
template <class T, class Same>
void check(const char *file, const Json &vector, const hc::Verdict<T> &plain,
           const hc::DetailedVerdict<T> &verdict, std::size_t input_size, Same same) {
  check(file, vector, std::optional<hc::Verdict<T>>(plain), verdict, input_size, same);
}

// A row with options: only the detailed door takes them.
template <class T, class Same>
void check(const char *file, const Json &vector, const hc::DetailedVerdict<T> &verdict,
           std::size_t input_size, Same same) {
  check(file, vector, std::optional<hc::Verdict<T>>(), verdict, input_size, same);
}

std::string decimal_text(const hc::Decimal &decimal) {
  char buffer[hc::Decimal::max_chars];
  return std::string(buffer, decimal.to_chars(buffer, buffer + sizeof buffer).ptr);
}

bool same_decimal(const Json &vector, const hc::Decimal &value) {
  const hc::Decimal magnitude{value.lo, value.hi, 0, false};
  return decimal_text(value) == vector.find("value")->text &&
         decimal_text(magnitude) == vector.find("magnitude")->text &&
         value.scale == vector.find("scale")->u64() &&
         value.negative == vector.find("negative")->boolean;
}

bool same_instant(const Json &vector, const hc::Timestamp &value) {
  return value.seconds == vector.find("seconds")->i64() &&
         value.nanos == vector.find("nanos")->i64();
}

bool same_span(const Json &vector, const hc::Duration &value) {
  return value.seconds == vector.find("seconds")->i64() &&
         value.nanos == vector.find("nanos")->i64();
}

bool same_date(const Json &vector, const hc::Date &value) {
  return value.year == vector.find("year")->u64() && value.month == vector.find("month")->u64() &&
         value.day == vector.find("day")->u64();
}

bool same_civil(const Json &vector, const hc::CivilDateTime &value) {
  return same_date(vector, value.date) && value.nanos_of_day == vector.find("nanos_of_day")->u64();
}

// The grouping policy bits a format declares: `grouping` is "western" or "indian", and absent
// means the policy bits its `flags` carry (none: Western), so a `formats.json` row can declare a
// policy no name spells.
std::uint32_t grouping_of(const Json &format) {
  const Json *named = format.find("grouping");
  if (named == nullptr) {
    return static_cast<std::uint32_t>(format.find("flags")->u64()) &
           HyperCastNumFormat_GROUPING_POLICY;
  }
  if (named->text == "western") {
    return HyperCastNumFormat_GROUPING_WESTERN;
  }
  if (named->text == "indian") {
    return HyperCastNumFormat_GROUPING_INDIAN;
  }
  ++failures;
  std::fprintf(stderr, "corpus: unknown grouping \"%s\"\n", named->text.c_str());
  return HyperCastNumFormat_GROUPING_WESTERN;
}

// What a row's `format` declares, field for field: the styles are `flags` without its policy
// bits (a style mask never carries one), which `grouping` names.
hc::NumFormat::Fields fields_of(const Json &format) {
  auto symbol = [&](const char *key) {
    const Json *field = format.find(key);
    return field ? std::string_view(field->text) : std::string_view();
  };
  hc::NumFormat::Fields fields;
  fields.decimal_sep = symbol("decimal_sep");
  fields.group_sep = symbol("group_sep");
  fields.styles =
      static_cast<hc::NumStyles>(static_cast<std::uint32_t>(format.find("flags")->u64()) &
                                 ~static_cast<std::uint32_t>(HyperCastNumFormat_GROUPING_POLICY));
  fields.grouping = static_cast<hc::Grouping>(grouping_of(format));
  fields.currency = symbol("currency");
  fields.currency_decimal_sep = symbol("currency_decimal_sep");
  fields.currency_group_sep = symbol("currency_group_sep");
  fields.minus_sign = symbol("minus_sign");
  fields.plus_sign = symbol("plus_sign");
  fields.percent = symbol("percent");
  // The digit zero is one character of UTF-8; the core takes its code point.
  const std::string_view zero = symbol("digit_zero");
  if (!zero.empty()) {
    auto cp = hc::cast_char(zero);
    fields.digit_zero = cp.value();
  }
  return fields;
}

// The format a row declares. Without a `format` the row reads as the core's null format does:
// the invariant one.
hc::NumFormat format_of(const char *file, const Json &vector) {
  const Json *format = vector.find("format");
  if (format == nullptr) {
    return hc::NumFormat::invariant();
  }
  if (auto declared = hc::NumFormat::declare(fields_of(*format))) {
    return *declared;
  }
  fail(file, vector, "the core refuses the vector's format");
  return hc::NumFormat::invariant();
}

// The date options and month names a row's `options` declare: flags by name, a pivot and a
// vocabulary. The names are views into the row, packed into this struct's own storage, which
// is why it is built in place and never copied.
struct Options {
  hc::DateOptions options;
  hc::MonthNames months;
  std::uint8_t storage[2048];
  Options() = default;
  Options(const Options &) = delete;
  Options &operator=(const Options &) = delete;
};

hc::DateStyles styles_of(const Json &options, bool &known) {
  hc::DateStyles styles = hc::DateStyles::none;
  if (const Json *flags = options.find("flags")) {
    for (const Json &flag : flags->items) {
      if (flag.text == "compact_date") {
        styles = styles | hc::DateStyles::compact_date;
      } else if (flag.text == "space_separator") {
        styles = styles | hc::DateStyles::space_separator;
      } else if (flag.text == "hour_offset") {
        styles = styles | hc::DateStyles::hour_offset;
      } else if (flag.text == "common_forms") {
        styles = styles | hc::DateStyles::common_forms;
      } else {
        known = false;
      }
    }
  }
  return styles;
}

bool options_of(const char *file, const Json &vector, Options &out) {
  const Json &options = *vector.find("options");
  bool known = true;
  const hc::DateStyles styles = styles_of(options, known);
  const Json *pivot = options.find("pivot");
  if (!known) {
    fail(file, vector, "unknown date option flag");
    return false;
  }
  if (auto declared =
          hc::DateOptions::declare(styles, pivot ? static_cast<std::uint32_t>(pivot->u64()) : 0)) {
    out.options = *declared;
  } else {
    fail(file, vector, "the core refuses the vector's date options");
    return false;
  }
  if (const Json *months = options.find("months")) {
    std::vector<hc::MonthName> entries;
    for (const Json &pair : months->items) {
      entries.push_back({static_cast<std::uint8_t>(pair.items[0].u64()), pair.items[1].text});
    }
    if (auto declared = hc::MonthNames::declare(entries.data(), entries.size(), out.storage)) {
      out.months = *declared;
    } else {
      fail(file, vector, "the core refuses the vector's month names");
      return false;
    }
  }
  return true;
}

// A plain door as a callable: the name is an overload set, its checked door beside it.
#define PLAIN(door) [](hc::Text text, const hc::NumFormat &format) { return door(text, format); }

template <class T, class Plain, class Detailed>
void integer(const char *file, const Json &vector, Plain plain, Detailed detailed) {
  const std::string &input = vector.find("input")->text;
  const hc::NumFormat format = format_of(file, vector);
  check(file, vector, plain(input, format), detailed(input, format), input.size(), [&](T value) {
    const Json &expected = *vector.find("value");
    if constexpr (std::is_signed_v<T>) {
      return static_cast<std::int64_t>(value) == expected.i64();
    } else {
      return expected.text[0] != '-' && static_cast<std::uint64_t>(value) == expected.u64();
    }
  });
}

// The plain shapes the constraints work must leave alone: Fault's fields and Verdict's layout
// and accessors. A change to either fails to compile here.
static_assert(std::is_same_v<decltype(hc::Fault::reason), hc::CastFailure> &&
                  std::is_same_v<decltype(hc::Fault::offset), std::size_t> &&
                  std::is_same_v<decltype(hc::Fault::length), std::size_t>,
              "Fault is a reason and a byte span");
struct FaultShape {
  hc::CastFailure reason;
  std::size_t offset;
  std::size_t length;
};
static_assert(sizeof(hc::Fault) == sizeof(FaultShape), "Fault holds nothing else");
static_assert(std::is_same_v<std::underlying_type_t<hc::CastFailure>, std::int32_t> &&
                  static_cast<int>(hc::CastFailure::empty) == 1 &&
                  static_cast<int>(hc::CastFailure::malformed) == 2 &&
                  static_cast<int>(hc::CastFailure::out_of_range) == 3,
              "the reasons are the core's codes");
struct VerdictShape {
  std::int32_t value;
  hc::Fault fault;
  bool ok;
};
static_assert(sizeof(hc::Verdict<std::int32_t>) == sizeof(VerdictShape),
              "a Verdict is the value, a Fault and which");
static_assert(!std::is_default_constructible_v<hc::Verdict<std::int32_t>> &&
                  std::is_trivially_copyable_v<hc::Verdict<std::int32_t>>,
              "a Verdict is always one case, and plain");
static_assert(
    std::is_same_v<decltype(&hc::Verdict<std::int32_t>::ok),
                   bool (hc::Verdict<std::int32_t>::*)() const noexcept> &&
        std::is_same_v<decltype(&hc::Verdict<std::int32_t>::value_if),
                       const std::int32_t *(hc::Verdict<std::int32_t>::*)() const noexcept> &&
        std::is_same_v<decltype(&hc::Verdict<std::int32_t>::fault_if),
                       const hc::Fault *(hc::Verdict<std::int32_t>::*)() const noexcept> &&
        std::is_same_v<decltype(&hc::Verdict<std::int32_t>::value),
                       const std::int32_t &(hc::Verdict<std::int32_t>::*)() const noexcept> &&
        std::is_same_v<decltype(&hc::Verdict<std::int32_t>::fault),
                       const hc::Fault &(hc::Verdict<std::int32_t>::*)() const noexcept>,
    "Verdict's accessors keep their signatures");
static_assert(std::is_same_v<decltype(hc::cast_i32("1", hc::NumFormat::invariant())),
                             hc::Verdict<std::int32_t>> &&
                  std::is_same_v<decltype(hc::cast_text("a")), hc::Verdict<std::string_view>>,
              "the plain doors return the plain Verdict");
// Every refusal message: the core's codes worded from the Rust Refusal Display text (as every
// binding words them), and the header's own code 0, which is no core refusal.
static_assert(hc::Refusal{1}.message() ==
                  "Constraints refused (code 1): a constraint the door does not take.",
              "refusal 1's message");
static_assert(hc::Refusal{5}.message() ==
                  "Constraints refused (code 5): multipleOf is not positive.",
              "refusal 5's message");
static_assert(hc::Refusal{6}.message() ==
                  "Constraints refused (code 6): a rounding without multipleOf.",
              "refusal 6's message");
static_assert(hc::Refusal{8}.message() ==
                  "Constraints refused (code 8): a bound, step or enum entry the door cannot hold.",
              "refusal 8's message");
static_assert(hc::Refusal{9}.message() ==
                  "Constraints refused (code 9): an enum entry that is not UTF-8.",
              "refusal 9's message");
static_assert(hc::Refusal{11}.message() ==
                  "Constraints refused (code 11): a malformed constraints layout.",
              "refusal 11's message");
static_assert(!std::is_convertible_v<std::int32_t, hc::Refusal>,
              "a Refusal is never made from a bare integer");
static_assert(!std::is_constructible_v<hc::Declared<hc::Constraints<double>>, int>,
              "nor is a Declared");
static_assert(hc::Refusal{14}.message() ==
                  "Constraints refused (code 14): a rule this binding does not name.",
              "a code this header does not name keeps its number");
static_assert(hc::Refusal{2}.message() ==
                  "Constraints refused (code 2): a rule this binding does not name.",
              "a retired code is one this header does not name");
static_assert(hc::Refusal{0}.message() ==
                  "The enum list does not fit: the EnumStorage is smaller than "
                  "storage_size(entries), or a text entry is longer than 4294967295 UTF-8 bytes.",
              "the header's own refusal");
static_assert(std::string_view(hc::Refusal{0}.message().c_str()).find("Constraints refused") ==
                  std::string_view::npos,
              "code 0 is not worded as a core refusal");
// No view into a temporary: a Refusal's message is its own value, and every accessor that
// views into its object (a RefusalMessage's, a KeywordName's, a NumFormat's or DateFormat's
// symbols) takes a named one; on a temporary the call does not compile.
template <class T, class = void> struct views_into : std::false_type {};
template <class T>
struct views_into<T, std::void_t<decltype(std::declval<T>().view())>> : std::true_type {};
template <class T, class = void> struct symbol_of : std::false_type {};
template <class T>
struct symbol_of<T, std::void_t<decltype(std::declval<T>().currency())>> : std::true_type {};
template <class T, class = void> struct period_of : std::false_type {};
template <class T>
struct period_of<T, std::void_t<decltype(std::declval<T>().am())>> : std::true_type {};
static_assert(
    std::is_same_v<decltype(std::declval<const hc::Refusal &>().message()), hc::RefusalMessage> &&
        !std::is_convertible_v<hc::RefusalMessage, std::string_view>,
    "a refusal's message is held by value, never a view into the Refusal");
static_assert(views_into<const hc::RefusalMessage &>::value &&
                  !views_into<hc::RefusalMessage>::value &&
                  !views_into<const hc::RefusalMessage>::value,
              "RefusalMessage::view() only from a named message");
static_assert(views_into<const hc::KeywordName &>::value && !views_into<hc::KeywordName>::value,
              "KeywordName::view() only from a named name");
static_assert(symbol_of<const hc::NumFormat &>::value && !symbol_of<hc::NumFormat>::value,
              "a NumFormat's symbols only from a named format");
static_assert(period_of<const hc::DateFormat &>::value && !period_of<hc::DateFormat>::value,
              "a DateFormat's day periods only from a named format");
static_assert(sizeof(hc::Refusal) == sizeof(std::int32_t), "a Refusal is its code");
// The constraints layout as the header carries it.
static_assert(sizeof(HyperCastRawConstraints) == HyperCastRawConstraints_SIZE &&
                  alignof(HyperCastRawConstraints) == 8 &&
                  sizeof(HyperCastRawCheck) == HyperCastRawCheck_SIZE &&
                  sizeof(HyperCastRawSpan) == HyperCastRawSpan_SIZE,
              "RawConstraints 104, RawCheck 12, RawSpan 8");

// The double a constraints vector names: a number, or NaN, Infinity, -Infinity and -0, which
// JSON cannot spell, as strings; strtod reads both.
double number_of(const Json &json) { return json.f64(); }

template <class F> auto bits(F value) {
  std::conditional_t<sizeof(F) == 8, std::uint64_t, std::uint32_t> out = 0;
  std::memcpy(&out, &value, sizeof value);
  return out;
}

hc::Rounding rounding_of(const Json *json) {
  if (json == nullptr || json->text == "strict") {
    return hc::Rounding::strict;
  }
  if (json->text == "midpoint") {
    return hc::Rounding::midpoint;
  }
  if (json->text == "floor") {
    return hc::Rounding::floor;
  }
  return hc::Rounding::ceiling;
}

template <class T>
hc::Checked<T> checked_door(hc::Text text, const hc::NumFormat &format,
                            const hc::Constraints<T> &constraints) {
  if constexpr (std::is_same_v<T, std::int8_t>) {
    return hc::cast_i8(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::int16_t>) {
    return hc::cast_i16(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::int32_t>) {
    return hc::cast_i32(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::int64_t>) {
    return hc::cast_i64(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::uint8_t>) {
    return hc::cast_u8(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::uint16_t>) {
    return hc::cast_u16(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::uint32_t>) {
    return hc::cast_u32(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::uint64_t>) {
    return hc::cast_u64(text, format, constraints);
  } else if constexpr (std::is_same_v<T, float>) {
    return hc::cast_f32(text, format, constraints);
  } else if constexpr (std::is_same_v<T, double>) {
    return hc::cast_f64(text, format, constraints);
  } else if constexpr (std::is_same_v<T, hc::Decimal>) {
    return hc::cast_decimal(text, format, constraints);
  } else {
    return hc::cast_text(text, constraints);
  }
}

template <class T>
hc::Checked<T> checked_from_double(double value, const hc::Constraints<T> &constraints) {
  if constexpr (std::is_same_v<T, std::int8_t>) {
    return hc::cast_i8_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::int16_t>) {
    return hc::cast_i16_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::int32_t>) {
    return hc::cast_i32_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::int64_t>) {
    return hc::cast_i64_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::uint8_t>) {
    return hc::cast_u8_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::uint16_t>) {
    return hc::cast_u16_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::uint32_t>) {
    return hc::cast_u32_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::uint64_t>) {
    return hc::cast_u64_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, float>) {
    return hc::cast_f32_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, double>) {
    return hc::cast_f64_from_double(value, constraints);
  } else {
    return hc::cast_decimal_from_double(value, constraints);
  }
}

// The same doors under hypercast::detailed.
template <class T>
hc::DetailedChecked<T> detailed_checked_door(hc::Text text, const hc::NumFormat &format,
                                             const hc::Constraints<T> &constraints) {
  if constexpr (std::is_same_v<T, std::int8_t>) {
    return hc::detailed::cast_i8(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::int16_t>) {
    return hc::detailed::cast_i16(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::int32_t>) {
    return hc::detailed::cast_i32(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::int64_t>) {
    return hc::detailed::cast_i64(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::uint8_t>) {
    return hc::detailed::cast_u8(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::uint16_t>) {
    return hc::detailed::cast_u16(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::uint32_t>) {
    return hc::detailed::cast_u32(text, format, constraints);
  } else if constexpr (std::is_same_v<T, std::uint64_t>) {
    return hc::detailed::cast_u64(text, format, constraints);
  } else if constexpr (std::is_same_v<T, float>) {
    return hc::detailed::cast_f32(text, format, constraints);
  } else if constexpr (std::is_same_v<T, double>) {
    return hc::detailed::cast_f64(text, format, constraints);
  } else if constexpr (std::is_same_v<T, hc::Decimal>) {
    return hc::detailed::cast_decimal(text, format, constraints);
  } else {
    return hc::detailed::cast_text(text, constraints);
  }
}

template <class T>
hc::DetailedChecked<T> detailed_checked_from_double(double value,
                                                    const hc::Constraints<T> &constraints) {
  if constexpr (std::is_same_v<T, std::int8_t>) {
    return hc::detailed::cast_i8_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::int16_t>) {
    return hc::detailed::cast_i16_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::int32_t>) {
    return hc::detailed::cast_i32_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::int64_t>) {
    return hc::detailed::cast_i64_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::uint8_t>) {
    return hc::detailed::cast_u8_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::uint16_t>) {
    return hc::detailed::cast_u16_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::uint32_t>) {
    return hc::detailed::cast_u32_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, std::uint64_t>) {
    return hc::detailed::cast_u64_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, float>) {
    return hc::detailed::cast_f32_from_double(value, constraints);
  } else if constexpr (std::is_same_v<T, double>) {
    return hc::detailed::cast_f64_from_double(value, constraints);
  } else {
    return hc::detailed::cast_decimal_from_double(value, constraints);
  }
}

// The checked value against the vector's "value": integers exactly, floats by their bits (so
// a -0.0 fails), a decimal by its sign, magnitude and scale (its canonical text), and text by
// its bytes.
template <class T> bool same_checked_value(const Json &expected, const T &value) {
  if constexpr (std::is_same_v<T, double>) {
    return bits(value) == bits(std::strtod(expected.text.c_str(), nullptr));
  } else if constexpr (std::is_same_v<T, float>) {
    return bits(value) == bits(std::strtof(expected.text.c_str(), nullptr));
  } else if constexpr (std::is_same_v<T, hc::Decimal>) {
    const std::string &text = expected.text;
    const std::size_t point = text.find('.');
    const std::size_t scale = point == std::string::npos ? 0 : text.size() - point - 1;
    return decimal_text(value) == text && value.scale == scale &&
           value.negative == (text[0] == '-');
  } else if constexpr (std::is_same_v<T, std::string_view>) {
    return value == expected.text;
  } else if constexpr (std::is_signed_v<T>) {
    return static_cast<std::int64_t>(value) == expected.i64();
  } else {
    return expected.text[0] != '-' && static_cast<std::uint64_t>(value) == expected.u64();
  }
}

// One constraints vector: the declaration through Constraints<T>::declare, the public
// factory (refused, with the vector's code), then the cast through the checked door.
template <class T> void constrained(const char *file, const Json &vector) {
  using C = hc::Constraints<T>;
  const Json &declared = *vector.find("constraints");
  typename C::Fields fields;
  const auto value = [&](const char *key, std::optional<hc::ConstraintValue> &slot) {
    if (const Json *json = declared.find(key)) {
      slot = hc::ConstraintValue(std::string_view(json->text));
    }
  };
  value("minimum", fields.minimum);
  value("exclusiveMinimum", fields.exclusive_minimum);
  value("maximum", fields.maximum);
  value("exclusiveMaximum", fields.exclusive_maximum);
  value("multipleOf", fields.multiple_of);
  fields.rounding = rounding_of(declared.find("rounding"));
  if (const Json *json = declared.find("minLength")) {
    fields.min_length = static_cast<std::uint32_t>(json->u64());
  }
  if (const Json *json = declared.find("maxLength")) {
    fields.max_length = static_cast<std::uint32_t>(json->u64());
  }
  if (const Json *json = declared.find("trim")) {
    fields.trim = json->boolean;
  }
  if (const Json *json = declared.find("caseSensitive")) {
    fields.case_sensitive = json->boolean;
  }
  std::vector<typename C::Entry> entries;
  std::uint8_t storage[1024];
  const Json *list = declared.find("enum");
  if (list != nullptr) {
    for (const Json &entry : list->items) {
      entries.emplace_back(std::string_view(entry.text));
    }
  }
  const hc::Declared<C> declaration =
      list != nullptr ? C::declare(fields, entries.data(), entries.size(), storage)
                      : C::declare(fields);

  ++vectors;
  const std::string &expect = vector.find("expect")->text;
  if (expect == "refused") {
    const hc::Refusal *refusal = declaration.refusal_if();
    const std::int64_t code = vector.find("refusal")->i64();
    if (refusal == nullptr) {
      fail(file, vector, "declared, expected a refusal");
    } else if (refusal->code != code) {
      fail(file, vector, "wrong refusal code");
    } else if (const hc::RefusalMessage message = refusal->message();
               message.view().find("Constraints refused (code " + std::to_string(code) + "): ") !=
               0) {
      fail(file, vector, "wrong refusal message");
    }
    return;
  }
  const C *constraints = declaration.value_if();
  if (constraints == nullptr) {
    fail(file, vector, "refused, expected a declaration");
    return;
  }

  std::string bytes;
  std::size_t input_size = 0;
  std::optional<hc::Checked<T>> checked;
  std::optional<hc::DetailedChecked<T>> detailed;
  if (const Json *number = vector.find("from_f64")) {
    if constexpr (C::is_text) {
      fail(file, vector, "a double at the text door");
      return;
    } else {
      checked = checked_from_double(number_of(*number), *constraints);
      detailed = detailed_checked_from_double(number_of(*number), *constraints);
    }
  } else {
    if (const Json *raw = vector.find("input_bytes")) {
      for (const Json &byte : raw->items) {
        bytes += static_cast<char>(byte.u64());
      }
    } else {
      bytes = vector.find("input")->text;
    }
    input_size = bytes.size();
    const hc::Text text(reinterpret_cast<const std::uint8_t *>(bytes.data()), bytes.size());
    const hc::NumFormat format = format_of(file, vector);
    checked = checked_door(text, format, *constraints);
    detailed = detailed_checked_door(text, format, *constraints);
  }
  // The detailed door is the plain one's answer with the detail added.
  if (!(detailed->to_checked() == *checked)) {
    fail(file, vector, "the plain door and the detailed door disagree");
  }

  if (const hc::CheckedValue<T> *ok = checked->value_if()) {
    if (expect != "ok") {
      fail(file, vector, "passed, expected a rejection");
      return;
    }
    if (!same_checked_value(*vector.find("value"), ok->value)) {
      fail(file, vector, "wrong value");
    }
    if (ok->adjusted != vector.find("adjusted")->boolean) {
      fail(file, vector, "wrong adjusted");
    }
    const Json *index = vector.find("enum_index");
    if (index == nullptr ? ok->enum_index.has_value()
                         : ok->enum_index != static_cast<std::uint32_t>(index->u64())) {
      fail(file, vector, "wrong enum_index");
    }
    if constexpr (C::is_text) {
      // The caller's own bytes: the declared entry in its storage, or the input.
      const auto *at = reinterpret_cast<const std::uint8_t *>(ok->value.data());
      const bool in_storage = at >= storage && at + ok->value.size() <= storage + sizeof storage;
      const bool in_input = ok->value.data() >= bytes.data() &&
                            ok->value.data() + ok->value.size() <= bytes.data() + bytes.size();
      if (ok->enum_index.has_value() ? !in_storage : !in_input) {
        fail(file, vector, "the text is not a view of the caller's own bytes");
      }
    }
    return;
  }
  const hc::Rejection<T> &rejection = checked->rejection();
  if (const hc::Failure *failure = detailed->rejection().failure_if()) {
    check_detail(file, vector, *failure);
  } else if (vector.find("detail") != nullptr) {
    fail(file, vector, "names a detail but is a violation");
  }
  std::size_t offset = 0;
  std::size_t length = 0;
  if (const hc::Violation<T> *violation = rejection.violation_if()) {
    if (expect != "violation") {
      fail(file, vector, "a violation, expected otherwise");
    } else if (hc::name(violation->keyword) != vector.find("keyword")->text) {
      fail(file, vector, "wrong keyword");
    } else if (const Json *value = vector.find("value");
               value == nullptr || !same_checked_value(*value, violation->value)) {
      // The value as the door cast it, held to the vector as a success's is.
      fail(file, vector, "wrong violation value");
    }
    if constexpr (C::is_text) {
      // The judged text, a view of the caller's input.
      if (violation->value.data() < bytes.data() ||
          violation->value.data() + violation->value.size() > bytes.data() + bytes.size()) {
        fail(file, vector, "the violation's text is not a view of the caller's input");
      }
    }
    offset = violation->offset;
    length = violation->length;
  } else {
    const hc::Fault &fault = rejection.fault();
    if (expect != reason_name(fault.reason)) {
      fail(file, vector, "wrong verdict");
    }
    offset = fault.offset;
    length = fault.length;
  }
  if (offset + length > input_size) {
    fail(file, vector, "span outside the input");
  }
  if (const Json *span = vector.find("fault")) {
    if (span->items.size() != 2 || span->items[0].u64() != offset ||
        span->items[1].u64() != length) {
      fail(file, vector, "wrong span");
    }
  }
}

void constraints(const char *file, const Json &vector) {
  const std::string &door = vector.find("door")->text;
  if (door == "i8") {
    constrained<std::int8_t>(file, vector);
  } else if (door == "i16") {
    constrained<std::int16_t>(file, vector);
  } else if (door == "i32") {
    constrained<std::int32_t>(file, vector);
  } else if (door == "i64") {
    constrained<std::int64_t>(file, vector);
  } else if (door == "u8") {
    constrained<std::uint8_t>(file, vector);
  } else if (door == "u16") {
    constrained<std::uint16_t>(file, vector);
  } else if (door == "u32") {
    constrained<std::uint32_t>(file, vector);
  } else if (door == "u64") {
    constrained<std::uint64_t>(file, vector);
  } else if (door == "f32") {
    constrained<float>(file, vector);
  } else if (door == "f64") {
    constrained<double>(file, vector);
  } else if (door == "decimal") {
    constrained<hc::Decimal>(file, vector);
  } else if (door == "text") {
    constrained<std::string_view>(file, vector);
  } else {
    fail(file, vector, "unknown constraints door");
  }
}

// A declaration's decimal text through ConstraintValue::decimal, the core's conversion: the
// sign, magnitude and scale, or the refusal 8. "input_bytes" holds text that is not UTF-8.
void declaration(const char *file, const Json &vector) {
  ++vectors;
  std::string bytes;
  if (const Json *raw = vector.find("input_bytes")) {
    for (const Json &byte : raw->items) {
      bytes += static_cast<char>(byte.u64());
    }
  } else {
    bytes = vector.find("input")->text;
  }
  const std::optional<hc::Decimal> decimal = hc::ConstraintValue(std::string_view(bytes)).decimal();
  if (vector.find("expect")->text == "refused") {
    if (decimal.has_value() || vector.find("refusal")->i64() != 8) {
      fail(file, vector, "converted, expected the refusal 8");
    }
  } else if (!decimal.has_value()) {
    fail(file, vector, "refused, expected a decimal");
  } else if (!same_decimal(vector, *decimal)) {
    fail(file, vector, "wrong decimal");
  }
}

// The packed bytes of a month vocabulary a `month_names` row declares: each entry's month byte,
// little-endian length and name, then any `append_bytes` the row adds raw (a refusal row's way
// to cut an entry short or leave trailing bytes the builder cannot make).
std::vector<std::uint8_t> packed_months(const Json &row) {
  std::vector<std::uint8_t> packed;
  for (const Json &pair : row.find("months")->items) {
    packed.push_back(static_cast<std::uint8_t>(pair.items[0].u64()));
    const std::string &name = pair.items[1].text;
    for (int shift = 0; shift < 32; shift += 8) {
      packed.push_back(static_cast<std::uint8_t>(name.size() >> shift));
    }
    packed.insert(packed.end(), name.begin(), name.end());
  }
  if (const Json *raw = row.find("append_bytes")) {
    for (const Json &byte : raw->items) {
      packed.push_back(static_cast<std::uint8_t>(byte.u64()));
    }
  }
  return packed;
}

// A declaration row of formats.json: `expect` "ok" or "refused" (its `rule` is the Rust core's
// ordering label, which the C++ declarations do not distinguish: each answers only with
// std::nullopt). `white_space` rows pin the core's predicate, which C++ delegates to rather
// than restates, so they are not replayed here. `date_options` declares flags (by name, or as
// `flag_bits` when no name spells them) and a pivot; `month_names` a vocabulary, which the
// builder and the packed layout must each judge as the row says.
void format_declaration(const char *file, const Json &vector) {
  const std::string &kind = vector.find("kind")->text;
  if (kind == "white_space") {
    return;
  }
  ++vectors;
  bool declared = false;
  if (kind == "num_format") {
    declared = hc::NumFormat::declare(fields_of(*vector.find("format"))).has_value();
  } else if (kind == "day_periods") {
    const Json *am = vector.find("am");
    const Json *pm = vector.find("pm");
    declared =
        hc::DateFormat::declare(hc::DateOrder::year_month_day, am ? std::string_view(am->text) : "",
                                pm ? std::string_view(pm->text) : "")
            .has_value();
  } else if (kind == "date_options") {
    bool known = true;
    hc::DateStyles styles = styles_of(vector, known);
    if (const Json *bits = vector.find("flag_bits")) {
      styles = static_cast<hc::DateStyles>(bits->u64());
    }
    const Json *pivot = vector.find("pivot");
    if (!known) {
      fail(file, vector, "unknown date option flag");
      return;
    }
    declared =
        hc::DateOptions::declare(styles, pivot ? static_cast<std::uint32_t>(pivot->u64()) : 0)
            .has_value();
  } else if (kind == "month_names") {
    const std::vector<std::uint8_t> packed = packed_months(vector);
    const bool from_packed = hc::MonthNames::from_packed(packed.data(), packed.size()).has_value();
    declared = from_packed;
    if (vector.find("append_bytes") == nullptr) {
      std::vector<hc::MonthName> entries;
      for (const Json &pair : vector.find("months")->items) {
        entries.push_back({static_cast<std::uint8_t>(pair.items[0].u64()), pair.items[1].text});
      }
      std::vector<std::uint8_t> storage(packed.size() + 16);
      const auto built = hc::MonthNames::declare(entries.data(), entries.size(),
                                                 hc::EnumStorage(storage.data(), storage.size()));
      if (built.has_value() != from_packed) {
        fail(file, vector, "the builder and the packed layout disagree");
      } else if (built.has_value() && (built->size() != packed.size() ||
                                       !std::equal(packed.begin(), packed.end(), built->data()))) {
        fail(file, vector, "the builder packed other bytes than the layout");
      }
      if (hc::MonthNames::storage_size(entries.data(), entries.size()) != packed.size()) {
        fail(file, vector, "storage_size is not the packed size");
      }
    }
  } else {
    fail(file, vector, "unknown formats kind");
    return;
  }
  const std::string &expect = vector.find("expect")->text;
  if (expect == "refused" && declared) {
    fail(file, vector, "declared, expected the core to refuse it");
  } else if (expect == "ok" && !declared) {
    fail(file, vector, "refused, expected it declared");
  } else if (expect != "ok" && expect != "refused") {
    fail(file, vector, "unknown expect");
  }
  ++declarations[kind];
}

// A grouping.json row: `western` and `indian` hold a shape to each policy at the plain door and its
// detailed twin, under each way this header makes a format; the flags rows hold the declaration.
void grouping_expect(const char *name, const Json &vector, const Json &want,
                     const hc::NumFormat &format, const char *how) {
  const std::string &input = vector.find("input")->text;
  const auto plain = hc::cast_f64(input, format);
  const auto named = hc::detailed::cast_i64(input, format);
  const auto whole = hc::cast_i64(input, format);
  std::string where = std::string(how) + ": ";
  if (const Json *fault = want.find("fault")) {
    const auto offset = static_cast<std::uint32_t>(fault->items[0].u64());
    const auto length = static_cast<std::uint32_t>(fault->items[1].u64());
    const hc::Fault expected{hc::CastFailure::malformed, offset, length};
    if (plain.ok() || !(plain.fault() == expected) || whole.ok() || !(whole.fault() == expected)) {
      fail(name, vector, (where + "the plain door did not refuse the span").c_str());
    }
    if (named.ok() || !(named.failure() == hc::Failure{hc::CastFailure::malformed,
                                                       hc::Detail::group_size, offset, length})) {
      fail(name, vector, (where + "the detailed door is not group_size on the span").c_str());
    }
    return;
  }
  const double value = want.find("value")->f64();
  if (!plain.ok() || plain.value() != value) {
    fail(name, vector, (where + "the plain door did not read the value").c_str());
  }
  if (!named.ok() || static_cast<double>(named.value()) != value) {
    fail(name, vector, (where + "the detailed door did not read the value").c_str());
  }
}

void grouping_row(const char *name, const Json &vector) {
  const std::string &kind = vector.find("kind")->text;
  ++vectors;
  hc::NumFormat::Fields fields;
  fields.decimal_sep = ".";
  fields.group_sep = ",";
  const auto invariant = hc::NumFormat::invariant();
  const auto indian = invariant.with_grouping(hc::Grouping::indian);
  if (kind == "shape") {
    grouping_expect(name, vector, *vector.find("western"), invariant, "invariant");
    grouping_expect(name, vector, *vector.find("western"), *hc::NumFormat::declare(fields),
                    "declared without a policy");
    grouping_expect(name, vector, *vector.find("western"),
                    indian.with_grouping(hc::Grouping::western), "with_grouping to Western");
    grouping_expect(name, vector, *vector.find("indian"), indian, "with_grouping to Indian");
    fields.grouping = hc::Grouping::indian;
    grouping_expect(name, vector, *vector.find("indian"), *hc::NumFormat::declare(fields),
                    "declared Indian");
  } else if (kind == "detect") {
    grouping_expect(name, vector, vector, hc::NumFormat::detect(), "detect");
  } else if (kind == "refused_flags" || kind == "accepted_flags") {
    const auto flags = static_cast<std::uint32_t>(vector.find("flags")->u64());
    // Whole in the styles, as a stale mask arrives, and split as the fields name them.
    fields.styles = static_cast<hc::NumStyles>(flags);
    const bool whole = hc::NumFormat::declare(fields).has_value();
    fields.styles = static_cast<hc::NumStyles>(flags & 0xFFu);
    fields.grouping = static_cast<hc::Grouping>(flags & ~0xFFu);
    const auto split = hc::NumFormat::declare(fields);
    if (kind == "refused_flags" && (whole || split.has_value())) {
      fail(name, vector, "a retired policy was declared");
    }
    if (kind == "accepted_flags" && (!split.has_value() || split->raw().format.flags != flags ||
                                     whole != ((flags & ~0xFFu) == 0))) {
      fail(name, vector, "a policy was refused, or stored as something else");
    }
    // The core's own judge, reached with a layout no declaration can make.
    HyperCastRawNumFormat raw = invariant.raw();
    raw.format.flags = flags;
    std::int32_t out = 77;
    const std::int32_t code =
        ::cast_detailed(4, 0, &raw, nullptr, nullptr, nullptr, 0, nullptr, 0, &out, nullptr);
    if ((code < 0) != (kind == "refused_flags") || out != 77) {
      fail(name, vector, "cast_detailed answered the flags wrongly");
    }
  } else {
    fail(name, vector, "unknown grouping row");
  }
}

void replay(const char *name, const Json &vector) {
  const std::string &input = vector.find("input") ? vector.find("input")->text : std::string();
  const std::string_view file(name);
  const auto order = [&] { return static_cast<hc::DateOrder>(vector.find("order")->u64()); };
  // A row with `options` goes through the detailed date doors alone, which take them.
  const bool optioned = vector.find("options") != nullptr;
  Options options;
  if (optioned && !options_of(name, vector, options)) {
    return;
  }
  if (file == "boolean.json") {
    check(name, vector, hc::cast_bool(input), hc::detailed::cast_bool(input), input.size(),
          [&](bool value) { return value == vector.find("value")->boolean; });
  } else if (file == "char.json") {
    check(name, vector, hc::cast_char(input), hc::detailed::cast_char(input), input.size(),
          [&](char32_t value) { return value == vector.find("value")->u64(); });
  } else if (file == "integer.json") {
    const std::string &type = vector.find("type")->text;
    if (type == "i8") {
      integer<std::int8_t>(name, vector, PLAIN(hc::cast_i8), PLAIN(hc::detailed::cast_i8));
    } else if (type == "i16") {
      integer<std::int16_t>(name, vector, PLAIN(hc::cast_i16), PLAIN(hc::detailed::cast_i16));
    } else if (type == "i32") {
      integer<std::int32_t>(name, vector, PLAIN(hc::cast_i32), PLAIN(hc::detailed::cast_i32));
    } else if (type == "i64") {
      integer<std::int64_t>(name, vector, PLAIN(hc::cast_i64), PLAIN(hc::detailed::cast_i64));
    } else if (type == "u8") {
      integer<std::uint8_t>(name, vector, PLAIN(hc::cast_u8), PLAIN(hc::detailed::cast_u8));
    } else if (type == "u16") {
      integer<std::uint16_t>(name, vector, PLAIN(hc::cast_u16), PLAIN(hc::detailed::cast_u16));
    } else if (type == "u32") {
      integer<std::uint32_t>(name, vector, PLAIN(hc::cast_u32), PLAIN(hc::detailed::cast_u32));
    } else if (type == "u64") {
      integer<std::uint64_t>(name, vector, PLAIN(hc::cast_u64), PLAIN(hc::detailed::cast_u64));
    } else {
      fail(name, vector, "unknown integer type");
    }
  } else if (file == "real.json") {
    const hc::NumFormat format = format_of(name, vector);
    if (vector.find("type")->text == "f32") {
      check(name, vector, hc::cast_f32(input, format), hc::detailed::cast_f32(input, format),
            input.size(),
            [&](float value) { return same_checked_value(*vector.find("value"), value); });
    } else {
      check(name, vector, hc::cast_f64(input, format), hc::detailed::cast_f64(input, format),
            input.size(),
            [&](double value) { return same_checked_value(*vector.find("value"), value); });
    }
  } else if (file == "decimal.json") {
    const hc::NumFormat format = format_of(name, vector);
    check(name, vector, hc::cast_decimal(input, format), hc::detailed::cast_decimal(input, format),
          input.size(), [&](const hc::Decimal &value) { return same_decimal(vector, value); });
  } else if (file == "uuid.json") {
    check(name, vector, hc::cast_uuid(input), hc::detailed::cast_uuid(input), input.size(),
          [&](const hc::Uuid &value) {
            static const char digits[] = "0123456789abcdef";
            std::string hex;
            for (std::uint8_t byte : value.bytes) {
              hex += digits[byte >> 4];
              hex += digits[byte & 0x0F];
            }
            return hex == vector.find("value")->text;
          });
  } else if (file == "timestamp.json") {
    auto same = [&](const hc::Timestamp &value) { return same_instant(vector, value); };
    if (optioned) {
      check(name, vector, hc::detailed::cast_timestamp(input, options.options), input.size(), same);
    } else {
      check(name, vector, hc::cast_timestamp(input), hc::detailed::cast_timestamp(input),
            input.size(), same);
    }
  } else if (file == "unix.json") {
    const auto precision = static_cast<hc::UnixPrecision>(vector.find("precision")->u64());
    check(name, vector, hc::cast_unix(input, precision), hc::detailed::cast_unix(input, precision),
          input.size(), [&](const hc::Timestamp &value) { return same_instant(vector, value); });
  } else if (file == "excel_serial.json") {
    const auto epoch = static_cast<hc::ExcelEpoch>(vector.find("epoch")->u64());
    check(name, vector, hc::cast_excel_serial(input, epoch),
          hc::detailed::cast_excel_serial(input, epoch), input.size(),
          [&](const hc::Timestamp &value) { return same_instant(vector, value); });
  } else if (file == "date.json") {
    auto same = [&](const hc::Date &value) { return same_date(vector, value); };
    if (optioned) {
      check(name, vector, hc::detailed::cast_date(input, options.options), input.size(), same);
    } else {
      check(name, vector, hc::cast_date(input), hc::detailed::cast_date(input), input.size(), same);
    }
  } else if (file == "date_order.json") {
    auto same = [&](const hc::Date &value) { return same_date(vector, value); };
    if (optioned) {
      check(name, vector, hc::detailed::cast_date(input, order(), options.options, options.months),
            input.size(), same);
    } else {
      check(name, vector, hc::cast_date(input, order()), hc::detailed::cast_date(input, order()),
            input.size(), same);
    }
  } else if (file == "datetime.json") {
    const Json *am = vector.find("am");
    const Json *pm = vector.find("pm");
    auto same = [&](const hc::CivilDateTime &value) { return same_civil(vector, value); };
    if (am == nullptr && pm == nullptr) {
      if (optioned) {
        check(name, vector,
              hc::detailed::cast_datetime(input, order(), options.options, options.months),
              input.size(), same);
      } else {
        check(name, vector, hc::cast_datetime(input, order()),
              hc::detailed::cast_datetime(input, order()), input.size(), same);
      }
    } else if (auto format = hc::DateFormat::declare(order(), am ? std::string_view(am->text) : "",
                                                     pm ? std::string_view(pm->text) : "")) {
      if (optioned) {
        check(name, vector,
              hc::detailed::cast_datetime(input, *format, options.options, options.months),
              input.size(), same);
      } else {
        check(name, vector, hc::cast_datetime(input, *format),
              hc::detailed::cast_datetime(input, *format), input.size(), same);
      }
    } else {
      fail(name, vector, "the core refuses the vector's day periods");
    }
  } else if (file == "time.json") {
    check(name, vector, hc::cast_time(input), hc::detailed::cast_time(input), input.size(),
          [&](std::chrono::nanoseconds value) {
            return static_cast<std::uint64_t>(value.count()) == vector.find("nanos")->u64();
          });
  } else if (file == "duration.json") {
    check(name, vector, hc::cast_duration(input), hc::detailed::cast_duration(input), input.size(),
          [&](const hc::Duration &value) { return same_span(vector, value); });
  } else if (file == "typed.json") {
    // The double is named by its bits, which spell NaN and the infinities JSON cannot.
    const std::uint64_t bits = std::strtoull(vector.find("bits")->text.c_str(), nullptr, 16);
    double value = 0;
    std::memcpy(&value, &bits, sizeof value);
    const std::string &door = vector.find("door")->text;
    // A typed door's fault has no text to index: its span must be empty, which an input of
    // size 0 holds it to.
    if (door == "decimal") {
      check(name, vector, hc::cast_decimal_from_double(value),
            hc::detailed::cast_decimal_from_double(value), 0,
            [&](const hc::Decimal &out) { return same_decimal(vector, out); });
    } else if (door == "excel_serial") {
      const auto epoch = static_cast<hc::ExcelEpoch>(vector.find("epoch")->u64());
      check(name, vector, hc::cast_excel_serial_from_double(value, epoch),
            hc::detailed::cast_excel_serial_from_double(value, epoch), 0,
            [&](const hc::CivilDateTime &out) { return same_civil(vector, out); });
    } else if (door == "excel_time") {
      check(name, vector, hc::cast_excel_time(value), hc::detailed::cast_excel_time(value), 0,
            [&](std::chrono::nanoseconds out) {
              return static_cast<std::uint64_t>(out.count()) == vector.find("nanos")->u64();
            });
    } else if (door == "excel_duration") {
      check(name, vector, hc::cast_excel_duration(value), hc::detailed::cast_excel_duration(value),
            0, [&](const hc::Duration &out) { return same_span(vector, out); });
    } else {
      fail(name, vector, "unknown typed door");
    }
  } else if (file == "constraints.json") {
    constraints(name, vector);
  } else if (file == "declarations.json") {
    declaration(name, vector);
  } else if (file == "formats.json") {
    format_declaration(name, vector);
  } else if (file == "grouping.json") {
    grouping_row(name, vector);
  } else {
    fail(name, vector, "no replay for this file");
  }
}

bool read(const std::filesystem::path &path, std::string &out) {
  std::ifstream file(path, std::ios::binary);
  out.assign(std::istreambuf_iterator<char>(file), std::istreambuf_iterator<char>());
  return file.good() || file.eof();
}

} // namespace

int main(int argc, char **argv) {
  if (argc != 2) {
    std::fprintf(stderr, "usage: %s <corpus directory>\n", argv[0]);
    return 2;
  }
  // Every *.json the directory holds, in order: one the replay has no door for fails, and
  // so does a corpus missing any of the files it knows.
  std::error_code error;
  std::vector<std::string> names;
  for (std::filesystem::directory_iterator entry(argv[1], error), end; !error && entry != end;
       entry.increment(error)) {
    if (entry->path().extension() == ".json") {
      names.push_back(entry->path().filename().string());
    }
  }
  std::sort(names.begin(), names.end());
  if (error || names.size() < 18) {
    std::fprintf(stderr, "corpus: %s holds %zu corpus files, expected 18 or more\n", argv[1],
                 names.size());
    return 1;
  }
  for (const std::string &name : names) {
    std::string source;
    Json root;
    if (!read(std::filesystem::path(argv[1]) / name, source) || !Parser(source).parse(root) ||
        root.kind != Json::Kind::array || root.items.empty()) {
      std::fprintf(stderr, "corpus: cannot read %s/%s\n", argv[1], name.c_str());
      return 1;
    }
    for (const Json &vector : root.items) {
      replay(name.c_str(), vector);
    }
  }
  if (failures > 0) {
    std::fprintf(stderr, "corpus: %d of %d vectors failed\n", failures, vectors);
    return 1;
  }
  // The replay must have judged each kind of declaration, so a row type the corpus gains and
  // this replay skips cannot pass for lack of looking.
  for (const char *kind : {"num_format", "day_periods", "date_options", "month_names"}) {
    if (declarations[kind] == 0) {
      std::fprintf(stderr, "corpus: no %s row was judged in formats.json\n", kind);
      return 1;
    }
  }
  std::fprintf(stderr, "corpus: %d vectors in %zu files pass\n", vectors, names.size());
  const std::string_view version = hc::native_version();
  std::printf("%.*s\n", static_cast<int>(version.size()), version.data());
  return 0;
}
