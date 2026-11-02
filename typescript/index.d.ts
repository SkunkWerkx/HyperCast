import type {
  CastFailure as Reason,
  CheckedVerdict,
  Constraints,
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
  castConstraintsCheck,
  castDeclaredDecimal,
  NumFormat,
  DateFormat,
  Constraints,
  NumStyles,
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
} from './binding'
/** The closed set of reasons a cast can fail. */
export type CastFailure = Reason
/** The three reasons by name, for a caller who would rather not spell the strings. */
export declare const CastFailure: {
  readonly Empty: 'empty'
  readonly Malformed: 'malformed'
  readonly OutOfRange: 'outOfRange'
}
/** A rule a violation names, in the OpenAPI spelling; open, so a switch needs a default arm. */
export type Keyword = Violation['keyword']
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
export type HyperCastErrorCode = 'ERR_HYPERCAST_UNAVAILABLE'
/** The one error this package throws: no addon loaded, so no door can answer. */
export interface HyperCastError extends Error {
  code: HyperCastErrorCode
}
export function isHyperCastError(value: unknown): value is HyperCastError
