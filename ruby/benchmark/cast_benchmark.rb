#!/usr/bin/env ruby
# frozen_string_literal: true

# benchmark-ips pairs: hypercast doors vs Ruby's closest parse. Honesty notes baked in:
# Integer()/Float() have no grouping knob (the ungrouped Cast row is the like-for-like)
# and raise on failure (a different error contract than a verdict); Ruby has no stdlib
# UUID or ISO-duration parser, so those doors run unopposed, printed for the record.
# Run: ruby benchmark/cast_benchmark.rb

$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))

require "benchmark/ips"
require "date"
require "time"
require "hypercast"

INVARIANT = HyperCast::NumFormat::INVARIANT
INT = "1234567"
INT_GROUPED = "1,234,567"
FLOAT = "12345.6789"
UUID_TEXT = "01020304-0506-0708-090a-0b0c0d0e0f10"
TIMESTAMP = "2026-01-02T15:04:05.123456789Z"
ISO_SPAN = "PT1H30M15.5S"

puts "== boolean =="
Benchmark.ips do |x|
  x.report("HyperCast.bool") { HyperCast.bool("true") }
  x.compare!
end

MESSY_DATETIME = "1/7/2026 3:04 PM"
MESSY_DATE = "1/7/2026"
EURO_NUMBER = "1.234.567,89"
EUROZONE = HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", flags: HyperCast::ALL_STYLES)

puts "== integer =="
Benchmark.ips do |x|
  x.report("Integer()") { Integer(INT) }
  x.report("HyperCast.i32") { HyperCast.i32(INT, INVARIANT) }
  x.report("HyperCast.i32 grouped") { HyperCast.i32(INT_GROUPED, INVARIANT) }
  x.compare!
end

puts "== real =="
Benchmark.ips do |x|
  x.report("Float()") { Float(FLOAT) }
  x.report("HyperCast.f64") { HyperCast.f64(FLOAT, INVARIANT) }
  x.compare!
end

puts "== uuid (no stdlib parser to pair against) =="
Benchmark.ips do |x|
  x.report("HyperCast.uuid") { HyperCast.uuid(UUID_TEXT) }
  x.compare!
end

puts "== timestamp =="
Benchmark.ips do |x|
  x.report("Time.iso8601") { Time.iso8601(TIMESTAMP) }
  x.report("HyperCast.timestamp") { HyperCast.timestamp(TIMESTAMP) }
  x.compare!
end

puts "== duration (no stdlib ISO-8601 parser to pair against) =="
Benchmark.ips do |x|
  x.report("HyperCast.duration") { HyperCast.duration(ISO_SPAN) }
  x.compare!
end

puts "== datetime (messy civil shape) =="
Benchmark.ips do |x|
  # DateTime.strptime is the stdlib parser that accepts this text at all.
  x.report("DateTime.strptime") { DateTime.strptime(MESSY_DATETIME, "%m/%d/%Y %l:%M %p") }
  x.report("HyperCast.datetime") { HyperCast.datetime(MESSY_DATETIME, :month_day_year) }
  x.compare!
end

puts "== date (declared order) =="
Benchmark.ips do |x|
  x.report("Date.strptime") { Date.strptime(MESSY_DATE, "%m/%d/%Y") }
  x.report("HyperCast.date ordered") { HyperCast.date(MESSY_DATE, :month_day_year) }
  x.compare!
end

puts "== separator detection vs a declared format =="
Benchmark.ips do |x|
  x.report("HyperCast.f64 DETECT") { HyperCast.f64(EURO_NUMBER, HyperCast::NumFormat::DETECT) }
  x.report("HyperCast.f64 declared") { HyperCast.f64(EURO_NUMBER, EUROZONE) }
  x.compare!
end
