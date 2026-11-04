//! Every HyperCast door that reads text, plus [`Door::Text`] for the bytes themselves,
//! numbered as the C ABI numbers them: the doors a constraint can be bound to, and the doors
//! the detailed exports dispatch to. Numbering the doors here lets any parser name one, not
//! only the crate that first needed it.

use crate::{DateOrder, ExcelEpoch, UnixPrecision};

/// One HyperCast door, plus [`Door::Text`] for the bytes themselves.
///
/// | Code | Door | Code | Door |
/// | --- | --- | --- | --- |
/// | 1 | [`Bool`](Door::Bool) | 13 | [`Timestamp`](Door::Timestamp) |
/// | 2–5 | [`I8`](Door::I8) … [`I64`](Door::I64) | 14 | [`Unix`](Door::Unix) |
/// | 6–9 | [`U8`](Door::U8) … [`U64`](Door::U64) | 15 | [`Date`](Door::Date) |
/// | 10 | [`F32`](Door::F32) | 16 | [`Time`](Door::Time) |
/// | 11 | [`F64`](Door::F64) | 17 | [`Duration`](Door::Duration) |
/// | 12 | [`Uuid`](Door::Uuid) | 18 | [`Text`](Door::Text) |
/// | 19 | [`Decimal`](Door::Decimal) | 20 | [`DateOrdered`](Door::DateOrdered) |
/// | 21 | [`DateTime`](Door::DateTime) | 22 | [`ExcelSerial`](Door::ExcelSerial) |
/// | 23 | [`Char`](Door::Char) | | |
///
/// New doors may be added in a minor release. Matches outside this crate need a
/// wildcard arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Door {
    /// A natural-language boolean.
    Bool,
    /// A signed 8-bit integer.
    I8,
    /// A signed 16-bit integer.
    I16,
    /// A signed 32-bit integer.
    I32,
    /// A signed 64-bit integer.
    I64,
    /// An unsigned 8-bit integer.
    U8,
    /// An unsigned 16-bit integer.
    U16,
    /// An unsigned 32-bit integer.
    U32,
    /// An unsigned 64-bit integer.
    U64,
    /// A finite 32-bit real.
    F32,
    /// A finite 64-bit real.
    F64,
    /// A UUID in any text format the door reads.
    Uuid,
    /// An instant as a protobuf `{seconds, nanos}` pair.
    Timestamp,
    /// A Unix-epoch integer at the declared precision (never guessed from magnitude).
    Unix(UnixPrecision),
    /// A calendar date.
    Date,
    /// A time of day.
    Time,
    /// A duration.
    Duration,
    /// Text itself, uncast: [`cast_text`](crate::cast_text)'s valid UTF-8, untrimmed, which
    /// the text constraints (`minLength`, `maxLength`, a text `enum`) bind to.
    Text,
    /// An exact decimal: sign, 96-bit magnitude, base-10 scale. Never a float.
    Decimal,
    /// A separated calendar date under the declared field order.
    DateOrdered(DateOrder),
    /// A zone-less civil date-time under the declared field order.
    DateTime(DateOrder),
    /// An Excel date serial under the declared date system.
    ExcelSerial(ExcelEpoch),
    /// One Unicode scalar value, verbatim or as a declared code point
    /// ([`cast_char`](crate::cast_char)).
    Char,
}

impl Door {
    /// The ABI discriminant. `0` is never a door; a door that declares something — the
    /// Unix precision, a date order, an Excel date system — carries it beside the code,
    /// numbered as HyperCast's own exports number it.
    pub const fn code(self) -> u32 {
        match self {
            Door::Bool => 1,
            Door::I8 => 2,
            Door::I16 => 3,
            Door::I32 => 4,
            Door::I64 => 5,
            Door::U8 => 6,
            Door::U16 => 7,
            Door::U32 => 8,
            Door::U64 => 9,
            Door::F32 => 10,
            Door::F64 => 11,
            Door::Uuid => 12,
            Door::Timestamp => 13,
            Door::Unix(_) => 14,
            Door::Date => 15,
            Door::Time => 16,
            Door::Duration => 17,
            Door::Text => 18,
            Door::Decimal => 19,
            Door::DateOrdered(_) => 20,
            Door::DateTime(_) => 21,
            Door::ExcelSerial(_) => 22,
            Door::Char => 23,
        }
    }

    /// Decodes an ABI discriminant; `param` is read only by the doors that declare
    /// something: the Unix precision (`1` seconds … `4` nanoseconds), a date order
    /// (`1` year-month-day, `2` month-day-year, `3` day-month-year), or an Excel date
    /// system (`1` 1900, `2` 1904). Every other door ignores it here; the detailed exports
    /// also require it to be `0` there.
    pub const fn from_code(code: u32, param: u32) -> Option<Door> {
        Some(match code {
            1 => Door::Bool,
            2 => Door::I8,
            3 => Door::I16,
            4 => Door::I32,
            5 => Door::I64,
            6 => Door::U8,
            7 => Door::U16,
            8 => Door::U32,
            9 => Door::U64,
            10 => Door::F32,
            11 => Door::F64,
            12 => Door::Uuid,
            13 => Door::Timestamp,
            14 => match UnixPrecision::from_code(param) {
                Some(precision) => Door::Unix(precision),
                None => return None,
            },
            15 => Door::Date,
            16 => Door::Time,
            17 => Door::Duration,
            18 => Door::Text,
            19 => Door::Decimal,
            20 => match DateOrder::from_code(param) {
                Some(order) => Door::DateOrdered(order),
                None => return None,
            },
            21 => match DateOrder::from_code(param) {
                Some(order) => Door::DateTime(order),
                None => return None,
            },
            22 => match ExcelEpoch::from_code(param) {
                Some(epoch) => Door::ExcelSerial(epoch),
                None => return None,
            },
            23 => Door::Char,
            _ => return None,
        })
    }

    /// True for the doors whose ABI code carries a parameter: [`Unix`](Door::Unix),
    /// [`DateOrdered`](Door::DateOrdered), [`DateTime`](Door::DateTime) and
    /// [`ExcelSerial`](Door::ExcelSerial).
    // The detailed exports' parameter rule (ffi.rs), so unused in a build without `exports`.
    #[cfg_attr(not(feature = "exports"), allow(dead_code))]
    pub(crate) const fn declares(self) -> bool {
        matches!(
            self,
            Door::Unix(_) | Door::DateOrdered(_) | Door::DateTime(_) | Door::ExcelSerial(_)
        )
    }

    /// True for the doors that read a [`NumFormat`](crate::NumFormat) (integers, reals and decimals).
    pub const fn is_numeric(self) -> bool {
        matches!(
            self,
            Door::I8
                | Door::I16
                | Door::I32
                | Door::I64
                | Door::U8
                | Door::U16
                | Door::U32
                | Door::U64
                | Door::F32
                | Door::F64
                | Door::Decimal
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_door() -> [Door; 31] {
        [
            Door::Bool,
            Door::I8,
            Door::I16,
            Door::I32,
            Door::I64,
            Door::U8,
            Door::U16,
            Door::U32,
            Door::U64,
            Door::F32,
            Door::F64,
            Door::Uuid,
            Door::Timestamp,
            Door::Unix(UnixPrecision::Seconds),
            Door::Unix(UnixPrecision::Millis),
            Door::Unix(UnixPrecision::Micros),
            Door::Unix(UnixPrecision::Nanos),
            Door::Date,
            Door::Time,
            Door::Duration,
            Door::Text,
            Door::Decimal,
            Door::DateOrdered(DateOrder::YearMonthDay),
            Door::DateOrdered(DateOrder::MonthDayYear),
            Door::DateOrdered(DateOrder::DayMonthYear),
            Door::DateTime(DateOrder::YearMonthDay),
            Door::DateTime(DateOrder::MonthDayYear),
            Door::DateTime(DateOrder::DayMonthYear),
            Door::ExcelSerial(ExcelEpoch::Y1900),
            Door::ExcelSerial(ExcelEpoch::Y1904),
            Door::Char,
        ]
    }

    fn param(door: Door) -> u32 {
        match door {
            Door::Unix(p) => p as u32,
            Door::DateOrdered(o) | Door::DateTime(o) => o as u32,
            Door::ExcelSerial(e) => e as u32,
            _ => 0,
        }
    }

    #[test]
    fn codes_round_trip() {
        for door in every_door() {
            assert_eq!(Door::from_code(door.code(), param(door)), Some(door));
        }
    }

    #[test]
    fn codes_are_pinned() {
        let table = [
            (Door::Bool, 1),
            (Door::I8, 2),
            (Door::I16, 3),
            (Door::I32, 4),
            (Door::I64, 5),
            (Door::U8, 6),
            (Door::U16, 7),
            (Door::U32, 8),
            (Door::U64, 9),
            (Door::F32, 10),
            (Door::F64, 11),
            (Door::Uuid, 12),
            (Door::Timestamp, 13),
            (Door::Unix(UnixPrecision::Seconds), 14),
            (Door::Date, 15),
            (Door::Time, 16),
            (Door::Duration, 17),
            (Door::Text, 18),
            (Door::Decimal, 19),
            (Door::DateOrdered(DateOrder::YearMonthDay), 20),
            (Door::DateTime(DateOrder::YearMonthDay), 21),
            (Door::ExcelSerial(ExcelEpoch::Y1900), 22),
            (Door::Char, 23),
        ];
        for (door, code) in table {
            assert_eq!(door.code(), code, "{door:?}");
        }
    }

    #[test]
    fn unknown_codes_refused() {
        assert_eq!(Door::from_code(0, 0), None);
        assert_eq!(Door::from_code(24, 0), None);
        assert_eq!(Door::from_code(14, 5), None);
        assert_eq!(Door::from_code(20, 0), None);
        assert_eq!(Door::from_code(22, 3), None);
    }

    #[test]
    fn numeric_doors() {
        for code in 0..=24 {
            let door = Door::from_code(code, 1);
            let numeric = door.is_some_and(Door::is_numeric);
            assert_eq!(numeric, (2..=11).contains(&code) || code == 19, "code {code}");
        }
    }
}
