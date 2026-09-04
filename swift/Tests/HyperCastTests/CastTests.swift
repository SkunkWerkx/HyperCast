import Foundation
import XCTest
@testable import HyperCast

/// Binding-level behavior the corpus can't express: the enum union's mandatory-exhaustive
/// switch, locale bridging, Swift-type fidelity (native unsigned, attosecond-backed
/// `Duration`, digit-perfect `DateComponents`), and the caller-bug guards.
final class CastTests: XCTestCase {
    /// The core crate's own `version = "..."` from `rust/Cargo.toml`, found by walking up
    /// from this file the way `CorpusTests` finds `corpus/` — so the version assertion
    /// follows a release bump instead of going stale on it.
    private static let crateVersion: String = {
        var dir = URL(fileURLWithPath: #filePath)
        while true {
            let manifest = dir.appendingPathComponent("rust/Cargo.toml")
            if let text = try? String(contentsOf: manifest, encoding: .utf8) {
                // Split on any newline, not on "\n": Swift treats "\r\n" as one Character, so a
                // CRLF checkout (Windows) would otherwise read as a single line. Bit CI for real.
                for line in text.split(whereSeparator: \.isNewline) where line.hasPrefix("version = \"") {
                    return String(line.dropFirst("version = \"".count).prefix { $0 != "\"" })
                }
                fatalError("no version = \"...\" line in \(manifest.path)")
            }
            let parent = dir.deletingLastPathComponent()
            if parent.path == dir.path { break }
            dir = parent
        }
        fatalError("rust/Cargo.toml not found above \(#filePath)")
    }()

    func testVerdictSwitchIsExhaustiveWithTwoCases() throws {
        // This compiling at all is the feature — and in Swift it's not opt-in: the
        // compiler rejects a non-exhaustive switch over an enum, full stop.
        let rendered: String = switch try Cast.i32("42", format: .invariant) {
        case .success(let value): "ok \(value)"
        case .fault(let fault): "fault \(fault.reason)"
        }
        XCTAssertEqual(rendered, "ok 42")
    }

    func testFaultSpanPointsAtTheOffendingByte() throws {
        XCTAssertEqual(
            try Cast.i32("  12x4", format: .invariant),
            .fault(Fault(reason: .malformed, offset: 4, length: 1)))
    }

    func testStringDoorsTranscodeNonAsciiSeparators() throws {
        let french = NumFormat(decimalSeparator: ",", groupSeparator: " ", styles: .all)
        XCTAssertEqual(try Cast.f64("1 234,5", format: french), .success(1234.5))
    }

    func testLocaleBridge() throws {
        let german = NumFormat.from(locale: Locale(identifier: "de_DE"))
        XCTAssertEqual(german.decimalSeparator, ",")
        XCTAssertEqual(german.groupSeparator, ".")
        XCTAssertEqual(try Cast.f64("1.234,5", format: german), .success(1234.5))
    }

    func testCurrencySymbolComesFromTheLocale() throws {
        // en_US's own symbol, at every edge the core admits: leading, after the sign,
        // before it with whitespace, and wrapped by accounting parentheses.
        let enUs = NumFormat.from(locale: Locale(identifier: "en_US"))
        XCTAssertEqual(enUs.currencySymbol, "$")
        XCTAssertTrue(enUs.styles.contains(.currency))
        XCTAssertEqual(try Cast.i32("$1,234", format: enUs), .success(1234))
        XCTAssertEqual(try Cast.i32("-$5", format: enUs), .success(-5))
        XCTAssertEqual(try Cast.f64("$ -5", format: enUs), .success(-5))
        XCTAssertEqual(try Cast.i32("($5)", format: enUs), .success(-5))
        XCTAssertEqual(try Cast.decimal("($1,234.50)", format: enUs), .success(Decimal(string: "-1234.50")!))
    }

    func testTrailingMultibyteCurrencySymbol() throws {
        // A declared symbol is matched whole at the edge, so one that happens to contain
        // the group separator (`kr.` under `.` grouping) is no trouble.
        let danish = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "kr.")
        XCTAssertEqual(try Cast.decimal("1.234,50 kr.", format: danish), .success(Decimal(string: "1234.50")!))
        let euro = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "€")
        XCTAssertEqual(try Cast.i32("5 €", format: euro), .success(5))
    }

    func testCurrencySymbolFillsTheNativeFieldEndToEnd() throws {
        // Sixteen bytes exactly — both words of the packed field have to reach the core
        // intact for the symbol to be recognized at all.
        let widest = String(repeating: "¤", count: 8)
        XCTAssertEqual(widest.utf8.count, 16)
        let format = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all, currencySymbol: widest)
        XCTAssertEqual(try Cast.i32("\(widest)42", format: format), .success(42))
        XCTAssertEqual(try Cast.i32("42 \(widest)", format: format), .success(42))
    }

    func testDeclaredSymbolWithTheFlagOffIsMalformedAtTheSymbol() throws {
        let declaredOnly = NumFormat(
            decimalSeparator: ".", groupSeparator: ",", styles: NumStyles.all.subtracting(.currency),
            currencySymbol: "$")
        XCTAssertEqual(
            try Cast.i32("$5", format: declaredOnly),
            .fault(Fault(reason: .malformed, offset: 0, length: 1)))
        // And the flag with nothing declared is a no-op — the invariant profile.
        XCTAssertEqual(NumFormat.invariant.currencySymbol, "")
        XCTAssertEqual(
            try Cast.i32("$5", format: .invariant),
            .fault(Fault(reason: .malformed, offset: 0, length: 1)))
    }

    func testDecimalIsExactWhereADoubleIsNot() throws {
        XCTAssertEqual(try Cast.decimal("0.1", format: .invariant), .success(Decimal(string: "0.1")!))
        XCTAssertEqual(try Cast.decimal("-0.025", format: .invariant), .success(Decimal(string: "-0.025")!))
        XCTAssertEqual(try Cast.decimal("50%", format: .invariant), .success(Decimal(string: "0.5")!))
        // The full 96 bits, above and below the high word's 2⁶⁴ boundary, and the widest scale.
        XCTAssertEqual(
            try Cast.decimal("79228162514264337593543950335", format: .invariant),
            .success(Decimal(string: "79228162514264337593543950335")!))
        XCTAssertEqual(
            try Cast.decimal("-7.9228162514264337593543950335", format: .invariant),
            .success(Decimal(string: "-7.9228162514264337593543950335")!))
        XCTAssertEqual(
            try Cast.decimal("18446744073709551616", format: .invariant),
            .success(Decimal(string: "18446744073709551616")!))
        // Never rounds: a 29th place is out of range, not approximated.
        XCTAssertEqual(
            try Cast.decimal("0.00000000000000000000000000001", format: .invariant),
            .fault(Fault(reason: .outOfRange, offset: 0, length: 31)))
    }

    func testDecimalIsCanonical() throws {
        // The core trims exact trailing fraction zeros, so `1.10`, `1.1` and `1.1000` are
        // all magnitude 11 at scale 1 — exactly the form Foundation's Decimal represents.
        let canonical = Decimal(sign: .plus, exponent: -1, significand: 11)
        for text in ["1.10", "1.1", "1.1000"] {
            XCTAssertEqual(try Cast.decimal(text, format: .invariant), .success(canonical), text)
        }
        XCTAssertEqual(
            try Cast.decimal("1.0000000000000000000000000000000", format: .invariant),
            .success(Decimal(1)))
    }

    func testDecimalZeroIsNeverNegative() throws {
        guard case .success(let value) = try Cast.decimal("-0.00", format: .invariant) else {
            return XCTFail("-0.00 should parse")
        }
        XCTAssertTrue(value.isZero)
        XCTAssertEqual(value.sign, .plus)
        XCTAssertEqual(value, Decimal(string: "0.00")!)
    }

    func testNativeVersionIsTheLoadedLibrarysOwn() throws {
        XCTAssertEqual(try Cast.nativeVersion(), Self.crateVersion)
    }

    func testIsAvailableAgreesWithTheLoad() throws {
        XCTAssertTrue(Cast.isAvailable)
        XCTAssertNoThrow(try Cast.nativeVersion())
    }

    func testGenericNumericDoorReachesEveryTarget() throws {
        let invariant = NumFormat.invariant
        XCTAssertEqual(try Cast.numeric("-8", format: invariant), Verdict<Int8>.success(-8))
        XCTAssertEqual(try Cast.numeric("-16", format: invariant), Verdict<Int16>.success(-16))
        XCTAssertEqual(try Cast.numeric("(32)", format: invariant), Verdict<Int32>.success(-32))
        XCTAssertEqual(try Cast.numeric("-64", format: invariant), Verdict<Int64>.success(-64))
        XCTAssertEqual(try Cast.numeric("255", format: invariant), Verdict<UInt8>.success(255))
        XCTAssertEqual(try Cast.numeric("65535", format: invariant), Verdict<UInt16>.success(65535))
        XCTAssertEqual(try Cast.numeric("0xFFFFFFFF", format: invariant), Verdict<UInt32>.success(.max))
        XCTAssertEqual(
            try Cast.numeric("18446744073709551615", format: invariant), Verdict<UInt64>.success(.max))
        XCTAssertEqual(try Cast.numeric("1.5", format: invariant), Verdict<Float>.success(1.5))
        XCTAssertEqual(try Cast.numeric("50%", format: invariant), Verdict<Double>.success(0.5))
        XCTAssertEqual(
            try Cast.numeric("0.1", format: invariant), Verdict<Decimal>.success(Decimal(string: "0.1")!))
        // Each target keeps its own door's rules: a decimal point is never an integer, and
        // the integer door faults at the point itself.
        XCTAssertEqual(
            try Cast.numeric("1.5", format: invariant),
            Verdict<Int32>.fault(Fault(reason: .malformed, offset: 1, length: 1)))
        // The bytes and buffer forms route the same way.
        XCTAssertEqual(try Cast.numeric(Array("42".utf8), format: invariant), Verdict<UInt8>.success(42))
    }

    func testGenericNumericDoorPassesFaultsThrough() throws {
        let verdict: Verdict<Int32> = try Cast.numeric("  12x4", format: .invariant)
        XCTAssertEqual(verdict, .fault(Fault(reason: .malformed, offset: 4, length: 1)))
        let empty: Verdict<Decimal> = try Cast.numeric("   ", format: .invariant)
        XCTAssertNil(Cast.optional(empty))
    }

    func testRawNumFormatIsTheNativeStructsThirtyTwoBytes() {
        // u32 × 4 at 0/4/8/12, then the 16 symbol bytes at 16: 32 bytes, no padding
        // between the words. Swift's 8-byte alignment for the tuple is stricter than the
        // native struct's 4, which is compatible in the caller-allocated direction.
        XCTAssertEqual(MemoryLayout<Cast.RawNumFormat>.size, 32)
        XCTAssertEqual(MemoryLayout<Cast.RawNumFormat>.stride, 32)
    }

    func testCurrencySymbolPacksAsItsUtf8Bytes() {
        // `kr.` is 0x6B 0x72 0x2E; the low word's in-memory bytes must be exactly those.
        let danish = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "kr.")
        XCTAssertEqual(danish.currencyLength, 3)
        XCTAssertEqual(withUnsafeBytes(of: danish.currencyLow) { Array($0.prefix(3)) }, Array("kr.".utf8))
        XCTAssertEqual(danish.currencyHigh, 0)
        XCTAssertEqual(NumFormat.invariant.currencyLength, 0)
    }

    func testUuidAgreesWithThePlatformsOwnParser() throws {
        let text = "01020304-0506-0708-090a-0b0c0d0e0f10"
        XCTAssertEqual(try Cast.uuid(text), .success(UUID(uuidString: text)!))
        XCTAssertEqual(try Cast.uuid("urn:uuid:\(text)"), .success(UUID(uuidString: text)!))
    }

    func testUnsignedDoorsAreNativelyUnsigned() throws {
        XCTAssertEqual(try Cast.u8("255", format: .invariant), .success(UInt8(255)))
        XCTAssertEqual(try Cast.u64("18446744073709551615", format: .invariant), .success(UInt64.max))
    }

    func testTimestampNormalizesToUtc() throws {
        XCTAssertEqual(
            try Cast.timestamp("2026-01-02T15:04:05+05:00"),
            .success(Date(timeIntervalSince1970: 1_767_348_245)))
    }

    func testUnixMapsTheDeclaredPrecision() throws {
        XCTAssertEqual(
            try Cast.unix("-1", precision: .seconds),
            .success(Date(timeIntervalSince1970: -1)))
        XCTAssertEqual(
            try Cast.unix("1700000000123", precision: .milliseconds),
            .success(Date(timeIntervalSince1970: 1_700_000_000.123)))
    }

    func testDateTimeAndDurationKeepFullFidelity() throws {
        XCTAssertEqual(
            try Cast.date("2026-01-02"),
            .success(DateComponents(year: 2026, month: 1, day: 2)))
        XCTAssertEqual(
            try Cast.time("15:04:05.123456789"),
            .success(DateComponents(hour: 15, minute: 4, second: 5, nanosecond: 123_456_789)))
        // Duration is attosecond-backed — the core's nanoseconds carry exactly, both signs.
        XCTAssertEqual(try Cast.duration("P1DT6H"), .success(.seconds(108_000)))
        XCTAssertEqual(
            try Cast.duration("-1.5s"),
            .success(Duration.seconds(-1) + .nanoseconds(-500_000_000)))
        XCTAssertEqual(
            try Cast.duration("00:00:00.123456789"),
            .success(Duration.nanoseconds(123_456_789)))
    }

    func testOptionalPresentsEmptyAsNil() throws {
        XCTAssertNil(Cast.optional(try Cast.i32("   ", format: .invariant)))
        XCTAssertEqual(Cast.optional(try Cast.i32("42", format: .invariant)), .success(42))
        XCTAssertEqual(
            Cast.optional(try Cast.i32("abc", format: .invariant)),
            .fault(Fault(reason: .malformed, offset: 0, length: 1)))
    }

    func testBytesAndStringDoorsAgree() throws {
        XCTAssertEqual(try Cast.bool("yes"), try Cast.bool(Array("yes".utf8)))
        XCTAssertEqual(try Cast.bool("yes"), .success(true))
    }
    func testDateOrderDisambiguatesLikeTheCulturesDo() throws {
        // The canonical ambiguity: 1/7/2026 is January 7th under en-US's month-first short
        // dates and July 1st under en-GB's day-first ones — resolved only by declaration,
        // with the declaration derived from the real locales' own short-date patterns.
        let enUs = DateOrder.from(locale: Locale(identifier: "en_US"))
        let enGb = DateOrder.from(locale: Locale(identifier: "en_GB"))
        XCTAssertEqual(enUs, .monthDayYear)
        XCTAssertEqual(enGb, .dayMonthYear)
        guard case .success(let us) = try Cast.date("1/7/2026", order: enUs) else {
            return XCTFail("en-US order should parse")
        }
        XCTAssertEqual(us, DateComponents(year: 2026, month: 1, day: 7))
        guard case .success(let gb) = try Cast.date("1/7/2026", order: enGb) else {
            return XCTFail("en-GB order should parse")
        }
        XCTAssertEqual(gb, DateComponents(year: 2026, month: 7, day: 1))
        // Undeclared, the door stays strict ISO — the ambiguity is never guessed at.
        guard case .fault(let fault) = try Cast.date("1/7/2026") else {
            return XCTFail("undeclared slash date should be malformed")
        }
        XCTAssertEqual(fault.reason, .malformed)
    }

    func testDateTimeReadsTheMessyCivilShapes() throws {
        // The AM/PM world, zone-less: no timeZone component because the text named no
        // zone — fusing one is the caller's job, never the parser's guess.
        guard case .success(let us) = try Cast.dateTime("1/7/2026 3:04 PM", order: .monthDayYear) else {
            return XCTFail("en-US order should parse")
        }
        XCTAssertEqual(
            us, DateComponents(year: 2026, month: 1, day: 7, hour: 15, minute: 4, second: 0, nanosecond: 0))
        XCTAssertNil(us.timeZone)
        guard case .success(let gb) = try Cast.dateTime("1/7/2026 3:04 PM", order: .dayMonthYear) else {
            return XCTFail("en-GB order should parse")
        }
        XCTAssertEqual(gb.month, 7)
        XCTAssertEqual(gb.day, 1)
        // A zone suffix is not this door's business — timestamp is the instant door.
        guard case .fault(let fault) = try Cast.dateTime("1/7/2026 15:04:05Z", order: .monthDayYear) else {
            return XCTFail("zoned text should be malformed through the civil door")
        }
        XCTAssertEqual(fault.reason, .malformed)
    }

}
