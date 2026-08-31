//! Allocation-free parsers for scalars from untrusted text — booleans, numerics, UUIDs,
//! and temporals. Every cast returns a verdict: the value, or a [`Fault`] carrying a closed
//! [`Reason`] and the offending byte span. Never panics on bad input, never allocates —
//! semantics ported from Svartalfheim's `Norse.Primitives` parser family, mechanics from
//! this project's own HyperUuid (one Rust `cdylib`, every host binding calls straight in).
//!
//! - [`cast_bool`] — the natural-language boolean lexicon
//! - [`cast_i8`]…[`cast_u64`] — the integer family under a caller-declared [`NumFormat`]
//! - [`cast_f32`] / [`cast_f64`] — finite reals only, percent notation included
//! - [`cast_uuid`] — every .NET `Guid` text format plus `urn:uuid:`-style prefixes,
//!   16 RFC 9562-ordered bytes out
//! - [`cast_timestamp`] / [`cast_unix`] — instants to protobuf's `{seconds, nanos}` pair
//! - [`cast_date`] / [`cast_time`] / [`cast_duration`] — the remaining temporal shapes,
//!   likewise protobuf-formed
//!
//! Text comes in as anything byte-viewable — `&str`, `String`, `&[u8]`, `Vec<u8>` — read
//! as UTF-8 bytes; each door trims ASCII whitespace and treats trimmed-empty input as
//! [`Reason::Empty`], which [`optional`] (and every binding's optional door) surfaces as
//! absent. [`Fault`] implements [`core::error::Error`], so a verdict composes with `?`
//! and ordinary error chains when propagate-on-failure is the caller's idiom.

mod boolean;
mod ffi;
mod integer;
mod real;
mod temporal;
mod uuid;
mod verdict;

#[cfg(feature = "python")]
mod python_ext;
#[cfg(feature = "ruby")]
mod ruby_ext;

pub use boolean::cast_bool;
pub use integer::{cast_i8, cast_i16, cast_i32, cast_i64, cast_u8, cast_u16, cast_u32, cast_u64};
pub use real::{cast_f32, cast_f64};
pub use temporal::{
    cast_date, cast_duration, cast_time, cast_timestamp, cast_unix, UnixPrecision,
    MAX_DURATION_SECONDS, MAX_TIMESTAMP_SECONDS, MIN_TIMESTAMP_SECONDS,
};
pub use uuid::cast_uuid;
pub use verdict::{Date, Duration, Fault, NumFormat, Reason, Timestamp};

/// Presents a verdict optionally: an [`Reason::Empty`] fault becomes `Ok(None)` — Rust's
/// absent — and everything else flows through untouched. The same presentation helper
/// every binding ships (`optional`/`Optional`), for callers whose absent-is-fine doors
/// shouldn't treat missing input as a failure.
pub fn optional<T>(verdict: Result<T, Fault>) -> Result<Option<T>, Fault> {
    match verdict {
        Ok(value) => Ok(Some(value)),
        Err(Fault { reason: Reason::Empty, .. }) => Ok(None),
        Err(fault) => Err(fault),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INVARIANT: NumFormat = NumFormat::INVARIANT;

    fn reason<T: core::fmt::Debug>(verdict: Result<T, Fault>) -> Reason {
        verdict.unwrap_err().reason
    }

    // --- boolean ---

    #[test]
    fn bool_recognizes_the_full_lexicon_case_insensitively() {
        for text in ["true", "TRUE", "t", "yes", "Y", "1", "on", "enabled", "Active", "checked", "in"] {
            assert_eq!(cast_bool(text.as_bytes()), Ok(true), "{text}");
        }
        for text in ["false", "F", "no", "N", "0", "off", "Disabled", "inactive", "unchecked", "Out"] {
            assert_eq!(cast_bool(text.as_bytes()), Ok(false), "{text}");
        }
    }

    #[test]
    fn bool_trims_ascii_whitespace() {
        assert_eq!(cast_bool(b"\ttrue\n"), Ok(true));
        assert_eq!(cast_bool(b"  Y  "), Ok(true));
    }

    #[test]
    fn bool_rejects_unrecognized_and_empty() {
        assert_eq!(reason(cast_bool(b"maybe")), Reason::Malformed);
        assert_eq!(reason(cast_bool(b"truee")), Reason::Malformed);
        assert_eq!(reason(cast_bool(b"2")), Reason::Malformed);
        assert_eq!(reason(cast_bool(b"")), Reason::Empty);
        assert_eq!(reason(cast_bool(b" \t\r\n ")), Reason::Empty);
    }

    #[test]
    fn bool_fault_spans_the_trimmed_token() {
        let fault = cast_bool(b"  maybe  ").unwrap_err();
        assert_eq!((fault.offset, fault.len), (2, 5));
    }

    // --- integers ---

    #[test]
    fn int_parses_the_svartalfheim_notation_set() {
        assert_eq!(cast_i32(b"42", &INVARIANT), Ok(42));
        assert_eq!(cast_i32(b"  7  ", &INVARIANT), Ok(7));
        assert_eq!(cast_i32(b"+13", &INVARIANT), Ok(13));
        assert_eq!(cast_i32(b"-13", &INVARIANT), Ok(-13));
        assert_eq!(cast_i32(b"1,234", &INVARIANT), Ok(1234));
        assert_eq!(cast_i32(b"(1,234)", &INVARIANT), Ok(-1234));
        assert_eq!(cast_i32(b"1e3", &INVARIANT), Ok(1000));
        assert_eq!(cast_i32(b"0x2A", &INVARIANT), Ok(42));
        assert_eq!(cast_i32(b"&H2A", &INVARIANT), Ok(42));
        assert_eq!(cast_i32(b"0b1010", &INVARIANT), Ok(10));
    }

    #[test]
    fn int_honors_declared_eurozone_grouping() {
        let eurozone = NumFormat { decimal_sep: ',', group_sep: '.', flags: NumFormat::ALL };
        assert_eq!(cast_i32(b"1.234", &eurozone), Ok(1234));
        assert_eq!(reason(cast_i32(b"1,5", &eurozone)), Reason::Malformed);
    }

    #[test]
    fn int_reads_hex_as_the_twos_complement_bit_pattern() {
        assert_eq!(cast_i8(b"0xFF", &INVARIANT), Ok(-1));
        assert_eq!(cast_i8(b"0x7F", &INVARIANT), Ok(127));
        assert_eq!(cast_u8(b"0xFF", &INVARIANT), Ok(255));
        assert_eq!(reason(cast_i8(b"0x1FF", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(cast_i64(b"0xFFFFFFFFFFFFFFFF", &INVARIANT), Ok(-1));
    }

    #[test]
    fn int_rejects_decimal_points_in_any_disguise() {
        assert_eq!(reason(cast_i32(b"12.5", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"1.5e0", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"1e-2", &INVARIANT)), Reason::Malformed);
    }

    #[test]
    fn int_rejects_signed_radix_and_garbage() {
        assert_eq!(reason(cast_i32(b"-0x1F", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"abc", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"0x", &INVARIANT)), Reason::Malformed);
    }

    #[test]
    fn int_range_is_the_types_own() {
        assert_eq!(cast_u8(b"0", &INVARIANT), Ok(0));
        assert_eq!(cast_u8(b"255", &INVARIANT), Ok(255));
        assert_eq!(reason(cast_u8(b"256", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(reason(cast_u8(b"-1", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(cast_i8(b"127", &INVARIANT), Ok(i8::MAX));
        assert_eq!(cast_i8(b"-128", &INVARIANT), Ok(i8::MIN));
        assert_eq!(cast_i16(b"32767", &INVARIANT), Ok(i16::MAX));
        assert_eq!(cast_u16(b"65535", &INVARIANT), Ok(u16::MAX));
        assert_eq!(cast_u32(b"4294967295", &INVARIANT), Ok(u32::MAX));
        assert_eq!(cast_i64(b"9223372036854775807", &INVARIANT), Ok(i64::MAX));
        assert_eq!(cast_i64(b"-9223372036854775808", &INVARIANT), Ok(i64::MIN));
        assert_eq!(cast_u64(b"18446744073709551615", &INVARIANT), Ok(u64::MAX));
        assert_eq!(reason(cast_i64(b"99999999999999999999999", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(reason(cast_i32(b"5e40", &INVARIANT)), Reason::OutOfRange);
    }

    #[test]
    fn int_group_separator_must_sit_between_digits() {
        assert_eq!(reason(cast_i32(b",1", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"1,", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"1,,2", &INVARIANT)), Reason::Malformed);
        // Group sizes are not validated — the separator just has to sit between digits.
        assert_eq!(cast_i32(b"12,34", &INVARIANT), Ok(1234));
    }

    #[test]
    fn int_fault_points_at_the_offending_byte() {
        let fault = cast_i32(b"  12x4", &INVARIANT).unwrap_err();
        assert_eq!((fault.reason, fault.offset, fault.len), (Reason::Malformed, 4, 1));
        let fault = cast_i32("12\u{00A0}4".as_bytes(), &INVARIANT).unwrap_err();
        assert_eq!((fault.reason, fault.offset, fault.len), (Reason::Malformed, 2, 2));
    }

    #[test]
    fn int_flags_gate_each_lenience() {
        let strict = NumFormat { flags: 0, ..NumFormat::INVARIANT };
        assert_eq!(cast_i32(b"1234", &strict), Ok(1234));
        assert_eq!(reason(cast_i32(b"1,234", &strict)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"(12)", &strict)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"1e3", &strict)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"0x2A", &strict)), Reason::Malformed);
    }

    // --- reals ---

    #[test]
    fn real_parses_the_svartalfheim_notation_set() {
        assert_eq!(cast_f64(b"1.5", &INVARIANT), Ok(1.5));
        assert_eq!(cast_f64(b"  2.25  ", &INVARIANT), Ok(2.25));
        assert_eq!(cast_f64(b"-3.5", &INVARIANT), Ok(-3.5));
        assert_eq!(cast_f64(b"1,234.5", &INVARIANT), Ok(1234.5));
        assert_eq!(cast_f64(b"(2.5)", &INVARIANT), Ok(-2.5));
        assert_eq!(cast_f64(b"2.5e3", &INVARIANT), Ok(2500.0));
        assert_eq!(cast_f64(b"2.5e-3", &INVARIANT), Ok(0.0025));
        assert_eq!(cast_f64(b"50%", &INVARIANT), Ok(0.5));
        assert_eq!(cast_f64(b"25.5%", &INVARIANT), Ok(0.255));
        assert_eq!(cast_f32(b"1.5", &INVARIANT), Ok(1.5f32));
        assert_eq!(cast_f64(b".5", &INVARIANT), Ok(0.5));
    }

    #[test]
    fn real_honors_declared_eurozone_separators() {
        let eurozone = NumFormat { decimal_sep: ',', group_sep: '.', flags: NumFormat::ALL };
        assert_eq!(cast_f64(b"1.234,5", &eurozone), Ok(1234.5));
        let french = NumFormat { decimal_sep: ',', group_sep: '\u{00A0}', flags: NumFormat::ALL };
        assert_eq!(cast_f64("1\u{00A0}234,5".as_bytes(), &french), Ok(1234.5));
    }

    #[test]
    fn real_admits_only_finite_values() {
        // The literals Rust's own parser would accept are Malformed here by construction...
        assert_eq!(reason(cast_f64(b"NaN", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"Infinity", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"-inf", &INVARIANT)), Reason::Malformed);
        // ...and a well-formed magnitude that overflows the type is OutOfRange.
        assert_eq!(reason(cast_f64(b"1e400", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(reason(cast_f32(b"1e39", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(cast_f64(b"1e39", &INVARIANT), Ok(1e39));
    }

    #[test]
    fn real_rejects_malformed_shapes() {
        assert_eq!(reason(cast_f64(b"abc", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"1.2.3", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"%", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"1,234.5,6", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"", &INVARIANT)), Reason::Empty);
    }

    // --- uuid ---

    const KNOWN: [u8; 16] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f, 0x10,
    ];

    #[test]
    fn uuid_accepts_every_dotnet_format_and_prefix() {
        for text in [
            "01020304-0506-0708-090a-0b0c0d0e0f10",
            "0102030405060708090a0b0c0d0e0f10",
            "{01020304-0506-0708-090a-0b0c0d0e0f10}",
            "(01020304-0506-0708-090a-0b0c0d0e0f10)",
            "  01020304-0506-0708-090a-0b0c0d0e0f10  ",
            "urn:uuid:01020304-0506-0708-090a-0b0c0d0e0f10",
            "GUID:01020304-0506-0708-090a-0b0c0d0e0f10",
            "uuid:01020304-0506-0708-090a-0b0c0d0e0f10",
            "01020304-0506-0708-090A-0B0C0D0E0F10",
            "{0x01020304,0x0506,0x0708,{0x09,0x0a,0x0b,0x0c,0x0d,0x0e,0x0f,0x10}}",
        ] {
            assert_eq!(cast_uuid(text.as_bytes()), Ok(KNOWN), "{text}");
        }
    }

    #[test]
    fn uuid_rejects_malformed_shapes() {
        for text in [
            "not-a-guid",
            "GUID:not-a-guid",
            "01020304-0506-0708-090a-0b0c0d0e0f10-extra",
            "01020304-0506-0708-090a-0b0c0d0e0f1",
            "{01020304-0506-0708-090a-0b0c0d0e0f10)",
            "GUID:",
        ] {
            assert_eq!(reason(cast_uuid(text.as_bytes())), Reason::Malformed, "{text}");
        }
        assert_eq!(reason(cast_uuid(b"   ")), Reason::Empty);
    }

    // --- timestamp ---

    #[test]
    fn timestamp_parses_rfc3339_and_normalizes_to_utc() {
        // RFC 9562's own test-vector instant: 2022-02-22T19:22:22Z = 1645557742.
        assert_eq!(
            cast_timestamp(b"2022-02-22T19:22:22Z"),
            Ok(Timestamp { seconds: 1_645_557_742, nanos: 0 })
        );
        assert_eq!(
            cast_timestamp(b"2026-01-02T15:04:05.123Z"),
            Ok(Timestamp { seconds: 1_767_366_245, nanos: 123_000_000 })
        );
        // 15:04:05+05:00 is 10:04:05Z.
        assert_eq!(
            cast_timestamp(b"2026-01-02T15:04:05+05:00").unwrap().seconds,
            cast_timestamp(b"2026-01-02T10:04:05Z").unwrap().seconds
        );
        assert_eq!(cast_timestamp(b"1970-01-01T00:00:00Z"), Ok(Timestamp { seconds: 0, nanos: 0 }));
        assert_eq!(
            cast_timestamp(b"2026-01-02t15:04:05z"),
            cast_timestamp(b"2026-01-02T15:04:05Z")
        );
        // -00:00 is RFC 3339's "offset unknown", still a UTC instant.
        assert_eq!(
            cast_timestamp(b"2026-01-02T15:04:05-00:00"),
            cast_timestamp(b"2026-01-02T15:04:05Z")
        );
    }

    #[test]
    fn timestamp_window_boundaries_are_legal_values() {
        assert_eq!(
            cast_timestamp(b"0001-01-01T00:00:00Z"),
            Ok(Timestamp { seconds: MIN_TIMESTAMP_SECONDS, nanos: 0 })
        );
        assert_eq!(
            cast_timestamp(b"9999-12-31T23:59:59.999999999Z"),
            Ok(Timestamp { seconds: MAX_TIMESTAMP_SECONDS, nanos: 999_999_999 })
        );
    }

    #[test]
    fn timestamp_faults_outside_the_window_as_out_of_range() {
        assert_eq!(reason(cast_timestamp(b"0000-01-01T00:00:00Z")), Reason::OutOfRange);
        // A legal wall time whose offset pushes it past an edge.
        assert_eq!(reason(cast_timestamp(b"0001-01-01T00:00:00+01:00")), Reason::OutOfRange);
        assert_eq!(reason(cast_timestamp(b"9999-12-31T23:59:59-01:00")), Reason::OutOfRange);
    }

    #[test]
    fn timestamp_demands_a_zone_and_the_t_separator() {
        assert_eq!(reason(cast_timestamp(b"2026-01-02T15:04:05")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"2026-01-02 15:04:05Z")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"1/2/2026 3:04 PM")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"1700000000")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"2026-01-02T15:04Z")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"2026-01-02T15:04:05+0500")), Reason::Malformed);
    }

    #[test]
    fn timestamp_rejects_impossible_calendar_and_clock_readings() {
        assert_eq!(reason(cast_timestamp(b"2026-02-29T00:00:00Z")), Reason::Malformed);
        assert!(cast_timestamp(b"2024-02-29T00:00:00Z").is_ok());
        assert_eq!(reason(cast_timestamp(b"2026-13-01T00:00:00Z")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"2026-01-02T24:00:00Z")), Reason::Malformed);
        // Leap seconds have no protobuf representation; a deterministic core doesn't smear.
        assert_eq!(reason(cast_timestamp(b"2016-12-31T23:59:60Z")), Reason::Malformed);
    }

    #[test]
    fn timestamp_fraction_carries_nanosecond_fidelity() {
        assert_eq!(cast_timestamp(b"1970-01-01T00:00:00.000000001Z").unwrap().nanos, 1);
        assert_eq!(reason(cast_timestamp(b"1970-01-01T00:00:00.0000000001Z")), Reason::Malformed);
        // Sub-second nanos count forward even before the epoch (protobuf convention).
        assert_eq!(
            cast_timestamp(b"1969-12-31T23:59:59.5Z"),
            Ok(Timestamp { seconds: -1, nanos: 500_000_000 })
        );
    }

    // --- unix ---

    #[test]
    fn unix_reads_the_declared_precision_and_never_guesses() {
        assert_eq!(
            cast_unix(b"1700000000", UnixPrecision::Seconds),
            Ok(Timestamp { seconds: 1_700_000_000, nanos: 0 })
        );
        assert_eq!(
            cast_unix(b"1700000000000", UnixPrecision::Millis),
            Ok(Timestamp { seconds: 1_700_000_000, nanos: 0 })
        );
        assert_eq!(
            cast_unix(b"1700000000123456", UnixPrecision::Micros),
            Ok(Timestamp { seconds: 1_700_000_000, nanos: 123_456_000 })
        );
        assert_eq!(
            cast_unix(b"1700000000123456789", UnixPrecision::Nanos),
            Ok(Timestamp { seconds: 1_700_000_000, nanos: 123_456_789 })
        );
    }

    #[test]
    fn unix_handles_pre_epoch_values_with_forward_counting_nanos() {
        assert_eq!(
            cast_unix(b"-1", UnixPrecision::Seconds),
            Ok(Timestamp { seconds: -1, nanos: 0 })
        );
        assert_eq!(
            cast_unix(b"-1", UnixPrecision::Millis),
            Ok(Timestamp { seconds: -1, nanos: 999_000_000 })
        );
    }

    #[test]
    fn unix_rejects_non_integers_and_the_out_of_window() {
        assert_eq!(reason(cast_unix(b"1700000000.5", UnixPrecision::Seconds)), Reason::Malformed);
        assert_eq!(reason(cast_unix(b"not-a-number", UnixPrecision::Seconds)), Reason::Malformed);
        assert_eq!(reason(cast_unix(b"253402300800", UnixPrecision::Seconds)), Reason::OutOfRange);
        assert_eq!(reason(cast_unix(b"-62135596801", UnixPrecision::Seconds)), Reason::OutOfRange);
        assert_eq!(cast_unix(b"253402300799", UnixPrecision::Seconds).unwrap().seconds, MAX_TIMESTAMP_SECONDS);
    }

    // --- date ---

    #[test]
    fn date_accepts_exactly_the_iso_profile() {
        assert_eq!(cast_date(b"2026-01-02"), Ok(Date { year: 2026, month: 1, day: 2 }));
        assert_eq!(cast_date(b"  2026-01-02  "), Ok(Date { year: 2026, month: 1, day: 2 }));
        assert_eq!(cast_date(b"2024-02-29"), Ok(Date { year: 2024, month: 2, day: 29 }));
        assert_eq!(cast_date(b"0001-01-01"), Ok(Date { year: 1, month: 1, day: 1 }));
        assert_eq!(cast_date(b"9999-12-31"), Ok(Date { year: 9999, month: 12, day: 31 }));
    }

    #[test]
    fn date_rejects_everything_else() {
        for text in ["1/2/2026", "2026-01-02T00:00:00", "2026/01/02", "garbage", "2026-02-29", "2026-00-01", "2026-01-00"] {
            assert_eq!(reason(cast_date(text.as_bytes())), Reason::Malformed, "{text}");
        }
        assert_eq!(reason(cast_date(b"0000-01-01")), Reason::OutOfRange);
        assert_eq!(reason(cast_date(b"")), Reason::Empty);
    }

    // --- time ---

    #[test]
    fn time_reads_the_24_hour_profile_to_nanos_since_midnight() {
        assert_eq!(cast_time(b"15:04:05"), Ok(54_245_000_000_000));
        assert_eq!(cast_time(b"15:04:05.123"), Ok(54_245_123_000_000));
        assert_eq!(cast_time(b"15:04"), Ok(54_240_000_000_000));
        assert_eq!(cast_time(b"00:00:00"), Ok(0));
        assert_eq!(cast_time(b"23:59:59.999999999"), Ok(86_399_999_999_999));
    }

    #[test]
    fn time_rejects_non_iso_readings() {
        for text in ["3:04:05 PM", "25:00", "noon", "15:60", "15:04:60", "15:04:05.0000000001"] {
            assert_eq!(reason(cast_time(text.as_bytes())), Reason::Malformed, "{text}");
        }
        assert_eq!(reason(cast_time(b"")), Reason::Empty);
    }

    // --- duration ---

    #[test]
    fn duration_parses_all_three_shapes_to_the_same_span() {
        let ninety_minutes = Duration { seconds: 5_400, nanos: 0 };
        assert_eq!(cast_duration(b"01:30:00"), Ok(ninety_minutes));
        assert_eq!(cast_duration(b"PT1H30M"), Ok(ninety_minutes));
        assert_eq!(cast_duration(b"5400s"), Ok(ninety_minutes));
        assert_eq!(cast_duration(b"1.06:00:00"), Ok(Duration { seconds: 108_000, nanos: 0 }));
        assert_eq!(cast_duration(b"P1DT6H"), Ok(Duration { seconds: 108_000, nanos: 0 }));
        assert_eq!(cast_duration(b"P2W"), Ok(Duration { seconds: 1_209_600, nanos: 0 }));
        assert_eq!(cast_duration(b"01:30"), Ok(ninety_minutes));
    }

    #[test]
    fn duration_carries_same_signed_fractions() {
        assert_eq!(cast_duration(b"PT1.5S"), Ok(Duration { seconds: 1, nanos: 500_000_000 }));
        assert_eq!(cast_duration(b"-PT1H"), Ok(Duration { seconds: -3_600, nanos: 0 }));
        assert_eq!(cast_duration(b"-1.5s"), Ok(Duration { seconds: -1, nanos: -500_000_000 }));
        assert_eq!(cast_duration(b"3.000000001s"), Ok(Duration { seconds: 3, nanos: 1 }));
        assert_eq!(cast_duration(b"-00:00:00.5"), Ok(Duration { seconds: 0, nanos: -500_000_000 }));
        assert_eq!(cast_duration(b"00:00:00"), Ok(Duration { seconds: 0, nanos: 0 }));
    }

    #[test]
    fn duration_rejects_calendar_units_and_shapeless_input() {
        for text in ["P1Y", "P2M", "P", "PT", "P3DT", "PT1H30", "90m", "garbage", "PT1.5H"] {
            assert_eq!(reason(cast_duration(text.as_bytes())), Reason::Malformed, "{text}");
        }
        assert_eq!(reason(cast_duration(b"")), Reason::Empty);
    }

    #[test]
    fn duration_bounds_the_protobuf_window() {
        assert_eq!(
            cast_duration(b"315576000000s"),
            Ok(Duration { seconds: MAX_DURATION_SECONDS, nanos: 0 })
        );
        assert_eq!(reason(cast_duration(b"315576000001s")), Reason::OutOfRange);
        assert_eq!(reason(cast_duration(b"-315576000001s")), Reason::OutOfRange);
        assert_eq!(reason(cast_duration(b"PT999999999999999999S")), Reason::OutOfRange);
        assert_eq!(reason(cast_duration(b"P9999999999999999W")), Reason::OutOfRange);
        // A digit run past the parse-sanity bound is shape noise, not a big number.
        assert_eq!(reason(cast_duration(b"PT9999999999999999999S")), Reason::Malformed);
    }

    #[test]
    fn duration_colon_hours_cap_at_23_without_a_day_part() {
        assert_eq!(reason(cast_duration(b"25:00:00")), Reason::Malformed);
        assert_eq!(cast_duration(b"1.01:00:00"), Ok(Duration { seconds: 90_000, nanos: 0 }));
    }

    // --- first-class Rust citizenship: the same end-user surface the bindings present ---

    #[test]
    fn doors_accept_anything_byte_viewable() {
        assert_eq!(cast_bool("yes"), Ok(true));
        assert_eq!(cast_bool(String::from("off")), Ok(false));
        assert_eq!(cast_i32("(1,234)", &INVARIANT), Ok(-1234));
        let owned: Vec<u8> = String::from("6ba7b810-9dad-11d1-80b4-00c04fd430c8").into_bytes();
        assert!(cast_uuid(owned).is_ok());
        assert_eq!(cast_time("23:59:59"), Ok(86_399_000_000_000));
    }

    #[test]
    fn faults_display_the_reason_and_span() {
        assert_eq!(cast_bool("   ").unwrap_err().to_string(), "empty input");
        assert_eq!(cast_bool("maybe").unwrap_err().to_string(), "malformed input at bytes 0..5");
        assert_eq!(cast_u8("256", &INVARIANT).unwrap_err().to_string(), "out of range input at bytes 0..3");
    }

    #[test]
    fn faults_compose_with_ordinary_error_propagation() {
        fn parse_pair(a: &str, b: &str) -> Result<(bool, i32), Box<dyn core::error::Error>> {
            Ok((cast_bool(a)?, cast_i32(b, &INVARIANT)?))
        }
        assert_eq!(parse_pair("on", "42").unwrap(), (true, 42));
        // The integer door's span points precisely at the offending byte, not the token.
        let err = parse_pair("on", "4x2").unwrap_err();
        assert_eq!(err.to_string(), "malformed input at bytes 1..2");
        assert_eq!(err.downcast_ref::<Fault>().unwrap().reason, Reason::Malformed);
    }

    #[test]
    fn optional_presents_empty_as_absent_and_nothing_else() {
        assert_eq!(optional(cast_bool("  ")), Ok(None));
        assert_eq!(optional(cast_bool("true")), Ok(Some(true)));
        assert_eq!(optional(cast_bool("maybe")).unwrap_err().reason, Reason::Malformed);
    }
}
