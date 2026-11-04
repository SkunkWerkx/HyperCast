// 1.1 adds a grouping policy, a detailed fault and date options without changing what 1.0 code
// sees. This file is 1.0 code, written as 1.0 wrote it: the same declarations, the same calls
// with the same arities, the same shapes read back, the same comparisons. The one behaviour
// that moved on purpose is `NumFormat`'s default grouping (strict Western), and its escape
// hatch is held here too.
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const {
  Constraints,
  DateFormat,
  DateOrder,
  Door,
  ExcelEpoch,
  Grouping,
  NumFormat,
  NumStyles,
  Rounding,
  UnixPrecision,
} = api

test('a 1.0 declaration, with no grouping field, still declares and reads back', () => {
  const declared = NumFormat.declare({
    decimalSep: ',',
    groupSep: '.',
    flags: NumStyles.All,
    currency: 'EUR',
    minusSign: '\u2212',
  })
  assert.ok(declared.ok)
  const format = declared.value
  // The getters 1.0 had, with the values 1.0 gave. `flags` is the style mask, whole.
  assert.equal(format.decimalSep, ',')
  assert.equal(format.groupSep, '.')
  assert.equal(format.flags, NumStyles.All)
  assert.equal(format.currency, 'EUR')
  assert.equal(format.currencyDecimalSep, '')
  assert.equal(format.minusSign, '\u2212')
  assert.equal(format.digitZero, '')
  assert.deepEqual(api.castF64('1.234,5', format), { ok: true, value: 1234.5 })
  assert.deepEqual(api.castI32('EUR 1.234', format), { ok: true, value: 1234 })
})

test('the 1.0 enums keep their values, and the doors 1.0 had keep their numbers', () => {
  assert.deepEqual(
    { ...NumStyles },
    {
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
    },
  )
  assert.deepEqual(
    { ...UnixPrecision },
    { Seconds: 1, Milliseconds: 2, Microseconds: 3, Nanoseconds: 4 },
  )
  assert.deepEqual({ ...DateOrder }, { YearMonthDay: 1, MonthDayYear: 2, DayMonthYear: 3 })
  assert.deepEqual({ ...ExcelEpoch }, { Y1900: 1, Y1904: 2 })
  assert.deepEqual({ ...Rounding }, { Strict: 0, Midpoint: 1, Floor: 2, Ceiling: 3 })
  const had = {
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
    Text: 18,
    Decimal: 19,
  }
  for (const [name, code] of Object.entries(had)) {
    assert.equal(Door[name as keyof typeof Door], code, name)
  }
})

test('the plain verdicts keep their shapes, whatever the detailed doors add', () => {
  const failed = api.castI32('12x')
  assert.deepEqual(failed, { ok: false, fault: { reason: 'malformed', offset: 2, length: 1 } })
  assert.deepEqual(Object.keys(failed), ['ok', 'fault'])
  assert.ok(!failed.ok)
  assert.deepEqual(Object.keys(failed.fault), ['reason', 'offset', 'length'])
  assert.deepEqual(api.castI32('12'), { ok: true, value: 12 })
  // A checked door's fault and violation are 1.0's too.
  const step = Constraints.declare({ door: Door.I32, maximum: '10' })
  assert.ok(step.ok)
  assert.deepEqual(api.castChecked('12x', step.value), {
    ok: false,
    fault: { reason: 'malformed', offset: 2, length: 1 },
  })
  assert.deepEqual(api.castChecked('12', step.value), {
    ok: false,
    violation: { keyword: 'maximum', offset: 0, length: 2 },
  })
  assert.deepEqual(api.castChecked('7', step.value), {
    ok: true,
    value: 7,
    adjusted: false,
    enumIndex: null,
  })
  // A detailed fault is a `Fault` with more: what 1.0 code reads from it is still there.
  const detailed = api.castDetailed(Door.I32, '12x')
  assert.ok(!detailed.ok)
  const asPlain: import('../index.js').Fault = detailed.fault
  assert.deepEqual(
    { reason: asPlain.reason, offset: asPlain.offset, length: asPlain.length },
    failed.fault,
  )
})

test('the date doors keep their 1.0 arities and answers', () => {
  const order = DateOrder.DayMonthYear
  assert.deepEqual(api.castDate('2026-01-07'), {
    ok: true,
    value: { year: 2026, month: 1, day: 7 },
  })
  assert.deepEqual(api.castDate('7/1/2026', order), {
    ok: true,
    value: { year: 2026, month: 1, day: 7 },
  })
  assert.deepEqual(api.castTimestamp('2026-01-07T15:04:05Z'), {
    ok: true,
    value: { seconds: 1767798245, nanos: 0 },
  })
  assert.deepEqual(api.castDatetime('7/1/2026 15:04', order), {
    ok: true,
    value: { year: 2026, month: 1, day: 7, hour: 15, minute: 4, second: 0, nanos: 0 },
  })
  const german = DateFormat.declare({ order, pm: 'nachm.' })
  assert.ok(german.ok)
  assert.deepEqual(api.castDatetime('7/1/2026 3:04 nachm.', german.value), {
    ok: true,
    value: { year: 2026, month: 1, day: 7, hour: 15, minute: 4, second: 0, nanos: 0 },
  })
  // No options: the shapes 1.0 refused are still refused, however the extra shapes are spelled.
  assert.ok(!api.castDate('20260107').ok)
  assert.ok(!api.castTimestamp('2026-01-07 15:04:05Z').ok)
  assert.ok(!api.castTimestamp('2026-01-07T15:04:05+00').ok)
  assert.ok(!api.castDate('7/1/26', order).ok)
  assert.ok(!api.castDate('7-Jan-2026', order).ok)
})

test('a fault compares equal as it did: structurally, by reason, offset and length', () => {
  const first = api.castI32('1x')
  const again = api.castI32('1x')
  assert.deepEqual(first, again)
  assert.notEqual(first, again)
  assert.ok(!first.ok && !again.ok)
  assert.deepEqual(first.fault, { reason: 'malformed', offset: 1, length: 1 })
  assert.deepEqual(JSON.parse(JSON.stringify(first.fault)), first.fault)
  assert.deepEqual({ ...first.fault }, first.fault)
  const { reason, offset, length } = first.fault
  assert.deepEqual([reason, offset, length], ['malformed', 1, 1])
})

test('the one moved default: a 1.0 declaration reads groups strictly, and Legacy is the way back', () => {
  const strict = NumFormat.declare({ decimalSep: '.', groupSep: ',', flags: NumStyles.All })
  const legacy = NumFormat.declare({
    decimalSep: '.',
    groupSep: ',',
    flags: NumStyles.All,
    grouping: Grouping.Legacy,
  })
  assert.ok(strict.ok && legacy.ok)
  assert.equal(strict.value.grouping, Grouping.Western)
  assert.equal(legacy.value.grouping, Grouping.Legacy)
  assert.deepEqual(api.castI32('12,34', legacy.value), { ok: true, value: 1234 })
  assert.deepEqual(api.castI32('12,34', strict.value), {
    ok: false,
    fault: { reason: 'malformed', offset: 2, length: 3 },
  })
  // A door given no format reads NumFormat.invariant(), strict too; 1,234 is fine either way.
  assert.deepEqual(api.castI32('12,34'), api.castI32('12,34', NumFormat.invariant()))
  assert.deepEqual(api.castI32('1,234'), { ok: true, value: 1234 })
})
