let native = null
let loadError = null
try {
  native = require('./binding.js')
} catch (error) {
  loadError = error
}
const unavailable = (name) => () => {
  const error = new Error(`hypercast: the native addon did not load, so ${name}() is unavailable`, {
    cause: loadError,
  })
  error.code = 'ERR_HYPERCAST_UNAVAILABLE'
  throw error
}
const door = (name) => (native ? native[name] : unavailable(name))
const nativeVersion = door('nativeVersion')
const castBool = door('castBool')
const castChar = door('castChar')
const castI8 = door('castI8')
const castI16 = door('castI16')
const castI32 = door('castI32')
const castI64 = door('castI64')
const castU8 = door('castU8')
const castU16 = door('castU16')
const castU32 = door('castU32')
const castU64 = door('castU64')
const castF32 = door('castF32')
const castF64 = door('castF64')
const castDecimal = door('castDecimal')
const castUuid = door('castUuid')
const castTimestamp = door('castTimestamp')
const castUnix = door('castUnix')
const castExcelSerial = door('castExcelSerial')
const castDate = door('castDate')
const castDatetime = door('castDatetime')
const castTime = door('castTime')
const castDuration = door('castDuration')
const castDecimalFromNumber = door('castDecimalFromNumber')
const castExcelSerialFromNumber = door('castExcelSerialFromNumber')
const castExcelTime = door('castExcelTime')
const castExcelDuration = door('castExcelDuration')
// The format classes, or stand-ins whose static methods throw the same unavailable error.
const formatClass = (name, statics) => {
  if (native) return native[name]
  const stand = { [name]: class {} }[name]
  for (const method of statics) stand[method] = unavailable(`${name}.${method}`)
  return stand
}
const NumFormat = formatClass('NumFormat', ['declare', 'invariant', 'detect'])
const DateFormat = formatClass('DateFormat', ['declare'])
// Frozen literal fallbacks, so the enums exist (and keep their values) when the addon did not load.
const NumStyles = native
  ? native.NumStyles
  : Object.freeze({
      None: 0,
      Grouping: 1,
      Parentheses: 2,
      Exponent: 4,
      RadixPrefixes: 8,
      Percent: 16,
      SeparatorDetect: 32,
      Currency: 64,
      TrailingSign: 128,
      All: 95,
    })
const UnixPrecision = native
  ? native.UnixPrecision
  : Object.freeze({ Seconds: 1, Milliseconds: 2, Microseconds: 3, Nanoseconds: 4 })
const DateOrder = native
  ? native.DateOrder
  : Object.freeze({ YearMonthDay: 1, MonthDayYear: 2, DayMonthYear: 3 })
const ExcelEpoch = native ? native.ExcelEpoch : Object.freeze({ Y1900: 1, Y1904: 2 })
const CastFailure = native
  ? native.CastFailure
  : Object.freeze({ Empty: 'empty', Malformed: 'malformed', OutOfRange: 'outOfRange' })
const isAvailable = () => native !== null
const isHyperCastError = (value) =>
  value instanceof Error &&
  typeof value.code === 'string' &&
  value.code.startsWith('ERR_HYPERCAST_')
// An `empty` fault is absent: null. Everything else, a value that is no verdict included,
// comes back as it went in.
const optional = (verdict) =>
  verdict !== null &&
  typeof verdict === 'object' &&
  verdict.ok === false &&
  verdict.fault !== null &&
  typeof verdict.fault === 'object' &&
  verdict.fault.reason === 'empty'
    ? null
    : verdict
module.exports = {
  nativeVersion,
  isAvailable,
  isHyperCastError,
  optional,
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
  CastFailure,
}
