// Date options: opt-in shapes (a compact date, a space for the T, an hour-only offset), a
// pivot for two-digit years and caller-built month names, accepted by the date doors (the
// plain ones, which keep their verdicts, and `castDetailed`, which says what failed).
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const { DateFormat, DateOptions, DateOrder, DateStyles, Door, MonthNames } = api

const ENGLISH: [number, string][] = [
  [1, 'Jan'],
  [2, 'Feb'],
  [3, 'Mar'],
  [4, 'Apr'],
  [5, 'May'],
  [6, 'Jun'],
  [7, 'Jul'],
  [8, 'Aug'],
  [9, 'Sep'],
  [9, 'Sept'],
  [10, 'Oct'],
  [11, 'Nov'],
  [12, 'Dec'],
]
const FRENCH: [number, string][] = [
  [1, 'janv.'],
  [2, 'févr.'],
  [3, 'mars'],
  [4, 'avr.'],
  [5, 'mai'],
  [6, 'juin'],
  [7, 'juil.'],
  [8, 'août'],
  [9, 'sept.'],
  [10, 'oct.'],
  [11, 'nov.'],
  [12, 'déc.'],
]

function names(entries: [number, string][]): InstanceType<typeof MonthNames> {
  const declared = MonthNames.declare(entries)
  assert.ok(declared.ok, declared.ok ? '' : declared.message)
  return declared.value
}
function options(
  spec: Partial<import('../index.js').DateOptionsSpec> = {},
): InstanceType<typeof DateOptions> {
  const declared = DateOptions.declare(spec)
  assert.ok(declared.ok, declared.ok ? '' : declared.message)
  return declared.value
}
const date = (year: number, month: number, day: number) => ({
  ok: true,
  value: { year, month, day },
})

test('the style constants are the core bits, and COMMON_FORMS is the three together', () => {
  assert.deepEqual(
    { ...DateStyles },
    { None: 0, CompactDate: 1, SpaceSeparator: 2, HourOffset: 4, CommonForms: 7 },
  )
  assert.equal(
    DateStyles.CommonForms,
    DateStyles.CompactDate | DateStyles.SpaceSeparator | DateStyles.HourOffset,
  )
  const common = DateOptions.commonForms()
  assert.equal(common.flags, DateStyles.CommonForms)
  assert.equal(common.pivot, 0)
  assert.equal(common.months, null)
})

test('each style opens its one shape at its one door', () => {
  const compact = options({ flags: DateStyles.CompactDate })
  assert.deepEqual(api.castDate('20260107', undefined, compact), date(2026, 1, 7))
  assert.ok(!api.castDate('20260107').ok)
  assert.ok(!api.castDate('20261301', undefined, compact).ok)
  const space = options({ flags: DateStyles.SpaceSeparator })
  const instant = { ok: true, value: { seconds: 1767798245, nanos: 0 } }
  assert.deepEqual(api.castTimestamp('2026-01-07 15:04:05Z', space), instant)
  assert.deepEqual(api.castTimestamp('2026-01-07T15:04:05Z', space), instant)
  assert.ok(!api.castTimestamp('2026-01-07 15:04:05+00', space).ok)
  const hours = options({ flags: DateStyles.HourOffset })
  assert.deepEqual(api.castTimestamp('2026-01-07T15:04:05+00', hours), instant)
  assert.deepEqual(api.castTimestamp('2026-01-07T20:04:05+05', hours), instant)
  assert.ok(!api.castTimestamp('2026-01-07 15:04:05Z', hours).ok)
})

test('COMMON_FORMS works at every date door unchanged', () => {
  const common = DateOptions.commonForms()
  assert.deepEqual(api.castTimestamp('2026-01-07 15:04:05+00', common), {
    ok: true,
    value: { seconds: 1767798245, nanos: 0 },
  })
  assert.deepEqual(api.castDate('20260107', undefined, common), date(2026, 1, 7))
  // A style a door does not read is ignored: 20260107 is no date at an ordered door.
  const ordered = api.castDate('20260107', DateOrder.YearMonthDay, common)
  assert.ok(!ordered.ok)
  assert.ok(api.castDatetime('7/1/2026 15:04', DateOrder.DayMonthYear, common).ok)
  // It never supplies a pivot or a vocabulary.
  assert.ok(!api.castDate('7/1/26', DateOrder.DayMonthYear, common).ok)
  assert.ok(!api.castDate('7-Jan-2026', DateOrder.DayMonthYear, common).ok)
})

test('a pivot reads two-digit years in a 100-year window ending at it', () => {
  const pivot2029 = options({ pivot: 2029 })
  const dmy = DateOrder.DayMonthYear
  assert.deepEqual(api.castDate('7/1/26', dmy, pivot2029), date(2026, 1, 7))
  assert.deepEqual(api.castDate('7/1/29', dmy, pivot2029), date(2029, 1, 7))
  assert.deepEqual(api.castDate('7/1/30', dmy, pivot2029), date(1930, 1, 7))
  assert.deepEqual(api.castDate('7/1/2026', dmy, pivot2029), date(2026, 1, 7))
  // The window edges: 100 reads 00 as year 100 and 01 as year 1; 9999 reads 99 as 9999.
  assert.deepEqual(api.castDate('1/1/00', dmy, options({ pivot: 100 })), date(100, 1, 1))
  assert.deepEqual(api.castDate('1/1/01', dmy, options({ pivot: 100 })), date(1, 1, 1))
  assert.deepEqual(api.castDate('1/1/99', dmy, options({ pivot: 9999 })), date(9999, 1, 1))
  // Without a pivot a two-digit year is a width fault; with one, 1 and 3 digits still are.
  const without = api.castDetailed(Door.DateOrdered, '7/1/26', dmy)
  assert.ok(!without.ok)
  assert.equal(without.fault.detail, 'fieldWidth')
  assert.ok(!api.castDate('7/1/026', dmy, pivot2029).ok)
  assert.ok(!api.castDate('7/1/6', dmy, pivot2029).ok)
  // The date-time door reads it too.
  assert.deepEqual(api.castDatetime('7/1/26 15:04', dmy, pivot2029), {
    ok: true,
    value: { year: 2026, month: 1, day: 7, hour: 15, minute: 4, second: 0, nanos: 0 },
  })
})

test('month names replace the month field where the declared order puts it', () => {
  const english = names(ENGLISH)
  const dmy = DateOrder.DayMonthYear
  // A vocabulary alone opens names and nothing else.
  assert.deepEqual(api.castDate('7-Jan-2026', dmy, english), date(2026, 1, 7))
  assert.deepEqual(api.castDate('7-SEPT-2026', dmy, english), date(2026, 9, 7))
  assert.ok(!api.castDate('7-Jan-26', dmy, english).ok)
  assert.deepEqual(
    api.castDate('7-Jan-26', dmy, options({ pivot: 2029, months: english })),
    date(2026, 1, 7),
  )
  assert.deepEqual(api.castDate('Jan-7-2026', DateOrder.MonthDayYear, english), date(2026, 1, 7))
  // A vocabulary never overrides the declared order.
  const reordered = api.castDetailed(Door.DateOrdered, 'Jan-07-2026', dmy, english)
  assert.ok(!reordered.ok)
  assert.equal(reordered.fault.detail, 'digitExpected')
  // An unknown name is not recognised, spanning the word.
  const unknown = api.castDetailed(Door.DateOrdered, '7-Foo-2026', dmy, english)
  assert.ok(!unknown.ok)
  assert.equal(unknown.fault.detail, 'notRecognized')
  assert.deepEqual([unknown.fault.offset, unknown.fault.length], [2, 3])
})

test('a dotted month name, the . separator and a pivot together', () => {
  // Review focus 2: punctuation is part of the name and never stripped.
  const french = options({ pivot: 2029, months: names(FRENCH) })
  const dmy = DateOrder.DayMonthYear
  assert.deepEqual(api.castDate('7.janv..26', dmy, french), date(2026, 1, 7))
  assert.deepEqual(api.castDate('7.janv..2026', dmy, french), date(2026, 1, 7))
  const bare = api.castDetailed(Door.DateOrdered, '7.janv.26', dmy, french)
  assert.ok(!bare.ok)
  assert.equal(bare.fault.reason, 'malformed')
  assert.equal(bare.fault.detail, 'notRecognized')
  assert.ok(!api.castDate('7.janv.26', dmy, french).ok)
  // Declaring `janv` as well reads it.
  const both = options({ pivot: 2029, months: names([...FRENCH, [1, 'janv']]) })
  assert.deepEqual(api.castDate('7.janv.26', dmy, both), date(2026, 1, 7))
})

test('the options a date-time door takes ride with a DateFormat', () => {
  const german = DateFormat.declare({ order: DateOrder.DayMonthYear, pm: 'nachm.' })
  assert.ok(german.ok)
  const common = options({ pivot: 2029, months: names(ENGLISH) })
  assert.deepEqual(api.castDatetime('07-Jan-26 3:04 nachm.', german.value, common), {
    ok: true,
    value: { year: 2026, month: 1, day: 7, hour: 15, minute: 4, second: 0, nanos: 0 },
  })
  const twin = api.castDetailed(Door.DateTime, '07-Jan-26 3:04 nachm.', german.value, common)
  assert.deepEqual(twin, api.castDatetime('07-Jan-26 3:04 nachm.', german.value, common))
})

test('options a door does not read are ignored, and a broken one is a contract violation', () => {
  // Read by Timestamp, Date, DateOrdered and DateTime only.
  for (const ignored of [5, 'x', {}, Symbol('s'), options(), names(ENGLISH)]) {
    assert.deepEqual(api.castDetailed(Door.I32, '12', undefined, ignored as never), {
      ok: true,
      value: 12,
    })
    assert.deepEqual(api.castDetailed(Door.Bool, 'yes', undefined, ignored as never), {
      ok: true,
      value: true,
    })
  }
  const contract = {
    ok: false,
    fault: { reason: 'malformed', offset: 0, length: 0, detail: 'none', detailCode: 0 },
  }
  for (const broken of [5, 'x', {}, [], true, DateFormat.prototype]) {
    assert.deepEqual(
      api.castDetailed(Door.Date, '2026-01-07', undefined, broken as never),
      contract,
    )
    assert.deepEqual(
      api.castDetailed(Door.Timestamp, '2026-01-07T00:00:00Z', undefined, broken as never),
      contract,
    )
    const plain = api.castDate('2026-01-07', undefined, broken as never)
    assert.deepEqual(plain, { ok: false, fault: { reason: 'malformed', offset: 0, length: 0 } })
  }
})

test('declaring options: the refusals name their rule, and a pivot is never narrowed', () => {
  const refused = (spec: Partial<import('../index.js').DateOptionsSpec> | unknown) => {
    const declared = DateOptions.declare(spec as never)
    assert.ok(!declared.ok)
    return declared.message
  }
  assert.match(
    refused({ flags: 8 }),
    /Date options flags must be DateStyles members combined; got 8/,
  )
  assert.match(refused({ flags: 2 ** 31 }), /Date options flags must be DateStyles members/)
  for (const pivot of [1, 99, 10000, 65535, 65536, 65536 + 2029, 2 ** 32 - 1]) {
    assert.match(
      refused({ pivot }),
      /pivot must be 0 \(none\) or a year from 100 to 9999/,
      `${pivot}`,
    )
  }
  for (const pivot of [0, 100, 2029, 9999]) assert.ok(DateOptions.declare({ pivot }).ok, `${pivot}`)
  assert.match(refused({ months: {} }), /months must be a MonthNames/)
  assert.match(refused({ flags: 'x' }), /flags must be a whole number/)
  assert.match(refused({ pivot: 1.5 }), /pivot must be a whole number/)
  assert.match(refused(undefined), /must be an object/)
  // Flags win over the pivot, as the core judges them.
  assert.match(refused({ flags: 8, pivot: 1 }), /flags must be DateStyles/)
})

test('declaring month names: the entries, the packed layout and the rules', () => {
  const declared = MonthNames.declare(ENGLISH)
  assert.ok(declared.ok)
  const packed = declared.value.packed
  // A month byte, a little-endian u32 length, then the name's UTF-8, no padding.
  assert.deepEqual([...packed.subarray(0, 8)], [1, 3, 0, 0, 0, 0x4a, 0x61, 0x6e].slice(0, 8))
  assert.equal(
    packed.length,
    ENGLISH.reduce((total, [, name]) => total + 5 + name.length, 0),
  )
  const again = MonthNames.fromPacked(packed)
  assert.ok(again.ok)
  assert.deepEqual(again.value.packed, packed)
  // Entries are as declared; the options carry them.
  assert.deepEqual(options({ months: declared.value }).months?.packed, packed)
  assert.equal(options().months, null)
  // The refusals the core makes, each worded for its rule.
  const refusal = (entries: unknown) => {
    const result = MonthNames.declare(entries as never)
    assert.ok(!result.ok)
    return result.message
  }
  const without = (month: number) => ENGLISH.filter(([m]) => m !== month)
  assert.match(refusal(without(12)), /every month from 1 to 12/)
  assert.match(refusal([...ENGLISH, [0, 'x']]), /whole number from 1 to 12/)
  assert.match(refusal([...ENGLISH, [13, 'x']]), /whole number from 1 to 12/)
  assert.match(refusal([...ENGLISH, [256 + 1, 'x']]), /whole number from 1 to 12/)
  assert.match(refusal([...ENGLISH, [1.5, 'x']]), /whole number from 1 to 12/)
  assert.match(refusal([...ENGLISH, [2, '']]), /1 to 64 UTF-8 bytes/)
  assert.match(refusal([...ENGLISH, [2, 'x'.repeat(65)]]), /1 to 64 UTF-8 bytes/)
  assert.match(refusal([...ENGLISH, [2, 'Fe1']]), /no ASCII digit, slash, hyphen or white space/)
  assert.match(refusal([...ENGLISH, [2, 'a b']]), /no ASCII digit, slash, hyphen or white space/)
  assert.match(refusal([...ENGLISH, [2, 'JAN']]), /fold to the same spelling/)
  assert.match(refusal([...ENGLISH, [2, '\ud800']]), /not surrogate halves/)
  assert.match(refusal('x'), /array of \[month, name\] entries/)
  assert.match(refusal([[1]]), /entry 0 must be a \[month, name\] pair/)
  assert.match(refusal([[1, 2]]), /entry 0 must be a \[month, name\] pair with a string name/)
  assert.ok(MonthNames.declare([...ENGLISH, [2, 'x'.repeat(64)]]).ok)
  assert.ok(MonthNames.declare([...ENGLISH, [2, 'Fév.']]).ok)
  // Packed bytes the core refuses: cut short, trailing, not UTF-8.
  const bytes = (...more: number[]) => Uint8Array.from([...packed, ...more])
  for (const [bad, rule] of [
    [bytes(3, 5, 0, 0, 0, 97, 98), /packed entries/],
    [bytes(7), /packed entries/],
    [bytes(1, 2, 0, 0, 0, 0xc3, 0x28), /not UTF-8/],
    [packed.subarray(0, packed.length - 1), /packed entries/],
  ] as const) {
    const result = MonthNames.fromPacked(bad)
    assert.ok(!result.ok)
    assert.match(result.message, rule)
  }
  assert.ok(!MonthNames.fromPacked('x' as never).ok)
})

test('options pack as the C ABI carries them', () => {
  const view = (o: InstanceType<typeof DateOptions>) =>
    new DataView(o.packed.buffer, o.packed.byteOffset, o.packed.byteLength)
  const packed = view(options({ flags: DateStyles.CommonForms, pivot: 2029 }))
  assert.equal(packed.byteLength, 12)
  assert.deepEqual(
    [packed.getUint32(0, true), packed.getUint32(4, true), packed.getUint32(8, true)],
    [12, 7, 2029],
  )
})

test('the plain doors with options and castDetailed answer one parse', () => {
  const common = options({ flags: DateStyles.CommonForms, pivot: 2029, months: names(ENGLISH) })
  const rows: [string, import('../index.js').Door, number | undefined][] = [
    ['20260107', Door.Date, undefined],
    ['07-Jan-26', Door.DateOrdered, DateOrder.DayMonthYear],
    ['2026-01-07 15:04:05+00', Door.Timestamp, undefined],
    ['07-Jan-26 15:04', Door.DateTime, DateOrder.DayMonthYear],
    ['20260107x', Door.Date, undefined],
    ['07-Foo-26', Door.DateOrdered, DateOrder.DayMonthYear],
  ]
  for (const [text, door, order] of rows) {
    const twin = api.castDetailed(door, text, order, common)
    const plain =
      door === Door.Date
        ? api.castDate(text, undefined, common)
        : door === Door.DateOrdered
          ? api.castDate(text, order, common)
          : door === Door.Timestamp
            ? api.castTimestamp(text, common)
            : api.castDatetime(text, order as number, common)
    if (twin.ok) assert.deepEqual(twin, plain, text)
    else {
      assert.ok(!plain.ok, text)
      const { reason, offset, length } = twin.fault
      assert.deepEqual(plain.fault, { reason, offset, length }, text)
    }
  }
})
