import Foundation
import XCTest
@testable import HyperCast

/// Binding-level behavior the corpus can't express: the enum union's mandatory-exhaustive
/// switch, locale bridging, Swift-type fidelity (native unsigned, attosecond-backed
/// `Duration`, digit-perfect `DateComponents`), and the caller-bug guards.
final class CastTests: XCTestCase {
    /// The core crate's own `version = "..."` from `rust/Cargo.toml`, found by walking up
    /// from this file the way `CorpusTests` finds `corpus/` — so the version assertion
    /// follows a release bump instead of going stale on it. `nil` only under WASI, where the
    /// test module runs sandboxed with no view of the source tree. A device run (Android,
    /// through adb) has no source tree either, and is handed the version instead.
    private static let crateVersion: String? = {
        #if os(WASI)
            return nil
        #else
            if let handed = ProcessInfo.processInfo.environment["HYPERCAST_CRATE_VERSION"] {
                return handed
            }
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
        #endif
    }()

    func testVerdictSwitchIsExhaustiveWithTwoCases() throws {
        // This compiling at all is the feature — and in Swift it's not opt-in: the
        // compiler rejects a non-exhaustive switch over an enum, full stop.
        let rendered: String =
            switch Cast.i32("42", format: .invariant) {
            case .success(let value): "ok \(value)"
            case .fault(let fault): "fault \(fault.reason)"
            }
        XCTAssertEqual(rendered, "ok 42")
    }

    func testFaultSpanPointsAtTheOffendingByte() throws {
        XCTAssertEqual(
            Cast.i32("  12x4", format: .invariant),
            .fault(Fault(reason: .malformed, offset: 4, length: 1)))
    }

    func testStringDoorsTranscodeNonAsciiSeparators() throws {
        let french = NumFormat(decimalSeparator: ",", groupSeparator: " ", styles: .all)
        XCTAssertEqual(Cast.f64("1 234,5", format: french), .success(1234.5))
    }

    func testLocaleBridge() throws {
        let german = NumFormat.from(locale: Locale(identifier: "de_DE"))
        XCTAssertEqual(german.decimalSeparator, ",")
        XCTAssertEqual(german.groupSeparator, ".")
        XCTAssertEqual(Cast.f64("1.234,5", format: german), .success(1234.5))
        // A locale spelling every sign as ASCII declares none of them, and keeps the number
        // separators for a currency amount: the format is the one it always was.
        XCTAssertEqual(german.minusSign, "")
        XCTAssertEqual(german.plusSign, "")
        XCTAssertEqual(german.percentSymbol, "")
        XCTAssertEqual(german.currencyDecimalSeparator, "")
        XCTAssertEqual(german.currencyGroupSeparator, "")
        XCTAssertNil(german.digitZero)
        XCTAssertFalse(german.styles.contains(.trailingSign))
    }

    func testCurrencySymbolComesFromTheLocale() throws {
        // en_US's own symbol, at every edge the core admits: leading, after the sign,
        // before it with whitespace, and wrapped by accounting parentheses.
        let enUs = NumFormat.from(locale: Locale(identifier: "en_US"))
        XCTAssertEqual(enUs.currencySymbol, "$")
        XCTAssertTrue(enUs.styles.contains(.currency))
        XCTAssertEqual(Cast.i32("$1,234", format: enUs), .success(1234))
        XCTAssertEqual(Cast.i32("-$5", format: enUs), .success(-5))
        XCTAssertEqual(Cast.f64("$ -5", format: enUs), .success(-5))
        XCTAssertEqual(Cast.i32("($5)", format: enUs), .success(-5))
        XCTAssertEqual(Cast.decimal("($1,234.50)", format: enUs), .success(Decimal(string: "-1234.50")!))
    }

    func testTrailingMultibyteCurrencySymbol() throws {
        // A declared symbol is matched whole at the edge, so one that happens to contain
        // the group separator (`kr.` under `.` grouping) is no trouble.
        let danish = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "kr.")
        XCTAssertEqual(Cast.decimal("1.234,50 kr.", format: danish), .success(Decimal(string: "1234.50")!))
        let euro = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "€")
        XCTAssertEqual(Cast.i32("5 €", format: euro), .success(5))
    }

    func testCurrencySymbolFillsTheNativeFieldEndToEnd() throws {
        // Sixteen bytes exactly — both words of the packed field have to reach the core
        // intact for the symbol to be recognized at all.
        let widest = String(repeating: "¤", count: 8)
        XCTAssertEqual(widest.utf8.count, 16)
        let format = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all, currencySymbol: widest)
        XCTAssertEqual(Cast.i32("\(widest)42", format: format), .success(42))
        XCTAssertEqual(Cast.i32("42 \(widest)", format: format), .success(42))
    }

    func testDeclaredSymbolWithTheFlagOffIsMalformedAtTheSymbol() throws {
        let declaredOnly = NumFormat(
            decimalSeparator: ".", groupSeparator: ",", styles: NumStyles.all.subtracting(.currency),
            currencySymbol: "$")
        XCTAssertEqual(
            Cast.i32("$5", format: declaredOnly),
            .fault(Fault(reason: .malformed, offset: 0, length: 1)))
        // And the flag with nothing declared is a no-op — the invariant profile.
        XCTAssertEqual(NumFormat.invariant.currencySymbol, "")
        XCTAssertEqual(
            Cast.i32("$5", format: .invariant),
            .fault(Fault(reason: .malformed, offset: 0, length: 1)))
    }

    func testDecimalIsExactWhereADoubleIsNot() throws {
        XCTAssertEqual(Cast.decimal("0.1", format: .invariant), .success(Decimal(string: "0.1")!))
        XCTAssertEqual(Cast.decimal("-0.025", format: .invariant), .success(Decimal(string: "-0.025")!))
        XCTAssertEqual(Cast.decimal("50%", format: .invariant), .success(Decimal(string: "0.5")!))
        // The full 96 bits, above and below the high word's 2⁶⁴ boundary, and the widest scale.
        XCTAssertEqual(
            Cast.decimal("79228162514264337593543950335", format: .invariant),
            .success(Decimal(string: "79228162514264337593543950335")!))
        XCTAssertEqual(
            Cast.decimal("-7.9228162514264337593543950335", format: .invariant),
            .success(Decimal(string: "-7.9228162514264337593543950335")!))
        XCTAssertEqual(
            Cast.decimal("18446744073709551616", format: .invariant),
            .success(Decimal(string: "18446744073709551616")!))
        // Never rounds: a 29th place is out of range, not approximated.
        XCTAssertEqual(
            Cast.decimal("0.00000000000000000000000000001", format: .invariant),
            .fault(Fault(reason: .outOfRange, offset: 0, length: 31)))
    }

    func testDecimalIsCanonical() throws {
        // The core trims exact trailing fraction zeros, so `1.10`, `1.1` and `1.1000` are
        // all magnitude 11 at scale 1 — exactly the form Foundation's Decimal represents.
        let canonical = Decimal(sign: .plus, exponent: -1, significand: 11)
        for text in ["1.10", "1.1", "1.1000"] {
            XCTAssertEqual(Cast.decimal(text, format: .invariant), .success(canonical), text)
        }
        XCTAssertEqual(
            Cast.decimal("1.0000000000000000000000000000000", format: .invariant),
            .success(Decimal(1)))
    }

    func testDecimalZeroIsNeverNegative() throws {
        guard case .success(let value) = Cast.decimal("-0.00", format: .invariant) else {
            return XCTFail("-0.00 should parse")
        }
        XCTAssertTrue(value.isZero)
        XCTAssertEqual(value.sign, .plus)
        XCTAssertEqual(value, Decimal(string: "0.00")!)
    }

    func testNativeVersionIsTheLinkedCoresOwn() throws {
        let version = Cast.nativeVersion()
        if let crateVersion = Self.crateVersion {
            XCTAssertEqual(version, crateVersion)
        } else {
            let fields = version.split(separator: ".", omittingEmptySubsequences: false)
            XCTAssertEqual(fields.count, 3, "expected major.minor.patch, got \(version)")
            XCTAssertTrue(fields.allSatisfy { UInt8($0) != nil }, "expected major.minor.patch, got \(version)")
        }
    }

    func testIsAvailableBecauseTheCoreIsLinkedIn() throws {
        XCTAssertTrue(Cast.isAvailable)
        XCTAssertNoThrow(Cast.nativeVersion())
    }

    func testGenericNumericDoorReachesEveryTarget() throws {
        let invariant = NumFormat.invariant
        XCTAssertEqual(Cast.numeric("-8", format: invariant), Verdict<Int8>.success(-8))
        XCTAssertEqual(Cast.numeric("-16", format: invariant), Verdict<Int16>.success(-16))
        XCTAssertEqual(Cast.numeric("(32)", format: invariant), Verdict<Int32>.success(-32))
        XCTAssertEqual(Cast.numeric("-64", format: invariant), Verdict<Int64>.success(-64))
        XCTAssertEqual(Cast.numeric("255", format: invariant), Verdict<UInt8>.success(255))
        XCTAssertEqual(Cast.numeric("65535", format: invariant), Verdict<UInt16>.success(65535))
        XCTAssertEqual(Cast.numeric("0xFFFFFFFF", format: invariant), Verdict<UInt32>.success(.max))
        XCTAssertEqual(
            Cast.numeric("18446744073709551615", format: invariant), Verdict<UInt64>.success(.max))
        XCTAssertEqual(Cast.numeric("1.5", format: invariant), Verdict<Float>.success(1.5))
        XCTAssertEqual(Cast.numeric("50%", format: invariant), Verdict<Double>.success(0.5))
        XCTAssertEqual(
            Cast.numeric("0.1", format: invariant), Verdict<Decimal>.success(Decimal(string: "0.1")!))
        // Each target keeps its own door's rules: a decimal point is never an integer, and
        // the integer door faults at the point itself.
        XCTAssertEqual(
            Cast.numeric("1.5", format: invariant),
            Verdict<Int32>.fault(Fault(reason: .malformed, offset: 1, length: 1)))
        // The bytes and buffer forms route the same way.
        XCTAssertEqual(Cast.numeric(Array("42".utf8), format: invariant), Verdict<UInt8>.success(42))
    }

    func testGenericNumericDoorPassesFaultsThrough() throws {
        let verdict: Verdict<Int32> = Cast.numeric("  12x4", format: .invariant)
        XCTAssertEqual(verdict, .fault(Fault(reason: .malformed, offset: 4, length: 1)))
        let empty: Verdict<Decimal> = Cast.numeric("   ", format: .invariant)
        XCTAssertNil(Cast.optional(empty))
    }

    func testCharDoorReadsVerbatimAndDeclaredCodePoints() throws {
        XCTAssertEqual(Cast.char("A"), .success("A"))
        // Verbatim is checked before trimming: one space is a space, one digit is that digit.
        XCTAssertEqual(Cast.char(" "), .success(" "))
        XCTAssertEqual(Cast.char("6"), .success("6"))
        XCTAssertEqual(Cast.char("😀"), .success("\u{1F600}"))
        for spelling in ["65", "U+0041", "u+41", "0x41", "&H41", "&#65;", "&#x41;", "  U+0041  "] {
            XCTAssertEqual(Cast.char(spelling), .success("A"), spelling)
        }
        XCTAssertEqual(Cast.char(Array("U+00E9".utf8)), .success("é"))
        XCTAssertEqual(Cast.char("65x"), .fault(Fault(reason: .malformed, offset: 2, length: 1)))
        XCTAssertEqual(Cast.char("&#65"), .fault(Fault(reason: .malformed, offset: 0, length: 4)))
        XCTAssertEqual(Cast.char("U+D800"), .fault(Fault(reason: .outOfRange, offset: 0, length: 6)))
        XCTAssertEqual(Cast.char("U+110000"), .fault(Fault(reason: .outOfRange, offset: 0, length: 8)))
        XCTAssertNil(Cast.optional(Cast.char("   ")))
    }

    func testGenericScalarDoorReachesEveryTarget() throws {
        let invariant = NumFormat.invariant
        XCTAssertEqual(Cast.scalar("yes", format: invariant), Verdict<Bool>.success(true))
        XCTAssertEqual(Cast.scalar("-8", format: invariant), Verdict<Int8>.success(-8))
        XCTAssertEqual(Cast.scalar("-16", format: invariant), Verdict<Int16>.success(-16))
        XCTAssertEqual(Cast.scalar("(32)", format: invariant), Verdict<Int32>.success(-32))
        XCTAssertEqual(Cast.scalar("-64", format: invariant), Verdict<Int64>.success(-64))
        XCTAssertEqual(Cast.scalar("255", format: invariant), Verdict<UInt8>.success(255))
        XCTAssertEqual(Cast.scalar("65535", format: invariant), Verdict<UInt16>.success(65535))
        XCTAssertEqual(Cast.scalar("0xFFFFFFFF", format: invariant), Verdict<UInt32>.success(.max))
        XCTAssertEqual(
            Cast.scalar("18446744073709551615", format: invariant), Verdict<UInt64>.success(.max))
        XCTAssertEqual(Cast.scalar("1.5", format: invariant), Verdict<Float>.success(1.5))
        XCTAssertEqual(Cast.scalar("50%", format: invariant), Verdict<Double>.success(0.5))
        XCTAssertEqual(
            Cast.scalar("0.1", format: invariant), Verdict<Decimal>.success(Decimal(string: "0.1")!))
        XCTAssertEqual(
            Cast.scalar("{01234567-89ab-cdef-0123-456789abcdef}", format: invariant),
            Verdict<UUID>.success(UUID(uuidString: "01234567-89AB-CDEF-0123-456789ABCDEF")!))
        XCTAssertEqual(
            Cast.scalar("1970-01-01T00:00:01Z", format: invariant),
            Verdict<Date>.success(Date(timeIntervalSince1970: 1)))
        XCTAssertEqual(Cast.scalar("PT1M30S", format: invariant), Verdict<Duration>.success(.seconds(90)))
        XCTAssertEqual(Cast.scalar("&#233;", format: invariant), Verdict<Unicode.Scalar>.success("é"))
        // Each target keeps its own door's rules, faults included, and the format reaches
        // only the numeric doors.
        let eurozone = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .grouping)
        XCTAssertEqual(Cast.scalar("1.234,5", format: eurozone), Verdict<Double>.success(1234.5))
        // A zone-less instant is the timestamp door's fault, not a guess.
        let zoneless: Verdict<Date> = Cast.scalar("2026-01-02T03:04:05", format: invariant)
        XCTAssertEqual(zoneless, Cast.timestamp("2026-01-02T03:04:05"))
        if case .success = zoneless { XCTFail("a zone-less instant parsed") }
        XCTAssertEqual(
            Cast.scalar("maybe", format: invariant),
            Verdict<Bool>.fault(Fault(reason: .malformed, offset: 0, length: 5)))
        // The bytes form routes the same way.
        XCTAssertEqual(Cast.scalar(Array("U+41".utf8), format: invariant), Verdict<Unicode.Scalar>.success("A"))
    }

    func testRawNumFormatIsTheNativeStructsNinetyTwoBytes() {
        // A size word, flags, the digit zero and nine symbol fields with no padding: 92
        // bytes, 4-aligned — rust/src/abi.rs pins the same — and the day periods 40.
        XCTAssertEqual(MemoryLayout<Cast.RawNumFormat>.size, 92)
        XCTAssertEqual(MemoryLayout<Cast.RawNumFormat>.stride, 92)
        XCTAssertEqual(MemoryLayout<Cast.RawNumFormat>.alignment, 4)
        XCTAssertEqual(MemoryLayout<Cast.RawDayPeriods>.size, 40)
        XCTAssertEqual(MemoryLayout<Cast.RawDayPeriods>.stride, 40)
        XCTAssertEqual(MemoryLayout<Cast.RawDayPeriods>.alignment, 4)
    }

    func testScratchHoldsTheNativeShapes() {
        // The out-value is the widest shape a door writes (Decimal, Timestamp, CivilDateTime
        // and Duration are 16 bytes, 8-aligned) and the fault span is RawFault's two u32s.
        XCTAssertEqual(MemoryLayout<Cast.OutScratch>.size, 16)
        XCTAssertEqual(MemoryLayout<Cast.OutScratch>.alignment, 8)
        XCTAssertEqual(MemoryLayout<Cast.FaultScratch>.size, 8)
    }

    func testCurrencySymbolPacksAsItsUtf8Bytes() {
        // `kr.` is 0x6B 0x72 0x2E at the symbol's offset, its length in the field's last byte.
        let danish = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "kr.")
        var raw = danish.raw
        withUnsafeBytes(of: &raw) { bytes in
            XCTAssertEqual(Array(bytes[75..<78]), Array("kr.".utf8))
            XCTAssertEqual(bytes[91], 3)
        }
        var invariant = NumFormat.invariant.raw
        withUnsafeBytes(of: &invariant) { XCTAssertEqual($0[91], 0) }
    }

    func testUuidAgreesWithThePlatformsOwnParser() throws {
        let text = "01020304-0506-0708-090a-0b0c0d0e0f10"
        XCTAssertEqual(Cast.uuid(text), .success(UUID(uuidString: text)!))
        XCTAssertEqual(Cast.uuid("urn:uuid:\(text)"), .success(UUID(uuidString: text)!))
    }

    func testUnsignedDoorsAreNativelyUnsigned() throws {
        XCTAssertEqual(Cast.u8("255", format: .invariant), .success(UInt8(255)))
        XCTAssertEqual(Cast.u64("18446744073709551615", format: .invariant), .success(UInt64.max))
    }

    func testTimestampNormalizesToUtc() throws {
        XCTAssertEqual(
            Cast.timestamp("2026-01-02T15:04:05+05:00"),
            .success(Date(timeIntervalSince1970: 1_767_348_245)))
    }

    func testUnixMapsTheDeclaredPrecision() throws {
        XCTAssertEqual(
            Cast.unix("-1", precision: .seconds),
            .success(Date(timeIntervalSince1970: -1)))
        XCTAssertEqual(
            Cast.unix("1700000000123", precision: .milliseconds),
            .success(Date(timeIntervalSince1970: 1_700_000_000.123)))
    }

    func testDateTimeAndDurationKeepFullFidelity() throws {
        XCTAssertEqual(
            Cast.date("2026-01-02"),
            .success(DateComponents(year: 2026, month: 1, day: 2)))
        XCTAssertEqual(
            Cast.time("15:04:05.123456789"),
            .success(DateComponents(hour: 15, minute: 4, second: 5, nanosecond: 123_456_789)))
        // Duration is attosecond-backed — the core's nanoseconds carry exactly, both signs.
        XCTAssertEqual(Cast.duration("P1DT6H"), .success(.seconds(108_000)))
        XCTAssertEqual(
            Cast.duration("-1.5s"),
            .success(Duration.seconds(-1) + .nanoseconds(-500_000_000)))
        XCTAssertEqual(
            Cast.duration("00:00:00.123456789"),
            .success(Duration.nanoseconds(123_456_789)))
    }

    func testOptionalPresentsEmptyAsNil() throws {
        XCTAssertNil(Cast.optional(Cast.i32("   ", format: .invariant)))
        XCTAssertEqual(Cast.optional(Cast.i32("42", format: .invariant)), .success(42))
        XCTAssertEqual(
            Cast.optional(Cast.i32("abc", format: .invariant)),
            .fault(Fault(reason: .malformed, offset: 0, length: 1)))
    }

    func testBytesAndStringDoorsAgree() throws {
        XCTAssertEqual(Cast.bool("yes"), Cast.bool(Array("yes".utf8)))
        XCTAssertEqual(Cast.bool("yes"), .success(true))
    }
    func testDateOrderDisambiguatesLikeTheCulturesDo() throws {
        // The canonical ambiguity: 1/7/2026 is January 7th under en-US's month-first short
        // dates and July 1st under en-GB's day-first ones — resolved only by declaration,
        // with the declaration derived from the real locales' own short-date patterns.
        let enUs = DateOrder.from(locale: Locale(identifier: "en_US"))
        let enGb = DateOrder.from(locale: Locale(identifier: "en_GB"))
        XCTAssertEqual(enUs, .monthDayYear)
        XCTAssertEqual(enGb, .dayMonthYear)
        guard case .success(let us) = Cast.date("1/7/2026", order: enUs) else {
            return XCTFail("en-US order should parse")
        }
        XCTAssertEqual(us, DateComponents(year: 2026, month: 1, day: 7))
        guard case .success(let gb) = Cast.date("1/7/2026", order: enGb) else {
            return XCTFail("en-GB order should parse")
        }
        XCTAssertEqual(gb, DateComponents(year: 2026, month: 7, day: 1))
        // Undeclared, the door stays strict ISO — the ambiguity is never guessed at.
        guard case .fault(let fault) = Cast.date("1/7/2026") else {
            return XCTFail("undeclared slash date should be malformed")
        }
        XCTAssertEqual(fault.reason, .malformed)
    }

    func testDateTimeReadsTheMessyCivilShapes() throws {
        // The AM/PM world, zone-less: no timeZone component because the text named no
        // zone — fusing one is the caller's job, never the parser's guess.
        guard case .success(let us) = Cast.dateTime("1/7/2026 3:04 PM", order: .monthDayYear) else {
            return XCTFail("en-US order should parse")
        }
        XCTAssertEqual(
            us, DateComponents(year: 2026, month: 1, day: 7, hour: 15, minute: 4, second: 0, nanosecond: 0))
        XCTAssertNil(us.timeZone)
        guard case .success(let gb) = Cast.dateTime("1/7/2026 3:04 PM", order: .dayMonthYear) else {
            return XCTFail("en-GB order should parse")
        }
        XCTAssertEqual(gb.month, 7)
        XCTAssertEqual(gb.day, 1)
        // The English marker is read ahead of the clock too, and case-insensitively.
        XCTAssertEqual(Cast.dateTime("1/7/2026 pm 3:04", order: .monthDayYear), .success(us))
        // A zone suffix is not this door's business — timestamp is the instant door.
        guard case .fault(let fault) = Cast.dateTime("1/7/2026 15:04:05Z", order: .monthDayYear) else {
            return XCTFail("zoned text should be malformed through the civil door")
        }
        XCTAssertEqual(fault.reason, .malformed)
    }

    // MARK: - locale: doors

    func testLocaleDoorsReadEachLocalesSpelling() throws {
        let enUs = Locale(identifier: "en_US")
        let deDe = Locale(identifier: "de_DE")
        let frFr = Locale(identifier: "fr_FR")
        XCTAssertEqual(Cast.i32("1,234", locale: enUs), .success(1234))
        XCTAssertEqual(Cast.i32("1.234", locale: deDe), .success(1234))
        XCTAssertEqual(Cast.f64("1,234.5", locale: enUs), .success(1234.5))
        XCTAssertEqual(Cast.f64("1.234,5", locale: deDe), .success(1234.5))
        XCTAssertEqual(Cast.decimal("(1.234,50)", locale: deDe), .success(Decimal(string: "-1234.5")!))
        // French groups with whatever the locale data says (a narrow no-break space in
        // current CLDR), so the test spells it the same way rather than hard-coding it.
        let frGroup = try XCTUnwrap(frFr.groupingSeparator)
        XCTAssertEqual(Cast.f64("1\(frGroup)234,5", locale: frFr), .success(1234.5))
        // The same text under the wrong locale is read the other locale's way, or refused.
        guard case .fault(let fault) = Cast.f64("1.234,5", locale: enUs) else {
            return XCTFail("1.234,5 under en_US should fault")
        }
        XCTAssertEqual(fault.reason, .malformed)
    }

    func testLocaleDoorsMatchTheBridgedFormat() throws {
        for identifier in ["en_US", "de_DE", "fr_FR", "de_CH", "ja_JP"] {
            let locale = Locale(identifier: identifier)
            let format = NumFormat.from(locale: locale)
            for text in [
                "1234", "-12", "1.234", "1,234", "1.234,5", "1,234.5", "(42)", "1e3", "50%", "0x7F", "abc", "",
            ] {
                XCTAssertEqual(
                    Cast.i8(text, locale: locale), Cast.i8(text, format: format), "\(identifier) \(text)")
                XCTAssertEqual(Cast.i16(text, locale: locale), Cast.i16(text, format: format))
                XCTAssertEqual(Cast.i32(text, locale: locale), Cast.i32(text, format: format))
                XCTAssertEqual(Cast.i64(text, locale: locale), Cast.i64(text, format: format))
                XCTAssertEqual(Cast.u8(text, locale: locale), Cast.u8(text, format: format))
                XCTAssertEqual(Cast.u16(text, locale: locale), Cast.u16(text, format: format))
                XCTAssertEqual(Cast.u32(text, locale: locale), Cast.u32(text, format: format))
                XCTAssertEqual(Cast.u64(text, locale: locale), Cast.u64(text, format: format))
                XCTAssertEqual(Cast.f32(text, locale: locale), Cast.f32(text, format: format))
                XCTAssertEqual(Cast.f64(text, locale: locale), Cast.f64(text, format: format))
                XCTAssertEqual(Cast.decimal(text, locale: locale), Cast.decimal(text, format: format))
                let bytes = Array(text.utf8)
                XCTAssertEqual(Cast.i32(bytes, locale: locale), Cast.i32(bytes, format: format))
                bytes.withUnsafeBytes { raw in
                    XCTAssertEqual(Cast.f64(raw, locale: locale), Cast.f64(raw, format: format))
                }
                let viaNumeric: Verdict<Int32> = Cast.numeric(text, locale: locale)
                XCTAssertEqual(viaNumeric, Cast.i32(text, format: format))
                let viaScalar: Verdict<Double> = Cast.scalar(text, locale: locale)
                XCTAssertEqual(viaScalar, Cast.f64(text, format: format))
            }
        }
    }

    func testScalarLocaleDoorReachesNumericAndOtherTargets() throws {
        let deDe = Locale(identifier: "de_DE")
        let grouped: Verdict<Int32> = Cast.scalar("1.234", locale: deDe)
        XCTAssertEqual(grouped, .success(1234))
        let bytes: Verdict<Decimal> = Cast.scalar(Array("1.234,5".utf8), locale: deDe)
        XCTAssertEqual(bytes, .success(Decimal(string: "1234.5")!))
        // A locale changes nothing for a door that takes no format.
        let flag: Verdict<Bool> = Cast.scalar("yes", locale: deDe)
        XCTAssertEqual(flag, .success(true))
        let id: Verdict<UUID> = Cast.scalar("01020304-0506-0708-090a-0b0c0d0e0f10", locale: deDe)
        XCTAssertEqual(id, Cast.uuid("01020304-0506-0708-090a-0b0c0d0e0f10"))
        let letter: Verdict<Unicode.Scalar> = Cast.scalar("U+00E9", locale: deDe)
        XCTAssertEqual(letter, .success("é"))
    }

    func testLocaleFormatsAreCachedByIdentifierAndBounded() {
        let cache = LocaleFormats(derive: NumFormat.from(locale:))
        let deDe = Locale(identifier: "de_DE")
        XCTAssertEqual(cache.format(for: deDe), NumFormat.from(locale: deDe))
        XCTAssertEqual(cache.format(for: Locale(identifier: "de_DE")), NumFormat.from(locale: deDe))
        // More distinct locales than the bound: every answer stays right as entries are dropped.
        for identifier in Locale.availableIdentifiers.prefix(100) {
            let locale = Locale(identifier: identifier)
            XCTAssertEqual(cache.format(for: locale), NumFormat.from(locale: locale), identifier)
        }
    }

    // MARK: - declared symbols (0.9.0): read beside ASCII, never instead of it

    func testSeparatorsMayBeTwoCharacters() throws {
        let commaSpace = NumFormat(decimalSeparator: ".", groupSeparator: ", ", styles: .all)
        XCTAssertEqual(Cast.i32("1, 234, 567", format: commaSpace), .success(1_234_567))
        XCTAssertEqual(Cast.f64("1, 234.5", format: commaSpace), .success(1234.5))
        // The scalar initializer still declares the one-character shape.
        let fromScalars = NumFormat(
            decimalSeparator: Unicode.Scalar(","), groupSeparator: Unicode.Scalar("."), styles: .all)
        XCTAssertEqual(fromScalars, NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all))
    }

    func testDeclaredSignsAreReadBesideAscii() throws {
        let estonian = NumFormat(
            decimalSeparator: ",", groupSeparator: " ", styles: .all, minusSign: "−", plusSign: "＋")
        XCTAssertEqual(Cast.i32("−5", format: estonian), .success(-5))
        XCTAssertEqual(Cast.i32("-5", format: estonian), .success(-5))
        XCTAssertEqual(Cast.i32("＋5", format: estonian), .success(5))
        XCTAssertEqual(Cast.f64("−2,5", format: estonian), .success(-2.5))
        // Two signs, in either spelling, are malformed at the second.
        XCTAssertEqual(Cast.i32("−-5", format: estonian), .fault(Fault(reason: .malformed, offset: 3, length: 1)))
        // Undeclared, U+2212 is no sign at all.
        XCTAssertEqual(Cast.i32("−5", format: .invariant), .fault(Fault(reason: .malformed, offset: 0, length: 3)))
    }

    func testTrailingSignIsOptIn() throws {
        let trailing = NumFormat(
            decimalSeparator: ".", groupSeparator: ",", styles: [.all, .trailingSign], minusSign: "−")
        XCTAssertEqual(Cast.i32("5-", format: trailing), .success(-5))
        XCTAssertEqual(Cast.i32("5 -", format: trailing), .success(-5))
        XCTAssertEqual(Cast.f64("2.5−", format: trailing), .success(-2.5))
        XCTAssertEqual(Cast.decimal("2.5%-", format: trailing), .success(Decimal(string: "-0.025")!))
        // Not in `.all`: a trailing sign is a culture's pattern, declared, not assumed.
        XCTAssertFalse(NumStyles.all.contains(.trailingSign))
        guard case .fault(let fault) = Cast.i32("5-", format: .invariant) else {
            return XCTFail("a trailing sign under .invariant should be malformed")
        }
        XCTAssertEqual(fault.reason, .malformed)
    }

    func testCurrencySeparatorsApplyWhenTheSymbolIsPresent() throws {
        // A decimal comma in numbers and a decimal point in prices (en_NL's shape): the
        // currency separators are read only once the declared symbol is in the text.
        let dutch = NumFormat(
            decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "€",
            currencyDecimalSeparator: ".", currencyGroupSeparator: ",")
        XCTAssertEqual(Cast.decimal("€1,234.50", format: dutch), .success(Decimal(string: "1234.5")!))
        XCTAssertEqual(Cast.decimal("1.234,50", format: dutch), .success(Decimal(string: "1234.5")!))
        XCTAssertEqual(Cast.f64("€ -1,234.5", format: dutch), .success(-1234.5))
    }

    func testDeclaredPercentAndNativeDigits() throws {
        let arabic = NumFormat(
            decimalSeparator: "٫", groupSeparator: "٬", styles: .all, percentSymbol: "٪", digitZero: "\u{0660}")
        XCTAssertEqual(Cast.i32("١٢٣", format: arabic), .success(123))
        XCTAssertEqual(Cast.i32("123", format: arabic), .success(123))
        XCTAssertEqual(Cast.decimal("١٢٫٥", format: arabic), .success(Decimal(string: "12.5")!))
        XCTAssertEqual(Cast.f64("50٪", format: arabic), .success(0.5))
        XCTAssertEqual(Cast.f64("50%", format: arabic), .success(0.5))
        // Native digits reach the fraction and the exponent, but never a radix-prefixed body.
        XCTAssertEqual(Cast.f64("١٫٥e٢", format: arabic), .success(150))
        guard case .fault(let fault) = Cast.i32("0x١٢", format: arabic) else {
            return XCTFail("native digits after a radix prefix should be malformed")
        }
        XCTAssertEqual(fault.reason, .malformed)
        // An integer door never reads a percent, declared or not.
        guard case .fault = Cast.i32("50٪", format: arabic) else {
            return XCTFail("a percent on an integer door should be malformed")
        }
    }

    // MARK: - the locale bridge, field by field

    /// The hour a civil verdict carries, or `nil` for a fault.
    private func hour(of verdict: Verdict<DateComponents>) -> Int? {
        if case .success(let components) = verdict { return components.hour }
        return nil
    }

    func testLocaleBridgeDeclaresTheLocalesMinusSign() throws {
        // Estonian and Basque write U+2212; the bridge declares it, and ASCII `-` still reads.
        for identifier in ["et_EE", "eu_ES"] {
            let locale = Locale(identifier: identifier)
            let format = NumFormat.from(locale: locale)
            XCTAssertEqual(format.minusSign, "−", identifier)
            XCTAssertEqual(Cast.i32("−5", locale: locale), .success(-5), identifier)
            XCTAssertEqual(Cast.i32("-5", locale: locale), .success(-5), identifier)
            // What the locale's own formatter writes parses under its format.
            let formatter = NumberFormatter()
            formatter.locale = locale
            formatter.numberStyle = .decimal
            let written = try XCTUnwrap(formatter.string(from: -1234.5))
            XCTAssertEqual(Cast.f64(written, locale: locale), .success(-1234.5), "\(identifier): \(written)")
        }
    }

    func testLocaleBridgeDeclaresNativeDigitsAndThePercentSymbol() throws {
        // Arabic-Indic digits for ar_EG, extended Arabic-Indic for fa_IR, from the locale's
        // numbering system; the percent symbol and the minus sign as ICU spells them (bidi
        // marks included), so the locale's own output round-trips.
        let egypt = Locale(identifier: "ar_EG")
        let egyptian = NumFormat.from(locale: egypt)
        XCTAssertEqual(egyptian.digitZero, "\u{0660}")
        XCTAssertEqual(egyptian.decimalSeparator, "٫")
        XCTAssertEqual(egyptian.groupSeparator, "٬")
        let formatter = NumberFormatter()
        formatter.locale = egypt
        formatter.numberStyle = .decimal
        XCTAssertEqual(egyptian.percentSymbol, formatter.percentSymbol)
        XCTAssertEqual(egyptian.minusSign, formatter.minusSign)
        XCTAssertEqual(Cast.i32("١٢٣", locale: egypt), .success(123))
        XCTAssertEqual(Cast.decimal("١٢٫٥", locale: egypt), .success(Decimal(string: "12.5")!))
        XCTAssertEqual(Cast.f64("50\(egyptian.percentSymbol)", locale: egypt), .success(0.5))
        XCTAssertEqual(Cast.f64("50%", locale: egypt), .success(0.5))
        let written = try XCTUnwrap(formatter.string(from: -1234.5))
        XCTAssertEqual(Cast.f64(written, locale: egypt), .success(-1234.5), written)
        let persian = NumFormat.from(locale: Locale(identifier: "fa_IR"))
        XCTAssertEqual(persian.digitZero, "\u{06F0}")
        XCTAssertEqual(Cast.i32("۱۲۳", format: persian), .success(123))
        // A Latin-digit locale declares none; the known zeros start a digit block.
        XCTAssertNil(NumFormat.from(locale: Locale(identifier: "en_US")).digitZero)
        XCTAssertEqual(NumFormat.digitZero(ofNumberingSystem: "thai"), "\u{0E50}")
        XCTAssertEqual(NumFormat.digitZero(ofNumberingSystem: "fullwide"), "\u{FF10}")
        XCTAssertNil(NumFormat.digitZero(ofNumberingSystem: "hanidec"))
    }

    func testFormatterBridgeDeclaresTheTrailingSignAndCurrencySeparators() throws {
        // No CLDR locale writes `n-` in its decimal pattern, so the pattern comes from a
        // configured formatter, which is what `from(formatter:)` is for: a negative suffix
        // with no prefix is a trailing sign.
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "en_US")
        formatter.numberStyle = .decimal
        formatter.negativePrefix = ""
        formatter.negativeSuffix = "-"
        let trailing = NumFormat.from(formatter: formatter)
        XCTAssertTrue(trailing.styles.contains(.trailingSign))
        XCTAssertEqual(trailing.currencySymbol, "$")
        XCTAssertEqual(Cast.i32("5-", format: trailing), .success(-5))
        XCTAssertEqual(Cast.decimal("$1,234.50-", format: trailing), .success(Decimal(string: "-1234.5")!))
        // Parentheses are their own lenience, not a trailing sign.
        formatter.negativePrefix = "("
        formatter.negativeSuffix = ")"
        XCTAssertFalse(NumFormat.from(formatter: formatter).styles.contains(.trailingSign))
        // Monetary separators that differ from the number ones are declared; equal ones are
        // left to the core's fallback, which is the same reading.
        let dutch = NumberFormatter()
        dutch.locale = Locale(identifier: "nl_NL")
        dutch.numberStyle = .decimal
        dutch.currencyDecimalSeparator = "."
        dutch.currencyGroupingSeparator = ","
        let prices = NumFormat.from(formatter: dutch)
        XCTAssertEqual(prices.decimalSeparator, ",")
        XCTAssertEqual(prices.currencyDecimalSeparator, ".")
        XCTAssertEqual(prices.currencyGroupSeparator, ",")
        XCTAssertEqual(Cast.decimal("€ 1,234.50", format: prices), .success(Decimal(string: "1234.5")!))
        XCTAssertEqual(Cast.decimal("1.234,50", format: prices), .success(Decimal(string: "1234.5")!))
        XCTAssertEqual(NumFormat.from(locale: Locale(identifier: "nl_NL")).currencyDecimalSeparator, "")
    }

    func testLocaleBridgeDeclaresTheDayPeriods() throws {
        // Korean writes the designator ahead of the clock; the bridge reads it off a
        // DateFormatter, and the English pair still reads beside it.
        let korea = Locale(identifier: "ko_KR")
        let korean = DateFormat.from(locale: korea)
        XCTAssertEqual(korean.amSymbol, "오전")
        XCTAssertEqual(korean.pmSymbol, "오후")
        XCTAssertEqual(korean.order, DateOrder.from(locale: korea))
        let afternoon = DateComponents(year: 2026, month: 1, day: 7, hour: 15, minute: 4, second: 0, nanosecond: 0)
        XCTAssertEqual(Cast.dateTime("2026-01-07 오후 3:04", locale: korea), .success(afternoon))
        XCTAssertEqual(Cast.dateTime("2026-01-07 오후3:04", format: korean), .success(afternoon))
        XCTAssertEqual(Cast.dateTime("2026-01-07 3:04 PM", locale: korea), .success(afternoon))
        XCTAssertEqual(hour(of: Cast.dateTime(Array("2026-01-07 오전 3:04".utf8), locale: korea)), 3)
        // Spanish's designators carry a space inside, which is allowed; the ends are not.
        let spain = Locale(identifier: "es_ES")
        let spanish = DateFormat.from(locale: spain)
        XCTAssertEqual(spanish.order, .dayMonthYear)
        let spanishAfternoon = "7/1/2026 3:04 \(spanish.pmSymbol)"
        XCTAssertEqual(Cast.dateTime(spanishAfternoon, locale: spain), .success(afternoon), spanishAfternoon)
        // Undeclared, a culture's designator is no designator.
        guard case .fault(let fault) = Cast.dateTime("2026-01-07 오후 3:04", order: .yearMonthDay) else {
            return XCTFail("an undeclared designator should be malformed")
        }
        XCTAssertEqual(fault.reason, .malformed)
    }

    func testDayPeriodsTheFieldCannotCarryDeclareNone() throws {
        // The pair is declared together or not at all; Persian's AM designator is longer
        // than the 16-byte field, so neither is, and the text reads as English only.
        let persia = Locale(identifier: "fa_IR")
        let formatter = DateFormatter()
        formatter.locale = persia
        let fits = [formatter.amSymbol ?? "", formatter.pmSymbol ?? ""].allSatisfy {
            !$0.isEmpty && $0.utf8.count <= Interop.designatorMaxBytes
        }
        let persian = DateFormat.from(locale: persia)
        if fits {
            XCTAssertEqual(persian.amSymbol, formatter.amSymbol)
        } else {
            XCTAssertEqual(persian.amSymbol, "")
            XCTAssertEqual(persian.pmSymbol, "")
            XCTAssertFalse(persian.declaresPeriods)
        }
        XCTAssertEqual(hour(of: Cast.dateTime("2026-01-07 3:04 PM", locale: persia)), 15)
    }

    func testDeclaredDayPeriodsAreReadAtEitherSideOfTheClock() throws {
        let german = DateFormat(order: .dayMonthYear, amSymbol: "vorm.", pmSymbol: "nachm.")
        let afternoon = DateComponents(year: 2026, month: 7, day: 1, hour: 15, minute: 4, second: 0, nanosecond: 0)
        XCTAssertEqual(Cast.dateTime("1/7/2026 3:04 nachm.", format: german), .success(afternoon))
        XCTAssertEqual(Cast.dateTime("1/7/2026 nachm. 3:04", format: german), .success(afternoon))
        XCTAssertEqual(hour(of: Cast.dateTime("1/7/2026 3:04 vorm.", format: german)), 3)
        XCTAssertEqual(Cast.dateTime("1/7/2026 3:04 PM", format: german), .success(afternoon))
        // A format declaring nothing is the order-only door, bit for bit.
        let plain = DateFormat(order: .dayMonthYear)
        XCTAssertFalse(plain.declaresPeriods)
        for text in ["1/7/2026 3:04 PM", "1/7/2026 3:04 nachm.", "1/7/2026", "1/7/2026 15:04:05Z"] {
            XCTAssertEqual(Cast.dateTime(text, format: plain), Cast.dateTime(text, order: .dayMonthYear), text)
        }
    }

    func testTheUsersLocaleIsNeverCached() {
        // `.current` and `.autoupdatingCurrent` derive on every call — their identifier
        // does not carry a user's overrides — and an explicit identifier is served from the
        // cache after the first.
        final class Count: @unchecked Sendable {
            var derived = 0
        }
        let count = Count()
        let cache = LocaleFormats(derive: { locale -> NumFormat in
            count.derived += 1
            return NumFormat.from(locale: locale)
        })
        XCTAssertEqual(cache.format(for: .current), NumFormat.from(locale: .current))
        XCTAssertEqual(cache.format(for: .current), NumFormat.from(locale: .current))
        XCTAssertEqual(count.derived, 2)
        _ = cache.format(for: .autoupdatingCurrent)
        _ = cache.format(for: .autoupdatingCurrent)
        XCTAssertEqual(count.derived, 4)
        _ = cache.format(for: Locale(identifier: "de_DE"))
        _ = cache.format(for: Locale(identifier: "de_DE"))
        XCTAssertEqual(count.derived, 5)
        // And the doors read the user's settings through it.
        XCTAssertEqual(Cast.i32("42", locale: .current), Cast.i32("42", format: .from(locale: .current)))
    }
}
