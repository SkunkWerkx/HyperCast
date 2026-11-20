// An answer is built of properties and elements that are defined, never assigned: a setter a
// caller (or a library they loaded) put on `Object.prototype`, under a field's name or an
// array's index, does not run, so it can neither throw out of a door, nor see a value, nor keep
// a field off the answer.
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const { Constraints, DateFormat, DateOptions, DateOrder, Door, ExcelEpoch, MonthNames } = api
const { NumFormat, NumStyles, Rounding, UnixPrecision } = api

const declare = (spec: import('../index.js').ConstraintsSpec) => {
  const declared = Constraints.declare(spec)
  assert.ok(declared.ok)
  return declared.value
}
const even = declare({
  door: Door.I32,
  maximum: '40',
  multipleOf: '2',
  rounding: Rounding.Midpoint,
})
const big = declare({ door: Door.I64, maximum: 40n, enum: [7n, '8'] })
const money = declare({ door: Door.Decimal, minimum: '0', multipleOf: '0.01' })
const colours = declare({ door: Door.Text, enum: ['red', 'green'], trim: true, maxLength: 5 })
const MONTHS = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
].map((name, at): [number, string] => [at + 1, name])
const months = MonthNames.declare(MONTHS)
assert.ok(months.ok)
const bytes = new TextEncoder().encode('1234')

// Every kind of entry point, and every shape an answer takes. A class instance is read through
// its getters, so two answers compare by what they hold.
const calls: Record<string, () => unknown> = {
  'a value': () => [api.castI32('42'), api.castI32(bytes), api.castBool('yes'), api.castChar('x')],
  'a bigint': () => [api.castI64('-9007199254740993'), api.castU64('18446744073709551615')],
  'a real, a decimal and a uuid': () => [
    api.castF64('1,234.5'),
    api.castDecimal('-1234.50'),
    api.castUuid('{01020304-0506-0708-090a-0b0c0d0e0f10}'),
  ],
  'a fault': () => [api.castI32('12x'), api.castU8('256'), api.castUuid('nope')],
  'an absent text and a contract violation': () => [
    api.castI32(null),
    api.castI32(42 as never),
    api.castI32('1', {} as never),
  ],
  'a timestamp': () => [
    api.castTimestamp('2026-01-02T15:04:05.123456789Z'),
    api.castUnix('1700000000', UnixPrecision.Seconds),
    api.castExcelSerial('45000.5', ExcelEpoch.Y1900),
    api.castTimestamp('x'),
  ],
  'a date, a time and a civil date-time': () => [
    api.castDate('2026-01-02'),
    api.castDate('02 Jan 2026', DateOrder.DayMonthYear, months.value),
    api.castTime('15:04:05.5'),
    api.castDatetime('01/02/2026 15:04', DateOrder.MonthDayYear),
    api.castDate('2026-13-02'),
  ],
  'a duration': () => [api.castDuration('PT1H30M'), api.castDuration('x')],
  'a number already decoded': () => [
    api.castDecimalFromNumber(1.5),
    api.castDecimalFromNumber(Number.NaN),
    api.castExcelSerialFromNumber(45000.5, ExcelEpoch.Y1900),
    api.castExcelTime(0.5),
    api.castExcelDuration(1.25),
    api.castExcelTime('x' as never),
  ],
  'a detailed door': () => [
    api.castDetailed(Door.I32, '42'),
    api.castDetailed(Door.I32, '12x'),
    api.castDetailed(Door.Date, '2026-01-02'),
    api.castDetailed(Door.Timestamp, '2026-01-02T15:04:05Z'),
    api.castDetailed(Door.I32, 42 as never),
    api.castDetailedFromNumber(Door.Decimal, 1.5),
    api.castDetailedFromNumber(Door.Time, 0.5),
    api.castDetailedFromNumber(Door.Decimal, Number.NaN),
  ],
  'a checked value, and an adjusted one': () => [
    api.castChecked('38', even),
    api.castChecked('37', even),
    api.castChecked('7', big),
    api.castChecked('1.50', money),
    api.castCheckedFromNumber(37, even),
  ],
  'a violation': () => [
    api.castChecked('50', even),
    api.castChecked('9', big),
    api.castChecked('-1', money),
    api.castCheckedFromNumber(50, even),
    api.castText('purple', colours),
    api.castText('magenta', colours),
  ],
  'a checked fault and a checked contract violation': () => [
    api.castChecked('x', even),
    api.castChecked('1', colours),
    api.castChecked('1', undefined as never),
    api.castCheckedFromNumber('x' as never, even),
  ],
  'a detailed checked door': () => [
    api.castCheckedDetailed('38', even),
    api.castCheckedDetailed('37', even),
    api.castCheckedDetailed('50', even),
    api.castCheckedDetailed('x', even),
    api.castCheckedDetailedFromNumber(37, even),
    api.castCheckedDetailedFromNumber(50, even),
    api.castCheckedDetailedFromNumber(1.5, even),
  ],
  text: () => [
    api.castText('plain'),
    api.castText(new Uint8Array([0xff])),
    api.castText(' GREEN ', colours),
    api.castText('red', undefined as never),
    (api.castText as () => unknown)(),
  ],
  'a Constraints declaration': () => {
    const numeric = Constraints.declare({ door: Door.I64, maximum: 40n, enum: [7n, '8', '9'] })
    const text = Constraints.declare({ door: Door.Text, enum: ['a', 'b', 'c'], minLength: 1 })
    assert.ok(numeric.ok && text.ok)
    return [
      [numeric.ok, numeric.value.door, numeric.value.maximum, numeric.value.enum],
      [numeric.value.packed, numeric.value.packedEnum, numeric.value.minimum],
      [text.ok, text.value.enum, text.value.minLength, text.value.maxLength, text.value.trim],
      Constraints.declare({ door: Door.I32, minimum: 'x' }),
      Constraints.declare({ door: Door.Bool as never }),
      Constraints.declare(null as never),
      even.enum,
      big.enum,
      colours.enum,
    ]
  },
  'a declaration checked or read': () => [
    api.castConstraintsCheck({ door: Door.I32, maximum: '5' }),
    api.castConstraintsCheck({ door: Door.I32, maximum: 'x' }),
    api.castDeclaredDecimal('1e-6'),
    api.castDeclaredDecimal('x'),
  ],
  'a NumFormat declaration': () => {
    const format = NumFormat.declare({ decimalSep: ',', groupSep: '.', flags: NumStyles.All })
    assert.ok(format.ok)
    return [
      [format.ok, format.value.decimalSep, format.value.groupSep, format.value.packed],
      api.castF64('1.234,5', format.value),
      NumFormat.declare({ decimalSep: ',', groupSep: ',', flags: NumStyles.All }),
      NumFormat.declare(null as never),
      NumFormat.invariant().packed,
      NumFormat.detect().equals(NumFormat.detect()),
    ]
  },
  'the date declarations': () => {
    const format = DateFormat.declare({ order: DateOrder.DayMonthYear, am: 'vorm.', pm: 'nachm.' })
    const names = MonthNames.declare(MONTHS)
    assert.ok(format.ok && names.ok)
    const options = DateOptions.declare({ flags: 7, pivot: 2029, months: names.value })
    const packed = MonthNames.fromPacked(names.value.packed)
    assert.ok(options.ok && packed.ok)
    return [
      [format.ok, format.value.order, format.value.am, format.value.pm],
      [names.ok, names.value.packed, packed.ok, packed.value.equals(names.value)],
      [options.ok, options.value.flags, options.value.pivot, options.value.packed],
      options.value.months?.packed,
      DateOptions.commonForms().packed,
      api.castDatetime('02/01/2026 3:04 nachm.', format.value, options.value),
      DateFormat.declare({ order: 9 as never }),
      MonthNames.declare([[13, 'x']]),
      MonthNames.fromPacked(new Uint8Array(3)),
      DateOptions.declare({ flags: -1 as never }),
    ]
  },
  'an absent verdict': () => [api.optional(api.castI32('')), api.optional(api.castI32('1'))],
  'the version': () => api.nativeVersion(),
}

// Every name a field of an answer goes by, and the indices of the arrays it holds.
const fields = [
  ...['ok', 'value', 'fault', 'reason', 'offset', 'length', 'detail', 'detailCode', 'message'],
  ...['violation', 'keyword', 'adjusted', 'enumIndex', 'seconds', 'nanos', 'year', 'month', 'day'],
  ...['hour', 'minute', 'second'],
]
const indices = ['0', '1', '2']

// Plants a setter that counts its runs and throws, under one key of `Object.prototype`, for
// the length of one call. The getter reads as an object with no such property does.
function underSetter(key: string, call: () => unknown) {
  const proto = Object.prototype as Record<string, unknown>
  // Counted, not listed: `push` assigns, and would run a setter under an index itself.
  let ran = 0
  let answer: unknown
  let thrown: unknown
  Object.defineProperty(proto, key, {
    configurable: true,
    get: () => undefined,
    set() {
      ran += 1
      throw new Error(`the setter under ${key}`)
    },
  })
  try {
    answer = call()
  } catch (error) {
    thrown = error
  } finally {
    delete proto[key]
  }
  return { ran, answer, thrown }
}

test('no answer is assigned through a setter on Object.prototype', () => {
  const keys = [...fields, ...indices]
  const escapes: string[] = []
  for (const [name, call] of Object.entries(calls)) {
    const plain = call()
    for (const key of keys) {
      const { ran, answer, thrown } = underSetter(key, call)
      if (thrown !== undefined) {
        const frame = String((thrown as Error).stack).split('\n')[2]
        escapes.push(`${name}, a setter under ${key}: threw (${frame?.trim()})`)
      } else if (ran !== 0) {
        escapes.push(`${name}: the setter under ${key} ran ${ran} time(s)`)
      } else {
        assert.deepEqual(answer, plain, `${name}, a setter under ${key}`)
      }
    }
  }
  assert.deepEqual(escapes, [])
})

// An array's elements are each defined under their index, and nothing of the page's is called to
// make it: whatever `Array.of` has been replaced with, an enum reads as it was declared.
test('a whole enum is as declared, whatever Array.of has become', () => {
  const ENTRIES = 5000
  const entries = Array.from({ length: ENTRIES }, (_, at) => `entry ${at}`)
  const many = declare({ door: Door.Text, enum: entries })
  const read = () => [many.enum, big.enum, colours.enum, even.enum]
  const expected = [entries, [7n, '8'], ['red', 'green'], null]
  assert.deepEqual(read(), expected)
  const of = Array.of
  for (const stand of [
    undefined,
    () => 'not an array',
    () => ['some', 'other', 'array'],
    // As long as what was declared, so its length says nothing: of two entries, and of each.
    () => ['wrong', 'wrong'],
    (...items: unknown[]) => items.reverse(),
    (...items: unknown[]) => items.map(() => 'wrong'),
    () => {
      throw new Error('of')
    },
  ]) {
    let made: unknown
    try {
      ;(Array as { of: unknown }).of = stand
      made = read()
    } finally {
      Array.of = of
    }
    assert.deepEqual(made, expected)
  }
})

// The recorded loss, which is the WebAssembly runtime's and no call's: it keeps its handles, its
// references and its scopes in arrays that grow by assignment, so a setter under the number of a
// slot no call has filled yet runs, and one that throws stops the call. The module stays sound
// and the same call answers once the setter is gone. The native addon has no such table.
test('a slot the fallback has never used is assigned under its number (the recorded loss)', () => {
  const wasi =
    (require('../binding.js') as { __napiBindingTarget: string }).__napiBindingTarget !== 'native'
  const [FROM, TO, HELD] = [2_000, 20_000, 4_000]
  const spec = { order: DateOrder.DayMonthYear, am: 'vorm.', pm: 'nachm.' }
  // More declarations alive at once than this process has had: each holds a slot of its own.
  const held = new Set<unknown>()
  let [ran, threw] = [0, 0]
  try {
    for (let at = FROM; at < TO; at++) {
      Object.defineProperty(Object.prototype, String(at), {
        configurable: true,
        set() {
          ran++
          throw new Error(`the setter under ${at}`)
        },
      })
    }
    for (let made = 0; made < HELD; made++) {
      try {
        held.add(DateFormat.declare(spec))
      } catch {
        threw++
      }
    }
  } finally {
    for (let at = FROM; at < TO; at++) delete (Object.prototype as Record<string, unknown>)[at]
  }
  if (wasi) {
    assert.ok(ran > 0 && threw > 0, `${ran} setters ran and ${threw} calls threw`)
  } else {
    assert.deepEqual([ran, threw, held.size], [0, 0, HELD])
  }
  // Sound: the same call answers once the setters are gone.
  assert.ok(DateFormat.declare(spec).ok)
  assert.deepEqual(api.castI32('12'), { ok: true, value: 12 })
})
