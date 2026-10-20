import Benchmark
import Foundation
import HyperCast

// Each pair pits a HyperCast door (a direct call into the linked core) against Foundation's closest
// parse. Honesty notes baked in: Int()/Double() have no grouping knob (the ungrouped Cast
// row is the like-for-like); Foundation's ISO8601 parsing tops out at fractional-second
// precision it round-trips through Double; Swift has no stdlib ISO-duration or
// time-of-day parser, so those doors run unopposed, printed for the record.
let benchmarks: @Sendable () -> Void = {
    Benchmark.defaultConfiguration = .init(
        metrics: [.wallClock, .throughput, .mallocCountTotal],
        scalingFactor: .kilo,
        maxDuration: .seconds(2)
    )

    let invariant = NumFormat.invariant
    let intText = "1234567"
    let intGrouped = "1,234,567"
    let floatText = "12345.6789"
    let uuidText = "01020304-0506-0708-090a-0b0c0d0e0f10"
    let timestampText = "2026-01-02T15:04:05.123456789Z"
    let isoSpan = "PT1H30M15.5S"
    let iso8601 = Date.ISO8601FormatStyle(includingFractionalSeconds: true)

    // The messy civil shape and the Foundation parser that accepts it: a DateFormatter
    // with an explicit en-US pattern. Built once, outside the loop — constructing one per
    // call would measure DateFormatter's notorious setup cost, not its parse.
    let messyDateTimeText = "1/7/2026 3:04 PM"
    let messyDateText = "1/7/2026"
    let euroNumberText = "1.234.567,89"
    let usDateTimeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "M/d/yyyy h:mm a"
        return formatter
    }()
    let eurozone = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all)

    Benchmark("Cast.bool") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.bool("true"))
        }
    }

    Benchmark("Cast.i32") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.i32(intText, format: invariant))
        }
    }

    Benchmark("Cast.i32 grouped") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.i32(intGrouped, format: invariant))
        }
    }

    // A locale's format declares its currency symbol whether or not the text carries one:
    // the same grouped integer under en_US's format, `$` included.
    let enUs = NumFormat.from(locale: Locale(identifier: "en_US"))
    Benchmark("Cast.i32 grouped (en_US format)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.i32(intGrouped, format: enUs))
        }
    }

    // The locale: overloads. An explicit identifier is served from the cache; `.current` is
    // derived on every call, so a user's own number-format overrides are honored, and this
    // row is what that costs. The last row is the derivation itself, which a caller pays
    // once by holding the format.
    let enUsLocale = Locale(identifier: "en_US")
    Benchmark("Cast.i32 grouped (locale: en_US)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.i32(intGrouped, locale: enUsLocale))
        }
    }

    Benchmark("Cast.i32 grouped (locale: .current)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.i32(intGrouped, locale: .current))
        }
    }

    Benchmark("NumFormat.from(locale:)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(NumFormat.from(locale: enUsLocale))
        }
    }

    // The rejection path, against Foundation's own failure on the same mistake.
    Benchmark("Cast.i32 malformed") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.i32("1,234x567", format: invariant))
        }
    }

    Benchmark("Int(String) malformed") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(Int("1234x567"))
        }
    }

    Benchmark("Int(String)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(Int(intText))
        }
    }

    Benchmark("Cast.f64") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.f64(floatText, format: invariant))
        }
    }

    Benchmark("Double(String)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(Double(floatText))
        }
    }

    Benchmark("Cast.decimal") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.decimal(floatText, format: invariant))
        }
    }

    Benchmark("Decimal(string:)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(Decimal(string: floatText))
        }
    }

    Benchmark("Cast.uuid") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.uuid(uuidText))
        }
    }

    Benchmark("UUID(uuidString:)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(UUID(uuidString: uuidText))
        }
    }

    Benchmark("Cast.timestamp") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.timestamp(timestampText))
        }
    }

    Benchmark("Date.ISO8601FormatStyle.parse") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try iso8601.parse(timestampText))
        }
    }

    Benchmark("Cast.duration (no Foundation parser to pair against)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.duration(isoSpan))
        }
    }
    Benchmark("Cast.dateTime (messy civil shape)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.dateTime(messyDateTimeText, order: .monthDayYear))
        }
    }

    Benchmark("DateFormatter.date (M/d/yyyy h:mm a)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(usDateTimeFormatter.date(from: messyDateTimeText))
        }
    }

    // The same civil shape with a culture's designators declared, and the locale: form of
    // the door, which serves ko_KR's format from the cache.
    let koreanDateTimeText = "2026-01-07 오후 3:04"
    let korean = DateFormat(order: .yearMonthDay, amSymbol: "오전", pmSymbol: "오후")
    let koKr = Locale(identifier: "ko_KR")
    Benchmark("Cast.dateTime (declared day periods)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.dateTime(koreanDateTimeText, format: korean))
        }
    }

    Benchmark("Cast.dateTime (locale: ko_KR)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.dateTime(koreanDateTimeText, locale: koKr))
        }
    }

    Benchmark("Cast.date (declared order)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.date(messyDateText, order: .monthDayYear))
        }
    }

    Benchmark("Cast.f64 (separator detection)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.f64(euroNumberText, format: .detect))
        }
    }

    Benchmark("Cast.f64 (declared eurozone format)") { benchmark in
        for _ in benchmark.scaledIterations {
            blackHole(try Cast.f64(euroNumberText, format: eurozone))
        }
    }

}
