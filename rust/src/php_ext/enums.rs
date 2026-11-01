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
