// Replays the conformance corpus (corpus/*.json at the repository root), the files every
// binding's suite replays, through hypercast.hpp: each vector's verdict, value and fault span,
// byte for byte, since a span here indexes the same UTF-8 the vector holds. A vector that
// fails is a break in the polyglot contract. Also holds every fault span inside its input.
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
      integer<std::int8_t>(name, vector, hc::cast_i8);
    } else if (type == "i16") {
      integer<std::int16_t>(name, vector, hc::cast_i16);
    } else if (type == "i32") {
      integer<std::int32_t>(name, vector, hc::cast_i32);
    } else if (type == "i64") {
      integer<std::int64_t>(name, vector, hc::cast_i64);
    } else if (type == "u8") {
      integer<std::uint8_t>(name, vector, hc::cast_u8);
    } else if (type == "u16") {
      integer<std::uint16_t>(name, vector, hc::cast_u16);
    } else if (type == "u32") {
      integer<std::uint32_t>(name, vector, hc::cast_u32);
    } else if (type == "u64") {
      integer<std::uint64_t>(name, vector, hc::cast_u64);
    } else {
      fail(name, vector, "unknown integer type");
    }
  } else if (file == "real.json") {
    if (vector.find("type")->text == "f32") {
      check(name, vector, hc::cast_f32(input, format_of(name, vector)), input.size(),
            [&](float value) { return value == static_cast<float>(vector.find("value")->f64()); });
    } else {
      check(name, vector, hc::cast_f64(input, format_of(name, vector)), input.size(),
            [&](double value) { return value == vector.find("value")->f64(); });
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
  if (error || names.size() < 15) {
    std::fprintf(stderr, "corpus: %s holds %zu corpus files, expected 15 or more\n", argv[1],
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
