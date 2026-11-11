import Foundation
import XCTest
@testable import HyperCast

/// What 1.1 adds beside the corpus: grouping (the constructor's default, its explicit
/// option, the locale bridge), the fault detail and its unknown codes, the opt-in date
/// options, the version guard, and the 1.0 surface exactly as 1.0 code uses it.
final class DetailedTests: XCTestCase {
    // MARK: - the 1.0 surface beside the 1.1 detail

    /// A door's fault equals the 1.0 fault for the same failure, though it carries a detail; the
    /// 1.0-only surface itself is `OneDotZeroSurfaceTests`, which compiles against 1.0 unchanged.
    func testADetailIsBesideEqualityNeverInIt() {
        let handBuilt = Fault(reason: .malformed, offset: 4, length: 1)
        let fromDoor = Cast.i32("  12x4", format: .invariant)
        XCTAssertEqual(fromDoor, .fault(handBuilt))
        XCTAssertNotNil(fromDoor.fault?.detail)
        XCTAssertNil(handBuilt.detail)
        XCTAssertEqual(handBuilt.detailCode, 0)
        XCTAssertEqual(
            Fault(reason: .malformed, offset: 4, length: 1, detailCode: 999), handBuilt, "equality ignores the detail")
        XCTAssertEqual(Interop.fault(code: 2, offset: 4, length: 1).detailCode, 0)
        XCTAssertEqual(Rejection<Int32>.fault(handBuilt), Rejection<Int32>.fault(fromDoor.fault!))
    }

    // MARK: - the detail vocabulary

    func testEveryPublishedDetailHasItsCodeAndSpelling() {
        let table = CorpusTests.detailCodes
        XCTAssertEqual(table.count, 32)
        for (name, code) in table {
            let detail = FaultDetail(rawValue: code)
            XCTAssertEqual(detail.name, name)
            XCTAssertEqual(detail.rawValue, code)
            XCTAssertEqual(detail.code, code)
            XCTAssertEqual(detail.description, name)
            XCTAssertTrue(detail.isPublished)
        }
        XCTAssertEqual(FaultDetail(rawValue: 11), .groupSize)
        XCTAssertEqual(FaultDetail(rawValue: 32), .notAScalar)
        XCTAssertEqual(FaultDetail(code: 11), .groupSize)
        // The members are distinct values, and a set of them holds all 32.
        XCTAssertEqual(Set(table.values.map { FaultDetail(rawValue: $0) }).count, 32)
    }

    /// A code the binding doesn't name: kept, never refused, and the fault around it is still
    /// whole — a decoded `RawFaultDetail` carrying 999.
    func testAnUnpublishedDetailCodeDecodesWithoutTrappingAndKeepsTheSpan() {
        for code in [UInt32(999), 33, 0, UInt32.max] {
            let detail = FaultDetail(rawValue: code)
            XCTAssertEqual(detail.rawValue, code)
            XCTAssertNil(detail.name)
            XCTAssertFalse(detail.isPublished)
            XCTAssertEqual(detail.description, "detail \(code)")
        }
        XCTAssertEqual(FaultDetail(rawValue: 999), FaultDetail(code: 999))
        XCTAssertNotEqual(FaultDetail(rawValue: 33), FaultDetail.notAScalar)

        // The layout a detailed export writes: the capacity word, offset, length, detail.
        var raw: Interop.RawFaultDetail = (UInt32(Interop.rawFaultDetailSize), 2, 3, 999)
        for reason in [Int32(1), 2, 3] {
            let fault = withUnsafeBytes(of: &raw) { Interop.fault(code: reason, faultDetail: $0) }
            XCTAssertEqual(fault.reason, CastFailure(rawValue: reason))
            XCTAssertEqual(fault.offset, 2)
            XCTAssertEqual(fault.length, 3)
            XCTAssertEqual(fault.detailCode, 999)
            XCTAssertEqual(fault.detail, FaultDetail(rawValue: 999))
            XCTAssertNil(fault.detail?.name)
        }
        // The same fault, taken as a verdict, still switches like any other.
        let verdict: Verdict<Int32> = .fault(Interop.fault(code: 2, offset: 2, length: 3, detail: 999))
        guard case .fault(let fault) = verdict else { return XCTFail("a fault") }
        XCTAssertEqual(fault, Fault(reason: .malformed, offset: 2, length: 3))
    }

    func testTheRawFaultAndDateOptionLayoutsAreTheHeadersOwn() {
        XCTAssertEqual(MemoryLayout<Interop.RawFaultDetail>.size, 16)
        XCTAssertEqual(MemoryLayout<Interop.RawFaultDetail>.alignment, 4)
        XCTAssertEqual(MemoryLayout<Interop.RawDateOptions>.size, 12)
        XCTAssertEqual(MemoryLayout<Interop.RawDateOptions>.alignment, 4)
        XCTAssertEqual(MemoryLayout<Cast.DetailScratch>.size, Cast.headerRawFaultDetailSize)
        XCTAssertEqual(Interop.rawFaultDetailSize, Cast.headerRawFaultDetailSize)
        XCTAssertEqual(Interop.rawDateOptionsSize, Cast.headerRawDateOptionsSize)
        XCTAssertEqual(Cast.headerRawDateOptionsMinSize, 8)
        XCTAssertEqual(
            Cast.headerGroupingBits,
            [Grouping.western.rawValue, Grouping.indian.rawValue, 0x300])
        XCTAssertEqual(Interop.freshRawFaultDetail.0, 16)
        let options = DateOptions(flags: .commonForms, pivot: 2029)
        XCTAssertEqual(options.raw.0, 12)
        XCTAssertEqual(options.raw.1, 7)
        XCTAssertEqual(options.raw.2, 2029)
    }

    // MARK: - grouping

    func testTheDefaultAndTheInvariantProfileAreWestern() {
        XCTAssertEqual(NumFormat.invariant.grouping, .western)
        XCTAssertEqual(NumFormat.detect.grouping, .western)
        XCTAssertEqual(NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all).grouping, .western)
        XCTAssertEqual(NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all), .invariant)
        XCTAssertEqual(NumFormat.invariant.styles, .all, "the styles never carry the policy")
        XCTAssertEqual(
            Cast.i32("12,34", format: .invariant),
            .fault(Fault(reason: .malformed, offset: 2, length: 3)))
        guard case .fault(let fault) = Cast.i32("12,34", format: .invariant) else { return XCTFail("12,34 reads") }
        XCTAssertEqual(fault.detail, .groupSize)
        XCTAssertEqual(Cast.i32("1,234", format: .invariant), .success(1234))
        XCTAssertEqual(Cast.i32("1234", format: .invariant), .success(1234))
        guard case .fault(let leftmost) = Cast.i32("1234,567", format: .invariant) else { return XCTFail("reads") }
        XCTAssertEqual(leftmost, Fault(reason: .malformed, offset: 0, length: 4))
    }

    func testIndianIsExplicitAndReplacesThePolicy() {
        XCTAssertEqual(
            Cast.i32("1,23,456", format: .invariant), .fault(Fault(reason: .malformed, offset: 1, length: 3)))
        XCTAssertEqual(Cast.i32("0,125", format: .invariant), .fault(Fault(reason: .malformed, offset: 0, length: 1)))
        XCTAssertEqual(
            Cast.i32("1234,567", format: .invariant), .fault(Fault(reason: .malformed, offset: 0, length: 4)))
        let indian = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all, grouping: .indian)
        XCTAssertEqual(indian.grouping, .indian)
        XCTAssertEqual(Cast.i32("12,34,567", format: indian), .success(1234567))
        XCTAssertEqual(
            Cast.i32("1,234,567", format: indian),
            .fault(Fault(reason: .malformed, offset: 1, length: 4)))
        // Replaced, never combined: Indian after Western is Indian.
        XCTAssertEqual(NumFormat.invariant.withGrouping(.indian).grouping, .indian)
        XCTAssertEqual(NumFormat.invariant.withGrouping(.indian).withGrouping(.western).grouping, .western)
        XCTAssertEqual(NumFormat.invariant.withGrouping(.indian).raw.1 & 0x300, 0x200)
    }

    func testWithStylesPreservesThePolicyAndEverythingElse() {
        for grouping in Grouping.allCases {
            let base = NumFormat(
                decimalSeparator: ",", groupSeparator: ".", styles: .all, grouping: grouping, currencySymbol: "€")
            let narrowed = base.withStyles([.grouping, .currency])
            XCTAssertEqual(narrowed.grouping, grouping)
            XCTAssertEqual(narrowed.styles, [.grouping, .currency])
            XCTAssertEqual(narrowed.currencySymbol, "€")
            XCTAssertEqual(narrowed.decimalSeparator, ",")
            XCTAssertEqual(narrowed.withStyles(.all), base)
        }
    }

    func testTheFlagsWordIsRefusedWhereTheCoreRefusesIt() {
        XCTAssertNil(NumFormat.refusal(ofFlags: 0))
        XCTAssertNil(NumFormat.refusal(ofFlags: 0xFF | 0x200))
        XCTAssertNotNil(NumFormat.refusal(ofFlags: 0xFF | 0x100), "bit 8 is retired")
        XCTAssertNotNil(NumFormat.refusal(ofFlags: 0x300), "bit 8 is retired")
        XCTAssertNotNil(NumFormat.refusal(ofFlags: 0x400), "bit 10 is undefined")
        XCTAssertNotNil(NumFormat.refusal(ofFlags: 1 << 31))
        // A style mask carrying the policy is refused, not silently merged.
        XCTAssertNotNil(NumFormat.refusal(ofFlags: NumStyles(rawValue: 0x1FF).rawValue | 0x300))
    }

    /// Review focus 1: native digits with a space-like group separator under Western.
    func testNativeDigitsWithASpaceLikeSeparatorAreCountedInDigitsNotBytes() {
        let arabic = NumFormat(
            decimalSeparator: ".", groupSeparator: "\u{202F}", styles: .all, digitZero: "\u{0660}")
        XCTAssertEqual(Cast.i32("\u{0661}\u{202F}\u{0662}\u{0663}\u{0664}", format: arabic), .success(1234))
        // Any of the three space-like spellings reads as the declared one.
        XCTAssertEqual(Cast.i32("\u{0661} \u{0662}\u{0663}\u{0664}", format: arabic), .success(1234))
        let verdict = Cast.i32("\u{0661}\u{0662}\u{202F}\u{0663}\u{0664}", format: arabic)
        guard case .fault(let fault) = verdict else { return XCTFail("١٢ ٣٤ reads") }
        XCTAssertEqual(fault.reason, .malformed)
        XCTAssertEqual(fault.detail, .groupSize)
        // The span is the separator (3 bytes) and the two Arabic-Indic digits (2 bytes each),
        // starting after the two digits the group before it spent (4 bytes).
        XCTAssertEqual(fault.offset, 4)
        XCTAssertEqual(fault.length, 7)
    }

    /// A policy does not turn grouping on.
    func testAPolicyWithGroupingOffStillForbidsTheGroupSeparator() {
        let noGrouping = NumFormat.invariant.withStyles(NumStyles.all.subtracting(.grouping))
        XCTAssertEqual(noGrouping.grouping, .western)
        guard case .fault(let fault) = Cast.i32("1,234", format: noGrouping) else {
            return XCTFail("1,234 reads with grouping off")
        }
        XCTAssertEqual(fault.reason, .malformed)
        XCTAssertEqual(fault.offset, 1)
        XCTAssertEqual(Cast.i32("1234", format: noGrouping), .success(1234))
    }

    /// An old export, handed a 1.1 policy bit, ignores it and reads Western.
    func testAnOldExportIgnoresAPolicyBitAndTheDoorDoesNot() {
        let indian = NumFormat.invariant.withGrouping(.indian)
        var raw = indian.raw
        var out: Cast.OutScratch = (0, 0)
        var fault: Cast.FaultScratch = (0, 0)
        let text = Array("1,23,456".utf8)
        let code = text.withUnsafeBytes { input in
            withUnsafeMutableBytes(of: &out) { outRaw in
                withUnsafeMutableBytes(of: &fault) { faultRaw in
                    withUnsafeBytes(of: &raw) { formatRaw in
                        Cast.oneDotZero.i32(
                            Cast.inputPointer(input), UInt(input.count), formatRaw.baseAddress, outRaw.baseAddress,
                            faultRaw.baseAddress)
                    }
                }
            }
        }
        XCTAssertEqual(code, CastFailure.malformed.rawValue, "the 1.0 export refuses 1,23,456")
        XCTAssertEqual(fault.0, 1)
        XCTAssertEqual(fault.1, 3)
        XCTAssertEqual(Cast.i32(text, format: indian), .success(123456))
    }

    // MARK: - the locale bridge reads the platform's patterns

    private func formatters(_ identifier: String) -> (number: NumberFormatter, currency: NumberFormatter) {
        let locale = Locale(identifier: identifier)
        let number = NumberFormatter()
        number.locale = locale
        number.numberStyle = .decimal
        let currency = NumberFormatter()
        currency.locale = locale
        currency.numberStyle = .currency
        return (number, currency)
    }

    /// The actual patterns of the locales the owner's rule names, as this platform ships them.
    func testTheActualPatternsDeclareWhatTheRuleSays() {
        func declared(_ identifier: String) -> (Grouping, Bool) {
            let format = NumFormat.from(locale: Locale(identifier: identifier))
            return (format.grouping, format.styles.contains(.grouping))
        }
        for identifier in ["en_US", "de_DE", "fr_FR", "de_CH", "ja_JP", "en_GB", "en_BG"] {
            XCTAssertTrue(declared(identifier) == (.western, true), identifier)
        }
        XCTAssertEqual(Cast.i32("1,234,567", locale: Locale(identifier: "en_US")), .success(1_234_567))
        XCTAssertEqual(
            Cast.i32("12,34", locale: Locale(identifier: "en_US")),
            .fault(Fault(reason: .malformed, offset: 2, length: 3)))
        #if canImport(Darwin)
            // en_BG: its currency pattern doesn't group, its number pattern does: the flag stays.
            let bulgaria = formatters("en_BG")
            XCTAssertTrue(bulgaria.number.usesGroupingSeparator)
            XCTAssertFalse(bulgaria.currency.usesGroupingSeparator)
            for identifier in ["en_IN", "hi_IN", "mr_IN", "ta_IN", "ml_IN", "or_IN", "bn_IN"] {
                let pair = formatters(identifier)
                XCTAssertEqual(pair.number.groupingSize, 3, identifier)
                XCTAssertEqual(pair.number.secondaryGroupingSize, 2, identifier)
                XCTAssertTrue(declared(identifier) == (.indian, true), identifier)
            }
            // Toki Pona groups by twos, secondary 0 meaning "repeat the primary": no policy names
            // that, so Western, which refuses it rather than misreading it.
            for identifier in ["tok", "tok_001"] {
                let pair = formatters(identifier)
                XCTAssertEqual(pair.number.groupingSize, 2)
                XCTAssertEqual(pair.number.secondaryGroupingSize, 0)
                XCTAssertTrue(declared(identifier) == (.western, true), identifier)
            }
            // en_US_POSIX groups in neither pattern (size 0): the flag is cleared.
            XCTAssertTrue(declared("en_US_POSIX").1 == false)
            XCTAssertEqual(Cast.i32("1234", locale: Locale(identifier: "en_US_POSIX")), .success(1234))
            XCTAssertEqual(Cast.i32("1,234", locale: Locale(identifier: "en_US_POSIX")).fault?.reason, .malformed)

            XCTAssertEqual(Cast.i32("12,34,567", locale: Locale(identifier: "en_IN")), .success(1_234_567))
            XCTAssertEqual(Cast.i32("1,234,567", locale: Locale(identifier: "en_IN")).fault?.detail, .groupSize)
        #else
            // swift-corelibs-foundation gives every locale's NumberFormatter `#,##0.###`, and writes
            // with it: en_IN's 1,234,567. The bridge reads what the platform writes, so Western.
            for identifier in ["en_IN", "hi_IN", "ml_IN", "tok", "en_BG", "en_US_POSIX"] {
                let pair = formatters(identifier)
                XCTAssertEqual(pair.number.groupingSize, 3, identifier)
                XCTAssertEqual(pair.number.secondaryGroupingSize, 0, identifier)
                XCTAssertTrue(declared(identifier) == (.western, true), identifier)
            }
            XCTAssertEqual(Cast.i32("1,234,567", locale: Locale(identifier: "en_IN")), .success(1_234_567))
            XCTAssertEqual(Cast.i32("12,34,567", locale: Locale(identifier: "en_IN")).fault?.detail, .groupSize)
        #endif
    }

    /// One policy and one flag for number and currency values: every mixed case.
    func testNumberAndCurrencyPatternsShareOnePolicyAndOneFlag() {
        typealias Shape = NumFormat.GroupingShape
        func resolve(_ number: Shape, _ currency: Shape) -> String {
            let (grouping, groups) = NumFormat.grouping(number: number, currency: currency)
            return groups ? "\(grouping)" : "cleared"
        }
        XCTAssertEqual(resolve(.western, .western), "western")
        XCTAssertEqual(resolve(.indian, .indian), "indian")
        XCTAssertEqual(resolve(.western, .indian), "western")
        XCTAssertEqual(resolve(.indian, .western), "indian")
        XCTAssertEqual(resolve(.indian, .none), "indian")
        XCTAssertEqual(resolve(.none, .indian), "western")
        XCTAssertEqual(resolve(.western, .none), "western")
        XCTAssertEqual(resolve(.none, .western), "western")
        XCTAssertEqual(resolve(.western, .other), "western")
        XCTAssertEqual(resolve(.other, .western), "western")
        XCTAssertEqual(resolve(.indian, .other), "indian")
        XCTAssertEqual(resolve(.other, .other), "western")
        XCTAssertEqual(resolve(.other, .none), "western")
        XCTAssertEqual(resolve(.none, .other), "western")
        XCTAssertEqual(resolve(.none, .none), "cleared")
    }

    /// The pattern is read in full from a formatter: a secondary size of 0 repeats the primary,
    /// `usesGroupingSeparator` off or a primary size of 0 is no grouping, and the caller's own
    /// overrides decide the number pattern, whatever the locale's name.
    func testAFormatterBridgeReadsTheFormattersOwnPattern() {
        func shape(primary: Int, secondary: Int, uses: Bool = true) -> NumFormat.GroupingShape {
            let formatter = NumberFormatter()
            formatter.locale = Locale(identifier: "en_US")
            formatter.numberStyle = .decimal
            formatter.usesGroupingSeparator = uses
            formatter.groupingSize = primary
            formatter.secondaryGroupingSize = secondary
            return NumFormat.GroupingShape(of: formatter)
        }
        XCTAssertEqual(shape(primary: 3, secondary: 0), .western)
        XCTAssertEqual(shape(primary: 3, secondary: 3), .western)
        XCTAssertEqual(shape(primary: 3, secondary: 2), .indian)
        XCTAssertEqual(shape(primary: 2, secondary: 0), .other)
        XCTAssertEqual(shape(primary: 2, secondary: 2), .other)
        XCTAssertEqual(shape(primary: 4, secondary: 0), .other)
        XCTAssertEqual(shape(primary: 3, secondary: 4), .other)
        XCTAssertEqual(shape(primary: 0, secondary: 0), .none)
        XCTAssertEqual(shape(primary: 3, secondary: 2, uses: false), .none)

        // The number pattern decides: an Indian number formatter beside en_US's Western currency is Indian.
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "en_US")
        formatter.numberStyle = .decimal
        formatter.groupingSize = 3
        formatter.secondaryGroupingSize = 2
        XCTAssertEqual(NumFormat.from(formatter: formatter).grouping, .indian)
        // A caller who turns the number grouping off leaves the currency's: Western, the flag stays.
        formatter.usesGroupingSeparator = false
        let numberOff = NumFormat.from(formatter: formatter)
        XCTAssertEqual(numberOff.grouping, .western)
        XCTAssertTrue(numberOff.styles.contains(.grouping))
        // en_IN with its number pattern overridden to Western is Western, whatever its currency says.
        let overridden = NumberFormatter()
        overridden.locale = Locale(identifier: "en_IN")
        overridden.numberStyle = .decimal
        overridden.groupingSize = 3
        overridden.secondaryGroupingSize = 3
        XCTAssertEqual(NumFormat.from(formatter: overridden).grouping, .western)
    }

    /// The bridge reads what the platform writes, for number and currency values alike, where the
    /// pattern is the policy's. Not `tok`, whose twos end in a last group no policy reads.
    func testTheBridgeReadsThePlatformsOwnNumberAndCurrencyValues() {
        let identifiers = [
            "en_US", "en_GB", "de_DE", "fr_FR", "de_CH", "ja_JP", "en_IN", "hi_IN", "ml_IN", "ta_IN", "mr_IN", "or_IN",
            "bn_IN", "en_BG", "en_US_POSIX", "ar_EG", "es_ES", "pt_BR", "ru_RU", "sv_SE",
        ]
        for identifier in identifiers {
            let locale = Locale(identifier: identifier)
            let pair = formatters(identifier)
            for number in [1234567.891, 12345.5, 999.0, 1000.0, 12345678.0, -98765.25] {
                // ar_EG's currency spelling (`ج.م.` between bidi marks) is read by neither the base
                // nor this change, for a reason that has nothing to do with grouping.
                let numberShape = NumFormat.GroupingShape(of: pair.number)
                let currencyShape = NumFormat.GroupingShape(of: pair.currency)
                let kinds = [("number", pair.number), ("currency", pair.currency)].filter {
                    !($0.0 == "currency"
                        && (identifier == "ar_EG" || (numberShape == .indian) != (currencyShape == .indian)))
                }
                for (kind, formatter) in kinds {
                    guard let text = formatter.string(from: NSNumber(value: number)) else { continue }
                    let verdict = Cast.f64(text, locale: locale)
                    guard case .success(let value) = verdict else {
                        XCTFail("\(identifier) \(kind) '\(text)': \(verdict)")
                        continue
                    }
                    // A currency amount is rounded to the currency's own digits (ja_JP has none).
                    let tolerance = formatter === pair.currency ? 0.5 : 0
                    XCTAssertEqual(value, number, accuracy: tolerance, "\(identifier) '\(text)'")
                }
            }
        }
    }

    /// Every locale the platform ships: no bridge call traps, its policy is the function of its
    /// two patterns, the styles never carry a policy, and on Apple's data `en_BG` is the only one
    /// whose patterns differ in whether they group.
    func testEveryLocaleDeclaresThePolicyItsPatternsName() {
        var counts: [Grouping: Int] = [:]
        var cleared = 0
        var oneSided: [String] = []
        for identifier in Locale.availableIdentifiers {
            let locale = Locale(identifier: identifier)
            let pair = formatters(identifier)
            let number = NumFormat.GroupingShape(of: pair.number)
            let currency = NumFormat.GroupingShape(of: pair.currency)
            let (grouping, groups) = NumFormat.grouping(number: number, currency: currency)
            let format = NumFormat.from(locale: locale)
            XCTAssertEqual(format.grouping, grouping, identifier)
            XCTAssertEqual(format.styles.contains(.grouping), groups, identifier)
            XCTAssertEqual(format.styles.rawValue & ~NumStyles.definedBits, 0, identifier)
            if groups { counts[grouping, default: 0] += 1 } else { cleared += 1 }
            if (number == .none) != (currency == .none) { oneSided.append(identifier) }
            XCTAssertEqual(grouping, number == .indian ? .indian : .western, identifier)
        }
        XCTAssertGreaterThan(counts[.western] ?? 0, 900)
        #if canImport(Darwin)
            XCTAssertGreaterThan(counts[.indian] ?? 0, 10)
            XCTAssertEqual(oneSided, ["en_BG"])
            XCTAssertEqual(cleared, 1, "en_US_POSIX alone groups in neither")
        #else
            // swift-corelibs-foundation's patterns are all Western.
            XCTAssertEqual(counts[.indian], nil)
            XCTAssertEqual(oneSided, [])
            XCTAssertEqual(cleared, 0)
        #endif
    }

    // MARK: - the opt-in date shapes

    func testTheCommonFormsProfileOpensThreeShapesAndSuppliesNoPivotOrNames() {
        XCTAssertEqual(DateOptions.commonForms.flags, [.compactDate, .spaceSeparator, .hourOffset])
        XCTAssertNil(DateOptions.commonForms.pivot)
        XCTAssertNil(DateOptions.commonForms.months)
        XCTAssertEqual(DateOptions.none.flags, [])
        XCTAssertEqual(DateOptions.Flags.commonForms.rawValue, 7)
        // Without the options, each door keeps its strict shape.
        XCTAssertNotEqual(Cast.date("20260107").isSuccess, true)
        XCTAssertNotEqual(Cast.timestamp("2026-01-07 03:04:05Z").isSuccess, true)
        XCTAssertNotEqual(Cast.timestamp("2026-01-07T03:04:05+00").isSuccess, true)
        // With them they open, and an irrelevant known option is ignored.
        let common = DateOptions.commonForms
        XCTAssertEqual(
            Cast.date("20260107", options: common), .success(DateComponents(year: 2026, month: 1, day: 7)))
        XCTAssertEqual(
            Cast.timestamp("2026-01-07 03:04:05+00", options: common),
            Cast.timestamp("2026-01-07T03:04:05Z"))
        XCTAssertEqual(
            Cast.date("07/01/2026", order: .dayMonthYear, options: common),
            .success(DateComponents(year: 2026, month: 1, day: 7)))
    }

    func testAPivotReadsTwoDigitYearsInAHundredYearWindow() {
        let options = DateOptions(pivot: 2029)
        let expect = { (year: Int) in Verdict<DateComponents>.success(DateComponents(year: year, month: 1, day: 7)) }
        XCTAssertEqual(Cast.date("7/1/26", order: .monthDayYear, options: options), expect(2026).swappedDayMonth)
        XCTAssertEqual(Cast.date("26/01/07", order: .yearMonthDay, options: options), expect(2026))
        XCTAssertEqual(Cast.date("29/01/07", order: .yearMonthDay, options: options), expect(2029))
        XCTAssertEqual(Cast.date("30/01/07", order: .yearMonthDay, options: options), expect(1930))
        XCTAssertEqual(
            Cast.date("26/01/07", order: .yearMonthDay), .fault(Fault(reason: .malformed, offset: 0, length: 2)))
        guard case .fault(let fault) = Cast.date("26/01/07", order: .yearMonthDay) else {
            return XCTFail("a pivot-less yy")
        }
        XCTAssertEqual(fault.detail, .fieldWidth)
    }

    func testMonthNamesReadInTheMonthFieldAndNeverReorderIt() {
        let english = MonthNames([
            (1, "Jan"), (2, "Feb"), (3, "Mar"), (4, "Apr"), (5, "May"), (6, "Jun"), (7, "Jul"), (8, "Aug"),
            (9, "Sep"), (9, "Sept"), (10, "Oct"), (11, "Nov"), (12, "Dec"),
        ])
        let options = DateOptions(pivot: 2029, months: english)
        XCTAssertEqual(
            Cast.date("07-Jan-26", order: .dayMonthYear, options: options),
            .success(DateComponents(year: 2026, month: 1, day: 7)))
        XCTAssertEqual(
            Cast.date("7-JAN-2026", order: .dayMonthYear, options: DateOptions(months: english)),
            .success(DateComponents(year: 2026, month: 1, day: 7)))
        XCTAssertEqual(
            Cast.date("7-Sept-2026", order: .dayMonthYear, options: DateOptions(months: english)),
            .success(DateComponents(year: 2026, month: 9, day: 7)))
        XCTAssertEqual(
            Cast.dateTime("7-Jan-2026 3:04 PM", order: .dayMonthYear, options: DateOptions(months: english)),
            .success(DateComponents(year: 2026, month: 1, day: 7, hour: 15, minute: 4, second: 0, nanosecond: 0)))
        // Without the vocabulary the name is not a month.
        XCTAssertNotEqual(Cast.date("7-Jan-2026", order: .dayMonthYear).isSuccess, true)
        // A name never reorders: the declared order picks the month position.
        XCTAssertNotEqual(
            Cast.date("Jan-7-2026", order: .dayMonthYear, options: DateOptions(months: english)).isSuccess, true)
    }

    /// Review focus 2: a dotted name, the `.` separator and a pivot together.
    func testADottedMonthNameKeepsItsPunctuationAndTheSeparatorBesideIt() {
        let french = MonthNames([
            (1, "janv."), (2, "févr."), (3, "mars"), (4, "avr."), (5, "mai"), (6, "juin"), (7, "juil."), (8, "août"),
            (9, "sept."), (10, "oct."), (11, "nov."), (12, "déc."),
        ])
        let options = DateOptions(pivot: 2029, months: french)
        XCTAssertEqual(
            Cast.date("7.janv..26", order: .dayMonthYear, options: options),
            .success(DateComponents(year: 2026, month: 1, day: 7)))
        guard case .fault(let fault) = Cast.date("7.janv.26", order: .dayMonthYear, options: options) else {
            return XCTFail("7.janv.26 reads")
        }
        XCTAssertEqual(fault.reason, .malformed)
        // The word `janv` is read up to the next separator, and it is not a declared name.
        XCTAssertEqual(fault.offset, 2)
        XCTAssertEqual(fault.length, 4)
        XCTAssertEqual(fault.detail, .notRecognized)
    }

    func testDateOptionsAndMonthNamesRefuseWhatTheCoreRefuses() {
        XCTAssertNil(DateOptions.refusal(flags: 7, pivot: 0))
        XCTAssertNil(DateOptions.refusal(flags: 0, pivot: 100))
        XCTAssertNil(DateOptions.refusal(flags: 0, pivot: 9999))
        XCTAssertNotNil(DateOptions.refusal(flags: 8, pivot: 0))
        XCTAssertNotNil(DateOptions.refusal(flags: 0, pivot: 99))
        XCTAssertNotNil(DateOptions.refusal(flags: 0, pivot: 10000))
        XCTAssertNotNil(DateOptions.refusal(flags: 0, pivot: 65536 + 2029), "never truncated to a u16")
        // 0 declares none, as C#, Java, C++ and the ABI read it.
        XCTAssertNil(DateOptions(pivot: 0).pivot)
        XCTAssertEqual(DateOptions(pivot: 0), DateOptions())
        XCTAssertThrowsError(try MonthNames.declare([]))
        XCTAssertThrowsError(try MonthNames.declare([(13, "x")]))
        let full = (1...12).map {
            (month: $0, name: "m\(["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"][$0 - 1])")
        }
        XCTAssertNoThrow(try MonthNames.declare(full))
        XCTAssertThrowsError(try MonthNames.declare(full + [(month: 2, name: "MA")]), "a folded duplicate")
        XCTAssertEqual(try MonthNames.declare(full).packed.first, 1)
    }

    // MARK: - the old exports and the detailed doors agree on a Western format

    func testTheDetailedDoorsAgreeWithTheOldExportsOnWesternGrouping() {
        let western = NumFormat.invariant
        let inputs = [
            "", " ", "1", "-1", "(1,234)", "1,234", "12,34", "1,2,3", ",1", "1,", "1,,2", "0x1F", "1e3", "1e", "abc",
            "12a", "99999999999", "-99999999999", "1.5", "NaN", "inf", "$5", "5%", "١٢٣",
        ]
        for text in inputs {
            let bytes = Array(text.utf8)
            for door in 0..<3 {
                let (code, offset, length) = bytes.withUnsafeBytes { input -> (Int32, UInt32, UInt32) in
                    var raw = western.raw
                    var out: Cast.OutScratch = (0, 0)
                    var fault: Cast.FaultScratch = (0, 0)
                    let code = withUnsafeMutableBytes(of: &out) { outRaw in
                        withUnsafeMutableBytes(of: &fault) { faultRaw in
                            withUnsafeBytes(of: &raw) { formatRaw in
                                let call = [Cast.oneDotZero.i32, Cast.oneDotZero.f64, Cast.oneDotZero.decimal][door]
                                return call(
                                    Cast.inputPointer(input), UInt(input.count), formatRaw.baseAddress,
                                    outRaw.baseAddress, faultRaw.baseAddress)
                            }
                        }
                    }
                    return (code, fault.0, fault.1)
                }
                let verdictFault: Fault? =
                    switch door {
                    case 0: Cast.i32(bytes, format: western).fault
                    case 1: Cast.f64(bytes, format: western).fault
                    default: Cast.decimal(bytes, format: western).fault
                    }
                if code == 0 {
                    XCTAssertNil(verdictFault, "'\(text)' door \(door)")
                } else {
                    XCTAssertEqual(verdictFault?.reason.rawValue, code, "'\(text)' door \(door)")
                    XCTAssertEqual(verdictFault?.offset, Int(offset), "'\(text)' door \(door) offset")
                    XCTAssertEqual(verdictFault?.length, Int(length), "'\(text)' door \(door) length")
                    XCTAssertGreaterThanOrEqual(verdictFault?.detailCode ?? 0, 1, "'\(text)' door \(door) detail")
                }
            }
        }
    }

    func testTheOldTextAndCheckedExportsAgreeWithTheDetailedDoors() {
        for bytes: [UInt8] in [[], [0x61], [0xFF], [0x61, 0xC3]] {
            var span: Cast.FaultScratch = (0, 0)
            var fault: Cast.FaultScratch = (0, 0)
            let code = bytes.withUnsafeBytes { input in
                withUnsafeMutableBytes(of: &span) { spanRaw in
                    withUnsafeMutableBytes(of: &fault) { faultRaw in
                        Cast.oneDotZero.text(
                            Cast.inputPointer(input), UInt(input.count), spanRaw.baseAddress, faultRaw.baseAddress)
                    }
                }
            }
            switch Cast.text(bytes) {
            case .success: XCTAssertEqual(code, 0, "\(bytes)")
            case .fault(let detailed):
                XCTAssertEqual(detailed.reason.rawValue, code, "\(bytes)")
                XCTAssertEqual(detailed.offset, Int(fault.0), "\(bytes)")
                XCTAssertEqual(detailed.length, Int(fault.1), "\(bytes)")
            }
        }
        // A checked door: the old export's code for a violation is the detailed one's, and the
        // detail is 0 there (a violation names a keyword, not a detail).
        let constraints = Constraints<Int32>(maximum: 10)
        guard case .rejection(.violation(let violation)) = Cast.i32("11", format: .invariant, constraints: constraints)
        else { return XCTFail("11 breaks the maximum") }
        XCTAssertEqual(violation, Violation(keyword: .maximum, value: 11, offset: 0, length: 2))
    }

    func testEveryFailureOfAChar_Uuid_Time_AndDurationDoorNamesItsDetail() {
        for fault in [
            Cast.bool("maybe").fault, Cast.char("").fault, Cast.uuid("nope").fault, Cast.time("25:00").fault,
            Cast.duration("1x").fault, Cast.text([0xFF]).fault, Cast.timestamp("2026").fault,
            Cast.decimalFromDouble(.nan).fault, Cast.excelTime(-1).fault, Cast.excelDuration(.infinity).fault,
            Cast.excelSerialFromDouble(60, epoch: .y1900).fault,
        ] {
            XCTAssertNotNil(fault?.detail)
        }
        XCTAssertEqual(Cast.decimalFromDouble(.nan).fault?.detail, .notFinite)
        XCTAssertEqual(Cast.excelSerialFromDouble(60, epoch: .y1900).fault?.detail, .nonexistentDate)
        XCTAssertEqual(Cast.text([0xFF]).fault?.detail, .invalidUtf8)
        guard
            case .rejection(.fault(let fault)) = Cast.i32(
                "x", format: .invariant, constraints: Constraints<Int32>(maximum: 10))
        else { return XCTFail("x parses") }
        XCTAssertEqual(fault.detail, .unexpectedCharacter)
        guard
            case .rejection(.fault(let typed)) = Cast.i32FromDouble(.nan, constraints: Constraints<Int32>(maximum: 10))
        else {
            return XCTFail("NaN parses")
        }
        XCTAssertEqual(typed.detail, .notFinite)
    }
}

extension Verdict {
    var fault: Fault? { if case .fault(let fault) = self { fault } else { nil } }
    var isSuccess: Bool { if case .success = self { true } else { false } }
}

extension Verdict where T == DateComponents {
    /// The same verdict with the day and month swapped, for a test that spells one order's
    /// expectation from another's.
    var swappedDayMonth: Verdict<DateComponents> {
        guard case .success(let value) = self else { return self }
        return .success(DateComponents(year: value.year, month: value.day, day: value.month))
    }
}
