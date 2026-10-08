import Foundation
import HyperCast

// One door of every ABI shape, crossed through the public API, with the answers checked:
// plain (bool, uuid, timestamp, date, time, duration), numeric with a format (integers, a
// real, the exact decimal), the ones that take a mode (unix, ordered dates), and the typed
// doors that read a Double instead of text, with and without a declared epoch. Exits
// non-zero on the first thing that is wrong, so the build-and-run is the test.

func check(_ condition: Bool, _ what: String) {
    guard condition else {
        print("FAILED: \(what)")
        exit(1)
    }
}

do {
    check(Cast.isAvailable, "isAvailable")
    let version = try Cast.nativeVersion()
    check(version.split(separator: ".").count == 3, "nativeVersion is major.minor.patch, got \(version)")

    check(try Cast.bool("yes") == .success(true), "bool")
    check(try Cast.char("U+00E9") == .success("é"), "char")
    check(try Cast.char(" ") == .success(" "), "char verbatim")
    let generic: Verdict<Unicode.Scalar> = try Cast.scalar("&#x41;", format: .invariant)
    check(generic == .success("A"), "scalar char")
    check(try Cast.i32("-42", format: .invariant) == .success(-42), "i32")
    check(try Cast.i64("9223372036854775807", format: .invariant) == .success(.max), "i64 max")
    check(try Cast.u64("18446744073709551615", format: .invariant) == .success(.max), "u64 max")
    check(try Cast.f64("2.5e3", format: .invariant) == .success(2500), "f64")
    check(try Cast.decimal("0.1", format: .invariant) == .success(Decimal(string: "0.1")!), "decimal is exact")
    check(
        try Cast.decimal("79228162514264337593543950335", format: .invariant)
            == .success(Decimal(string: "79228162514264337593543950335")!),
        "decimal carries the full 96 bits")

    let uuid = UUID(uuidString: "2ed6657d-e927-568b-95e1-2665a8aea6a2")!
    check(try Cast.uuid("2ed6657d-e927-568b-95e1-2665a8aea6a2") == .success(uuid), "uuid")

    check(
        try Cast.timestamp("2023-11-14T22:13:20Z") == .success(Date(timeIntervalSince1970: 1_700_000_000)),
        "timestamp")
    check(
        try Cast.unix("1700000000", precision: .seconds) == .success(Date(timeIntervalSince1970: 1_700_000_000)),
        "unix seconds")
    guard case .success(let date) = try Cast.date("2024-02-29") else {
        print("FAILED: date did not parse")
        exit(1)
    }
    check(date.year == 2024 && date.month == 2 && date.day == 29, "date fields")
    guard case .success(let time) = try Cast.time("13:45:30") else {
        print("FAILED: time did not parse")
        exit(1)
    }
    check(time.hour == 13 && time.minute == 45 && time.second == 30, "time fields")
    check(try Cast.duration("PT1H30M") == .success(.seconds(5400)), "duration")
    let tenth = 0.1, fifth = 0.2
    check(
        try Cast.decimalFromDouble(tenth + fifth) == .success(Decimal(string: "0.30000000000000004")!),
        "decimalFromDouble")
    guard case .success(let serial) = try Cast.excelSerialFromDouble(45292.75, epoch: .y1900) else {
        print("FAILED: excelSerialFromDouble did not read")
        exit(1)
    }
    check(
        serial.year == 2024 && serial.month == 1 && serial.day == 1 && serial.hour == 18,
        "excelSerialFromDouble fields")
    check(try Cast.excelDuration(1.5) == .success(.seconds(129_600)), "excelDuration")

    // A fault is a value, with the reason and the offending span.
    check(
        try Cast.i32("12x", format: .invariant) == .fault(Fault(reason: .malformed, offset: 2, length: 1)),
        "a malformed integer faults at the offending byte")
    check(try Cast.i32("", format: .invariant) == .fault(Fault(reason: .empty, offset: 0, length: 0)), "empty input")
    guard case .fault(let overflow) = try Cast.i32("2147483648", format: .invariant) else {
        print("FAILED: an out-of-range i32 was accepted")
        exit(1)
    }
    check(overflow.reason == .outOfRange, "out of range")

    print("hypercast \(version) smoke test passed")
} catch {
    print("FAILED: \(error)")
    exit(1)
}
