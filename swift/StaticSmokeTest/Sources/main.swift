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
    let version = Cast.nativeVersion()
    check(version.split(separator: ".").count == 3, "nativeVersion is major.minor.patch, got \(version)")

    check(Cast.bool("yes") == .success(true), "bool")
    check(Cast.char("U+00E9") == .success("é"), "char")
    check(Cast.char(" ") == .success(" "), "char verbatim")
    let generic: Verdict<Unicode.Scalar> = Cast.scalar("&#x41;", format: .invariant)
    check(generic == .success("A"), "scalar char")
    check(Cast.i32("-42", format: .invariant) == .success(-42), "i32")
    check(Cast.i64("9223372036854775807", format: .invariant) == .success(.max), "i64 max")
    check(Cast.u64("18446744073709551615", format: .invariant) == .success(.max), "u64 max")
    check(Cast.f64("2.5e3", format: .invariant) == .success(2500), "f64")
    check(Cast.decimal("0.1", format: .invariant) == .success(Decimal(string: "0.1")!), "decimal is exact")
    check(
        Cast.decimal("79228162514264337593543950335", format: .invariant)
            == .success(Decimal(string: "79228162514264337593543950335")!),
        "decimal carries the full 96 bits")

    let uuid = UUID(uuidString: "2ed6657d-e927-568b-95e1-2665a8aea6a2")!
    check(Cast.uuid("2ed6657d-e927-568b-95e1-2665a8aea6a2") == .success(uuid), "uuid")

    check(
        Cast.timestamp("2023-11-14T22:13:20Z") == .success(Date(timeIntervalSince1970: 1_700_000_000)),
        "timestamp")
    check(
        Cast.unix("1700000000", precision: .seconds) == .success(Date(timeIntervalSince1970: 1_700_000_000)),
        "unix seconds")
    guard case .success(let date) = Cast.date("2024-02-29") else {
        print("FAILED: date did not parse")
        exit(1)
    }
    check(date.year == 2024 && date.month == 2 && date.day == 29, "date fields")
    guard case .success(let time) = Cast.time("13:45:30") else {
        print("FAILED: time did not parse")
        exit(1)
    }
    check(time.hour == 13 && time.minute == 45 && time.second == 30, "time fields")
    check(Cast.duration("PT1H30M") == .success(.seconds(5400)), "duration")
    let tenth = 0.1, fifth = 0.2
    check(
        Cast.decimalFromDouble(tenth + fifth) == .success(Decimal(string: "0.30000000000000004")!),
        "decimalFromDouble")
    guard case .success(let serial) = Cast.excelSerialFromDouble(45292.75, epoch: .y1900) else {
        print("FAILED: excelSerialFromDouble did not read")
        exit(1)
    }
    check(
        serial.year == 2024 && serial.month == 1 && serial.day == 1 && serial.hour == 18,
        "excelSerialFromDouble fields")
    check(Cast.excelDuration(1.5) == .success(.seconds(129_600)), "excelDuration")

    // A fault is a value, with the reason and the offending span.
    check(
        Cast.i32("12x", format: .invariant) == .fault(Fault(reason: .malformed, offset: 2, length: 1)),
        "a malformed integer faults at the offending byte")
    check(Cast.i32("", format: .invariant) == .fault(Fault(reason: .empty, offset: 0, length: 0)), "empty input")
    guard case .fault(let overflow) = Cast.i32("2147483648", format: .invariant) else {
        print("FAILED: an out-of-range i32 was accepted")
        exit(1)
    }
    check(overflow.reason == .outOfRange, "out of range")

    print("hypercast \(version) smoke test passed")
} catch {
    print("FAILED: \(error)")
    exit(1)
}
