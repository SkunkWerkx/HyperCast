let native = null
let loadError = null
try {
  native = require('./binding.js')
} catch (error) {
  loadError = error
}
// The addon is the core compiled in, so it must be this package's: a stale platform package, or
// an addon NAPI_RS_NATIVE_LIBRARY_PATH names, that is not this version or lacks the 1.1 detailed
// doors is no addon here, and every door says so rather than answering as a 1.0 core would.
if (native !== null) {
  try {
    const expected = require('./package.json').version
    const actual = native.nativeVersion()
    if (actual !== expected) {
      throw new Error(`the addon is core ${actual}, and this package is ${expected}`)
    }
    if (typeof native.castDetailed !== 'function') {
      throw new Error('the addon has no castDetailed, so it is older than 1.1')
    }
  } catch (error) {
    loadError = error
    native = null
  }
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
const castDetailed = door('castDetailed')
const castDetailedFromNumber = door('castDetailedFromNumber')
const castCheckedDetailed = door('castCheckedDetailed')
const castCheckedDetailedFromNumber = door('castCheckedDetailedFromNumber')
const castChecked = door('castChecked')
const castCheckedFromNumber = door('castCheckedFromNumber')
// castText chooses plain or constrained by arity in the addon itself (its module_exports hook),
// so every entry, this one, the WebAssembly fallback and its browser entry, agrees.
const castText = door('castText')
const castConstraintsCheck = door('castConstraintsCheck')
const castDeclaredDecimal = door('castDeclaredDecimal')
// The format classes, or stand-ins whose static methods throw the same unavailable error.
const formatClass = (name, statics) => {
  if (native) return native[name]
  const stand = { [name]: class {} }[name]
  for (const method of statics) stand[method] = unavailable(`${name}.${method}`)
  return stand
}
const NumFormat = formatClass('NumFormat', ['declare', 'invariant', 'detect'])
const DateFormat = formatClass('DateFormat', ['declare'])
const Constraints = formatClass('Constraints', ['declare'])
const DateOptions = formatClass('DateOptions', ['declare', 'commonForms'])
const MonthNames = formatClass('MonthNames', ['declare', 'fromPacked'])
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
const Grouping = native ? native.Grouping : Object.freeze({ Western: 0, Indian: 512 })
const DateStyles = native
  ? native.DateStyles
  : Object.freeze({ None: 0, CompactDate: 1, SpaceSeparator: 2, HourOffset: 4, CommonForms: 7 })
const UnixPrecision = native
  ? native.UnixPrecision
  : Object.freeze({ Seconds: 1, Milliseconds: 2, Microseconds: 3, Nanoseconds: 4 })
const DateOrder = native
  ? native.DateOrder
  : Object.freeze({ YearMonthDay: 1, MonthDayYear: 2, DayMonthYear: 3 })
const ExcelEpoch = native ? native.ExcelEpoch : Object.freeze({ Y1900: 1, Y1904: 2 })
const Door = native
  ? native.Door
  : Object.freeze({
      Bool: 1,
      I8: 2,
      I16: 3,
      I32: 4,
      I64: 5,
      U8: 6,
      U16: 7,
      U32: 8,
      U64: 9,
      F32: 10,
      F64: 11,
      Uuid: 12,
      Timestamp: 13,
      Unix: 14,
      Date: 15,
      Time: 16,
      Duration: 17,
      Text: 18,
      Decimal: 19,
      DateOrdered: 20,
      DateTime: 21,
      ExcelSerial: 22,
      Char: 23,
    })
const Rounding = native
  ? native.Rounding
  : Object.freeze({ Strict: 0, Midpoint: 1, Floor: 2, Ceiling: 3 })
// The keywords a violation names, by name. Open: a code this binding does not name yet
// arrives as "keyword N", so a switch over keywords needs a default arm.
const Keyword = Object.freeze({
  Minimum: 'minimum',
  ExclusiveMinimum: 'exclusiveMinimum',
  Maximum: 'maximum',
  ExclusiveMaximum: 'exclusiveMaximum',
  MultipleOf: 'multipleOf',
  MinLength: 'minLength',
  MaxLength: 'maxLength',
  Enum: 'enum',
})
const CastFailure = native
  ? native.CastFailure
  : Object.freeze({ Empty: 'empty', Malformed: 'malformed', OutOfRange: 'outOfRange' })
// What failed, by name. Open: a code this binding does not name yet arrives as 'unknown' with
// its number in detailCode, so a switch over details needs a default arm.
const Detail = native
  ? native.Detail
  : Object.freeze({
      None: 'none',
      Blank: 'blank',
      UnexpectedCharacter: 'unexpectedCharacter',
      DigitExpected: 'digitExpected',
      DelimiterExpected: 'delimiterExpected',
      TrailingText: 'trailingText',
      NoDigits: 'noDigits',
      SignMisplaced: 'signMisplaced',
      ParenthesesUnbalanced: 'parenthesesUnbalanced',
      MarkRepeated: 'markRepeated',
      GroupMisplaced: 'groupMisplaced',
      GroupSize: 'groupSize',
      GroupInFraction: 'groupInFraction',
      AmbiguousSeparator: 'ambiguousSeparator',
      FractionNotAllowed: 'fractionNotAllowed',
      FieldWidth: 'fieldWidth',
      WrongLength: 'wrongLength',
      ComponentMissing: 'componentMissing',
      NotRecognized: 'notRecognized',
      UnknownUnit: 'unknownUnit',
      InvalidUtf8: 'invalidUtf8',
      TooLong: 'tooLong',
      NotFinite: 'notFinite',
      NotWhole: 'notWhole',
      NegativeNotAllowed: 'negativeNotAllowed',
      TooLarge: 'tooLarge',
      TooSmall: 'tooSmall',
      TooPrecise: 'tooPrecise',
      DateFieldOutOfRange: 'dateFieldOutOfRange',
      TimeFieldOutOfRange: 'timeFieldOutOfRange',
      OutsideWindow: 'outsideWindow',
      NonexistentDate: 'nonexistentDate',
      NotAScalar: 'notAScalar',
      Unknown: 'unknown',
    })
const isAvailable = () => native !== null
const isHyperCastError = (value) =>
  value instanceof Error &&
  typeof value.code === 'string' &&
  value.code.startsWith('ERR_HYPERCAST_')
// An `empty` fault is absent: null. Everything else, a violation and a value that is no
// verdict included, comes back as it went in.
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
  castChecked,
  castCheckedFromNumber,
  castDetailed,
  castDetailedFromNumber,
  castCheckedDetailed,
  castCheckedDetailedFromNumber,
  castText,
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
  Keyword,
  CastFailure,
  Detail,
}
