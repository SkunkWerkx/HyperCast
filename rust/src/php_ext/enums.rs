//! The int-backed enums, with 1.0's names, cases and values (the core's ABI codes).

use ext_php_rs::prelude::*;

/// Why a cast failed.
///
/// Registered with ext-php-rs's enum builder like the others, but its traits are written out
/// here rather than derived by `#[php_enum]`, whose name check refuses a case called `Empty`
/// (a keyword as a function name, though PHP accepts it as a case name, and 1.0 declared it).
#[derive(Clone, Copy)]
pub enum CastFailure {
    Empty = 1,
    Malformed = 2,
    OutOfRange = 3,
}

impl ext_php_rs::class::RegisteredClass for CastFailure {
    const CLASS_NAME: &'static str = "HyperCast\\CastFailure";
    const BUILDER_MODIFIER: Option<
        fn(ext_php_rs::builders::ClassBuilder) -> ext_php_rs::builders::ClassBuilder,
    > = None;
    const EXTENDS: Option<ext_php_rs::class::ClassEntryInfo> = None;
    const IMPLEMENTS: &'static [ext_php_rs::class::ClassEntryInfo] = &[];
    const FLAGS: ext_php_rs::flags::ClassFlags = ext_php_rs::flags::ClassFlags::Enum;
    const DOC_COMMENTS: &'static [&'static str] = &[" Why a cast failed."];

    fn get_metadata() -> &'static ext_php_rs::class::ClassMetadata<Self> {
        static METADATA: ext_php_rs::class::ClassMetadata<CastFailure> =
            ext_php_rs::class::ClassMetadata::new(&[]);
        &METADATA
    }

    fn method_builders()
    -> Vec<(ext_php_rs::builders::FunctionBuilder<'static>, ext_php_rs::flags::MethodFlags)> {
        use ext_php_rs::internal::class::PhpClassImpl;
        ext_php_rs::internal::class::PhpClassImplCollector::<Self>::default().get_methods()
    }

    fn constructor() -> Option<ext_php_rs::class::ConstructorMeta<Self>> {
        None
    }

    fn constants() -> &'static [(
        &'static str,
        &'static dyn ext_php_rs::convert::IntoZvalDyn,
        &'static [&'static str],
        ext_php_rs::flags::ConstantFlags,
    )] {
        use ext_php_rs::internal::class::PhpClassImpl;
        ext_php_rs::internal::class::PhpClassImplCollector::<Self>::default().get_constants()
    }
}

impl ext_php_rs::enum_::RegisteredEnum for CastFailure {
    const CASES: &'static [ext_php_rs::enum_::EnumCase] = &[
        ext_php_rs::enum_::EnumCase {
            name: "Empty",
            discriminant: Some(ext_php_rs::enum_::Discriminant::Int(1)),
            docs: &[],
        },
        ext_php_rs::enum_::EnumCase {
            name: "Malformed",
            discriminant: Some(ext_php_rs::enum_::Discriminant::Int(2)),
            docs: &[],
        },
        ext_php_rs::enum_::EnumCase {
            name: "OutOfRange",
            discriminant: Some(ext_php_rs::enum_::Discriminant::Int(3)),
            docs: &[],
        },
    ];

    fn from_name(name: &str) -> ext_php_rs::error::Result<Self> {
        match name {
            "Empty" => Ok(Self::Empty),
            "Malformed" => Ok(Self::Malformed),
            "OutOfRange" => Ok(Self::OutOfRange),
            _ => Err(ext_php_rs::error::Error::InvalidEnumCase { case: name.to_string() }),
        }
    }

    fn to_name(&self) -> &'static str {
        match self {
            Self::Empty => "Empty",
            Self::Malformed => "Malformed",
            Self::OutOfRange => "OutOfRange",
        }
    }
}

impl From<crate::Reason> for CastFailure {
    fn from(reason: crate::Reason) -> Self {
        match reason {
            crate::Reason::Empty => CastFailure::Empty,
            crate::Reason::Malformed => CastFailure::Malformed,
            crate::Reason::OutOfRange => CastFailure::OutOfRange,
        }
    }
}

/// The order of a date's numeric fields.
#[php_enum]
#[php(name = "HyperCast\\DateOrder")]
#[derive(Clone, Copy)]
pub enum DateOrder {
    #[php(value = 1)]
    Ymd,
    #[php(value = 2)]
    Mdy,
    #[php(value = 3)]
    Dmy,
}

impl From<DateOrder> for crate::DateOrder {
    fn from(order: DateOrder) -> Self {
        match order {
            DateOrder::Ymd => crate::DateOrder::YearMonthDay,
            DateOrder::Mdy => crate::DateOrder::MonthDayYear,
            DateOrder::Dmy => crate::DateOrder::DayMonthYear,
        }
    }
}

/// Excel's two date systems.
#[php_enum]
#[php(name = "HyperCast\\ExcelEpoch")]
#[derive(Clone, Copy)]
pub enum ExcelEpoch {
    #[php(value = 1)]
    Y1900,
    #[php(value = 2)]
    Y1904,
}

impl From<ExcelEpoch> for crate::ExcelEpoch {
    fn from(epoch: ExcelEpoch) -> Self {
        match epoch {
            ExcelEpoch::Y1900 => crate::ExcelEpoch::Y1900,
            ExcelEpoch::Y1904 => crate::ExcelEpoch::Y1904,
        }
    }
}

/// The unit of a Unix timestamp.
#[php_enum]
#[php(name = "HyperCast\\UnixPrecision")]
#[derive(Clone, Copy)]
pub enum UnixPrecision {
    #[php(value = 1)]
    Seconds,
    #[php(value = 2)]
    Milliseconds,
    #[php(value = 3)]
    Microseconds,
    #[php(value = 4)]
    Nanoseconds,
}

impl From<UnixPrecision> for crate::UnixPrecision {
    fn from(precision: UnixPrecision) -> Self {
        match precision {
            UnixPrecision::Seconds => crate::UnixPrecision::Seconds,
            UnixPrecision::Milliseconds => crate::UnixPrecision::Millis,
            UnixPrecision::Microseconds => crate::UnixPrecision::Micros,
            UnixPrecision::Nanoseconds => crate::UnixPrecision::Nanos,
        }
    }
}

/// The door a `Constraints` declaration is bound to: a numeric door or the text door, with
/// the core's ABI codes.
#[php_enum]
#[php(name = "HyperCast\\Door")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Door {
    #[php(value = 2)]
    I8,
    #[php(value = 3)]
    I16,
    #[php(value = 4)]
    I32,
    #[php(value = 5)]
    I64,
    #[php(value = 6)]
    U8,
    #[php(value = 7)]
    U16,
    #[php(value = 8)]
    U32,
    #[php(value = 9)]
    U64,
    #[php(value = 10)]
    F32,
    #[php(value = 11)]
    F64,
    #[php(value = 19)]
    Decimal,
    #[php(value = 18)]
    Text,
}

impl From<Door> for crate::Door {
    fn from(door: Door) -> Self {
        match door {
            Door::I8 => crate::Door::I8,
            Door::I16 => crate::Door::I16,
            Door::I32 => crate::Door::I32,
            Door::I64 => crate::Door::I64,
            Door::U8 => crate::Door::U8,
            Door::U16 => crate::Door::U16,
            Door::U32 => crate::Door::U32,
            Door::U64 => crate::Door::U64,
            Door::F32 => crate::Door::F32,
            Door::F64 => crate::Door::F64,
            Door::Decimal => crate::Door::Decimal,
            Door::Text => crate::Door::Text,
        }
    }
}

impl Door {
    /// The case's name, as `HyperCast\Door::Text` spells it.
    pub(crate) fn case_name(self) -> &'static str {
        match self {
            Door::I8 => "I8",
            Door::I16 => "I16",
            Door::I32 => "I32",
            Door::I64 => "I64",
            Door::U8 => "U8",
            Door::U16 => "U16",
            Door::U32 => "U32",
            Door::U64 => "U64",
            Door::F32 => "F32",
            Door::F64 => "F64",
            Door::Decimal => "Decimal",
            Door::Text => "Text",
        }
    }
}

/// What a value that is not a multiple of `multipleOf` becomes: refused (`Strict`), or
/// rounded to the nearest multiple (`Midpoint`, ties to even), down (`Floor`) or up
/// (`Ceiling`). The core's ABI codes.
#[php_enum]
#[php(name = "HyperCast\\Rounding")]
#[derive(Clone, Copy)]
pub enum Rounding {
    #[php(value = 0)]
    Strict,
    #[php(value = 1)]
    Midpoint,
    #[php(value = 2)]
    Floor,
    #[php(value = 3)]
    Ceiling,
}

impl Rounding {
    /// The core's rounding code.
    pub(crate) fn code(self) -> u32 {
        match self {
            Rounding::Strict => 0,
            Rounding::Midpoint => 1,
            Rounding::Floor => 2,
            Rounding::Ceiling => 3,
        }
    }
}

/// The rule a `Violation` broke, a case per keyword this binding names, with the core's ABI
/// codes. The OpenAPI spelling is the case's name with a lowercase first letter
/// (`ExclusiveMinimum` is `exclusiveMinimum`). More keywords (`format`, `pattern`) may join
/// in a minor release: a `Violation` keeps a code this binding does not name yet as the bare
/// `int`, so a `match` over keywords needs a `default` arm.
///
/// Registered with ext-php-rs's enum builder like the others, but its traits are written out
/// here rather than derived by `#[php_enum]`, whose name check refuses a case called `Enum`
/// (a keyword as a function name, though PHP accepts it as a case name), as for `CastFailure`.
#[derive(Clone, Copy)]
pub enum Keyword {
    Minimum = 1,
    ExclusiveMinimum = 2,
    Maximum = 3,
    ExclusiveMaximum = 4,
    MultipleOf = 5,
    MinLength = 6,
    MaxLength = 7,
    Enum = 8,
}

/// The keywords' cases, in code order, with their names.
const KEYWORD_CASES: [(Keyword, &str); 8] = [
    (Keyword::Minimum, "Minimum"),
    (Keyword::ExclusiveMinimum, "ExclusiveMinimum"),
    (Keyword::Maximum, "Maximum"),
    (Keyword::ExclusiveMaximum, "ExclusiveMaximum"),
    (Keyword::MultipleOf, "MultipleOf"),
    (Keyword::MinLength, "MinLength"),
    (Keyword::MaxLength, "MaxLength"),
    (Keyword::Enum, "Enum"),
];

impl ext_php_rs::class::RegisteredClass for Keyword {
    const CLASS_NAME: &'static str = "HyperCast\\Keyword";
    const BUILDER_MODIFIER: Option<
        fn(ext_php_rs::builders::ClassBuilder) -> ext_php_rs::builders::ClassBuilder,
    > = None;
    const EXTENDS: Option<ext_php_rs::class::ClassEntryInfo> = None;
    const IMPLEMENTS: &'static [ext_php_rs::class::ClassEntryInfo] = &[];
    const FLAGS: ext_php_rs::flags::ClassFlags = ext_php_rs::flags::ClassFlags::Enum;
    const DOC_COMMENTS: &'static [&'static str] = &[
        " The rule a `Violation` broke: a case per keyword this binding names, its value the",
        " core's code. A `Violation` keeps a code this binding does not name yet as the bare",
        " `int`, so a `match` over keywords needs a `default` arm.",
    ];

    fn get_metadata() -> &'static ext_php_rs::class::ClassMetadata<Self> {
        static METADATA: ext_php_rs::class::ClassMetadata<Keyword> =
            ext_php_rs::class::ClassMetadata::new(&[]);
        &METADATA
    }

    fn method_builders()
    -> Vec<(ext_php_rs::builders::FunctionBuilder<'static>, ext_php_rs::flags::MethodFlags)> {
        use ext_php_rs::internal::class::PhpClassImpl;
        ext_php_rs::internal::class::PhpClassImplCollector::<Self>::default().get_methods()
    }

    fn constructor() -> Option<ext_php_rs::class::ConstructorMeta<Self>> {
        None
    }

    fn constants() -> &'static [(
        &'static str,
        &'static dyn ext_php_rs::convert::IntoZvalDyn,
        &'static [&'static str],
        ext_php_rs::flags::ConstantFlags,
    )] {
        use ext_php_rs::internal::class::PhpClassImpl;
        ext_php_rs::internal::class::PhpClassImplCollector::<Self>::default().get_constants()
    }
}

impl ext_php_rs::enum_::RegisteredEnum for Keyword {
    const CASES: &'static [ext_php_rs::enum_::EnumCase] = &[
        ext_php_rs::enum_::EnumCase {
            name: "Minimum",
            discriminant: Some(ext_php_rs::enum_::Discriminant::Int(1)),
            docs: &[],
        },
        ext_php_rs::enum_::EnumCase {
            name: "ExclusiveMinimum",
            discriminant: Some(ext_php_rs::enum_::Discriminant::Int(2)),
            docs: &[],
        },
        ext_php_rs::enum_::EnumCase {
            name: "Maximum",
            discriminant: Some(ext_php_rs::enum_::Discriminant::Int(3)),
            docs: &[],
        },
        ext_php_rs::enum_::EnumCase {
            name: "ExclusiveMaximum",
            discriminant: Some(ext_php_rs::enum_::Discriminant::Int(4)),
            docs: &[],
        },
        ext_php_rs::enum_::EnumCase {
            name: "MultipleOf",
            discriminant: Some(ext_php_rs::enum_::Discriminant::Int(5)),
            docs: &[],
        },
        ext_php_rs::enum_::EnumCase {
            name: "MinLength",
            discriminant: Some(ext_php_rs::enum_::Discriminant::Int(6)),
            docs: &[],
        },
        ext_php_rs::enum_::EnumCase {
            name: "MaxLength",
            discriminant: Some(ext_php_rs::enum_::Discriminant::Int(7)),
            docs: &[],
        },
        ext_php_rs::enum_::EnumCase {
            name: "Enum",
            discriminant: Some(ext_php_rs::enum_::Discriminant::Int(8)),
            docs: &[],
        },
    ];

    fn from_name(name: &str) -> ext_php_rs::error::Result<Self> {
        KEYWORD_CASES
            .iter()
            .find(|(_, case)| *case == name)
            .map(|(keyword, _)| *keyword)
            .ok_or_else(|| ext_php_rs::error::Error::InvalidEnumCase { case: name.to_string() })
    }

    fn to_name(&self) -> &'static str {
        KEYWORD_CASES[self.code() as usize - 1].1
    }
}

impl Keyword {
    /// The core's keyword code.
    pub(crate) fn code(self) -> u32 {
        self as u32
    }

    /// The case for a core keyword code, or `None` for one this binding does not name.
    pub(crate) fn from_code(code: u32) -> Option<Keyword> {
        KEYWORD_CASES.iter().map(|(keyword, _)| *keyword).find(|keyword| keyword.code() == code)
    }
}
