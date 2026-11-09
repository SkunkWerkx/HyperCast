// The declarations are values: `equals` is true for one declaring the same thing, however
// each was made, as the core compares them, and false for anything else; `===` stays identity.
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const {
  DateFormat,
  DateOptions,
  DateOrder,
  DateStyles,
  Grouping,
  MonthNames,
  NumFormat,
  NumStyles,
} = api
type NumFormatSpec = import('../index.js').NumFormatSpec

function value<T>(declared: { ok: true; value: T } | { ok: false; message: string }): T {
  assert.ok(declared.ok, declared.ok ? '' : declared.message)
  return declared.value
}

const base: NumFormatSpec = { decimalSep: '.', groupSep: ',', flags: NumStyles.All }

test('a format equals one declaring the same thing, however it was made', () => {
  const built = value(NumFormat.declare(base))
  assert.notEqual(built, NumFormat.invariant())
  assert.ok(built.equals(NumFormat.invariant()))
  assert.ok(NumFormat.invariant().equals(built))
  assert.ok(built.equals(built))
  const indian = value(built.withGrouping(Grouping.Indian))
  assert.ok(value(indian.withGrouping(Grouping.Western)).equals(built))
  assert.ok(value(NumFormat.declare({ ...base, grouping: Grouping.Indian })).equals(indian))
  const others = [
    indian,
    NumFormat.detect(),
    value(NumFormat.declare({ ...base, decimalSep: ',', groupSep: '.' })),
    value(NumFormat.declare({ ...base, flags: NumStyles.Grouping })),
    value(NumFormat.declare({ ...base, currency: '$' })),
    value(NumFormat.declare({ ...base, minusSign: '−' })),
    value(NumFormat.declare({ ...base, digitZero: '٠' })),
  ]
  for (const other of others) assert.ok(!other.equals(built), other.decimalSep + other.flags)
  for (const stranger of [undefined, null, 0, 'x', {}, built.packed, DateOptions.commonForms()]) {
    assert.equal(built.equals(stranger), false)
  }
})

test('date declarations equal those declaring the same thing', () => {
  const german = value(
    DateFormat.declare({ order: DateOrder.DayMonthYear, am: 'vorm.', pm: 'nachm.' }),
  )
  assert.ok(
    german.equals(
      value(DateFormat.declare({ order: DateOrder.DayMonthYear, am: 'vorm.', pm: 'nachm.' })),
    ),
  )
  assert.ok(
    !german.equals(
      value(DateFormat.declare({ order: DateOrder.MonthDayYear, am: 'vorm.', pm: 'nachm.' })),
    ),
  )
  assert.ok(
    !german.equals(value(DateFormat.declare({ order: DateOrder.DayMonthYear, am: 'vorm.' }))),
  )
  assert.ok(!german.equals(NumFormat.invariant()))

  const entries: [number, string][] = Array.from({ length: 12 }, (_, i) => [
    i + 1,
    `m${String.fromCharCode(98 + i)}`,
  ])
  const names = value(MonthNames.declare(entries))
  const read = value(MonthNames.fromPacked(names.packed))
  assert.ok(read.equals(names))
  assert.ok(!value(MonthNames.declare([...entries].reverse())).equals(names))
  assert.ok(!names.equals(names.packed))

  const options = value(
    DateOptions.declare({ flags: DateStyles.CompactDate, pivot: 2029, months: names }),
  )
  assert.ok(
    options.equals(
      value(DateOptions.declare({ flags: DateStyles.CompactDate, pivot: 2029, months: read })),
    ),
  )
  for (const other of [
    value(DateOptions.declare({ flags: DateStyles.CompactDate, pivot: 2029 })),
    value(DateOptions.declare({ flags: DateStyles.CommonForms, pivot: 2029, months: names })),
    value(DateOptions.declare({ flags: DateStyles.CompactDate, pivot: 2030, months: names })),
  ]) {
    assert.ok(!options.equals(other))
  }
  assert.ok(
    DateOptions.commonForms().equals(value(DateOptions.declare({ flags: DateStyles.CommonForms }))),
  )
})
