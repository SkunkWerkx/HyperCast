// Grouping: a `NumFormat` declares its policy apart from its styles, Western by default, and
// every door that takes a format honours it, the checked doors included.
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const { Constraints, Door, Grouping, NumFormat, NumStyles } = api

type Format = InstanceType<typeof NumFormat>

function declare(spec: Partial<import('../index.js').NumFormatSpec> = {}): Format {
  const declared = NumFormat.declare({
    decimalSep: '.',
    groupSep: ',',
    flags: NumStyles.All,
    ...spec,
  })
  assert.ok(declared.ok, declared.ok ? '' : declared.message)
  return declared.value
}

const detailed = (
  text: import('../index.js').Text,
  format: Format,
  door: import('../index.js').Door = Door.I32,
) => api.castDetailed(door, text, format)

test('the policy constants: Western sets no bit, Indian is bit 9', () => {
  assert.deepEqual({ ...Grouping }, { Western: 0, Indian: 512 })
})

test('a declaration with no grouping is Western; so are invariant() and detect()', () => {
  assert.equal(declare().grouping, Grouping.Western)
  assert.equal(NumFormat.invariant().grouping, Grouping.Western)
  assert.equal(NumFormat.detect().grouping, Grouping.Western)
  assert.equal(declare({ grouping: Grouping.Indian }).grouping, Grouping.Indian)
  // The style mask never carries the policy.
  assert.equal(declare().flags, NumStyles.All)
  assert.equal(declare({ grouping: Grouping.Indian }).flags, NumStyles.All)
  assert.equal(NumFormat.invariant().flags, NumStyles.All)
  assert.equal(NumFormat.detect().flags, NumStyles.All | NumStyles.SeparatorDetect)
})

test('a door given no format reads NumFormat.invariant()', () => {
  for (const text of ['1,234', '12,34', '1234,567', '1,234.5', '1,2345', '12', '']) {
    assert.deepEqual(api.castI32(text), api.castI32(text, NumFormat.invariant()), text)
    assert.deepEqual(api.castF64(text), api.castF64(text, NumFormat.invariant()), text)
    assert.deepEqual(api.castDecimal(text), api.castDecimal(text, NumFormat.invariant()), text)
    assert.deepEqual(
      api.castDetailed(Door.I32, text),
      api.castDetailed(Door.I32, text, NumFormat.invariant()),
      text,
    )
  }
  assert.ok(!api.castI32('12,34').ok)
})

test('Western: groups of three counted from the decimal separator', () => {
  const western = declare()
  for (const [text, value] of [
    ['1,234,567', 1234567],
    ['123,456', 123456],
    ['12,345', 12345],
    ['1,234', 1234],
    ['1234567', 1234567],
  ] as const) {
    assert.deepEqual(api.castI32(text, western), { ok: true, value }, text)
  }
  for (const text of ['12,34', '1234,567', '1,2345', '1,,234', ',123', '123,']) {
    assert.ok(!api.castI32(text, western).ok, text)
  }
  assert.deepEqual(api.castF64('1,234.5', western), { ok: true, value: 1234.5 })
  assert.deepEqual(api.castDecimal('1,234,567.89', western), { ok: true, value: '1234567.89' })
  // Under 'once a numeral groups, every group conforms', and no grouping in the fraction.
  const fraction = detailed('1.234,5', western, Door.F64)
  assert.ok(!fraction.ok)
  assert.equal(fraction.fault.detail, 'groupInFraction')
})

test('the spans of an invalid group, and the leftmost winner', () => {
  const western = declare()
  const at = (text: string) => {
    const verdict = detailed(text, western)
    assert.ok(!verdict.ok, text)
    return [verdict.fault.detail, verdict.fault.offset, verdict.fault.length]
  }
  // An invalid group spans its separator; an oversized leftmost group has none.
  assert.deepEqual(at('12,34'), ['groupSize', 2, 3])
  assert.deepEqual(at('1234,567'), ['groupSize', 0, 4])
  assert.deepEqual(at('1234,5678'), ['groupSize', 0, 4])
  // An empty group spans the separator that made it.
  assert.deepEqual(at(',123'), ['groupMisplaced', 0, 1])
})

test('Indian: three beside the decimal separator and twos beyond it', () => {
  const indian = declare({ grouping: Grouping.Indian })
  assert.deepEqual(api.castI32('12,34,567', indian), { ok: true, value: 1234567 })
  assert.deepEqual(api.castI32('1,23,456', indian), { ok: true, value: 123456 })
  assert.ok(!api.castI32('1,234,567', indian).ok)
  assert.ok(!api.castI32('123,456', indian).ok)
  const refused = detailed('1,234,567', indian)
  assert.ok(!refused.ok)
  assert.equal(refused.fault.detail, 'groupSize')
})

test('withGrouping chooses a policy in both directions, and refuses what is not one', () => {
  const indian = NumFormat.invariant().withGrouping(Grouping.Indian)
  assert.ok(indian.ok)
  assert.equal(indian.value.grouping, Grouping.Indian)
  assert.deepEqual(api.castI32('1,23,456', indian.value), { ok: true, value: 123456 })
  assert.ok(!api.castI32('1,234,567', indian.value).ok)
  const western = indian.value.withGrouping(Grouping.Western)
  assert.ok(western.ok)
  assert.equal(western.value.grouping, Grouping.Western)
  assert.deepEqual(api.castI32('1,234,567', western.value), { ok: true, value: 1234567 })
  assert.deepEqual(api.castI32('1,23,456', western.value), {
    ok: false,
    fault: { reason: 'malformed', offset: 1, length: 3 },
  })
  for (const grouping of [256, 768, 1, 255, 1024, -1, 1.5, '512', null, undefined]) {
    const refused = indian.value.withGrouping(grouping as never)
    assert.ok(!refused.ok, `${grouping}`)
    assert.match(refused.message, /grouping must be a Grouping/)
  }
})

test('a policy does not turn grouping on', () => {
  // Review focus 5: with the Grouping style off, a group separator stays forbidden.
  const off = declare({ flags: 0 })
  for (const grouping of [Grouping.Western, Grouping.Indian]) {
    const format = declare({ flags: 0, grouping })
    const verdict = detailed('1,234', format)
    assert.ok(!verdict.ok)
    assert.equal(verdict.fault.detail, 'unexpectedCharacter')
    assert.deepEqual([verdict.fault.offset, verdict.fault.length], [1, 1])
  }
  assert.ok(!api.castI32('1,234', off).ok)
})

test('separator detection does not let a policy settle an ambiguous 1,234', () => {
  for (const grouping of [Grouping.Western, Grouping.Indian]) {
    const format = declare({ flags: NumStyles.All | NumStyles.SeparatorDetect, grouping })
    const verdict = detailed('1,234', format, Door.F64)
    assert.ok(!verdict.ok, `${grouping}`)
    assert.equal(verdict.fault.detail, 'ambiguousSeparator')
  }
})

test('native digits count as digits, whatever separator stands between groups', () => {
  // Review focus 1: Arabic-Indic digits with U+202F as the group separator.
  const format = declare({ groupSep: '\u202f', digitZero: '\u0660' })
  const ok = api.castDetailed(Door.I32, '\u0661\u202f\u0662\u0663\u0664', format)
  assert.deepEqual(ok, { ok: true, value: 1234 })
  const text = '\u0661\u0662\u202f\u0663\u0664'
  const strings = detailed(text, format)
  assert.ok(!strings.ok)
  assert.equal(strings.fault.detail, 'groupSize')
  // The group is its separator and two digits: three UTF-16 units in the string, seven bytes.
  assert.deepEqual([strings.fault.offset, strings.fault.length], [2, 3])
  const bytes = detailed(new TextEncoder().encode(text), format)
  assert.ok(!bytes.ok)
  assert.deepEqual([bytes.fault.offset, bytes.fault.length], [4, 7])
})

test('currency group separators are judged by the policy too', () => {
  const format = declare({
    flags: NumStyles.All,
    currency: '$',
    currencyGroupSep: '\u00a0',
    currencyDecimalSep: ',',
  })
  assert.deepEqual(api.castI32('$1\u00a0234', format), { ok: true, value: 1234 })
  assert.ok(!api.castI32('$12\u00a034', format).ok)
  const indian = declare({
    flags: NumStyles.All,
    currency: '$',
    currencyGroupSep: '\u00a0',
    currencyDecimalSep: ',',
    grouping: Grouping.Indian,
  })
  assert.ok(!api.castI32('$12\u00a034', indian).ok)
  assert.deepEqual(api.castI32('$1\u00a023\u00a0456', indian), { ok: true, value: 123456 })
  assert.ok(!api.castI32('$1\u00a023\u00a0456', format).ok)
})

test('the checked doors honour the policy, as the 1.0 export would not', () => {
  const constraints = Constraints.declare({ door: Door.I32, maximum: '100000' })
  assert.ok(constraints.ok)
  const western = declare()
  const indian = declare({ grouping: Grouping.Indian })
  const refused = api.castChecked('12,34', constraints.value, western)
  assert.deepEqual(refused, { ok: false, fault: { reason: 'malformed', offset: 2, length: 3 } })
  assert.deepEqual(api.castChecked('1,23,456', constraints.value, indian), {
    ok: false,
    violation: { keyword: 'maximum', value: 123456, offset: 0, length: 8 },
  })
  assert.deepEqual(api.castChecked('12,345', constraints.value, western), {
    ok: true,
    value: 12345,
    adjusted: false,
    enumIndex: null,
  })
  // Refused through the plain checked door and the detailed one alike.
  const twin = api.castCheckedDetailed('12,34', constraints.value, western)
  assert.ok(!twin.ok && 'fault' in twin)
  assert.equal(twin.fault.detail, 'groupSize')
  assert.deepEqual(api.castChecked('12,34', constraints.value), refused)
})

test('declarations: the policy is a Grouping, and flags hold styles only', () => {
  const refusal = (spec: Partial<import('../index.js').NumFormatSpec>) => {
    const declared = NumFormat.declare({ decimalSep: '.', groupSep: ',', flags: 0, ...spec })
    assert.ok(!declared.ok)
    return declared.message
  }
  assert.match(
    refusal({ flags: 256 }),
    /flags must not carry a grouping policy; declare it as grouping/,
  )
  assert.match(refusal({ flags: 768 }), /flags must not carry a grouping policy/)
  assert.match(refusal({ flags: 1024 }), /flags holds a bit no NumStyles member names/)
  assert.match(refusal({ flags: 2 ** 31 }), /flags holds a bit no NumStyles member names/)
  for (const grouping of [256, 768, 1, 255, 257, -1, 1.5, '512', null, true]) {
    assert.match(
      refusal({ grouping: grouping as never }),
      /grouping must be a Grouping/,
      `${grouping}`,
    )
  }
  // The refusal never throws, and a good declaration after it still declares.
  assert.ok(NumFormat.declare({ decimalSep: '.', groupSep: ',', flags: 0 }).ok)
})

test('packed carries the policy in the flags word, for the C ABI to read', () => {
  const word = (format: Format) =>
    new DataView(format.packed.buffer, format.packed.byteOffset).getUint32(4, true)
  assert.equal(NumFormat.invariant().packed.length, 92)
  assert.equal(word(declare()), NumStyles.All)
  assert.equal(word(declare({ grouping: Grouping.Indian })), NumStyles.All | Grouping.Indian)
  assert.equal(word(declare({ grouping: Grouping.Western })), NumStyles.All)
})

test('a declared policy is judged by a 1.1 core', () => {
  // The addon links the core it was built against, so a 1.0 core cannot be loaded under it,
  // and a build against one fails to compile (README, "Versions"). What a load-time check
  // would prove is asserted by behaviour: the Indian policy is read, here, by the core.
  assert.match(api.nativeVersion(), /^\d+\.\d+\.\d+$/)
  assert.deepEqual(api.castDetailed(Door.I32, '1,23,456', declare({ grouping: Grouping.Indian })), {
    ok: true,
    value: 123456,
  })
  const verdict = api.castDetailed(Door.I32, '12,34', declare({ grouping: Grouping.Western }))
  assert.ok(!verdict.ok)
  assert.equal(verdict.fault.detail, 'groupSize')
  assert.ok(!api.castI32('12,34', declare({ grouping: Grouping.Indian })).ok)
})

test("the enum index.js hands out without an addon is the addon's, with the same values", () => {
  const { readFileSync }: typeof import('node:fs') = require('node:fs')
  const source = readFileSync(require.resolve('../index.js'), 'utf8')
  const fallback =
    /const Grouping = native \? native\.Grouping : Object\.freeze\(\{ (.*?) \}\)/.exec(source)
  assert.ok(fallback, 'index.js spells the fallback on one line')
  const named = Object.fromEntries(
    (fallback[1] ?? '').split(', ').map((pair) => {
      const [key, value] = pair.split(': ')
      return [key, Number(value)]
    }),
  )
  assert.deepEqual(named, { Western: Grouping.Western, Indian: Grouping.Indian })
})
