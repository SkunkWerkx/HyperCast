// Validation constraints as a TypeScript caller holds them: the declaration (its refusals and
// what it reports back), the checked verdict and its narrowing, the door a declaration is bound
// to, UTF-16 input that has no UTF-8 form, and the plain verdict's shape, which none of this
// may change. `bun run typecheck` compiles this file, so the `@ts-expect-error` lines are
// assertions too.
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { existsSync, readFileSync } = require('node:fs') as typeof import('node:fs')
const { endianness } = require('node:os') as typeof import('node:os')
const path: typeof import('node:path') = require('node:path')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const { Constraints, Door, Keyword, NumFormat, NumStyles, Rounding, castChecked, castText } = api
type CheckedVerdict<T> = import('../index.js').CheckedVerdict<T>
type ConstraintsSpec = import('../index.js').ConstraintsSpec
type Violation = import('../index.js').Violation
type KeywordName = import('../index.js').Keyword

function declare(spec: ConstraintsSpec): InstanceType<typeof Constraints> {
  const declared = Constraints.declare(spec)
  assert.ok(declared.ok, declared.ok ? '' : declared.message)
  return declared.value
}

function refusal(spec: unknown): string {
  const declared = Constraints.declare(spec as ConstraintsSpec)
  assert.ok(
    !declared.ok,
    JSON.stringify(spec, (_k, v) => (typeof v === 'bigint' ? `${v}n` : v)),
  )
  return declared.message
}

const contract = { ok: false, fault: { reason: 'malformed', offset: 0, length: 0 } }

// Exhaustive over today's keywords, and open: the default arm is what a ninth one reaches.
function describe(violation: Violation): string {
  switch (violation.keyword) {
    case 'minimum':
    case 'exclusiveMinimum':
      return 'too small'
    case 'maximum':
    case 'exclusiveMaximum':
      return 'too large'
    case 'multipleOf':
      return 'off the step'
    case 'minLength':
    case 'maxLength':
      return 'wrong length'
    case 'enum':
      return 'not a listed value'
    default:
      return `breaks ${violation.keyword}`
  }
}

test('the README example: latitude, euro cash and a sex code', () => {
  const latitude = declare({
    door: Door.F64,
    minimum: '-90',
    maximum: '90',
    multipleOf: '0.000001',
    rounding: Rounding.Midpoint,
  })
  assert.deepEqual(castChecked('45.12345678', latitude), {
    ok: true,
    value: 45.123457,
    adjusted: true,
    enumIndex: null,
  })
  assert.deepEqual(castChecked('91', latitude), {
    ok: false,
    violation: { keyword: 'maximum', offset: 0, length: 2 },
  })

  const euro = NumFormat.declare({ decimalSep: ',', groupSep: '.', flags: NumStyles.All })
  assert.ok(euro.ok)
  const cash = declare({ door: Door.Decimal, minimum: '0', multipleOf: '0.05' })
  assert.deepEqual(castChecked('12,35', cash, euro.value), {
    ok: true,
    value: '12.35',
    adjusted: false,
    enumIndex: null,
  })
  const coins = castChecked('12,37', cash, euro.value)
  assert.ok(!coins.ok && 'violation' in coins)
  assert.equal(describe(coins.violation), 'off the step')

  const sex = declare({ door: Door.Text, trim: true, enum: ['Female', 'Male', 'Other'] })
  assert.deepEqual(castText(' MALE ', sex), {
    ok: true,
    value: 'Male',
    adjusted: false,
    enumIndex: 1,
  })
  assert.deepEqual(castText('Mail', sex), {
    ok: false,
    violation: { keyword: 'enum', offset: 0, length: 4 },
  })
})

test('narrowing reaches the value, the fault or the violation', () => {
  const small = declare({ door: Door.I32, minimum: '10' })
  const verdict: CheckedVerdict<number | bigint | string> = castChecked('9', small)
  // @ts-expect-error: value exists only once ok is known to be true
  assert.equal(verdict.value, undefined)
  if (verdict.ok) {
    assert.fail(`9 passed as ${verdict.value}`)
  } else if ('violation' in verdict) {
    const keyword: KeywordName = verdict.violation.keyword
    assert.equal(keyword, Keyword.Minimum)
    // @ts-expect-error: a violation carries no fault
    assert.equal(verdict.fault, undefined)
  } else {
    assert.fail(`9 faulted ${verdict.fault.reason}`)
  }
  // The value's type follows the door, which the declaration holds: I32 is a number.
  const ok = castChecked('12', small)
  assert.ok(ok.ok && typeof ok.value === 'number')
  const value: number = ok.value
  assert.equal(value, 12)
  // @ts-expect-error: a bound is never a number, which holds most decimals approximately
  const spec: ConstraintsSpec = { door: Door.F64, maximum: 0.1 }
  assert.match(refusal(spec), /^maximum must be a string or a bigint, not a number/)
  // A plain castText is the plain verdict, with no checked fields to narrow through.
  const plain = castText('x')
  assert.ok(plain.ok)
  // @ts-expect-error: the plain verdict has no adjusted
  assert.equal(plain.adjusted, undefined)
})

test('the plain verdict and fault keep their shapes', () => {
  assert.deepEqual(Object.keys(api.castI32('7')), ['ok', 'value'])
  const failed = api.castI32('x')
  assert.deepEqual(Object.keys(failed), ['ok', 'fault'])
  assert.ok(!failed.ok)
  assert.deepEqual(Object.keys(failed.fault), ['reason', 'offset', 'length'])
  assert.deepEqual(Object.keys(castText('x')), ['ok', 'value'])
  assert.deepEqual(Object.keys(castText('')), ['ok', 'fault'])
  // The checked shapes, beside them.
  const step = declare({ door: Door.I32, multipleOf: '2' })
  assert.deepEqual(Object.keys(castChecked('4', step)), ['ok', 'value', 'adjusted', 'enumIndex'])
  assert.deepEqual(Object.keys(castChecked('x', step)), ['ok', 'fault'])
  const broken = castChecked('3', step)
  assert.deepEqual(Object.keys(broken), ['ok', 'violation'])
  assert.ok(!broken.ok && 'violation' in broken)
  assert.deepEqual(Object.keys(broken.violation), ['keyword', 'offset', 'length'])
})

test('a refused declaration names the code and the rule; a broken one, the binding contract', () => {
  assert.equal(
    refusal({ door: Door.I32, minLength: 1 }),
    'Constraints refused (code 1): a constraint the door does not take.',
  )
  assert.equal(
    refusal({ door: Door.Text, enum: ['Male', 'MALE'] }),
    'Constraints refused (code 10): a duplicate enum entry.',
  )
  assert.equal(
    refusal({ door: Door.I32, multipleOf: '2', rounding: 9 }),
    'Constraints refused (code 11): a malformed constraints layout.',
  )
  // Leaving enum out declares none; an empty one is declared, and refused.
  for (const door of [Door.I32, Door.Text]) {
    assert.equal(
      refusal({ door, enum: [] }),
      'Constraints refused (code 12): an enum with no entries.',
    )
  }
  assert.equal(api.castConstraintsCheck({ door: Door.F64, enum: [] }), 1)
  for (const spec of [
    { door: Door.Text, enum: ['Male', ''] },
    { door: Door.Text, trim: true, enum: [' Male '] },
  ]) {
    assert.equal(
      refusal(spec),
      'Constraints refused (code 13): a text enum entry no input can match.',
    )
  }
  const spaced = declare({ door: Door.Text, enum: [' Male '] })
  assert.deepEqual(castText(' MALE ', spaced), {
    ok: true,
    value: ' Male ',
    adjusted: false,
    enumIndex: 0,
  })
  // The flags word, in the native byte order the core reads: HAS_ENUM (256) whenever declared.
  const flags = (packed: Uint8Array) =>
    new DataView(packed.buffer, packed.byteOffset, packed.byteLength).getUint32(
      4,
      endianness() === 'LE',
    )
  assert.equal(flags(spaced.packed) & 256, 256)
  assert.equal(flags(declare({ door: Door.Text }).packed) & 256, 0)
  assert.equal(declare({ door: Door.Text }).enum, null)
  assert.equal(api.castConstraintsCheck({ door: Door.I32, minLength: 1 }), 1)
  assert.equal(api.castConstraintsCheck({ door: Door.I32, minimum: '1' }), 0)
  // What the binding cannot lay out is -1 to castConstraintsCheck, and worded by declare.
  for (const [spec, message] of [
    [{ door: Door.I32, minimum: 1 }, /^minimum must be a string or a bigint, not a number/],
    [{ door: Door.I32, minimum: null }, /^minimum must be a string or a bigint$/],
    [{ door: Door.Text, minLength: true }, /^minLength must be a whole number/],
    [{ door: Door.Text, maxLength: '3' }, /^maxLength must be a whole number/],
    [{ door: Door.Text, maxLength: 1.5 }, /^maxLength must be a whole number/],
    [{ door: Door.Text, trim: 1 }, /^trim must be a boolean/],
    [{ door: Door.Text, enum: 'Male' }, /^enum must be an array/],
    [{ door: Door.Text, enum: [1] }, /^enum entries at Door.Text must be strings; entry 0/],
    [{ door: Door.I32, enum: [7] }, /^enum entry 0 must be a string or a bigint, not a number/],
    [{ door: Door.I32, rounding: 'midpoint' }, /^rounding must be a Rounding/],
    [{ door: 1 }, /^door must be a Door/],
    [{ door: 14 }, /^door must be a Door/],
    [{}, /^door must be a Door/],
    [null, /must be an object/],
  ] as const) {
    assert.match(refusal(spec), message)
    assert.equal(api.castConstraintsCheck(spec as unknown as ConstraintsSpec), -1)
  }
})

test('bounds and integer entries are decimal strings or bigints, converted by the core', () => {
  const wide = declare({ door: Door.U64, maximum: 18446744073709551615n, enum: [1n, '2', '3.0'] })
  assert.equal(wide.maximum, '18446744073709551615')
  assert.deepEqual(wide.enum, [1n, '2', '3.0'])
  // An integer enum is answered by the value, so no index.
  assert.deepEqual(castChecked('3', wide), {
    ok: true,
    value: 3n,
    adjusted: false,
    enumIndex: null,
  })
  assert.match(refusal({ door: Door.U64, maximum: 18446744073709551616n }), /code 8/)
  assert.match(refusal({ door: Door.I64, minimum: -(2n ** 200n) }), /code 8/)
  assert.match(refusal({ door: Door.F64, maximum: '0x10' }), /code 8/)
  assert.deepEqual(api.castDeclaredDecimal('1e-6'), { ok: true, value: '0.000001' })
  assert.deepEqual(api.castDeclaredDecimal('-0.0'), { ok: true, value: '0' })
  assert.match(
    (api.castDeclaredDecimal as (text: unknown) => { ok: boolean; message?: string })(5)
      .message as string,
    /^text must be a string or a Uint8Array/,
  )
})

test('a declaration reports what it declares, and its packed layout', () => {
  const c = declare({ door: Door.Decimal, exclusiveMinimum: '0', maximum: '1.50' })
  assert.equal(c.door, Door.Decimal)
  assert.equal(c.minimum, null)
  assert.equal(c.exclusiveMinimum, '0')
  assert.equal(c.maximum, '1.5')
  assert.equal(c.multipleOf, null)
  assert.equal(c.rounding, Rounding.Strict)
  assert.equal(c.minLength, null)
  assert.equal(c.trim, false)
  assert.equal(c.enum, null)
  const packed = c.packed
  assert.equal(packed.length, 72)
  const view = new DataView(packed.buffer, packed.byteOffset, packed.byteLength)
  const native = new Uint8Array(new Uint32Array([1]).buffer)[0] === 1
  assert.equal(view.getUint32(0, native), 72)
  assert.equal(view.getUint32(4, native), 2 | 4) // HAS_EXCLUSIVE_MINIMUM | HAS_MAXIMUM
  assert.equal(view.getUint32(40, native), 15) // upper (after lower, at 24): magnitude 15
  assert.equal(packed[52], 1) // its scale
  assert.equal(c.packedEnum.length, 0)
  const sex = declare({ door: Door.Text, minLength: 1, maxLength: 6, enum: ['Male', 'Ä'] })
  assert.deepEqual(sex.enum, ['Male', 'Ä'])
  assert.deepEqual([sex.minLength, sex.maxLength], [1, 6])
  assert.deepEqual([...sex.packedEnum], [4, 0, 0, 0, 77, 97, 108, 101, 2, 0, 0, 0, 0xc3, 0x84])
})

test('the layouts are the sizes the C header carries', (t) => {
  // The binding's own sizes, asserted on every leg: the packed declaration this package lays
  // out, and the RawCheck and RawSpan src/lib.rs pins at these sizes at compile time.
  const sizes = { HyperCastRawConstraints: 72, HyperCastRawCheck: 12, HyperCastRawSpan: 8 }
  assert.equal(declare({ door: Door.I32 }).packed.length, sizes.HyperCastRawConstraints)
  // The header is compared only where it is reachable: an installed copy, or a suite run from
  // the package directory alone (the Forge's musl legs stage no include/), has none.
  const header = path.resolve(__dirname, '..', '..', 'include', 'hypercast.h')
  if (!existsSync(header)) {
    t.diagnostic('no include/hypercast.h beside this package: only the literal sizes were checked')
    return
  }
  const text = readFileSync(header, 'utf8')
  const size = (name: string) => Number(new RegExp(`#define ${name}_SIZE (\\d+)`).exec(text)?.[1])
  for (const [name, bytes] of Object.entries(sizes)) {
    assert.equal(
      bytes,
      size(name),
      `${name}: this binding's size is ${bytes}, the header's ${size(name)}`,
    )
  }
})

test('a failed declaration never means "no rules": a missing value is a contract violation', () => {
  // A refused declaration has no `.value`; handed on anyway, it must not cast unjudged.
  const refused = Constraints.declare({ door: Door.Text, minLength: 5, maxLength: 2 })
  assert.ok(!refused.ok)
  const missing = (refused as unknown as { value?: InstanceType<typeof Constraints> }).value
  const loose = castText as (...args: unknown[]) => unknown
  assert.deepEqual(loose('x', missing), contract)
  assert.deepEqual(loose('x', undefined), contract)
  assert.deepEqual(loose('x', null), contract)
  assert.deepEqual(loose(null, undefined), contract)
  // One argument is the plain door, and only one argument is.
  assert.deepEqual(castText('x'), { ok: true, value: 'x' })
  assert.deepEqual(loose(), { ok: false, fault: { reason: 'empty', offset: 0, length: 0 } })
  // The checked numeric doors take no optional declaration: a missing one is refused too.
  const checked = castChecked as (...args: unknown[]) => unknown
  const fromNumber = api.castCheckedFromNumber as (...args: unknown[]) => unknown
  assert.deepEqual(checked('1', missing), contract)
  assert.deepEqual(checked('1', null), contract)
  assert.deepEqual(checked('1', undefined, NumFormat.invariant()), contract)
  assert.deepEqual(fromNumber(1, missing), contract)
  assert.deepEqual(fromNumber(1, null), contract)
  assert.deepEqual(fromNumber(1), contract)
})

test('the raw entry counts arguments too, so the browser entry agrees with the package', () => {
  // binding.js is what the WebAssembly fallback's own entries export (under
  // NAPI_RS_FORCE_WASI=error it is that module): the addon itself picks plain or constrained by
  // the number of arguments, with no wrapper in index.js to do it.
  const raw = require('../binding.js').castText as (...args: unknown[]) => unknown
  assert.equal(raw, castText)
  assert.deepEqual(raw('x', undefined), contract)
  assert.deepEqual(raw('x', null), contract)
  assert.deepEqual(raw('x', 7), contract)
  assert.deepEqual(raw('x', undefined, undefined), contract)
  assert.deepEqual(raw('x'), { ok: true, value: 'x' })
  assert.deepEqual(raw(), { ok: false, fault: { reason: 'empty', offset: 0, length: 0 } })
  const sex = declare({ door: Door.Text, trim: true, enum: ['Male'] })
  assert.deepEqual(raw(' MALE ', sex), { ok: true, value: 'Male', adjusted: false, enumIndex: 0 })
  assert.equal(raw.name, 'castText')
})

test('a declaration is bound to its door; any other door is a contract violation', () => {
  const text = declare({ door: Door.Text, maxLength: 3 })
  const number = declare({ door: Door.I32, maximum: '3' })
  const loose = castChecked as (...args: unknown[]) => unknown
  assert.deepEqual(loose('1', text), contract)
  assert.deepEqual(api.castCheckedFromNumber(1, text), contract)
  assert.deepEqual((castText as (...args: unknown[]) => unknown)('1', number), contract)
  assert.deepEqual(loose('1', {}), contract)
  assert.deepEqual(loose('1', NumFormat.invariant()), contract)
  assert.deepEqual(loose('1'), contract)
  assert.deepEqual(loose('1', number, text), contract)
  assert.deepEqual(
    (api.castCheckedFromNumber as (...a: unknown[]) => unknown)('1', number),
    contract,
  )
  assert.deepEqual(loose(null, number), {
    ok: false,
    fault: { reason: 'empty', offset: 0, length: 0 },
  })
})

test('text keeps the caller its own strings, spans in UTF-16 code units', () => {
  const sex = declare({ door: Door.Text, trim: true, enum: ['Female', 'Male', 'Other'] })
  const matched = castText(' OTHER ', sex)
  assert.ok(matched.ok)
  assert.equal(matched.value, 'Other')
  const trimmed = declare({ door: Door.Text, trim: true, maxLength: 3 })
  assert.deepEqual(castText('  😀ab  ', trimmed), {
    ok: true,
    value: '😀ab',
    adjusted: false,
    enumIndex: null,
  })
  // 👍🏽 is two code points, four code units, eight bytes.
  const one = declare({ door: Door.Text, maxLength: 1 })
  assert.deepEqual(castText('👍🏽', one), {
    ok: false,
    violation: { keyword: 'maxLength', offset: 0, length: 4 },
  })
  assert.deepEqual(castText(new TextEncoder().encode('👍🏽'), one), {
    ok: false,
    violation: { keyword: 'maxLength', offset: 0, length: 8 },
  })
  // Plain text, with no constraints, is the text itself.
  assert.deepEqual(castText('héllo'), { ok: true, value: 'héllo' })
  assert.deepEqual(castText(new Uint8Array([0x68, 0xc3, 0xa9])), { ok: true, value: 'hé' })
  assert.deepEqual(castText(new Uint8Array([0x61, 0xff])), {
    ok: false,
    fault: { reason: 'malformed', offset: 1, length: 1 },
  })
  assert.deepEqual(castText(' '), { ok: true, value: ' ' })
  assert.deepEqual(castText(null), { ok: false, fault: { reason: 'empty', offset: 0, length: 0 } })
})

test('a lone surrogate never becomes U+FFFD: refused as an entry, malformed as input', () => {
  // As an entry it has no UTF-8 form, which the core refuses (code 9).
  assert.equal(
    refusal({ door: Door.Text, enum: ['Male', '\ud800'] }),
    'Constraints refused (code 9): an enum entry that is not UTF-8.',
  )
  assert.equal(api.castConstraintsCheck({ door: Door.Text, enum: ['\udfff'] }), 9)
  // U+FFFD itself is an ordinary entry, so the two were never confused.
  declare({ door: Door.Text, enum: ['�'] })
  // As input it is malformed at its own code unit, checked and plain alike.
  const any = declare({ door: Door.Text, maxLength: 10 })
  const fault = { ok: false, fault: { reason: 'malformed', offset: 3, length: 1 } }
  assert.deepEqual(castText('a😀\ud800b', any), fault)
  assert.deepEqual(castText('a😀\ud800b'), fault)
  assert.deepEqual(castText('a😀\udc00', any), fault)
  const replaced = declare({ door: Door.Text, enum: ['�'] })
  assert.deepEqual(castText('\ud800', replaced), {
    ok: false,
    fault: { reason: 'malformed', offset: 0, length: 1 },
  })
  // At a numeric door it is malformed there too, in code units.
  const n = declare({ door: Door.I32, maximum: '9' })
  assert.deepEqual(castChecked('1\ud800', n), {
    ok: false,
    fault: { reason: 'malformed', offset: 1, length: 1 },
  })
  // And as a bound's text, the core refuses it as any other non-notation (code 8).
  assert.match(refusal({ door: Door.I32, maximum: '1\ud800' }), /code 8/)
})

test("a lone surrogate is malformed at every plain door too, the checked door's fault", () => {
  // Declared as the currency symbol, U+FFFD would have read a replaced surrogate as data.
  const replacement = NumFormat.declare({
    decimalSep: '.',
    groupSep: ',',
    flags: NumStyles.All,
    currency: '\ufffd',
  })
  assert.ok(replacement.ok)
  const format = replacement.value
  assert.deepEqual(api.castI32('\ud8005', format), {
    ok: false,
    fault: { reason: 'malformed', offset: 0, length: 1 },
  })
  assert.deepEqual(api.castI32('\ufffd5', format), { ok: true, value: 5 })
  const i32 = declare({ door: Door.I32 })
  for (const text of ['\ud8005', '5\udfff', '12\ud8003', '\ufffd1\udc00', 'é€\udc00x']) {
    const plain = api.castI32(text, format)
    assert.ok(!plain.ok && plain.fault.reason === 'malformed', text)
    assert.deepEqual(plain, castChecked(text, i32, format), text)
  }
  assert.deepEqual(api.castDecimal('12\ud8003'), {
    ok: false,
    fault: { reason: 'malformed', offset: 2, length: 1 },
  })
  for (const door of [api.castBool, api.castUuid, api.castTimestamp, api.castDuration]) {
    const verdict = door('\ud800')
    assert.ok(!verdict.ok && verdict.fault.reason === 'malformed', door.name)
  }
})

test('a number already decoded keeps its sign, and zero has one form', () => {
  const step = declare({ door: Door.F64, multipleOf: '0.1', rounding: Rounding.Midpoint })
  assert.deepEqual(api.castCheckedFromNumber(0.1 + 0.2, step), {
    ok: true,
    value: 0.3,
    adjusted: true,
    enumIndex: null,
  })
  const strict = declare({ door: Door.F64, multipleOf: '0.1' })
  assert.deepEqual(api.castCheckedFromNumber(0.1 + 0.2, strict), {
    ok: false,
    violation: { keyword: 'multipleOf', offset: 0, length: 0 },
  })
  const zero = api.castCheckedFromNumber(-0, declare({ door: Door.F64, minimum: '0' }))
  assert.ok(zero.ok && Object.is(zero.value, 0))
  const fine = declare({ door: Door.F64, multipleOf: '0.000001', rounding: Rounding.Midpoint })
  const rounded = castChecked('-0.0000004', fine)
  assert.ok(rounded.ok && Object.is(rounded.value, 0) && rounded.adjusted)
})

test('optional reads a checked verdict too: empty is null, a violation is not absent', () => {
  const c = declare({ door: Door.I32, maximum: '3' })
  assert.equal(api.optional(castChecked(' ', c)), null)
  const broken = api.optional(castChecked('4', c))
  assert.ok(broken !== null && !broken.ok && 'violation' in broken)
})

test('the keyword names, and the fallback literals', () => {
  assert.deepEqual(
    { ...Keyword },
    {
      Minimum: 'minimum',
      ExclusiveMinimum: 'exclusiveMinimum',
      Maximum: 'maximum',
      ExclusiveMaximum: 'exclusiveMaximum',
      MultipleOf: 'multipleOf',
      MinLength: 'minLength',
      MaxLength: 'maxLength',
      Enum: 'enum',
    },
  )
  assert.equal(describe({ keyword: 'format', offset: 0, length: 0 }), 'breaks format')
})
