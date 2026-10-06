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

    func testAFormatPacksAsTheCoreReadsIt() {
        var raw = Interop.rawFormat(
            NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "€"))
        XCTAssertEqual(MemoryLayout<Interop.RawNumFormat>.size, 32)
        withUnsafeBytes(of: &raw) { bytes in
            XCTAssertEqual(bytes.load(fromByteOffset: 0, as: UInt32.self), UInt32(UInt8(ascii: ",")))
            XCTAssertEqual(bytes.load(fromByteOffset: 4, as: UInt32.self), UInt32(UInt8(ascii: ".")))
            XCTAssertEqual(bytes.load(fromByteOffset: 8, as: UInt32.self), NumStyles.all.rawValue)
            XCTAssertEqual(bytes.load(fromByteOffset: 12, as: UInt32.self), 3)
            XCTAssertEqual(Array(bytes[16..<32]), Array("€".utf8) + [UInt8](repeating: 0, count: 13))
        }
    }
}
