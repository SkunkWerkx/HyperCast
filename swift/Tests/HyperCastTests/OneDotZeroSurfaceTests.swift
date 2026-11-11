import Foundation
import XCTest
@testable import HyperCast

/// The 1.0 API exactly as 1.0 code uses it, and nothing from 1.1: this file compiles and
/// passes unchanged against the 1.0 sources, so a 1.1 addition can't have moved anything
/// here. (A new initializer parameter or a new `Fault` field is invisible to it; a changed
/// label, a changed equality or a changed case is not.)
final class OneDotZeroSurfaceTests: XCTestCase {
    func testTheConstructorsOfOneDotZero() {
        let danish = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "kr.")
        XCTAssertEqual(danish.currencySymbol, "kr.")
        XCTAssertEqual(danish.decimalSeparator, ",")
        XCTAssertEqual(danish.groupSeparator, ".")
        XCTAssertEqual(danish.styles, .all)
        let scalars = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all)
        XCTAssertEqual(scalars, NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all))
        XCTAssertNotEqual(scalars, danish)
        let styled = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: [.grouping, .parentheses])
        XCTAssertEqual(styled.styles, [.grouping, .parentheses])
        XCTAssertEqual(Cast.i32("(1,234)", format: styled), .success(-1234))
        let full = NumFormat(
            decimalSeparator: ",", groupSeparator: ". ", styles: [.all, .trailingSign], currencySymbol: "€",
            currencyDecimalSeparator: ".", currencyGroupSeparator: ",", minusSign: "−", plusSign: "＋",
            percentSymbol: "٪", digitZero: "\u{0660}")
        XCTAssertEqual(full.digitZero, "\u{0660}")
        XCTAssertEqual(NumFormat.invariant.styles, .all)
        XCTAssertEqual(NumStyles(rawValue: 1), .grouping)
        let order = DateFormat(order: .dayMonthYear, amSymbol: "vorm.", pmSymbol: "nachm.")
        XCTAssertEqual(order.amSymbol, "vorm.")
    }

    func testFaultsAreBuiltCompareAndPatternMatchAsInOneDotZero() {
        let fault = Fault(reason: .malformed, offset: 4, length: 1)
        XCTAssertEqual(fault.reason, .malformed)
        XCTAssertEqual(fault.offset, 4)
        XCTAssertEqual(fault.length, 1)
        XCTAssertEqual(Cast.i32("  12x4", format: .invariant), .fault(fault))
        XCTAssertNotEqual(fault, Fault(reason: .malformed, offset: 4, length: 2))
        XCTAssertNotEqual(fault, Fault(reason: .empty, offset: 4, length: 1))
        XCTAssertNotEqual(fault, Fault(reason: .malformed, offset: 5, length: 1))
        XCTAssertEqual(Interop.fault(code: 2, offset: 4, length: 1), fault)
        XCTAssertEqual([CastFailure.empty, .malformed, .outOfRange].map(\.rawValue), [1, 2, 3])

        // The exhaustive two-arm switch, with the bindings of 1.0.
        switch Cast.u8("256", format: .invariant) {
        case .success: XCTFail("256 is not a u8")
        case .fault(let fault):
            XCTAssertEqual(fault.reason, .outOfRange)
            XCTAssertEqual(fault.offset, 0)
            XCTAssertEqual(fault.length, 3)
        }
        switch Cast.bool("yes") {
        case .success(let value): XCTAssertTrue(value)
        case .fault: XCTFail("yes is a boolean")
        }
        // The checked union, exhaustively.
        let constraints = Constraints<Int32>(maximum: 10)
        switch Cast.i32("11", format: .invariant, constraints: constraints) {
        case .success: XCTFail("11 breaks the maximum")
        case .rejection(.violation(let violation)):
            XCTAssertEqual(violation.keyword, .maximum)
            XCTAssertEqual(violation, Violation(keyword: .maximum, value: 11, offset: 0, length: 2))
        case .rejection(.fault): XCTFail("11 parses")
        }
        switch Cast.i32("x", format: .invariant, constraints: constraints) {
        case .success: XCTFail("x is not a number")
        case .rejection(.violation): XCTFail("x does not parse")
        case .rejection(.fault(let fault)): XCTAssertEqual(fault, Fault(reason: .malformed, offset: 0, length: 1))
        }
        XCTAssertEqual(Rejection<Int32>.fault(fault), .fault(Fault(reason: .malformed, offset: 4, length: 1)))
        XCTAssertNotEqual(
            Rejection<Int32>.fault(fault), .violation(Violation(keyword: .maximum, value: 11, offset: 4, length: 1)))
        let verdict: Verdict<Int32> = .fault(fault)
        XCTAssertEqual(verdict, .fault(fault))
        XCTAssertNotEqual(verdict, .success(1))
    }
}
