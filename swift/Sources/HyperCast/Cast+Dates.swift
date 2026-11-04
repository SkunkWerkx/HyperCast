import Foundation

// The date doors that take ``DateOptions``: the opt-in shapes (a compact date, a space or an
// hour-only offset in an instant, a pivot for two-digit years, declared month names), which the
// plain doors keep refusing. Each is the plain door of the same name with the options handed to
// the core beside the text; a door reads only the options that are its own, and ignores the
// rest.
extension Cast {

    // MARK: - timestamp

    /// Casts an RFC 3339 instant as ``timestamp(_:)-swift.type.method`` does, with the shapes
    /// `options` opens: ``DateOptions/Flags/spaceSeparator`` (a single space for the `T`) and
    /// ``DateOptions/Flags/hourOffset`` (`+00`, `-05`). The zone stays mandatory.
    public static func timestamp(_ text: String, options: DateOptions) -> Verdict<Date> {
        withUTF8(text) { timestamp($0, options: options) }
    }

    /// See ``timestamp(_:options:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func timestamp(_ utf8: [UInt8], options: DateOptions) -> Verdict<Date> {
        utf8.withUnsafeBytes { timestamp($0, options: options) }
    }

    /// See ``timestamp(_:options:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func timestamp(_ utf8: UnsafeRawBufferPointer, options: DateOptions) -> Verdict<Date> {
        parameterizedDoor(DoorCode.timestamp, 0, utf8, options: options, read: Interop.instant)
    }

    // MARK: - date

    /// Casts a calendar date as ``date(_:)-swift.type.method`` does, with the shapes `options`
    /// opens: ``DateOptions/Flags/compactDate`` (`20260107`).
    public static func date(_ text: String, options: DateOptions) -> Verdict<DateComponents> {
        withUTF8(text) { date($0, options: options) }
    }

    /// See ``date(_:options:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func date(_ utf8: [UInt8], options: DateOptions) -> Verdict<DateComponents> {
        utf8.withUnsafeBytes { date($0, options: options) }
    }

    /// See ``date(_:options:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func date(_ utf8: UnsafeRawBufferPointer, options: DateOptions) -> Verdict<DateComponents> {
        parameterizedDoor(DoorCode.date, 0, utf8, options: options, read: Interop.date)
    }

    /// Casts a separated calendar date as ``date(_:order:)-swift.type.method`` does, with what
    /// `options` declares: a ``DateOptions/pivot`` for two-digit years (`7/1/26`) and
    /// ``DateOptions/months`` for a month name in the month field (`07-Jan-26`, in the
    /// position `order` selects, with the date's own separator). A name never reorders the
    /// fields.
    public static func date(_ text: String, order: DateOrder, options: DateOptions) -> Verdict<DateComponents> {
        withUTF8(text) { date($0, order: order, options: options) }
    }

    /// See ``date(_:order:options:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func date(_ utf8: [UInt8], order: DateOrder, options: DateOptions) -> Verdict<DateComponents> {
        utf8.withUnsafeBytes { date($0, order: order, options: options) }
    }

    /// See ``date(_:order:options:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func date(_ utf8: UnsafeRawBufferPointer, order: DateOrder, options: DateOptions)
        -> Verdict<DateComponents>
    {
        parameterizedDoor(DoorCode.dateOrdered, order.rawValue, utf8, options: options, read: Interop.date)
    }

    // MARK: - date-time

    /// Casts a civil date-time as ``dateTime(_:order:)-swift.type.method`` does, with the
    /// ``DateOptions/pivot`` and ``DateOptions/months`` of its date part.
    public static func dateTime(_ text: String, order: DateOrder, options: DateOptions) -> Verdict<DateComponents> {
        withUTF8(text) { dateTime($0, order: order, options: options) }
    }

    /// See ``dateTime(_:order:options:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func dateTime(_ utf8: [UInt8], order: DateOrder, options: DateOptions) -> Verdict<DateComponents> {
        utf8.withUnsafeBytes { dateTime($0, order: order, options: options) }
    }

    /// See ``dateTime(_:order:options:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func dateTime(_ utf8: UnsafeRawBufferPointer, order: DateOrder, options: DateOptions)
        -> Verdict<DateComponents>
    {
        parameterizedDoor(DoorCode.dateTime, order.rawValue, utf8, options: options, read: Interop.civil)
    }

    /// Casts a civil date-time as ``dateTime(_:format:)-swift.type.method`` does, with the
    /// ``DateOptions/pivot`` and ``DateOptions/months`` of its date part.
    public static func dateTime(_ text: String, format: DateFormat, options: DateOptions) -> Verdict<DateComponents> {
        withUTF8(text) { dateTime($0, format: format, options: options) }
    }

    /// See ``dateTime(_:format:options:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func dateTime(_ utf8: [UInt8], format: DateFormat, options: DateOptions) -> Verdict<DateComponents> {
        utf8.withUnsafeBytes { dateTime($0, format: format, options: options) }
    }

    /// See ``dateTime(_:format:options:)-swift.type.method``; input as a raw view of UTF-8 bytes.
    public static func dateTime(_ utf8: UnsafeRawBufferPointer, format: DateFormat, options: DateOptions)
        -> Verdict<DateComponents>
    {
        dateTimeDoor(utf8, format.order, format, options, read: Interop.civil)
    }
}
