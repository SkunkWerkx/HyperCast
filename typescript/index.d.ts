import type {
  CastFailure as Reason,
  CheckedVerdict,
  Constraints,
  Detail as DetailName,
  DetailedCheckedVerdict,
  DetailedVerdict,
  Text,
  Verdict,
  Violation,
} from './binding'
export {
  nativeVersion,
  castBool,
  castChar,
  castI8,
  castI16,
  castI32,
  castI64,
  castU8,
  castU16,
  castU32,
  castU64,
  castF32,
  castF64,
  castDecimal,
  castUuid,
  castTimestamp,
  castUnix,
  castExcelSerial,
  castDate,
  castDatetime,
  castTime,
  castDuration,
  castDecimalFromNumber,
  castExcelSerialFromNumber,
  castExcelTime,
  castExcelDuration,
  castChecked,
  castCheckedFromNumber,
  castDetailed,
  castDetailedFromNumber,
  castCheckedDetailed,
  castCheckedDetailedFromNumber,
  castConstraintsCheck,
  castDeclaredDecimal,
  NumFormat,
  DateFormat,
  DateOptions,
  MonthNames,
  Constraints,
  NumStyles,
  Grouping,
  DateStyles,
  UnixPrecision,
  DateOrder,
  ExcelEpoch,
  Door,
  Rounding,
} from './binding'
export type {
  Verdict,
  Success,
  Failure,
  Fault,
  Declaration,
  Refusal,
  Text,
  NumFormatSpec,
  DateFormatSpec,
  DateOptionsSpec,
  Timestamp,
  CalendarDate,
  TimeOfDay,
  CivilDateTime,
  Duration,
  ConstraintsSpec,
  Checked,
  CheckedVerdict,
  Violated,
  Violation,
  Rejection,
  DetailedFault,
  DetailedFailure,
  DetailedVerdict,
  DetailedCheckedVerdict,
} from './binding'
/** The closed set of reasons a cast can fail. */
export type CastFailure = Reason
/** The three reasons by name, for a caller who would rather not spell the strings. */
export declare const CastFailure: {
  readonly Empty: 'empty'
  readonly Malformed: 'malformed'
  readonly OutOfRange: 'outOfRange'
}
/** What failed: the core's published detail vocabulary by name, `'none'` for code 0 (a contract violation) and `'unknown'` for a code this binding does not name yet, whose number is in `detailCode`. Open, like `Keyword`: a later release may name more, so a switch needs a default arm. */
export type Detail = DetailName | (string & {})
/** The detail names, for a caller who would rather not spell the strings. */
export declare const Detail: {
  readonly None: 'none'
  readonly Blank: 'blank'
  readonly UnexpectedCharacter: 'unexpectedCharacter'
  readonly DigitExpected: 'digitExpected'
  readonly DelimiterExpected: 'delimiterExpected'
  readonly TrailingText: 'trailingText'
  readonly NoDigits: 'noDigits'
  readonly SignMisplaced: 'signMisplaced'
  readonly ParenthesesUnbalanced: 'parenthesesUnbalanced'
  readonly MarkRepeated: 'markRepeated'
  readonly GroupMisplaced: 'groupMisplaced'
  readonly GroupSize: 'groupSize'
  readonly GroupInFraction: 'groupInFraction'
  readonly AmbiguousSeparator: 'ambiguousSeparator'
  readonly FractionNotAllowed: 'fractionNotAllowed'
  readonly FieldWidth: 'fieldWidth'
  readonly WrongLength: 'wrongLength'
  readonly ComponentMissing: 'componentMissing'
  readonly NotRecognized: 'notRecognized'
  readonly UnknownUnit: 'unknownUnit'
  readonly InvalidUtf8: 'invalidUtf8'
  readonly TooLong: 'tooLong'
  readonly NotFinite: 'notFinite'
  readonly NotWhole: 'notWhole'
  readonly NegativeNotAllowed: 'negativeNotAllowed'
  readonly TooLarge: 'tooLarge'
  readonly TooSmall: 'tooSmall'
  readonly TooPrecise: 'tooPrecise'
  readonly DateFieldOutOfRange: 'dateFieldOutOfRange'
  readonly TimeFieldOutOfRange: 'timeFieldOutOfRange'
  readonly OutsideWindow: 'outsideWindow'
  readonly NonexistentDate: 'nonexistentDate'
  readonly NotAScalar: 'notAScalar'
  readonly Unknown: 'unknown'
}
/** A rule a violation names, in the OpenAPI spelling; open, so a switch needs a default arm. */
export type Keyword = Violation<unknown>['keyword']
/** The eight keywords by name. A code this binding does not name yet arrives as "keyword N". */
export declare const Keyword: {
  readonly Minimum: 'minimum'
  readonly ExclusiveMinimum: 'exclusiveMinimum'
  readonly Maximum: 'maximum'
  readonly ExclusiveMaximum: 'exclusiveMaximum'
  readonly MultipleOf: 'multipleOf'
  readonly MinLength: 'minLength'
  readonly MaxLength: 'maxLength'
  readonly Enum: 'enum'
}
/** Text as text: the input itself once it is valid text, `empty` for empty input. */
export function castText(text: Text): Verdict<string>
/**
 * Text judged by `Constraints` for `Door.Text`: trimmed, lengths and a folded `enum`. Passing
 * the second argument at all makes the cast constrained: `undefined` or `null` there (a failed
 * declaration's missing `.value`) is a contract violation, never "no rules".
 */
export function castText(text: Text, constraints: Constraints): CheckedVerdict<string>
/** Whether the native addon or its WebAssembly fallback loaded. */
export function isAvailable(): boolean
/** An `empty` fault as `null`, the absent value; every other verdict as it is. */
export function optional<T>(verdict: Verdict<T>): Verdict<T> | null
/** The same for a checked verdict: a violation is not absent, and comes back as it is. */
export function optional<T>(verdict: CheckedVerdict<T>): CheckedVerdict<T> | null
/** The same for a detailed verdict: an `empty` fault is absent. */
export function optional<T>(verdict: DetailedVerdict<T>): DetailedVerdict<T> | null
/** The same for a detailed checked verdict. */
export function optional<T>(verdict: DetailedCheckedVerdict<T>): DetailedCheckedVerdict<T> | null
export type HyperCastErrorCode = 'ERR_HYPERCAST_UNAVAILABLE'
/** The one error this package throws: no addon loaded, so no door can answer. */
export interface HyperCastError extends Error {
  code: HyperCastErrorCode
}
export function isHyperCastError(value: unknown): value is HyperCastError
