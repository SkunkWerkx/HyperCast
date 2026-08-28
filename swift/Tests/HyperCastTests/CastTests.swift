import Foundation
import XCTest
@testable import HyperCast

/// Binding-level behavior the corpus can't express: the enum union's mandatory-exhaustive
/// switch, locale bridging, Swift-type fidelity (native unsigned, attosecond-backed
/// `Duration`, digit-perfect `DateComponents`), and the caller-bug guards.
final class CastTests: XCTestCase {
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
}
