# Runs inside the ruby.wasm interpreter `rbwasm build` made from ./Gemfile, under Node
# (node-test.mjs) and in headless Chrome (index.html). Raises on the first failed check, so
# both hosts see an exception rather than a page to read; returns the report otherwise.
require "/bundle/setup"
require "hypercast"

checks = []
check = lambda do |label, got, want|
  raise "FAIL #{label}: got #{got.inspect}, want #{want.inspect}" unless got == want

  checks << "ok #{label} (#{got.inspect})"
end
inv = HyperCast::NumFormat::INVARIANT
value = ->(result) { result.is_a?(HyperCast::Success) ? result.value : result }

check.("platform", RUBY_PLATFORM.include?("wasm32-wasi"), true)
check.("Magnus backend", HyperCast::BACKEND, :native)
check.("core version matches the gem", HyperCast.native_version, HyperCast::VERSION)
check.("i32 with parentheses and grouping", value.(HyperCast.i32("(1,234)", inv)), -1234)
check.("u32 at its maximum", value.(HyperCast.u32("4294967295", inv)), 4_294_967_295)
check.("u64 at its maximum", value.(HyperCast.u64("18446744073709551615", inv)), 18_446_744_073_709_551_615)
check.("i64 at its minimum", value.(HyperCast.i64("-9223372036854775808", inv)), -9_223_372_036_854_775_808)
check.("f64", value.(HyperCast.f64("1.5e3", inv)), 1500.0)
check.("bool", value.(HyperCast.bool("true")), true)
check.("char", value.(HyperCast.char("&#233;")), "é")
check.("uuid", value.(HyperCast.uuid("{2ED6657D-E927-568B-95E1-2665A8AEA6A2}")), "2ed6657d-e927-568b-95e1-2665a8aea6a2")
fault = HyperCast.i32("1€", inv)
check.("fault span", [fault.reason, fault.offset, fault.length], [:malformed, 1, 1])
dollars = HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: HyperCast::ALL_STYLES, currency: "$")
check.("custom format", value.(HyperCast.i32("($1,234)", dollars)), -1234)
check.("decimal", value.(HyperCast.decimal("-1,234.50", inv)).to_r, Rational(-2469, 2))
# The declared-option doors resolve their Symbol by raw VALUE, which the extension held as a
# u64 until the first wasm32 build: VALUE is 32 bits there.
check.("unix milliseconds", value.(HyperCast.unix("1700000000123", :milliseconds)).to_r,
       Rational(1_700_000_000_123, 1000))
check.("excel serial", value.(HyperCast.excel_serial("45000.5", :y1900)).to_i, 1_678_881_600)
check.("date, declared order", value.(HyperCast.date("03/10/2026", :day_month_year)).to_s, "2026-10-03")
check.("date, ISO", value.(HyperCast.date("2026-10-03")).to_s, "2026-10-03")
check.("timestamp", value.(HyperCast.timestamp("2026-10-03T12:34:56Z")).to_i, 1_791_030_896)
unknown = begin
  HyperCast.unix("1", :fortnights)
rescue KeyError => e
  e.message
end
check.("unknown option", unknown, "key not found: :fortnights")
checks.join("\n")
