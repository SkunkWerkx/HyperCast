// The verdict as a TypeScript caller holds it. `npm run typecheck` compiles this file, so the
// `@ts-expect-error` lines are assertions too: each must be an error, or tsc fails with
// "Unused '@ts-expect-error' directive". Narrowing on `ok` is what makes `value` and `fault`
// reachable, and a switch over `fault.reason` is exhaustive over exactly three reasons.
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const { castI32, castI64, castDate, optional, NumFormat, NumStyles } = api
type Verdict<T> = import('../index.js').Verdict<T>
type Fault = import('../index.js').Fault
type CastFailure = import('../index.js').CastFailure

// Exhaustive by construction: a fourth reason would make `reason` not `never` here.
function describe(fault: Fault): string {
  switch (fault.reason) {
    case 'empty':
      return 'absent'
    case 'malformed':
      return `malformed at ${fault.offset}`
    case 'outOfRange':
      return `out of range at ${fault.offset}`
    default: {
      const unreachable: never = fault.reason
      return unreachable
    }
  }
}

function unwrap<T>(verdict: Verdict<T>, fallback: T): T {
  return verdict.ok ? verdict.value : fallback
}

test('narrowing on ok reaches the value or the fault, and nothing else', () => {
  const verdict = castI32('(1,234)')
  // @ts-expect-error: value exists only once ok is known to be true
  assert.equal(verdict.value, -1234)
  // @ts-expect-error: fault exists only once ok is known to be false
  assert.equal(verdict.fault, undefined)
  if (verdict.ok) {
    const value: number = verdict.value
    assert.equal(value, -1234)
    // @ts-expect-error: a success carries no fault
    assert.equal(verdict.fault, undefined)
  } else {
    assert.fail(describe(verdict.fault))
  }
  const failed = castI32('12x')
  if (!failed.ok) {
    const reason: CastFailure = failed.fault.reason
    assert.equal(reason, 'malformed')
    assert.equal(describe(failed.fault), 'malformed at 2')
    // @ts-expect-error: a failure carries no value
    assert.equal(failed.value, undefined)
  } else {
    assert.fail('12x cast')
  }
  // The type parameter follows the door: castI64 is a bigint.
  const wide: bigint = unwrap(castI64('9007199254740993'), 0n)
  assert.equal(wide, 9007199254740993n)
  // @ts-expect-error: a number is no fallback for a bigint verdict
  unwrap(castI64('1'), 0)
})

test('reason strings are exactly the three', () => {
  // @ts-expect-error: 'out_of_range' is the corpus's spelling, not the binding's
  const wrong: CastFailure = 'out_of_range'
  assert.equal(wrong, 'out_of_range')
  assert.deepEqual(
    { ...api.CastFailure },
    {
      Empty: 'empty',
      Malformed: 'malformed',
      OutOfRange: 'outOfRange',
    },
  )
})

test('a string span is UTF-16 code units, a byte span is bytes', () => {
  // U+1F600 is two code units and four bytes; the 'x' after it is the malformed byte.
  const text = '😀x'
  const s = castI32(text)
  const b = castI32(new TextEncoder().encode(text))
  assert.ok(!s.ok && !b.ok)
  assert.deepEqual(s.fault, { reason: 'malformed', offset: 0, length: 2 })
  assert.deepEqual(b.fault, { reason: 'malformed', offset: 0, length: 4 })
  assert.equal(text.slice(s.fault.offset, s.fault.offset + s.fault.length), '😀')
  // A non-ASCII prefix moves the string offset by code units, not bytes: U+2212 declared
  // as a minus sign, then a second sign, malformed at byte 3 and code unit 1.
  const signs = NumFormat.declare({
    decimalSep: '.',
    groupSep: ',',
    flags: NumStyles.All,
    minusSign: '\u2212',
  })
  assert.ok(signs.ok)
  const minus = castI32('\u2212-5', signs.value)
  const minusBytes = castI32(new TextEncoder().encode('\u2212-5'), signs.value)
  assert.ok(!minus.ok && !minusBytes.ok)
  assert.deepEqual(minus.fault, { reason: 'malformed', offset: 1, length: 1 })
  assert.deepEqual(minusBytes.fault, { reason: 'malformed', offset: 3, length: 1 })
  // Whitespace around the text is trimmed, and the span still indexes what was passed.
  const padded = castI32('  12x  ')
  assert.ok(!padded.ok)
  assert.equal('  12x  '.slice(padded.fault.offset, padded.fault.offset + padded.fault.length), 'x')
})

test('optional turns an empty fault into null and passes everything else through', () => {
  assert.equal(optional(castI32('   ')), null)
  assert.equal(optional(castI32(null)), null)
  assert.deepEqual(optional(castI32('7')), { ok: true, value: 7 })
  assert.deepEqual(optional(castI32('x')), {
    ok: false,
    fault: { reason: 'malformed', offset: 0, length: 1 },
  })
  const maybe: Verdict<number> | null = optional(castI32(''))
  assert.equal(maybe, null)
  // Never throws, even for something that is not a verdict.
  const loose = optional as (value: unknown) => unknown
  for (const junk of [undefined, null, 1, 'x', {}, { ok: false }, { ok: false, fault: null }]) {
    assert.equal(loose(junk), junk)
  }
})

test('values take JavaScript shapes', () => {
  const date = castDate('1/7/2026', api.DateOrder.DayMonthYear)
  assert.deepEqual(date, { ok: true, value: { year: 2026, month: 7, day: 1 } })
  assert.deepEqual(api.castUuid('{01020304-0506-0708-090A-0B0C0D0E0F10}'), {
    ok: true,
    value: '01020304-0506-0708-090a-0b0c0d0e0f10',
  })
  assert.deepEqual(api.castDecimal('1,234.50'), { ok: true, value: '1234.5' })
  assert.deepEqual(api.castTimestamp('1969-12-31T23:59:59.5Z'), {
    ok: true,
    value: { seconds: -1, nanos: 500000000 },
  })
  assert.deepEqual(api.castTime('15:04:05.123456789'), {
    ok: true,
    value: { hour: 15, minute: 4, second: 5, nanos: 123456789 },
  })
  assert.deepEqual(api.castU64('18446744073709551615'), { ok: true, value: 2n ** 64n - 1n })
  // The format argument is optional: left out, it is the invariant format.
  const euro = NumFormat.declare({ decimalSep: ',', groupSep: '.', flags: NumStyles.All })
  assert.ok(euro.ok)
  assert.deepEqual(api.castF64('1.234,5', euro.value), { ok: true, value: 1234.5 })
  assert.deepEqual(api.castF64('1,234.5'), { ok: true, value: 1234.5 })
})
