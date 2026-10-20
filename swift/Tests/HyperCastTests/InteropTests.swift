import Foundation
import XCTest

import HyperCast

/// The public interop surface a package carrying HyperCast's verdicts across its own C ABI
/// reads with — imported without `@testable`, as such a package imports it: each value read
/// back from bytes laid out as the core lays them out, as the door of the same name presents
/// it.
final class InteropTests: XCTestCase {
    private func bytes(_ words: (UInt64, UInt64), _ read: (UnsafeRawBufferPointer) -> Void) {
        var words = words
        withUnsafeBytes(of: &words, read)
    }

    func testValuesReadAsTheDoorsPresentThem() throws {
        bytes((1_767_366_245, 123_456_789)) { raw in
            XCTAssertEqual(
                Interop.instant(raw).timeIntervalSince1970, 1_767_366_245.123_456_789, accuracy: 1e-6)
        }
        // {u16 2026, u8 1, u8 7}, then 54,245,123,456,789 ns of the day at 8.
        bytes((UInt64(2026) | UInt64(1) << 16 | UInt64(7) << 24, 54_245_123_456_789)) { raw in
            XCTAssertEqual(Interop.date(raw), DateComponents(year: 2026, month: 1, day: 7))
            XCTAssertEqual(
                Interop.civil(raw),
                DateComponents(
                    year: 2026, month: 1, day: 7, hour: 15, minute: 4, second: 5, nanosecond: 123_456_789))
        }
        bytes((54_245_123_456_789, 0)) { raw in
            XCTAssertEqual(
                Interop.time(raw), DateComponents(hour: 15, minute: 4, second: 5, nanosecond: 123_456_789))
        }
        bytes((UInt64(bitPattern: -1), UInt64(UInt32(bitPattern: -500_000_000)))) { raw in
            XCTAssertEqual(Interop.duration(raw), .milliseconds(-1_500))
        }
        // {u64 lo 12345, u32 hi 0, u8 scale 2, u8 negative 1}.
        bytes((12_345, UInt64(2) << 32 | UInt64(1) << 40)) { raw in
            XCTAssertEqual(Interop.decimal(raw), Decimal(string: "-123.45"))
        }
        let rfc: [UInt8] = [
            0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00,
        ]
        rfc.withUnsafeBytes { raw in
            XCTAssertEqual(Interop.uuid(raw), UUID(uuidString: "550E8400-E29B-41D4-A716-446655440000"))
        }
    }

    func testAFaultKeepsItsSpan() {
        XCTAssertEqual(Interop.fault(code: 2, offset: 3, length: 4), Fault(reason: .malformed, offset: 3, length: 4))
    }

    func testVersionsUnpack() {
        XCTAssertEqual(Interop.version(0x00_06_02), "0.6.2")
        XCTAssertEqual(Interop.version(0x01_02_03), "1.2.3")
    }

    /// A symbol field as the core reads it: the bytes up to the length byte after the
    /// field's capacity, with the rest of the field zero.
    private func symbol(
        _ bytes: UnsafeRawBufferPointer, at offset: Int, capacity: Int, file: StaticString = #filePath,
        line: UInt = #line
    ) -> String {
        let length = Int(bytes[offset + capacity])
        XCTAssertLessThanOrEqual(length, capacity, "length at \(offset + capacity)", file: file, line: line)
        XCTAssertTrue(
            bytes[(offset + length)..<(offset + capacity)].allSatisfy { $0 == 0 }, "padding at \(offset)", file: file,
            line: line)
        return String(decoding: bytes[offset..<(offset + length)], as: UTF8.self)
    }

    func testAFormatPacksAsTheCoreReadsIt() {
        // Every field declared, at the offsets rust/src/abi.rs pins: the size word, the
        // flags, the digit zero, then each symbol's bytes with its length after them.
        var raw = Interop.rawFormat(
            NumFormat(
                decimalSeparator: ",", groupSeparator: ". ", styles: [.all, .trailingSign], currencySymbol: "€",
                currencyDecimalSeparator: ".", currencyGroupSeparator: ",", minusSign: "−", plusSign: "＋",
                percentSymbol: "٪", digitZero: "\u{0660}"))
        XCTAssertEqual(MemoryLayout<Interop.RawNumFormat>.size, 92)
        XCTAssertEqual(MemoryLayout<Interop.RawNumFormat>.alignment, 4)
        XCTAssertEqual(Interop.rawFormatSize, 92)
        withUnsafeBytes(of: &raw) { bytes in
            XCTAssertEqual(bytes.load(fromByteOffset: 0, as: UInt32.self), 92)
            XCTAssertEqual(bytes.load(fromByteOffset: 4, as: UInt32.self), NumStyles.all.rawValue | 128)
            XCTAssertEqual(bytes.load(fromByteOffset: 8, as: UInt32.self), 0x660)
            XCTAssertEqual(symbol(bytes, at: 12, capacity: 8), ",")
            XCTAssertEqual(symbol(bytes, at: 21, capacity: 8), ". ")
            XCTAssertEqual(symbol(bytes, at: 30, capacity: 8), ".")
            XCTAssertEqual(symbol(bytes, at: 39, capacity: 8), ",")
            XCTAssertEqual(symbol(bytes, at: 48, capacity: 8), "−")
            XCTAssertEqual(symbol(bytes, at: 57, capacity: 8), "＋")
            XCTAssertEqual(symbol(bytes, at: 66, capacity: 8), "٪")
            XCTAssertEqual(symbol(bytes, at: 75, capacity: 16), "€")
            XCTAssertEqual(bytes[91], 3)
        }
        // Nothing declared beyond the separators: every other length is 0 and the digit
        // zero word is 0, which the core reads as its defaults.
        var invariant = Interop.rawFormat(.invariant)
        withUnsafeBytes(of: &invariant) { bytes in
            XCTAssertEqual(bytes.load(fromByteOffset: 8, as: UInt32.self), 0)
            for length in [38, 47, 56, 65, 74, 91] {
                XCTAssertEqual(bytes[length], 0, "length at \(length)")
            }
            XCTAssertEqual(symbol(bytes, at: 12, capacity: 8), ".")
            XCTAssertEqual(symbol(bytes, at: 21, capacity: 8), ",")
        }
    }

    func testDayPeriodsPackAsTheCoreReadsThem() {
        var raw = Interop.rawDayPeriods(DateFormat(order: .dayMonthYear, amSymbol: "vorm.", pmSymbol: "nachm."))
        XCTAssertEqual(MemoryLayout<Interop.RawDayPeriods>.size, 40)
        XCTAssertEqual(MemoryLayout<Interop.RawDayPeriods>.alignment, 4)
        XCTAssertEqual(Interop.rawDayPeriodsSize, 40)
        withUnsafeBytes(of: &raw) { bytes in
            XCTAssertEqual(bytes.load(fromByteOffset: 0, as: UInt32.self), 40)
            XCTAssertEqual(symbol(bytes, at: 4, capacity: 16), "vorm.")
            XCTAssertEqual(bytes[20], 5)
            XCTAssertEqual(symbol(bytes, at: 21, capacity: 16), "nachm.")
            XCTAssertEqual(bytes[37], 6)
            XCTAssertEqual(Array(bytes[38..<40]), [0, 0])
        }
    }
}
