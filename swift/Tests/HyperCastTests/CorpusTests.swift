import Foundation
import XCTest
@testable import HyperCast

/// Replays the shared conformance corpus (`corpus/*.json` at the repository root) — the
/// same files every other binding's suite replays — through this binding's `[UInt8]`
/// doors. Fault spans are byte offsets into the UTF-8 input, which is exactly what these
/// doors receive, so span assertions hold verbatim.
final class CorpusTests: XCTestCase {
    private static let corpusDirectory: URL = {
        var dir = URL(fileURLWithPath: #filePath)
        while dir.path != "/" {
            let candidate = dir.appendingPathComponent("corpus")
            if FileManager.default.fileExists(atPath: candidate.path) {
                return candidate
            }
            dir.deleteLastPathComponent()
        }
        fatalError("corpus directory not found above \(#filePath)")
    }()

    private func corpus(_ name: String) throws -> [[String: Any]] {
        let data = try Data(contentsOf: Self.corpusDirectory.appendingPathComponent(name))
        return try JSONSerialization.jsonObject(with: data) as! [[String: Any]]
    }

    private func inputBytes(_ vector: [String: Any]) -> [UInt8] {
        Array((vector["input"] as! String).utf8)
    }

    private func format(of vector: [String: Any]) -> NumFormat {
        guard let format = vector["format"] as? [String: Any] else { return .invariant }
        return NumFormat(
            decimalSeparator: (format["decimal_sep"] as! String).unicodeScalars.first!,
            groupSeparator: (format["group_sep"] as! String).unicodeScalars.first!,
            styles: NumStyles(rawValue: UInt32(format["flags"] as! Int)))
    }

    private func assertVerdict<T: Equatable>(
        _ domain: String, _ vector: [String: Any], _ verdict: Verdict<T>, _ expected: T?,
        file: StaticString = #filePath, line: UInt = #line
    ) {
        let input = vector["input"] as! String
        let expect = vector["expect"] as! String
        // The union payoff, in the language where it's mandatory: two cases, no default.
        switch verdict {
        case .success(let value):
            XCTAssertEqual(expect, "ok", "\(domain): '\(input)' unexpectedly parsed", file: file, line: line)
            XCTAssertEqual(value, expected, "\(domain): '\(input)'", file: file, line: line)
        case .fault(let fault):
            let reason: CastFailure? = switch expect {
            case "empty": .empty
            case "malformed": .malformed
            case "out_of_range": .outOfRange
            default: nil
            }
            XCTAssertEqual(fault.reason, reason, "\(domain): '\(input)'", file: file, line: line)
            if let span = vector["fault"] as? [Int] {
                XCTAssertEqual(fault.offset, span[0], "\(domain): '\(input)' fault offset", file: file, line: line)
                XCTAssertEqual(fault.length, span[1], "\(domain): '\(input)' fault length", file: file, line: line)
            }
        }
    }

    func testBooleanCorpus() throws {
        for vector in try corpus("boolean.json") {
            assertVerdict("boolean", vector, try Cast.bool(inputBytes(vector)), vector["value"] as? Bool)
        }
    }

    func testIntegerCorpus() throws {
        for vector in try corpus("integer.json") {
            let input = inputBytes(vector)
            let format = format(of: vector)
            // JSONSerialization surfaces integers as NSNumber; u64::MAX survives via int64Value's
            // bit pattern, re-read unsigned where the door is unsigned.
            let number = vector["value"] as? NSNumber
            switch vector["type"] as! String {
            case "i8":
                assertVerdict("integer", vector, try Cast.i8(input, format: format), number?.int8Value)
            case "i16":
                assertVerdict("integer", vector, try Cast.i16(input, format: format), number?.int16Value)
            case "i32":
                assertVerdict("integer", vector, try Cast.i32(input, format: format), number?.int32Value)
            case "i64":
                assertVerdict("integer", vector, try Cast.i64(input, format: format), number?.int64Value)
            case "u8":
                assertVerdict("integer", vector, try Cast.u8(input, format: format), number?.uint8Value)
            case "u16":
                assertVerdict("integer", vector, try Cast.u16(input, format: format), number?.uint16Value)
            case "u32":
                assertVerdict("integer", vector, try Cast.u32(input, format: format), number?.uint32Value)
            case "u64":
                assertVerdict("integer", vector, try Cast.u64(input, format: format), number?.uint64Value)
            case let other:
                XCTFail("integer: unknown type \(other)")
            }
        }
    }

    func testRealCorpus() throws {
        for vector in try corpus("real.json") {
            let input = inputBytes(vector)
            let format = format(of: vector)
            let number = vector["value"] as? NSNumber
            switch vector["type"] as! String {
            case "f32":
                assertVerdict("real", vector, try Cast.f32(input, format: format),
                              number.map { Float($0.doubleValue) })
            case "f64":
                assertVerdict("real", vector, try Cast.f64(input, format: format), number?.doubleValue)
            case let other:
                XCTFail("real: unknown type \(other)")
            }
        }
    }

    func testUuidCorpus() throws {
        for vector in try corpus("uuid.json") {
            var expected: UUID?
            if let hex = vector["value"] as? String {
                var bytes = [UInt8]()
                var index = hex.startIndex
                while index < hex.endIndex {
                    let next = hex.index(index, offsetBy: 2)
                    bytes.append(UInt8(hex[index..<next], radix: 16)!)
                    index = next
                }
                expected = bytes.withUnsafeBytes { UUID(uuid: $0.load(as: uuid_t.self)) }
            }
            assertVerdict("uuid", vector, try Cast.uuid(inputBytes(vector)), expected)
        }
    }

    private func expectedInstant(_ vector: [String: Any]) -> Date? {
        guard let seconds = vector["seconds"] as? Int64 else { return nil }
        let nanos = vector["nanos"] as! Int64
        return Date(timeIntervalSince1970: Double(seconds) + Double(nanos) / 1_000_000_000)
    }

    func testTimestampCorpus() throws {
        for vector in try corpus("timestamp.json") {
            assertVerdict("timestamp", vector, try Cast.timestamp(inputBytes(vector)), expectedInstant(vector))
        }
    }

    func testUnixCorpus() throws {
        for vector in try corpus("unix.json") {
            let precision = UnixPrecision(rawValue: UInt32(vector["precision"] as! Int))!
            assertVerdict("unix", vector, try Cast.unix(inputBytes(vector), precision: precision),
                          expectedInstant(vector))
        }
    }

    func testExcelSerialCorpus() throws {
        for vector in try corpus("excel_serial.json") {
            let epoch = ExcelEpoch(rawValue: UInt32(vector["epoch"] as! Int))!
            assertVerdict("excel_serial", vector, try Cast.excelSerial(inputBytes(vector), epoch: epoch),
                          expectedInstant(vector))
        }
    }

    func testDateCorpus() throws {
        for vector in try corpus("date.json") {
            var expected: DateComponents?
            if let year = vector["year"] as? Int {
                expected = DateComponents(
                    year: year, month: (vector["month"] as! Int), day: (vector["day"] as! Int))
            }
            assertVerdict("date", vector, try Cast.date(inputBytes(vector)), expected)
        }
    }

    func testDateOrderCorpus() throws {
        for vector in try corpus("date_order.json") {
            guard let order = DateOrder(rawValue: UInt32(vector["order"] as! Int)) else {
                XCTFail("date_order: unknown order")
                continue
            }
            var expected: DateComponents?
            if let year = vector["year"] as? Int {
                expected = DateComponents(
                    year: year, month: (vector["month"] as! Int), day: (vector["day"] as! Int))
            }
            assertVerdict("date_order", vector, try Cast.date(inputBytes(vector), order: order), expected)
        }
    }

    func testDateTimeCorpus() throws {
        for vector in try corpus("datetime.json") {
            guard let order = DateOrder(rawValue: UInt32(vector["order"] as! Int)) else {
                XCTFail("datetime: unknown order")
                continue
            }
            var expected: DateComponents?
            if let year = vector["year"] as? Int {
                let nanos = UInt64(vector["nanos_of_day"] as! Int)
                let secondOfDay = nanos / 1_000_000_000
                expected = DateComponents(
                    year: year, month: (vector["month"] as! Int), day: (vector["day"] as! Int),
                    hour: Int(secondOfDay / 3_600),
                    minute: Int(secondOfDay % 3_600 / 60),
                    second: Int(secondOfDay % 60),
                    nanosecond: Int(nanos % 1_000_000_000))
            }
            assertVerdict("datetime", vector, try Cast.dateTime(inputBytes(vector), order: order), expected)
        }
    }

    func testTimeCorpus() throws {
        for vector in try corpus("time.json") {
            var expected: DateComponents?
            if let nanosOfDay = (vector["nanos"] as? NSNumber)?.uint64Value {
                let (secondOfDay, nano) = nanosOfDay.quotientAndRemainder(dividingBy: 1_000_000_000)
                let (hour, rest) = secondOfDay.quotientAndRemainder(dividingBy: 3_600)
                let (minute, second) = rest.quotientAndRemainder(dividingBy: 60)
                expected = DateComponents(
                    hour: Int(hour), minute: Int(minute), second: Int(second), nanosecond: Int(nano))
            }
            assertVerdict("time", vector, try Cast.time(inputBytes(vector)), expected)
        }
    }

    func testDurationCorpus() throws {
        for vector in try corpus("duration.json") {
            var expected: Duration?
            if let seconds = vector["seconds"] as? Int64 {
                expected = Duration.seconds(seconds) + .nanoseconds(vector["nanos"] as! Int64)
            }
            assertVerdict("duration", vector, try Cast.duration(inputBytes(vector)), expected)
        }
    }
}
