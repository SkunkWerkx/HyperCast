import Foundation

// The numeric doors and the two generic ones, each also taking a `Locale` where it takes a
// ``NumFormat``, and the date-time door taking one where it takes a ``DateFormat`` — the type
// Foundation's own parsing is configured with (`FormatStyle.locale(_:)`,
// `NumberFormatter.locale`). The locale is mapped through ``NumFormat/from(locale:)`` (its
// symbols, every lenience on) or ``DateFormat/from(locale:)`` (its short-date order and day
// periods) and the door runs exactly as with that format.
//
// Non-optional, as Foundation's are: a caller wanting the user's settings says so with
// `.current` or `.autoupdatingCurrent`; nothing here falls back to one.
//
// The mapping builds a Foundation formatter, tens of microseconds — hundreds of times a
// door's own cost — so formats are cached by locale identifier, except for `.current` and
// `.autoupdatingCurrent`: those are the user's settings, and on Apple platforms the user's
// own number-format overrides are not in the identifier, so the two derive on every call
// (see ``LocaleFormats``). A hot loop over the user's locale derives once with
// ``NumFormat/from(locale:)`` and passes the format.
extension Cast {

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``i8(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func i8(_ text: String, locale: Locale) throws -> Verdict<Int8> {
        try i8(text, format: localeFormat(locale))
    }

    /// See ``i8(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i8(_ utf8: [UInt8], locale: Locale) throws -> Verdict<Int8> {
        try i8(utf8, format: localeFormat(locale))
    }

    /// See ``i8(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func i8(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<Int8> {
        try i8(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``i16(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func i16(_ text: String, locale: Locale) throws -> Verdict<Int16> {
        try i16(text, format: localeFormat(locale))
    }

    /// See ``i16(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i16(_ utf8: [UInt8], locale: Locale) throws -> Verdict<Int16> {
        try i16(utf8, format: localeFormat(locale))
    }

    /// See ``i16(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func i16(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<Int16> {
        try i16(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``i32(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func i32(_ text: String, locale: Locale) throws -> Verdict<Int32> {
        try i32(text, format: localeFormat(locale))
    }

    /// See ``i32(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i32(_ utf8: [UInt8], locale: Locale) throws -> Verdict<Int32> {
        try i32(utf8, format: localeFormat(locale))
    }

    /// See ``i32(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func i32(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<Int32> {
        try i32(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``i64(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func i64(_ text: String, locale: Locale) throws -> Verdict<Int64> {
        try i64(text, format: localeFormat(locale))
    }

    /// See ``i64(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i64(_ utf8: [UInt8], locale: Locale) throws -> Verdict<Int64> {
        try i64(utf8, format: localeFormat(locale))
    }

    /// See ``i64(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func i64(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<Int64> {
        try i64(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``u8(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func u8(_ text: String, locale: Locale) throws -> Verdict<UInt8> {
        try u8(text, format: localeFormat(locale))
    }

    /// See ``u8(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u8(_ utf8: [UInt8], locale: Locale) throws -> Verdict<UInt8> {
        try u8(utf8, format: localeFormat(locale))
    }

    /// See ``u8(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func u8(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<UInt8> {
        try u8(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``u16(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func u16(_ text: String, locale: Locale) throws -> Verdict<UInt16> {
        try u16(text, format: localeFormat(locale))
    }

    /// See ``u16(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u16(_ utf8: [UInt8], locale: Locale) throws -> Verdict<UInt16> {
        try u16(utf8, format: localeFormat(locale))
    }

    /// See ``u16(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func u16(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<UInt16> {
        try u16(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``u32(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func u32(_ text: String, locale: Locale) throws -> Verdict<UInt32> {
        try u32(text, format: localeFormat(locale))
    }

    /// See ``u32(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u32(_ utf8: [UInt8], locale: Locale) throws -> Verdict<UInt32> {
        try u32(utf8, format: localeFormat(locale))
    }

    /// See ``u32(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func u32(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<UInt32> {
        try u32(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``u64(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func u64(_ text: String, locale: Locale) throws -> Verdict<UInt64> {
        try u64(text, format: localeFormat(locale))
    }

    /// See ``u64(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u64(_ utf8: [UInt8], locale: Locale) throws -> Verdict<UInt64> {
        try u64(utf8, format: localeFormat(locale))
    }

    /// See ``u64(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func u64(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<UInt64> {
        try u64(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``f32(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func f32(_ text: String, locale: Locale) throws -> Verdict<Float> {
        try f32(text, format: localeFormat(locale))
    }

    /// See ``f32(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func f32(_ utf8: [UInt8], locale: Locale) throws -> Verdict<Float> {
        try f32(utf8, format: localeFormat(locale))
    }

    /// See ``f32(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func f32(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<Float> {
        try f32(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``f64(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func f64(_ text: String, locale: Locale) throws -> Verdict<Double> {
        try f64(text, format: localeFormat(locale))
    }

    /// See ``f64(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func f64(_ utf8: [UInt8], locale: Locale) throws -> Verdict<Double> {
        try f64(utf8, format: localeFormat(locale))
    }

    /// See ``f64(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func f64(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<Double> {
        try f64(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``decimal(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func decimal(_ text: String, locale: Locale) throws -> Verdict<Decimal> {
        try decimal(text, format: localeFormat(locale))
    }

    /// See ``decimal(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func decimal(_ utf8: [UInt8], locale: Locale) throws -> Verdict<Decimal> {
        try decimal(utf8, format: localeFormat(locale))
    }

    /// See ``decimal(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func decimal(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<Decimal> {
        try decimal(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``numeric(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func numeric<T: NumericCastTarget>(_ text: String, locale: Locale) throws -> Verdict<T> {
        try numeric(text, format: localeFormat(locale))
    }

    /// See ``numeric(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func numeric<T: NumericCastTarget>(_ utf8: [UInt8], locale: Locale) throws -> Verdict<T> {
        try numeric(utf8, format: localeFormat(locale))
    }

    /// See ``numeric(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func numeric<T: NumericCastTarget>(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws
        -> Verdict<T>
    {
        try numeric(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its separators, signs and symbols, every
    /// lenience on — exactly ``scalar(_:format:)-swift.type.method`` with ``NumFormat/from(locale:)``.
    public static func scalar<T: ScalarCastTarget>(_ text: String, locale: Locale) throws -> Verdict<T> {
        try scalar(text, format: localeFormat(locale))
    }

    /// See ``scalar(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func scalar<T: ScalarCastTarget>(_ utf8: [UInt8], locale: Locale) throws -> Verdict<T> {
        try scalar(utf8, format: localeFormat(locale))
    }

    /// See ``scalar(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func scalar<T: ScalarCastTarget>(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<T>
    {
        try scalar(utf8, format: localeFormat(locale))
    }

    /// Casts under the format `locale` declares: its short-date field order and its day-period
    /// designators — exactly ``dateTime(_:format:)-swift.type.method`` with ``DateFormat/from(locale:)``.
    public static func dateTime(_ text: String, locale: Locale) throws -> Verdict<DateComponents> {
        try dateTime(text, format: localeDateFormat(locale))
    }

    /// See ``dateTime(_:locale:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func dateTime(_ utf8: [UInt8], locale: Locale) throws -> Verdict<DateComponents> {
        try dateTime(utf8, format: localeDateFormat(locale))
    }

    /// See ``dateTime(_:locale:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func dateTime(_ utf8: UnsafeRawBufferPointer, locale: Locale) throws -> Verdict<DateComponents> {
        try dateTime(utf8, format: localeDateFormat(locale))
    }

    /// ``NumFormat/from(locale:)``, cached by locale identifier.
    static func localeFormat(_ locale: Locale) -> NumFormat {
        numFormats.format(for: locale)
    }

    /// ``DateFormat/from(locale:)``, cached by locale identifier.
    static func localeDateFormat(_ locale: Locale) -> DateFormat {
        dateFormats.format(for: locale)
    }

    private static let numFormats = LocaleFormats(derive: NumFormat.from(locale:))
    private static let dateFormats = LocaleFormats(derive: DateFormat.from(locale:))
}

/// The locale-to-format cache behind the `locale:` doors, one for each format type:
/// identifier to format, under a lock, cleared rather than grown past a small bound — a
/// process parses under a handful of locales, and an unbounded map keyed by caller input
/// would be a leak. `NSLock` rather than `Synchronization.Mutex`, which needs macOS 15 and
/// iOS 18; this package's floors are 13 and 16.
///
/// `Locale.current` and `Locale.autoupdatingCurrent` are never cached: the user's own
/// number-format overrides on Apple platforms live in the locale, not its identifier, so an
/// entry keyed by `en_US` would serve a user who writes `en_US` with a decimal comma the
/// wrong format for as long as it lived. Those two derive on every call instead — a user's
/// settings are read when they are asked for — and an explicit identifier is served from the
/// cache. Which locale is the user's is `Locale`'s own equality.
final class LocaleFormats<Format: Sendable>: @unchecked Sendable {
    private static var capacity: Int { 32 }

    private let derive: @Sendable (Locale) -> Format
    private let lock = NSLock()
    private var formats: [String: Format] = [:]

    init(derive: @escaping @Sendable (Locale) -> Format) {
        self.derive = derive
    }

    func format(for locale: Locale) -> Format {
        if locale == .autoupdatingCurrent || locale == .current {
            return derive(locale)
        }
        let identifier = locale.identifier
        lock.lock()
        if let cached = formats[identifier] {
            lock.unlock()
            return cached
        }
        lock.unlock()
        let derived = derive(locale)
        lock.lock()
        if formats.count >= Self.capacity {
            formats.removeAll(keepingCapacity: true)
        }
        formats[identifier] = derived
        lock.unlock()
        return derived
    }
}
