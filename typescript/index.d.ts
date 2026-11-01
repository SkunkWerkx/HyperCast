import type { CastFailure as Reason, Verdict } from './binding'
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
  NumFormat,
  DateFormat,
  NumStyles,
  UnixPrecision,
  DateOrder,
  ExcelEpoch,
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
} from './binding'
/** The closed set of reasons a cast can fail. */
export type CastFailure = Reason
/** The three reasons by name, for a caller who would rather not spell the strings. */
export declare const CastFailure: {
  readonly Empty: 'empty'
  readonly Malformed: 'malformed'
  readonly OutOfRange: 'outOfRange'
}
/** Whether the native addon or its WebAssembly fallback loaded. */
export function isAvailable(): boolean
/** An `empty` fault as `null`, the absent value; every other verdict as it is. */
export function optional<T>(verdict: Verdict<T>): Verdict<T> | null
export type HyperCastErrorCode = 'ERR_HYPERCAST_UNAVAILABLE'
/** The one error this package throws: no addon loaded, so no door can answer. */
export interface HyperCastError extends Error {
  code: HyperCastErrorCode
}
export function isHyperCastError(value: unknown): value is HyperCastError
