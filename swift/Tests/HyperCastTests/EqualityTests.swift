import XCTest

import HyperCast

/// The declarations are values: equal when they declare the same thing, as the core compares
/// them, and hashing alike when equal, so they key a `Set` or a `Dictionary`.
final class EqualityTests: XCTestCase {
    func testAFormatHashesAsItCompares() {
        let built = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all)
        XCTAssertEqual(built, .invariant)
        XCTAssertEqual(built.hashValue, NumFormat.invariant.hashValue)
        let indian = built.withGrouping(.indian)
        XCTAssertEqual(indian.withGrouping(.western), built)
        XCTAssertEqual(indian.withGrouping(.western).hashValue, built.hashValue)
        let others = [
            indian,
            NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all),
            NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .grouping),
            NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all, currencySymbol: "$"),
            NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all, minusSign: "\u{2212}"),
            NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all, digitZero: "\u{0660}"),
            NumFormat.detect,
        ]
        for other in others {
            XCTAssertNotEqual(other, built)
        }
        XCTAssertEqual(Set(others + [built, .invariant]).count, others.count + 1)
        XCTAssertEqual(Set([NumStyles.all, NumStyles.all, .grouping]).count, 2)
    }

    func testDateDeclarationsHashAsTheyCompare() {
        let german = DateFormat(order: .dayMonthYear, amSymbol: "vorm.", pmSymbol: "nachm.")
        let again = DateFormat(order: .dayMonthYear, amSymbol: "vorm.", pmSymbol: "nachm.")
        XCTAssertEqual(german.hashValue, again.hashValue)
        let american = DateFormat(order: .monthDayYear, amSymbol: "vorm.", pmSymbol: "nachm.")
        XCTAssertEqual(Set([german, again, american]).count, 2)

        let entries = (1...12).map { (month: $0, name: "m" + String(UnicodeScalar(UInt8(96 + $0)))) }
        let names = MonthNames(entries)
        let options = DateOptions(flags: .compactDate, pivot: 2029, months: names)
        let same = DateOptions(flags: [.compactDate], pivot: 2029, months: MonthNames(entries))
        XCTAssertEqual(options, same)
        XCTAssertEqual(options.hashValue, same.hashValue)
        XCTAssertEqual(names.hashValue, MonthNames(entries).hashValue)
        let differing = [
            DateOptions(flags: .compactDate, pivot: 2029),
            DateOptions(flags: .commonForms, pivot: 2029, months: names),
            DateOptions(flags: .compactDate, pivot: 2030, months: names),
            DateOptions(flags: .compactDate, pivot: 2029, months: MonthNames(entries.reversed())),
        ]
        XCTAssertEqual(Set(differing + [options, same]).count, differing.count + 1)
    }
}
