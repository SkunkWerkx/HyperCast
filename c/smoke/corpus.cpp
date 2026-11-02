// Replays the conformance corpus (corpus/*.json at the repository root), the files every
// binding's suite replays, through hypercast.hpp: each vector's verdict, value and fault span,
// byte for byte, since a span here indexes the same UTF-8 the vector holds. A vector that
// fails is a break in the polyglot contract. Also holds every fault span inside its input.
// constraints.json goes through Constraints<T>::declare and the checked doors (refusals and
// the from_double doors included, floats compared by their bits), and declarations.json
// through ConstraintValue::decimal, the core's conversion of a declaration's text.
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

// The verdict against the vector's "expect" and "fault"; then `same` holds the value.
template <class T, class Same>
void check(const char *file, const Json &vector, const hc::Verdict<T> &verdict,
           std::size_t input_size, Same same) {
  ++vectors;
  const std::string &expect = vector.find("expect")->text;
  if (const T *value = verdict.value_if()) {
    if (expect != "ok") {
      fail(file, vector, "parsed, expected a fault");
    } else if (!same(*value)) {
      fail(file, vector, "wrong value");
    }
    return;
  }
  const hc::Fault &fault = verdict.fault();
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

hc::NumFormat format_of(const char *file, const Json &vector) {
  const Json *format = vector.find("format");
  if (format == nullptr) {
    return hc::NumFormat::invariant();
  }
  auto symbol = [&](const char *key) {
    const Json *field = format->find(key);
    return field ? std::string_view(field->text) : std::string_view();
  };
  hc::NumFormat::Fields fields;
  fields.decimal_sep = symbol("decimal_sep");
  fields.group_sep = symbol("group_sep");
  fields.styles = static_cast<hc::NumStyles>(format->find("flags")->u64());
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
  if (auto declared = hc::NumFormat::declare(fields)) {
    return *declared;
  }
  fail(file, vector, "the core refuses the vector's format");
  return hc::NumFormat::invariant();
}

// A plain door as a callable: the name is an overload set, its checked door beside it.
#define PLAIN(door) [](hc::Text text, const hc::NumFormat &format) { return door(text, format); }

template <class T, class Door> void integer(const char *file, const Json &vector, Door door) {
  const std::string &input = vector.find("input")->text;
  check(file, vector, door(input, format_of(file, vector)), input.size(), [&](T value) {
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
static_assert(hc::Refusal{2}.message() ==
                  "Constraints refused (code 2): minimum together with exclusiveMinimum.",
              "refusal 2's message");
static_assert(hc::Refusal{3}.message() ==
                  "Constraints refused (code 3): maximum together with exclusiveMaximum.",
              "refusal 3's message");
static_assert(hc::Refusal{4}.message() ==
                  "Constraints refused (code 4): the lower bound is not below the upper bound.",
              "refusal 4's message");
static_assert(hc::Refusal{5}.message() ==
                  "Constraints refused (code 5): multipleOf is not positive.",
              "refusal 5's message");
static_assert(hc::Refusal{6}.message() ==
                  "Constraints refused (code 6): a rounding without multipleOf.",
              "refusal 6's message");
static_assert(hc::Refusal{7}.message() ==
                  "Constraints refused (code 7): minLength is above maxLength.",
              "refusal 7's message");
static_assert(hc::Refusal{8}.message() ==
                  "Constraints refused (code 8): a bound, step or enum entry the door cannot hold.",
              "refusal 8's message");
static_assert(hc::Refusal{9}.message() ==
                  "Constraints refused (code 9): an enum entry that is not UTF-8.",
              "refusal 9's message");
static_assert(hc::Refusal{10}.message() == "Constraints refused (code 10): a duplicate enum entry.",
              "refusal 10's message");
static_assert(hc::Refusal{11}.message() ==
                  "Constraints refused (code 11): a malformed constraints layout.",
              "refusal 11's message");
static_assert(hc::Refusal{12}.message() ==
                  "Constraints refused (code 12): an enum with no entries.",
              "refusal 12's message");
static_assert(hc::Refusal{13}.message() ==
                  "Constraints refused (code 13): a text enum entry no input can match.",
              "refusal 13's message");
static_assert(!std::is_convertible_v<std::int32_t, hc::Refusal>,
              "a Refusal is never made from a bare integer");
static_assert(!std::is_constructible_v<hc::Declared<hc::Constraints<double>>, int>,
              "nor is a Declared");
static_assert(hc::Refusal{14}.message() ==
                  "Constraints refused (code 14): a rule this binding does not name.",
              "a code this header does not name keeps its number");
static_assert(hc::Refusal{0}.message() ==
                  "The enum list does not fit: the EnumStorage is smaller than "
                  "storage_size(entries), or a text entry is longer than 4294967295 UTF-8 bytes.",
              "the header's own refusal");
static_assert(hc::Refusal{0}.message().find("Constraints refused") == std::string_view::npos,
              "code 0 is not worded as a core refusal");
// The constraints layout as the header carries it.
static_assert(sizeof(HyperCastRawConstraints) == HyperCastRawConstraints_SIZE &&
                  alignof(HyperCastRawConstraints) == 8 &&
                  sizeof(HyperCastRawCheck) == HyperCastRawCheck_SIZE &&
                  sizeof(HyperCastRawSpan) == HyperCastRawSpan_SIZE,
              "RawConstraints 72, RawCheck 12, RawSpan 8");

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
    } else if (refusal->message().find("Constraints refused (code " + std::to_string(code) +
                                       "): ") != 0) {
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
  if (const Json *number = vector.find("from_f64")) {
    if constexpr (C::is_text) {
      fail(file, vector, "a double at the text door");
      return;
    } else {
      checked = checked_from_double(number_of(*number), *constraints);
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
    checked = checked_door(text, format_of(file, vector), *constraints);
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
  const hc::Rejection &rejection = checked->rejection();
  std::size_t offset = 0;
  std::size_t length = 0;
  if (const hc::Violation *violation = rejection.violation_if()) {
    if (expect != "violation") {
      fail(file, vector, "a violation, expected otherwise");
    } else if (hc::name(violation->keyword) != vector.find("keyword")->text) {
      fail(file, vector, "wrong keyword");
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

void replay(const char *name, const Json &vector) {
  const std::string &input = vector.find("input") ? vector.find("input")->text : std::string();
  const std::string_view file(name);
  const auto order = [&] { return static_cast<hc::DateOrder>(vector.find("order")->u64()); };
  if (file == "boolean.json") {
    check(name, vector, hc::cast_bool(input), input.size(),
          [&](bool value) { return value == vector.find("value")->boolean; });
  } else if (file == "char.json") {
    check(name, vector, hc::cast_char(input), input.size(),
          [&](char32_t value) { return value == vector.find("value")->u64(); });
  } else if (file == "integer.json") {
    const std::string &type = vector.find("type")->text;
    if (type == "i8") {
      integer<std::int8_t>(name, vector, PLAIN(hc::cast_i8));
    } else if (type == "i16") {
      integer<std::int16_t>(name, vector, PLAIN(hc::cast_i16));
    } else if (type == "i32") {
      integer<std::int32_t>(name, vector, PLAIN(hc::cast_i32));
    } else if (type == "i64") {
      integer<std::int64_t>(name, vector, PLAIN(hc::cast_i64));
    } else if (type == "u8") {
      integer<std::uint8_t>(name, vector, PLAIN(hc::cast_u8));
    } else if (type == "u16") {
      integer<std::uint16_t>(name, vector, PLAIN(hc::cast_u16));
    } else if (type == "u32") {
      integer<std::uint32_t>(name, vector, PLAIN(hc::cast_u32));
    } else if (type == "u64") {
      integer<std::uint64_t>(name, vector, PLAIN(hc::cast_u64));
    } else {
      fail(name, vector, "unknown integer type");
    }
  } else if (file == "real.json") {
    if (vector.find("type")->text == "f32") {
      check(name, vector, hc::cast_f32(input, format_of(name, vector)), input.size(),
            [&](float value) { return same_checked_value(*vector.find("value"), value); });
    } else {
      check(name, vector, hc::cast_f64(input, format_of(name, vector)), input.size(),
            [&](double value) { return same_checked_value(*vector.find("value"), value); });
    }
  } else if (file == "decimal.json") {
    check(name, vector, hc::cast_decimal(input, format_of(name, vector)), input.size(),
          [&](const hc::Decimal &value) { return same_decimal(vector, value); });
  } else if (file == "uuid.json") {
    check(name, vector, hc::cast_uuid(input), input.size(), [&](const hc::Uuid &value) {
      static const char digits[] = "0123456789abcdef";
      std::string hex;
      for (std::uint8_t byte : value.bytes) {
        hex += digits[byte >> 4];
        hex += digits[byte & 0x0F];
      }
      return hex == vector.find("value")->text;
    });
  } else if (file == "timestamp.json") {
    check(name, vector, hc::cast_timestamp(input), input.size(),
          [&](const hc::Timestamp &value) { return same_instant(vector, value); });
  } else if (file == "unix.json") {
    const auto precision = static_cast<hc::UnixPrecision>(vector.find("precision")->u64());
    check(name, vector, hc::cast_unix(input, precision), input.size(),
          [&](const hc::Timestamp &value) { return same_instant(vector, value); });
  } else if (file == "excel_serial.json") {
    const auto epoch = static_cast<hc::ExcelEpoch>(vector.find("epoch")->u64());
    check(name, vector, hc::cast_excel_serial(input, epoch), input.size(),
          [&](const hc::Timestamp &value) { return same_instant(vector, value); });
  } else if (file == "date.json") {
    check(name, vector, hc::cast_date(input), input.size(),
          [&](const hc::Date &value) { return same_date(vector, value); });
  } else if (file == "date_order.json") {
    check(name, vector, hc::cast_date(input, order()), input.size(),
          [&](const hc::Date &value) { return same_date(vector, value); });
  } else if (file == "datetime.json") {
    const Json *am = vector.find("am");
    const Json *pm = vector.find("pm");
    auto same = [&](const hc::CivilDateTime &value) { return same_civil(vector, value); };
    if (am == nullptr && pm == nullptr) {
      check(name, vector, hc::cast_datetime(input, order()), input.size(), same);
    } else if (auto format = hc::DateFormat::declare(order(), am ? std::string_view(am->text) : "",
                                                     pm ? std::string_view(pm->text) : "")) {
      check(name, vector, hc::cast_datetime(input, *format), input.size(), same);
    } else {
      fail(name, vector, "the core refuses the vector's day periods");
    }
  } else if (file == "time.json") {
    check(name, vector, hc::cast_time(input), input.size(), [&](std::chrono::nanoseconds value) {
      return static_cast<std::uint64_t>(value.count()) == vector.find("nanos")->u64();
    });
  } else if (file == "duration.json") {
    check(name, vector, hc::cast_duration(input), input.size(),
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
      check(name, vector, hc::cast_decimal_from_double(value), 0,
            [&](const hc::Decimal &out) { return same_decimal(vector, out); });
    } else if (door == "excel_serial") {
      const auto epoch = static_cast<hc::ExcelEpoch>(vector.find("epoch")->u64());
      check(name, vector, hc::cast_excel_serial_from_double(value, epoch), 0,
            [&](const hc::CivilDateTime &out) { return same_civil(vector, out); });
    } else if (door == "excel_time") {
      check(name, vector, hc::cast_excel_time(value), 0, [&](std::chrono::nanoseconds out) {
        return static_cast<std::uint64_t>(out.count()) == vector.find("nanos")->u64();
      });
    } else if (door == "excel_duration") {
      check(name, vector, hc::cast_excel_duration(value), 0,
            [&](const hc::Duration &out) { return same_span(vector, out); });
    } else {
      fail(name, vector, "unknown typed door");
    }
  } else if (file == "constraints.json") {
    constraints(name, vector);
  } else if (file == "declarations.json") {
    declaration(name, vector);
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
  if (error || names.size() < 17) {
    std::fprintf(stderr, "corpus: %s holds %zu corpus files, expected 17 or more\n", argv[1],
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
  std::fprintf(stderr, "corpus: %d vectors in %zu files pass\n", vectors, names.size());
  const std::string_view version = hc::native_version();
  std::printf("%.*s\n", static_cast<int>(version.size()), version.data());
  return 0;
}
