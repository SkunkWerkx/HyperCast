// The two declarations: what they accept, what they refuse and how they say so (a refusal,
// never an exception), and what a declared format reports back.
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const { DateFormat, DateOrder, NumFormat, NumStyles } = api
type NumFormatSpec = import('../index.js').NumFormatSpec

const base: NumFormatSpec = { decimalSep: '.', groupSep: ',', flags: NumStyles.All }

function refused(spec: unknown): string {
  const declared = NumFormat.declare(spec as NumFormatSpec)
  assert.ok(!declared.ok, JSON.stringify(spec))
  return declared.message
}

test('a NumFormat declaration refuses what the core cannot read, naming the rule', () => {
  assert.match(refused({ ...base, decimalSep: ',' }), /must differ; both are ","/)
  assert.match(refused({ ...base, decimalSep: '' }), /^Decimal separator must be 1 to 8/)
  assert.match(refused({ ...base, groupSep: '123456789' }), /^Group separator must be 1 to 8/)
  assert.match(refused({ ...base, groupSep: '1' }), /no ASCII digit/)
  assert.match(refused({ ...base, minusSign: '- ' }), /^Minus sign .* whitespace/)
  assert.match(refused({ ...base, percent: ' %' }), /^Percent symbol/)
  assert.match(refused({ ...base, currency: 'US $' }), /^Currency symbol/)
  assert.match(refused({ ...base, currency: '12345678901234567' }), /^Currency symbol/)
  assert.match(refused({ ...base, currencyDecimalSep: ',' }), /^Currency decimal and group/)
  assert.match(refused({ ...base, digitZero: '0' }), /^Digit zero/)
  assert.match(refused({ ...base, digitZero: '٠١' }), /^Digit zero/)
  assert.match(refused({ ...base, flags: -1 }), /^flags must be a whole number/)
  assert.match(refused({ ...base, flags: 1.5 }), /^flags must be a whole number/)
  assert.match(refused({ ...base, flags: '95' }), /^flags must be a whole number/)
  assert.match(refused({ groupSep: ',', flags: 0 }), /^decimalSep is required/)
  assert.match(refused({ ...base, currency: 5 }), /^currency must be a string/)
  assert.match(refused(null), /must be an object/)
})

test('a declared NumFormat reports what it declares', () => {
  const declared = NumFormat.declare({
    decimalSep: '٫',
    groupSep: '٬',
    flags: NumStyles.All | NumStyles.TrailingSign,
    currency: 'kr.',
    currencyDecimalSep: '.',
    minusSign: '−',
    plusSign: '＋',
    percent: '٪',
    digitZero: '٠',
  })
  assert.ok(declared.ok)
  const f = declared.value
  assert.equal(f.decimalSep, '٫')
  assert.equal(f.groupSep, '٬')
  assert.equal(f.flags, 95 | 128)
  assert.equal(f.currency, 'kr.')
  assert.equal(f.currencyDecimalSep, '.')
  assert.equal(f.currencyGroupSep, '')
  assert.equal(f.minusSign, '−')
  assert.equal(f.plusSign, '＋')
  assert.equal(f.percent, '٪')
  assert.equal(f.digitZero, '٠')
  assert.deepEqual(api.castI32('٫٠', f), {
    ok: false,
    fault: { reason: 'malformed', offset: 0, length: 1 },
  })
  assert.deepEqual(api.castI32('١٬٢٣٤', f), { ok: true, value: 1234 })
})

test('the invariant and detection formats, and the packed layout', () => {
  const invariant = NumFormat.invariant()
  assert.equal(invariant.decimalSep, '.')
  assert.equal(invariant.groupSep, ',')
  assert.equal(invariant.flags, NumStyles.All)
  assert.equal(NumFormat.detect().flags, NumStyles.All | NumStyles.SeparatorDetect)
  assert.deepEqual(api.castF64('1.234,5', NumFormat.detect()), { ok: true, value: 1234.5 })
  assert.deepEqual(api.castF64('1,000', NumFormat.detect()), {
    ok: false,
    fault: { reason: 'malformed', offset: 1, length: 1 },
  })
  // The core's size-tagged layout: the size, the flags, the digit zero, then the symbols.
  const packed = invariant.packed
  assert.equal(packed.length, 92)
  const view = new DataView(packed.buffer, packed.byteOffset, packed.byteLength)
  assert.equal(view.getUint32(0, true), 92)
  assert.equal(view.getUint32(4, true), NumStyles.All)
  assert.equal(view.getUint32(8, true), 0)
  assert.deepEqual([packed[12], packed[20]], [0x2e, 1])
  assert.deepEqual([packed[21], packed[29]], [0x2c, 1])
})

test('a DateFormat declaration refuses an undeclared order and a bad designator', () => {
  const refuse = (spec: unknown) => {
    const declared = DateFormat.declare(spec as { order: number })
    assert.ok(!declared.ok, JSON.stringify(spec))
    return declared.message
  }
  assert.match(refuse({ order: 4 }), /^order must be a DateOrder/)
  assert.match(refuse({}), /^order must be a DateOrder/)
  assert.match(refuse({ order: DateOrder.DayMonthYear, am: ' vorm.' }), /^Day period am/)
  assert.match(refuse({ order: DateOrder.DayMonthYear, pm: 'p1' }), /^Day period pm/)
  assert.match(refuse({ order: DateOrder.DayMonthYear, pm: 'x'.repeat(17) }), /^Day period pm/)
  const declared = DateFormat.declare({ order: DateOrder.DayMonthYear, am: 'a. m.', pm: 'p. m.' })
  assert.ok(declared.ok)
  assert.equal(declared.value.order, DateOrder.DayMonthYear)
  assert.equal(declared.value.am, 'a. m.')
  assert.equal(declared.value.pm, 'p. m.')
  assert.deepEqual(api.castDatetime('7/1/2026 3:04 p. m.', declared.value), {
    ok: true,
    value: { year: 2026, month: 1, day: 7, hour: 15, minute: 4, second: 0, nanos: 0 },
  })
})

test('a lone surrogate in a declared symbol is refused, never declared as a replacement', () => {
  const whole = /must be whole code points, not surrogate halves$/
  assert.match(refused({ ...base, currency: '\ud800' }), /^currency must be whole code points/)
  assert.match(refused({ ...base, decimalSep: 'x\udc00' }), whole)
  assert.match(refused({ ...base, minusSign: '\udbff-' }), /^minusSign /)
  assert.match(refused({ ...base, digitZero: '\ud800' }), /^digitZero /)
  const refuse = (spec: unknown) => {
    const declared = DateFormat.declare(spec as { order: number })
    assert.ok(!declared.ok, JSON.stringify(spec))
    return declared.message
  }
  assert.match(refuse({ order: DateOrder.DayMonthYear, am: 'vorm\ud800' }), /^am /)
  assert.match(refuse({ order: DateOrder.DayMonthYear, pm: '\udc00' }), /^pm /)
  // A real U+FFFD and a real pair are whole code points, and declare as they are.
  const replacement = NumFormat.declare({ ...base, currency: '�' })
  assert.ok(replacement.ok)
  assert.equal(replacement.value.currency, '�')
  const pair = NumFormat.declare({ ...base, currency: '💶' })
  assert.ok(pair.ok)
  assert.equal(pair.value.currency, '💶')
})
