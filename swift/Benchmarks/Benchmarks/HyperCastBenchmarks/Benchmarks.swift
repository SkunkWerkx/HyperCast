import Benchmark
import Foundation
import HyperCast

// Each pair pits a HyperCast door (dlopen crossing included) against Foundation's closest
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
}
