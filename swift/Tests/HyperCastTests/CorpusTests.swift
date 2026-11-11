import Foundation
import XCTest
@testable import HyperCast

/// Replays the shared conformance corpus (`corpus/*.json` at the repository root) — the
/// same files every other binding's suite replays — through this binding's `[UInt8]`
/// doors. Fault spans are byte offsets into the UTF-8 input, which is exactly what these
/// doors receive, so span assertions hold verbatim.
final class CorpusTests: XCTestCase {
    /// `nil` only under WASI, where the test module runs sandboxed with no view of the
    /// source tree; every corpus test is skipped there, and the doors are exercised by
    /// `CastTests` and by swift/StaticSmokeTest instead.
    private static let corpusDirectory: URL? = {
        #if os(WASI)
            return nil
        #else
            // A device run (Android, through adb) has the corpus pushed beside the test
            // bundle, nowhere above the host path #filePath names.
            if let pushed = ProcessInfo.processInfo.environment["HYPERCAST_CORPUS"] {
                return URL(fileURLWithPath: pushed)
            }
            var dir = URL(fileURLWithPath: #filePath)
            while dir.path != "/" {
                let candidate = dir.appendingPathComponent("corpus")
                if FileManager.default.fileExists(atPath: candidate.path) {
                    return candidate
                }
                dir.deleteLastPathComponent()
            }
            fatalError("corpus directory not found above \(#filePath)")
        #endif
    }()

    /// A corpus file's rows. The number after each key in `spelled` arrives as its literal
    /// text, a `String`, for `Double(_:)` or an integer initializer to read: JSONSerialization
    /// reads a number into an `NSNumber`, and swift-corelibs-foundation (Linux, Windows)
    /// rounds some 16- and 17-digit literals to a neighbour of the nearest double
    /// (`0.0999999999999999` became `0.09999999999999991`), where Darwin's is exact. The
    /// standard library's parse is correctly rounded on every platform.
    private func corpus(_ name: String, spelled: Set<String> = []) throws -> [[String: Any]] {
        guard let directory = Self.corpusDirectory else {
            throw XCTSkip("the conformance corpus is not reachable from a WASI sandbox")
        }
        let data = try Data(contentsOf: directory.appendingPathComponent(name))
        return try JSONSerialization.jsonObject(with: Self.quotingNumbers(after: spelled, in: data))
            as! [[String: Any]]
    }

    /// The JSON text with every number that is the value of a key in `keys` put in quotes,
    /// the rest byte for byte. Strings are copied whole, escapes included, so a key spelled
    /// inside a string is never mistaken for one.
    private static func quotingNumbers(after keys: Set<String>, in data: Data) -> Data {
        guard !keys.isEmpty else { return data }
        let quote = UInt8(ascii: "\""), backslash = UInt8(ascii: "\\"), colon = UInt8(ascii: ":")
        let numeric = Set("-+.eE0123456789".utf8), space = Set(" \t\r\n".utf8)
        let json = [UInt8](data)
        var out: [UInt8] = []
        out.reserveCapacity(json.count + json.count / 8)
        var lastString: String?  // the string just read, while only whitespace follows it
        var spelling = false  // a key in `keys` and its colon were just read
        var i = 0
        while i < json.count {
            let byte = json[i]
            if byte == quote {
                var end = i + 1
                while json[end] != quote { end += json[end] == backslash ? 2 : 1 }
                lastString = String(decoding: json[(i + 1)..<end], as: UTF8.self)
                spelling = false
                out += json[i...end]
                i = end + 1
            } else if spelling && numeric.contains(byte) {
                var end = i
                while end < json.count && numeric.contains(json[end]) { end += 1 }
                out.append(quote)
                out += json[i..<end]
                out.append(quote)
                spelling = false
                i = end
            } else {
                if byte == colon {
                    spelling = lastString.map(keys.contains) ?? false
                    lastString = nil
                } else if !space.contains(byte) {
                    lastString = nil
                    spelling = false
                }
                out.append(byte)
                i += 1
            }
        }
        return Data(out)
    }

    private func inputBytes(_ vector: [String: Any]) -> [UInt8] {
        Array((vector["input"] as! String).utf8)
    }

    /// The format a vector declares: the separators and flags always, and every further
    /// key — a symbol, the currency separators, the digit zero — only where the vector
    /// carries it; absent declares nothing.
    private func format(of vector: [String: Any]) -> NumFormat {
        // A row that declares no format reads the core's invariant one.
        guard let format = vector["format"] as? [String: Any] else { return NumFormat.invariant }
        let symbol = { (key: String) in format[key] as? String ?? "" }
        // `grouping` names the policy; absent, the row's flags carry it.
        let flags = UInt32(format["flags"] as! Int)
        let grouping: Grouping =
            switch format["grouping"] as? String {
            case nil: Grouping(rawValue: flags & Grouping.mask)!
            case "western": .western
            case "indian": .indian
            case let other?: fatalError("unknown grouping \(other)")
            }
        return NumFormat(
            decimalSeparator: symbol("decimal_sep"), groupSeparator: symbol("group_sep"),
            styles: NumStyles(rawValue: flags & ~Grouping.mask), grouping: grouping,
            currencySymbol: symbol("currency"),
            currencyDecimalSeparator: symbol("currency_decimal_sep"),
            currencyGroupSeparator: symbol("currency_group_sep"),
            minusSign: symbol("minus_sign"), plusSign: symbol("plus_sign"), percentSymbol: symbol("percent"),
            digitZero: (format["digit_zero"] as? String)?.unicodeScalars.first)
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
            let reason: CastFailure? =
                switch expect {
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
            assertDetail(domain, vector, fault, file: file, line: line)
        }
    }

    /// Every failure row names its `detail`: the fault carries that vocabulary member, under
    /// its raw code, which is the code the published table numbers it by.
    private func assertDetail(
        _ label: String, _ vector: [String: Any], _ fault: Fault, file: StaticString = #filePath, line: UInt = #line
    ) {
        let input = vector["input"] as? String ?? "\(vector["input_bytes"] ?? vector["from_f64"] ?? "")"
        guard let named = vector["detail"] as? String else {
            return XCTFail("\(label): '\(input)' fails but the row names no detail", file: file, line: line)
        }
        XCTAssertEqual(fault.detail?.name, named, "\(label): '\(input)' detail", file: file, line: line)
        XCTAssertEqual(
            fault.detailCode, Self.detailCodes[named], "\(label): '\(input)' detail code", file: file, line: line)
        XCTAssertGreaterThanOrEqual(fault.detailCode, 1, file: file, line: line)
    }

    /// The published detail table, name to code: the spec's numbers, restated here once.
    static let detailCodes: [String: UInt32] = {
        let names = [
            "blank", "unexpected_character", "digit_expected", "delimiter_expected", "trailing_text", "no_digits",
            "sign_misplaced", "parentheses_unbalanced", "mark_repeated", "group_misplaced", "group_size",
            "group_in_fraction", "ambiguous_separator", "fraction_not_allowed", "field_width", "wrong_length",
            "component_missing", "not_recognized", "unknown_unit", "invalid_utf8", "too_long", "not_finite",
            "not_whole", "negative_not_allowed", "too_large", "too_small", "too_precise", "date_field_out_of_range",
            "time_field_out_of_range", "outside_window", "nonexistent_date", "not_a_scalar",
        ]
        return Dictionary(uniqueKeysWithValues: names.enumerated().map { ($0.element, UInt32($0.offset + 1)) })
    }()

    /// The options a row declares: its named flags (or raw `flag_bits`), pivot and month names.
    private func options(of row: [String: Any]) -> DateOptions {
        let declared = row["options"] as! [String: Any]
        var flags: DateOptions.Flags = []
        for name in declared["flags"] as? [String] ?? [] {
            switch name {
            case "compact_date": flags.insert(.compactDate)
            case "space_separator": flags.insert(.spaceSeparator)
            case "hour_offset": flags.insert(.hourOffset)
            case "common_forms": flags.formUnion(.commonForms)
            case let other: XCTFail("unknown option flag \(other)")
            }
        }
        let months = (declared["months"] as? [[Any]]).map { entries in
            MonthNames(entries.map { (month: $0[0] as! Int, name: $0[1] as! String) })
        }
        return DateOptions(flags: flags, pivot: declared["pivot"] as? Int, months: months)
    }

    func testBooleanCorpus() throws {
        for vector in try corpus("boolean.json") {
            assertVerdict("boolean", vector, Cast.bool(inputBytes(vector)), vector["value"] as? Bool)
        }
    }

    func testCharCorpus() throws {
        // The corpus pins the scalar as its code-point integer; every scalar fits
        // `Unicode.Scalar`, so the comparison is exact with no narrowing.
        for vector in try corpus("char.json") {
            let expected = (vector["value"] as? Int).flatMap { Unicode.Scalar(UInt32($0)) }
            assertVerdict("char", vector, Cast.char(inputBytes(vector)), expected)
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
                assertVerdict("integer", vector, Cast.i8(input, format: format), number?.int8Value)
            case "i16":
                assertVerdict("integer", vector, Cast.i16(input, format: format), number?.int16Value)
            case "i32":
                assertVerdict("integer", vector, Cast.i32(input, format: format), number?.int32Value)
            case "i64":
                assertVerdict("integer", vector, Cast.i64(input, format: format), number?.int64Value)
            case "u8":
                assertVerdict("integer", vector, Cast.u8(input, format: format), number?.uint8Value)
            case "u16":
                assertVerdict("integer", vector, Cast.u16(input, format: format), number?.uint16Value)
            case "u32":
                assertVerdict("integer", vector, Cast.u32(input, format: format), number?.uint32Value)
            case "u64":
                assertVerdict("integer", vector, Cast.u64(input, format: format), number?.uint64Value)
            case let other:
                XCTFail("integer: unknown type \(other)")
            }
        }
    }

    /// A float verdict by its bits, so zero has one form: `-0.0 == 0.0` in Swift, and the core
    /// never delivers `-0.0`, which only a bit comparison can hold it to.
    private func bits(_ verdict: Verdict<Double>) -> Verdict<UInt64> {
        switch verdict {
        case .success(let value): .success(value.bitPattern)
        case .fault(let fault): .fault(fault)
        }
    }

    private func bits(_ verdict: Verdict<Float>) -> Verdict<UInt32> {
        switch verdict {
        case .success(let value): .success(value.bitPattern)
        case .fault(let fault): .fault(fault)
        }
    }

    func testRealCorpus() throws {
        for vector in try corpus("real.json", spelled: ["value"]) {
            let input = inputBytes(vector)
            let format = format(of: vector)
            // The f32 expectation is the nearest double narrowed, as the Rust suite reads it.
            let number = (vector["value"] as? String).map { Double($0)! }
            switch vector["type"] as! String {
            case "f32":
                assertVerdict(
                    "real", vector, bits(Cast.f32(input, format: format)), number.map { Float($0).bitPattern })
            case "f64":
                assertVerdict(
                    "real", vector, bits(Cast.f64(input, format: format)), number?.bitPattern)
            case let other:
                XCTFail("real: unknown type \(other)")
            }
        }
    }

    func testDecimalCorpus() throws {
        for vector in try corpus("decimal.json") {
            // `value` is the core's canonical rendering (`-1234.5`, trailing fraction
            // zeros trimmed) — exactly the form Foundation's Decimal represents, so it is
            // the contract the presentation is held to; the raw triple
            // (`magnitude`/`scale`/`negative`) is not exposed by this binding.
            let expected = (vector["value"] as? String).map { Decimal(string: $0)! }
            assertVerdict(
                "decimal", vector, Cast.decimal(inputBytes(vector), format: format(of: vector)), expected)
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
            assertVerdict("uuid", vector, Cast.uuid(inputBytes(vector)), expected)
        }
    }

    private func expectedInstant(_ vector: [String: Any]) -> Date? {
        guard let seconds = vector["seconds"] as? Int64 else { return nil }
        let nanos = vector["nanos"] as! Int64
        // The Double nearest the instant, but never one in the next second.
        let rounded = Double(seconds) + Double(nanos) / 1_000_000_000
        return Date(timeIntervalSince1970: rounded < Double(seconds + 1) ? rounded : rounded.nextDown)
    }

    func testTimestampCorpus() throws {
        for vector in try corpus("timestamp.json") {
            let verdict =
                vector["options"] == nil
                ? Cast.timestamp(inputBytes(vector))
                : Cast.timestamp(inputBytes(vector), options: options(of: vector))
            assertVerdict("timestamp", vector, verdict, expectedInstant(vector))
        }
    }

    func testUnixCorpus() throws {
        for vector in try corpus("unix.json") {
            let precision = UnixPrecision(rawValue: UInt32(vector["precision"] as! Int))!
            assertVerdict(
                "unix", vector, Cast.unix(inputBytes(vector), precision: precision),
                expectedInstant(vector))
        }
    }

    func testExcelSerialCorpus() throws {
        for vector in try corpus("excel_serial.json") {
            let epoch = ExcelEpoch(rawValue: UInt32(vector["epoch"] as! Int))!
            assertVerdict(
                "excel_serial", vector, Cast.excelSerial(inputBytes(vector), epoch: epoch),
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
            let verdict =
                vector["options"] == nil
                ? Cast.date(inputBytes(vector)) : Cast.date(inputBytes(vector), options: options(of: vector))
            assertVerdict("date", vector, verdict, expected)
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
            let verdict =
                vector["options"] == nil
                ? Cast.date(inputBytes(vector), order: order)
                : Cast.date(inputBytes(vector), order: order, options: options(of: vector))
            assertVerdict("date_order", vector, verdict, expected)
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
            // A vector declares its day periods together or not at all; without them the
            // order-only door is the one under test.
            let verdict: Verdict<DateComponents>
            if vector["options"] == nil {
                verdict =
                    if let am = vector["am"] as? String {
                        Cast.dateTime(
                            inputBytes(vector),
                            format: DateFormat(order: order, amSymbol: am, pmSymbol: vector["pm"] as! String))
                    } else {
                        Cast.dateTime(inputBytes(vector), order: order)
                    }
            } else {
                let declared = options(of: vector)
                verdict =
                    if let am = vector["am"] as? String {
                        Cast.dateTime(
                            inputBytes(vector),
                            format: DateFormat(order: order, amSymbol: am, pmSymbol: vector["pm"] as! String),
                            options: declared)
                    } else {
                        Cast.dateTime(inputBytes(vector), order: order, options: declared)
                    }
            }
            assertVerdict("datetime", vector, verdict, expected)
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
            assertVerdict("time", vector, Cast.time(inputBytes(vector)), expected)
        }
    }

    /// `formats.json`: the white-space table, and the declarations the core refuses. A
    /// constructor here traps on a refusal, so a declaration is judged through the
    /// `isDeclarable…` predicates the constructors check, and the rules that have a message
    /// (the flags, the date options, the month names) assert the binding's message for the
    /// rule the row names.
    func testFormatsCorpus() throws {
        var kinds = Set<String>()
        for vector in try corpus("formats.json") {
            let kind = vector["kind"] as! String
            kinds.insert(kind)
            switch kind {
            case "white_space":
                let spelling = vector["code_point"] as! String
                let value = UInt32(spelling.dropFirst(2), radix: 16)!
                let scalar = Unicode.Scalar(value)!
                XCTAssertEqual(NumFormat.isWhiteSpace(scalar), vector["white_space"] as! Bool, spelling)
            case "num_format":
                let format = vector["format"] as! [String: Any]
                let symbol = { (key: String) in format[key] as? String ?? "" }
                let flags = UInt32(format["flags"] as! Int)
                let rule = vector["rule"] as? String
                let refusal = NumFormat.refusal(ofFlags: flags)
                XCTAssertEqual(
                    Self.declares(
                        decimal: symbol("decimal_sep"), group: symbol("group_sep"), currency: symbol("currency"),
                        currencyDecimal: symbol("currency_decimal_sep"), currencyGroup: symbol("currency_group_sep"),
                        minus: symbol("minus_sign"), plus: symbol("plus_sign"), percent: symbol("percent"),
                        flags: flags),
                    vector["expect"] as! String == "ok", "\(format)")
                switch rule {
                case "grouping_policy":
                    XCTAssertTrue(refusal?.contains("names no grouping policy") ?? false, "\(format)")
                case "unknown_flag": XCTAssertTrue(refusal?.contains("are not defined") ?? false, "\(format)")
                default: XCTAssertNil(refusal, "\(format): its flags are declarable")
                }
            case "day_periods":
                let am = vector["am"] as? String ?? "", pm = vector["pm"] as? String ?? ""
                XCTAssertEqual(
                    DateFormat.isDeclarable(designator: am) && DateFormat.isDeclarable(designator: pm),
                    vector["expect"] as! String == "ok", "\(am) \(pm)")
            case "date_options":
                let flags: UInt32
                if let bits = vector["flag_bits"] as? Int {
                    flags = UInt32(bits)
                } else {
                    flags = (vector["flags"] as? [String] ?? []).reduce(0) { bits, name in
                        bits
                            | (["compact_date": 1, "space_separator": 2, "hour_offset": 4, "common_forms": 7][name]
                                ?? UInt32.max)
                    }
                }
                let pivot = UInt32(vector["pivot"] as? Int ?? 0)
                let refusal = DateOptions.refusal(flags: flags, pivot: pivot)
                XCTAssertEqual(refusal == nil, vector["expect"] as! String == "ok", "\(vector)")
                switch vector["rule"] as? String {
                case "flags": XCTAssertTrue(refusal?.contains("not defined") ?? false, "\(vector)")
                case "pivot": XCTAssertTrue(refusal?.contains("pivot") ?? false, "\(vector)")
                default: break
                }
            case "month_names":
                monthNamesRow(vector)
            case let other:
                XCTFail("formats: unknown kind \(other)")
            }
        }
        XCTAssertEqual(
            kinds, ["white_space", "num_format", "day_periods", "date_options", "month_names"])
    }

    /// A `month_names` row twice: through the binding's declaration (the entries, where the row
    /// spells them as such) and through the core, handed the packed layout the row spells, with
    /// any `append_bytes` a refusal row adds raw, which the binding's entries can't spell.
    private func monthNamesRow(_ vector: [String: Any]) {
        let entries = (vector["months"] as! [[Any]]).map { (month: $0[0] as! Int, name: $0[1] as! String) }
        let ok = vector["expect"] as! String == "ok"
        let rule = vector["rule"] as? String
        var packed: [UInt8] = []
        for entry in entries {
            let name = Array(entry.name.utf8)
            packed.append(UInt8(truncatingIfNeeded: entry.month))
            withUnsafeBytes(of: UInt32(name.count).littleEndian) { packed += $0 }
            packed += name
        }
        packed += (vector["append_bytes"] as? [Int] ?? []).map { UInt8($0) }
        // A length of 0 means no vocabulary to the core, so an empty one is judged by the binding alone.
        if !packed.isEmpty {
            XCTAssertEqual(Cast.vocabularyIsAccepted(packed), ok, "the core on \(vector)")
        }
        guard vector["append_bytes"] == nil else { return }
        switch Result { () throws(MonthNames.Refused) in try MonthNames.declare(entries) } {
        case .success:
            XCTAssertTrue(ok, "\(vector) was declared")
        case .failure(let refused):
            XCTAssertFalse(ok, "\(vector): \(refused.message)")
            let wanted: String =
                switch rule {
                case "month": "must be 1 through 12"
                case "name_length": "must be 1 to 64"
                case "character": "ASCII digit, a slash, a hyphen or white space"
                case "month_missing": "has no name"
                case "duplicate": "fold to the same spelling"
                default: "no such rule \(rule ?? "nil")"
                }
            XCTAssertTrue(refused.message.contains(wanted), "\(vector): \(refused.message)")
        }
    }

    /// Whether a declaration passes every rule the `NumFormat` constructor checks: each
    /// number separator, the two different, the pair a currency amount is read with
    /// different, each decimal separator free of white space, and the `flags` word.
    private static func declares(
        decimal: String, group: String, currency: String, currencyDecimal: String, currencyGroup: String,
        minus: String, plus: String, percent: String, flags: UInt32
    ) -> Bool {
        let inForce = (
            currencyDecimal.isEmpty ? decimal : currencyDecimal, currencyGroup.isEmpty ? group : currencyGroup
        )
        return NumFormat.isDeclarable(decimalSeparator: decimal) && NumFormat.isDeclarable(separator: group)
            && decimal != group
            && (currencyDecimal.isEmpty || NumFormat.isDeclarable(decimalSeparator: currencyDecimal))
            && (currencyGroup.isEmpty || NumFormat.isDeclarable(separator: currencyGroup))
            && inForce.0 != inForce.1 && NumFormat.isDeclarable(sign: minus) && NumFormat.isDeclarable(sign: plus)
            && NumFormat.isDeclarable(sign: percent) && NumFormat.isDeclarable(currencySymbol: currency)
            && NumFormat.refusal(ofFlags: flags) == nil
    }

    func testDurationCorpus() throws {
        for vector in try corpus("duration.json") {
            var expected: Duration?
            if let seconds = vector["seconds"] as? Int64 {
                expected = Duration.seconds(seconds) + .nanoseconds(vector["nanos"] as! Int64)
            }
            assertVerdict("duration", vector, Cast.duration(inputBytes(vector)), expected)
        }
    }

    /// The typed doors, by bits: each `Double` is rebuilt exactly from its IEEE 754 pattern —
    /// NaN and the infinities included, which JSON cannot spell. A typed door's fault has no
    /// span.
    func testTypedCorpus() throws {
        var doors = Set<String>()
        for vector in try corpus("typed.json") {
            let value = Double(bitPattern: UInt64(vector["bits"] as! String, radix: 16)!)
            let door = vector["door"] as! String
            doors.insert(door)
            var faultSpan: (Int, Int)?
            func note<T>(_ verdict: Verdict<T>) {
                if case .fault(let fault) = verdict { faultSpan = (fault.offset, fault.length) }
            }
            switch door {
            case "decimal":
                let verdict = Cast.decimalFromDouble(value)
                assertVerdict(
                    "typed decimal", vector, verdict, (vector["value"] as? String).map { Decimal(string: $0)! })
                note(verdict)
            case "excel_serial":
                let epoch = ExcelEpoch(rawValue: UInt32(vector["epoch"] as! Int))!
                var expected: DateComponents?
                if let year = vector["year"] as? Int {
                    let nanos = (vector["nanos_of_day"] as! NSNumber).uint64Value
                    let secondOfDay = nanos / 1_000_000_000
                    expected = DateComponents(
                        year: year, month: (vector["month"] as! Int), day: (vector["day"] as! Int),
                        hour: Int(secondOfDay / 3_600),
                        minute: Int(secondOfDay % 3_600 / 60),
                        second: Int(secondOfDay % 60),
                        nanosecond: Int(nanos % 1_000_000_000))
                }
                let verdict = Cast.excelSerialFromDouble(value, epoch: epoch)
                assertVerdict("typed excel_serial", vector, verdict, expected)
                note(verdict)
            case "excel_time":
                var expected: DateComponents?
                if let nanosOfDay = (vector["nanos"] as? NSNumber)?.uint64Value {
                    let (secondOfDay, nano) = nanosOfDay.quotientAndRemainder(dividingBy: 1_000_000_000)
                    let (hour, rest) = secondOfDay.quotientAndRemainder(dividingBy: 3_600)
                    let (minute, second) = rest.quotientAndRemainder(dividingBy: 60)
                    expected = DateComponents(
                        hour: Int(hour), minute: Int(minute), second: Int(second), nanosecond: Int(nano))
                }
                let verdict = Cast.excelTime(value)
                assertVerdict("typed excel_time", vector, verdict, expected)
                note(verdict)
            case "excel_duration":
                var expected: Duration?
                if let seconds = vector["seconds"] as? Int64 {
                    expected = Duration.seconds(seconds) + .nanoseconds(vector["nanos"] as! Int64)
                }
                let verdict = Cast.excelDuration(value)
                assertVerdict("typed excel_duration", vector, verdict, expected)
                note(verdict)
            default:
                XCTFail("typed: unknown door \(door)")
            }
            if let (offset, length) = faultSpan {
                XCTAssertEqual(offset, 0, "typed \(door): '\(vector["input"] as! String)' span offset")
                XCTAssertEqual(length, 0, "typed \(door): '\(vector["input"] as! String)' span length")
            }
        }
        XCTAssertEqual(doors, ["decimal", "excel_serial", "excel_time", "excel_duration"])
    }

    // MARK: - validation constraints

    /// The declaration a vector names, as a caller writes it: every bound, step and integer
    /// entry as the corpus's own text, so the corpus exercises this binding's conversion (the
    /// core's `cast_declared_decimal`), and the rest as the keyword's own Swift type. The
    /// public initializer traps on a refusal, so it is built through `declare`, the throwing
    /// function that initializer wraps, which is all of the public one but the trap.
    private func declaration<T: CheckedCastTarget>(
        _ vector: [String: Any], as _: T.Type
    ) -> Result<Constraints<T>, Constraints<T>.Refused> {
        let declared = vector["constraints"] as! [String: Any]
        let value = { (key: String) in (declared[key] as? String).map { ConstraintValue($0) } }
        let rounding: Rounding =
            switch declared["rounding"] as? String {
            case nil, "strict": .strict
            case "midpoint": .midpoint
            case "floor": .floor
            case "ceiling": .ceiling
            case let other?: fatalError("constraints: unknown rounding \(other)")
            }
        return Result { () throws(Constraints<T>.Refused) in
            try Constraints<T>.declare(
                minimum: value("minimum"), exclusiveMinimum: value("exclusiveMinimum"),
                maximum: value("maximum"), exclusiveMaximum: value("exclusiveMaximum"),
                multipleOf: value("multipleOf"), rounding: rounding,
                minLength: declared["minLength"] as? Int, maxLength: declared["maxLength"] as? Int,
                trim: declared["trim"] as? Bool ?? false,
                caseSensitive: declared["caseSensitive"] as? Bool ?? false,
                enum: (declared["enum"] as? [String]).map { $0.map { ConstraintValue($0) } })
        }
    }

    /// One row of `corpus/constraints.json` at one door: the refusal, or the verdict of every
    /// form the row can reach — the `String` and `[UInt8]` doors for text input (spans are
    /// UTF-8 bytes in both), the `[UInt8]` door alone for `input_bytes`, and the from-double
    /// door for `from_f64`. `same` compares values exactly: by bits for a float, by UTF-8
    /// bytes for text (Swift's `==` on `String` is canonical equivalence, so `é` would equal
    /// `e` and U+0301).
    private func replay<T: CheckedCastTarget>(
        _ vector: [String: Any], as type: T.Type,
        text: (String, NumFormat, Constraints<T>) -> CheckedVerdict<T>,
        bytes: ([UInt8], NumFormat, Constraints<T>) -> CheckedVerdict<T>,
        fromDouble: ((Double, Constraints<T>) -> CheckedVerdict<T>)?,
        expected: (Any) -> T, same: (T, T) -> Bool,
        file: StaticString = #filePath, line: UInt = #line
    ) {
        let subject = vector["input"] ?? vector["input_bytes"] ?? vector["from_f64"] ?? vector["constraints"]!
        let label = "constraints \(vector["door"]!) \(subject)"
        let expect = vector["expect"] as! String
        switch declaration(vector, as: type) {
        case .failure(let refused):
            XCTAssertEqual(expect, "refused", "\(label): refused \(refused.message)", file: file, line: line)
            XCTAssertEqual(refused.code, (vector["refusal"] as? Int).map(Int32.init), label, file: file, line: line)
            XCTAssertEqual(refused.message, (refused.code).map(Constraints<T>.refusalMessage), file: file, line: line)
            return
        case .success(let constraints):
            XCTAssertNotEqual(expect, "refused", "\(label): the declaration was accepted", file: file, line: line)
            var verdicts: [CheckedVerdict<T>] = []
            if let input = vector["input"] as? String {
                verdicts.append(text(input, format(of: vector), constraints))
                verdicts.append(bytes(Array(input.utf8), format(of: vector), constraints))
            } else if let raw = vector["input_bytes"] as? [Int] {
                verdicts.append(bytes(raw.map(UInt8.init), format(of: vector), constraints))
            } else {
                guard let fromDouble else {
                    return XCTFail("\(label): no from-double door", file: file, line: line)
                }
                verdicts.append(fromDouble(Double(vector["from_f64"] as! String)!, constraints))
            }
            for verdict in verdicts {
                assertChecked(label, vector, verdict, expected: expected, same: same, file: file, line: line)
            }
        }
    }

    private func assertChecked<T>(
        _ label: String, _ vector: [String: Any], _ verdict: CheckedVerdict<T>,
        expected: (Any) -> T, same: (T, T) -> Bool, file: StaticString, line: UInt
    ) {
        let expect = vector["expect"] as! String
        let span = vector["fault"] as? [Int]
        switch verdict {
        case .success(let checked):
            XCTAssertEqual(expect, "ok", "\(label): unexpectedly \(checked)", file: file, line: line)
            guard expect == "ok" else { return }
            let want = expected(vector["value"]!)
            XCTAssertTrue(same(checked.value, want), "\(label): \(checked.value) != \(want)", file: file, line: line)
            XCTAssertEqual(checked.adjusted, vector["adjusted"] as? Bool, "\(label): adjusted", file: file, line: line)
            XCTAssertEqual(
                checked.enumIndex, vector["enum_index"] as? Int, "\(label): enumIndex", file: file, line: line)
        case .rejection(.violation(let violation)):
            XCTAssertEqual(expect, "violation", "\(label): unexpectedly \(violation)", file: file, line: line)
            XCTAssertEqual(violation.keyword.name, vector["keyword"] as? String, label, file: file, line: line)
            guard expect == "violation" else { return }
            // The value as the door cast it, held to the corpus exactly as a success's is.
            let want = expected(vector["value"]!)
            XCTAssertTrue(
                same(violation.value, want), "\(label): violation value \(violation.value) != \(want)", file: file,
                line: line)
            XCTAssertEqual(violation.offset, span?[0], "\(label): offset", file: file, line: line)
            XCTAssertEqual(violation.length, span?[1], "\(label): length", file: file, line: line)
            XCTAssertNil(vector["detail"], "\(label): a violation names no detail", file: file, line: line)
        case .rejection(.fault(let fault)):
            let reason: CastFailure? =
                switch expect {
                case "empty": .empty
                case "malformed": .malformed
                case "out_of_range": .outOfRange
                default: nil
                }
            XCTAssertEqual(fault.reason, reason, "\(label): unexpectedly \(fault)", file: file, line: line)
            XCTAssertEqual(fault.offset, span?[0], "\(label): offset", file: file, line: line)
            XCTAssertEqual(fault.length, span?[1], "\(label): length", file: file, line: line)
            assertDetail(label, vector, fault, file: file, line: line)
        }
    }

    private func integer<T: FixedWidthInteger>(_ value: Any) -> T {
        T(value as! String)!
    }

    /// Every row of `corpus/constraints.json`, refusals and `from_f64` rows included, through
    /// the public doors. The `input_bytes` rows (invalid UTF-8) reach the text door through
    /// `[UInt8]`; no row is skipped. A lone surrogate is not a row here and cannot be one: a
    /// Swift `String` holds Unicode scalars only, so it can never carry a lone surrogate to the
    /// core as input or as an `enum` entry (see ``testALoneSurrogateCannotBeWrittenInAString``).
    func testConstraintsCorpus() throws {
        var replayed = 0
        var doors = Set<String>()
        let vectors = try corpus("constraints.json", spelled: ["value", "from_f64"])
        for vector in vectors {
            let door = vector["door"] as! String
            doors.insert(door)
            switch door {
            case "i8":
                replay(
                    vector, as: Int8.self, text: { Cast.i8($0, format: $1, constraints: $2) },
                    bytes: { Cast.i8($0, format: $1, constraints: $2) }, fromDouble: Cast.i8FromDouble,
                    expected: integer, same: ==)
            case "i16":
                replay(
                    vector, as: Int16.self, text: { Cast.i16($0, format: $1, constraints: $2) },
                    bytes: { Cast.i16($0, format: $1, constraints: $2) }, fromDouble: Cast.i16FromDouble,
                    expected: integer, same: ==)
            case "i32":
                replay(
                    vector, as: Int32.self, text: { Cast.i32($0, format: $1, constraints: $2) },
                    bytes: { Cast.i32($0, format: $1, constraints: $2) }, fromDouble: Cast.i32FromDouble,
                    expected: integer, same: ==)
            case "i64":
                replay(
                    vector, as: Int64.self, text: { Cast.i64($0, format: $1, constraints: $2) },
                    bytes: { Cast.i64($0, format: $1, constraints: $2) }, fromDouble: Cast.i64FromDouble,
                    expected: integer, same: ==)
            case "u8":
                replay(
                    vector, as: UInt8.self, text: { Cast.u8($0, format: $1, constraints: $2) },
                    bytes: { Cast.u8($0, format: $1, constraints: $2) }, fromDouble: Cast.u8FromDouble,
                    expected: integer, same: ==)
            case "u16":
                replay(
                    vector, as: UInt16.self, text: { Cast.u16($0, format: $1, constraints: $2) },
                    bytes: { Cast.u16($0, format: $1, constraints: $2) }, fromDouble: Cast.u16FromDouble,
                    expected: integer, same: ==)
            case "u32":
                replay(
                    vector, as: UInt32.self, text: { Cast.u32($0, format: $1, constraints: $2) },
                    bytes: { Cast.u32($0, format: $1, constraints: $2) }, fromDouble: Cast.u32FromDouble,
                    expected: integer, same: ==)
            case "u64":
                replay(
                    vector, as: UInt64.self, text: { Cast.u64($0, format: $1, constraints: $2) },
                    bytes: { Cast.u64($0, format: $1, constraints: $2) }, fromDouble: Cast.u64FromDouble,
                    expected: integer, same: ==)
            case "f32":
                replay(
                    vector, as: Float.self, text: { Cast.f32($0, format: $1, constraints: $2) },
                    bytes: { Cast.f32($0, format: $1, constraints: $2) }, fromDouble: Cast.f32FromDouble,
                    expected: { Float(Double($0 as! String)!) }, same: { $0.bitPattern == $1.bitPattern })
            case "f64":
                replay(
                    vector, as: Double.self, text: { Cast.f64($0, format: $1, constraints: $2) },
                    bytes: { Cast.f64($0, format: $1, constraints: $2) }, fromDouble: Cast.f64FromDouble,
                    expected: { Double($0 as! String)! }, same: { $0.bitPattern == $1.bitPattern })
            case "decimal":
                replay(
                    vector, as: Decimal.self, text: { Cast.decimal($0, format: $1, constraints: $2) },
                    bytes: { Cast.decimal($0, format: $1, constraints: $2) }, fromDouble: Cast.decimalFromDouble,
                    expected: { Decimal(string: $0 as! String)! }, same: ==)
            case "text":
                // The text door reads no format: the row's, if any, is not passed on.
                replay(
                    vector, as: String.self,
                    text: { text, _, constraints in Cast.text(text, constraints: constraints) },
                    bytes: { utf8, _, constraints in Cast.text(utf8, constraints: constraints) }, fromDouble: nil,
                    expected: { $0 as! String }, same: { Array($0.utf8) == Array($1.utf8) })
            default:
                XCTFail("constraints: unknown door \(door)")
            }
            replayed += 1
        }
        // Every row the file holds was replayed to the end: the count comes from the file.
        XCTAssertGreaterThan(vectors.count, 0)
        XCTAssertEqual(replayed, vectors.count)
        XCTAssertEqual(
            doors, ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "decimal", "text"])
    }

    /// `corpus/declarations.json`: each input through this binding's conversion of a bound
    /// (the core's `cast_declared_decimal`, which nothing here restates), asserting the raw
    /// triple, scale included, and through the public initializer at the decimal door, which
    /// holds any decimal: the layout's `upper` field is the same triple, and a refused input
    /// is the core's refusal 8.
    /// `corpus/grouping.json`: every shape under both policies, through every way a format is had.
    /// A declaration traps on a bad word, which XCTest can't survive, so the rule the trap reads
    /// is what `refused_flags` holds; `Grouping` has no case for a retired value at all.
    func testGroupingCorpus() throws {
        func holds(_ want: [String: Any], _ text: String, _ format: NumFormat, _ label: String) {
            let context = "\(label) '\(text)'"
            if let fault = want["fault"] as? [Int] {
                let expected = Fault(reason: .malformed, offset: fault[0], length: fault[1])
                XCTAssertEqual(Cast.i64(text, format: format).fault, expected, context)
                XCTAssertEqual(Cast.f64(text, format: format).fault, expected, context)
                XCTAssertEqual(Cast.decimal(text, format: format).fault, expected, context)
                XCTAssertEqual(Cast.i64(text, format: format).fault?.detail, .groupSize, context)
            } else {
                let value = want["value"] as! Int
                XCTAssertEqual(Cast.i64(text, format: format), .success(Int64(value)), context)
                XCTAssertEqual(Cast.f64(text, format: format), .success(Double(value)), context)
                XCTAssertNil(Cast.decimal(text, format: format).fault, context)
            }
        }
        func bridge(primary: Int, secondary: Int) -> NumFormat {
            let formatter = NumberFormatter()
            formatter.locale = Locale(identifier: "en_US")
            formatter.numberStyle = .decimal
            formatter.groupingSize = primary
            formatter.secondaryGroupingSize = secondary
            return NumFormat.from(formatter: formatter)
        }
        let indian = NumFormat.invariant.withGrouping(.indian)
        var shapes = 0
        for row in try corpus("grouping.json") {
            switch row["kind"] as! String {
            case "shape":
                shapes += 1
                let text = row["input"] as! String
                let west = row["western"] as! [String: Any], east = row["indian"] as! [String: Any]
                holds(west, text, .invariant, "invariant")
                holds(west, text, NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all), "constructor")
                holds(
                    west, text, NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all, grouping: .western),
                    "declared")
                holds(west, text, indian.withGrouping(.western), "indian to western")
                holds(west, text, bridge(primary: 3, secondary: 3), "bridge threes")
                holds(west, text, bridge(primary: 4, secondary: 0), "bridge fours")
                holds(
                    east, text, NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all, grouping: .indian),
                    "declared indian")
                holds(east, text, indian, "western to indian")
                holds(east, text, bridge(primary: 3, secondary: 2), "bridge ml, en_US's Western currency beside it")
            case "detect":
                holds(row, row["input"] as! String, .detect, "detect")
            case "refused_flags":
                let flags = UInt32(row["flags"] as! Int)
                XCTAssertNotNil(NumFormat.refusal(ofFlags: flags), "\(flags)")
                XCTAssertNil(Grouping(rawValue: flags & Grouping.mask), "\(flags)")
                XCTAssertTrue(NumFormat.refusal(ofFlags: flags)?.contains("names no grouping policy") ?? false)
            case "accepted_flags":
                let flags = UInt32(row["flags"] as! Int)
                XCTAssertNil(NumFormat.refusal(ofFlags: flags), "\(flags)")
                let made = NumFormat(
                    decimalSeparator: ".", groupSeparator: ",", styles: NumStyles(rawValue: flags & 0xFF),
                    grouping: Grouping(rawValue: flags & Grouping.mask)!)
                XCTAssertEqual(made.raw.1, flags)
            case let other:
                XCTFail("unknown kind \(other)")
            }
        }
        XCTAssertGreaterThanOrEqual(shapes, 18)
    }

    func testDeclarationsCorpus() throws {
        var replayed = 0
        let vectors = try corpus("declarations.json")
        for vector in vectors {
            replayed += 1
            // Invalid UTF-8 has no `String` spelling, so those rows reach the conversion as
            // bytes, and the public initializer, which takes text, cannot carry them.
            if let raw = vector["input_bytes"] as? [Int] {
                let converted = raw.map(UInt8.init).withUnsafeBytes { Constraints<Decimal>.declaredDecimal($0) }
                XCTAssertNil(converted.map(triple), "declarations: \(raw) converted")
                XCTAssertEqual(vector["refusal"] as? Int, 8, "declarations: \(raw)")
                continue
            }
            let input = vector["input"] as! String
            let converted = Constraints<Decimal>.declaredDecimal(input)
            let declared = Result { () throws(Constraints<Decimal>.Refused) in
                try Constraints<Decimal>.declare(maximum: ConstraintValue(input))
            }
            if vector["expect"] as! String == "refused" {
                XCTAssertNil(converted.map(triple), "declarations: '\(input)' converted")
                guard case .failure(let refused) = declared else {
                    XCTFail("declarations: '\(input)' was declared")
                    continue
                }
                XCTAssertEqual(refused.code, Int32(vector["refusal"] as! Int), "declarations: '\(input)'")
                continue
            }
            let want = Triple(
                magnitude: Decimal(string: vector["magnitude"] as! String)!,
                scale: UInt8(vector["scale"] as! Int), negative: vector["negative"] as! Bool)
            XCTAssertEqual(converted.map(triple), want, "declarations: '\(input)'")
            guard case .success(let constraints) = declared else {
                XCTFail("declarations: '\(input)' was refused")
                continue
            }
            let raw = Interop.rawConstraints(constraints)
            let upper = withUnsafeBytes(of: raw) { bytes in
                bytes.load(fromByteOffset: 40, as: Cast.OutScratch.self)
            }
            XCTAssertEqual(triple(upper), want, "declarations: '\(input)' upper")
            XCTAssertEqual(constraints.maximum, Decimal(string: vector["value"] as! String), "declarations: '\(input)'")
        }
        XCTAssertGreaterThan(vectors.count, 0)
        XCTAssertEqual(replayed, vectors.count)
    }

    private struct Triple: Equatable {
        let magnitude: Decimal
        let scale: UInt8
        let negative: Bool
    }

    /// The core's decimal as its parts: `lo: u64` at 0, `hi: u32` at 8, `scale` at 12,
    /// `negative` at 13. The magnitude is at most 2⁹⁶ − 1, exact in `Decimal`.
    private func triple(_ raw: Cast.OutScratch) -> Triple {
        withUnsafeBytes(of: raw) { bytes in
            let lo = bytes.load(fromByteOffset: 0, as: UInt64.self)
            let hi = bytes.load(fromByteOffset: 8, as: UInt32.self)
            return Triple(
                magnitude: Decimal(hi) * (Decimal(UInt64.max) + 1) + Decimal(lo),
                scale: bytes[12], negative: bytes[13] != 0)
        }
    }

    /// The plain shapes, which the checked doors leave alone: `Verdict<T>` is exactly
    /// `.success(T)` or `.fault(Fault)` (the switch has no default, so a third case, or a
    /// changed payload, stops this compiling), a `Fault` is exactly its reason, offset and
    /// length (plus the raw `detailCode`, which equality ignores), and `CastFailure` is exactly the
    /// core's three reasons.
    func testPlainShapesAreUnchanged() {
        func describe(_ verdict: Verdict<Int32>) -> String {
            switch verdict {
            case .success(let value): "success \(value)"
            case .fault(let fault): "fault \(fault.reason) \(fault.offset) \(fault.length)"
            }
        }
        XCTAssertEqual(describe(Cast.i32("12", format: .invariant)), "success 12")
        XCTAssertEqual(describe(Cast.i32("1x", format: .invariant)), "fault malformed 1 1")
        XCTAssertEqual(describe(Cast.i32("12,34", format: .invariant)), "fault malformed 2 3")
        let fault = Fault(reason: .outOfRange, offset: 1, length: 2)
        XCTAssertEqual(
            Mirror(reflecting: fault).children.map(\.label), ["reason", "offset", "length", "detailCode"])
        XCTAssertEqual(
            Mirror(reflecting: fault).children.map { "\(type(of: $0.value))" },
            ["CastFailure", "Int", "Int", "UInt32"])
        XCTAssertEqual([CastFailure.empty, .malformed, .outOfRange].map(\.rawValue), [1, 2, 3])
        XCTAssertNil(CastFailure(rawValue: 0))
        XCTAssertNil(CastFailure(rawValue: 4))
        // The plain text door answers in the plain verdict.
        let plain: Verdict<String> = Cast.text("Male")
        XCTAssertEqual(plain, .success("Male"))
    }

    /// A Swift `String` is a sequence of Unicode scalars, which excludes the surrogates, so the
    /// lone-surrogate rows other bindings carry (input, and an `enum` entry the core refuses as
    /// not UTF-8, code 9) have no Swift spelling: decoding a surrogate's bytes replaces them.
    /// Its bytes reach the core only through the `[UInt8]` door, as the invalid UTF-8 they are.
    func testALoneSurrogateCannotBeWrittenInAString() {
        let surrogate: [UInt8] = [0xED, 0xA0, 0x80]
        XCTAssertFalse(String(decoding: surrogate, as: UTF8.self).utf8.elementsEqual(surrogate))
        XCTAssertNil(Unicode.Scalar(UInt32(0xD800)))
        guard case .fault(let fault) = Cast.text(surrogate) else {
            return XCTFail("a lone surrogate's bytes were read as text")
        }
        XCTAssertEqual(fault.reason, .malformed)
        XCTAssertEqual(fault.offset, 0)
    }
}
