// The detailed doors: castDetailed, castDetailedFromNumber, castCheckedDetailed and
// castCheckedDetailedFromNumber answer the plain verdicts with a `DetailedFault`: the same
// reason and span, and what failed, by name and by the core's number.
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const { Constraints, DateFormat, DateOrder, Door, ExcelEpoch, NumFormat, UnixPrecision } = api

const none = { detail: 'none', detailCode: 0 }

test('a detailed failure is the plain fault plus a name and a number', () => {
  const verdict = api.castDetailed(Door.I32, '12x')
  assert.deepEqual(verdict, {
    ok: false,
    fault: {
      reason: 'malformed',
      offset: 2,
      length: 1,
      detail: 'unexpectedCharacter',
      detailCode: 2,
    },
  })
  assert.ok(!verdict.ok)
  assert.deepEqual(Object.keys(verdict.fault), [
    'reason',
    'offset',
    'length',
    'detail',
    'detailCode',
  ])
  // Spans are the caller's: UTF-16 units for a string, bytes for a Uint8Array.
  const wide = api.castDetailed(Door.I32, '😀1')
  const bytes = api.castDetailed(Door.I32, new TextEncoder().encode('😀1'))
  assert.ok(!wide.ok && !bytes.ok)
  assert.deepEqual([wide.fault.offset, wide.fault.length], [0, 2])
  assert.deepEqual([bytes.fault.offset, bytes.fault.length], [0, 4])
  // A success is the plain success, byte for byte.
  assert.deepEqual(api.castDetailed(Door.I32, '12'), api.castI32('12'))
  assert.deepEqual(api.castDetailed(Door.I64, '9007199254740993'), {
    ok: true,
    value: 9007199254740993n,
  })
})

test('absent text is empty with the detail blank; a blank text is the same', () => {
  const blank = { reason: 'empty', offset: 0, length: 0, detail: 'blank', detailCode: 1 }
  assert.deepEqual(api.castDetailed(Door.I32, undefined), { ok: false, fault: blank })
  assert.deepEqual(api.castDetailed(Door.I32, null), { ok: false, fault: blank })
  const spaces = api.castDetailed(Door.I32, '  ')
  assert.ok(!spaces.ok)
  assert.equal(spaces.fault.reason, 'empty')
  assert.equal(spaces.fault.detail, 'blank')
  assert.equal(api.optional(api.castDetailed(Door.I32, null)), null)
  assert.ok(api.optional(api.castDetailed(Door.I32, 'x')) !== null)
})

test('a contract violation is malformed at 0, with the detail none', () => {
  const contract = { ok: false, fault: { reason: 'malformed', offset: 0, length: 0, ...none } }
  for (const call of [
    () => api.castDetailed(999 as never, '1'),
    () => api.castDetailed(24 as never, '1'),
    () => api.castDetailed(0 as never, '1'),
    () => api.castDetailed('4' as never, '1'),
    () => api.castDetailed(Door.I32, 5 as never),
    () => api.castDetailed(Door.I32, '1', {} as never),
    () => api.castDetailed(Door.I32, '1', DateFormat.prototype as never),
    // A declaration at a door that declares nothing, and none at one that needs it.
    () => api.castDetailed(Door.Bool, 'yes', NumFormat.invariant()),
    () => api.castDetailed(Door.Text, 'x', DateOrder.DayMonthYear),
    () => api.castDetailed(Door.Unix, '1'),
    () => api.castDetailed(Door.Unix, '1', 9 as never),
    () => api.castDetailed(Door.DateOrdered, '1/1/2026'),
    () => api.castDetailed(Door.DateOrdered, '1/1/2026', 4 as never),
    () => api.castDetailed(Door.DateTime, '1/1/2026', 'x' as never),
    () => api.castDetailed(Door.ExcelSerial, '1'),
    () => api.castDetailed(Door.ExcelSerial, '1', 3 as never),
  ]) {
    assert.deepEqual(call(), contract, String(call))
  }
})

test('every door the core names is reachable, and answers what its plain door does', () => {
  const dmy = DateOrder.DayMonthYear
  const cases: [string, import('../index.js').Door, string, unknown, unknown][] = [
    ['bool', Door.Bool, 'yes', undefined, api.castBool('yes')],
    ['char', Door.Char, 'U+0041', undefined, api.castChar('U+0041')],
    ['i8', Door.I8, '-8', undefined, api.castI8('-8')],
    ['i16', Door.I16, '-16', undefined, api.castI16('-16')],
    ['i32', Door.I32, '-32', undefined, api.castI32('-32')],
    ['i64', Door.I64, '-64', undefined, api.castI64('-64')],
    ['u8', Door.U8, '8', undefined, api.castU8('8')],
    ['u16', Door.U16, '16', undefined, api.castU16('16')],
    ['u32', Door.U32, '32', undefined, api.castU32('32')],
    ['u64', Door.U64, '64', undefined, api.castU64('64')],
    ['f32', Door.F32, '1.5', undefined, api.castF32('1.5')],
    ['f64', Door.F64, '1.5e3', undefined, api.castF64('1.5e3')],
    ['decimal', Door.Decimal, '1.10', undefined, api.castDecimal('1.10')],
    [
      'uuid',
      Door.Uuid,
      '123e4567-e89b-12d3-a456-426614174000',
      undefined,
      api.castUuid('123e4567-e89b-12d3-a456-426614174000'),
    ],
    [
      'timestamp',
      Door.Timestamp,
      '2026-01-07T15:04:05Z',
      undefined,
      api.castTimestamp('2026-01-07T15:04:05Z'),
    ],
    [
      'unix',
      Door.Unix,
      '1767798245000',
      UnixPrecision.Milliseconds,
      api.castUnix('1767798245000', UnixPrecision.Milliseconds),
    ],
    ['date', Door.Date, '2026-01-07', undefined, api.castDate('2026-01-07')],
    ['time', Door.Time, '15:04:05', undefined, api.castTime('15:04:05')],
    ['duration', Door.Duration, 'PT1H', undefined, api.castDuration('PT1H')],
    ['date_ordered', Door.DateOrdered, '7/1/2026', dmy, api.castDate('7/1/2026', dmy)],
    ['datetime', Door.DateTime, '7/1/2026 15:04', dmy, api.castDatetime('7/1/2026 15:04', dmy)],
    [
      'excel_serial',
      Door.ExcelSerial,
      '46029.5',
      ExcelEpoch.Y1900,
      api.castExcelSerial('46029.5', ExcelEpoch.Y1900),
    ],
    ['text', Door.Text, 'héllo', undefined, { ok: true, value: 'héllo' }],
  ]
  for (const [name, door, text, declared, plain] of cases) {
    const verdict = api.castDetailed(door, text, declared as never)
    assert.ok(verdict.ok, `${name}: ${String(verdict.ok)}`)
    assert.deepEqual(verdict, plain, name)
    // The same door as bytes.
    const bytes = api.castDetailed(door, new TextEncoder().encode(text), declared as never)
    assert.deepEqual(bytes, plain, `${name} as bytes`)
  }
})

test('the typed doors: the four numbers a source already decoded', () => {
  assert.deepEqual(api.castDetailedFromNumber(Door.Decimal, 0.1), api.castDecimalFromNumber(0.1))
  assert.deepEqual(
    api.castDetailedFromNumber(Door.ExcelSerial, 46029.5, ExcelEpoch.Y1900),
    api.castExcelSerialFromNumber(46029.5, ExcelEpoch.Y1900),
  )
  assert.deepEqual(api.castDetailedFromNumber(Door.Time, 0.5), api.castExcelTime(0.5))
  assert.deepEqual(api.castDetailedFromNumber(Door.Duration, 1.5), api.castExcelDuration(1.5))
  const nan = api.castDetailedFromNumber(Door.Decimal, Number.NaN)
  assert.deepEqual(nan, {
    ok: false,
    fault: { reason: 'malformed', offset: 0, length: 0, detail: 'notFinite', detailCode: 22 },
  })
  const infinite = api.castDetailedFromNumber(Door.Decimal, Number.POSITIVE_INFINITY)
  assert.ok(!infinite.ok)
  assert.deepEqual([infinite.fault.reason, infinite.fault.detail], ['outOfRange', 'notFinite'])
  const negative = api.castDetailedFromNumber(Door.ExcelSerial, -1, ExcelEpoch.Y1900)
  assert.ok(!negative.ok)
  assert.equal(negative.fault.detail, 'negativeNotAllowed')
  const phantom = api.castDetailedFromNumber(Door.ExcelSerial, 60, ExcelEpoch.Y1900)
  assert.ok(!phantom.ok)
  assert.equal(phantom.fault.detail, 'nonexistentDate')
  // Review focus 3: a door valid for one export is no door of another.
  const contract = { ok: false, fault: { reason: 'malformed', offset: 0, length: 0, ...none } }
  for (const call of [
    () => api.castDetailedFromNumber(Door.Text, 1),
    () => api.castDetailedFromNumber(Door.I32, 1),
    () => api.castDetailedFromNumber(Door.Bool, 1),
    () => api.castDetailedFromNumber(Door.Unix, 1, UnixPrecision.Seconds as never),
    () => api.castDetailedFromNumber(Door.ExcelSerial, 1),
    () => api.castDetailedFromNumber(Door.Decimal, 1, ExcelEpoch.Y1900 as never),
    () => api.castDetailedFromNumber(Door.Decimal, '1' as never),
    () => api.castDetailedFromNumber(24 as never, 1),
  ]) {
    assert.deepEqual(call(), contract, String(call))
  }
})

test('the checked doors: a failure has its detail, a violation has none', () => {
  const step = Constraints.declare({ door: Door.I32, maximum: '10' })
  assert.ok(step.ok)
  assert.deepEqual(api.castCheckedDetailed('7', step.value), api.castChecked('7', step.value))
  assert.deepEqual(api.castCheckedDetailed('12', step.value), {
    ok: false,
    violation: { keyword: 'maximum', offset: 0, length: 2 },
  })
  assert.deepEqual(api.castCheckedDetailed('1x', step.value), {
    ok: false,
    fault: {
      reason: 'malformed',
      offset: 1,
      length: 1,
      detail: 'unexpectedCharacter',
      detailCode: 2,
    },
  })
  assert.deepEqual(api.castCheckedDetailed(null, step.value), {
    ok: false,
    fault: { reason: 'empty', offset: 0, length: 0, detail: 'blank', detailCode: 1 },
  })
  // The detailed door also takes the text door's constraints, as castText does.
  const text = Constraints.declare({ door: Door.Text, trim: true, enum: ['Yes', 'No'] })
  assert.ok(text.ok)
  assert.deepEqual(
    api.castCheckedDetailed('  yes ', text.value),
    api.castText('  yes ', text.value),
  )
  assert.deepEqual(api.castCheckedDetailed('maybe', text.value), api.castText('maybe', text.value))
  const bytes = api.castCheckedDetailed(new Uint8Array([0x61, 0xff]), text.value)
  assert.ok(!bytes.ok && 'fault' in bytes)
  assert.equal(bytes.fault.detail, 'invalidUtf8')
  // Typed: every span is empty.
  assert.deepEqual(
    api.castCheckedDetailedFromNumber(7, step.value),
    api.castCheckedFromNumber(7, step.value),
  )
  assert.deepEqual(api.castCheckedDetailedFromNumber(12, step.value), {
    ok: false,
    violation: { keyword: 'maximum', offset: 0, length: 0 },
  })
  assert.deepEqual(api.castCheckedDetailedFromNumber(Number.NaN, step.value), {
    ok: false,
    fault: { reason: 'malformed', offset: 0, length: 0, detail: 'notFinite', detailCode: 22 },
  })
  const contract = { ok: false, fault: { reason: 'malformed', offset: 0, length: 0, ...none } }
  assert.deepEqual(api.castCheckedDetailedFromNumber(1, text.value), contract)
  assert.deepEqual(api.castCheckedDetailed('1', undefined as never), contract)
  assert.deepEqual(api.castCheckedDetailed('1', step.value, {} as never), contract)
  // The plain doors keep refusing what they refused: text constraints at castChecked.
  assert.deepEqual(api.castChecked('1', text.value), {
    ok: false,
    fault: { reason: 'malformed', offset: 0, length: 0 },
  })
})

test('Detail names every published code once, and none is "unknown"', () => {
  const names = Object.values(api.Detail)
  assert.equal(new Set(names).size, names.length)
  assert.equal(api.Detail.None, 'none')
  assert.equal(api.Detail.Unknown, 'unknown')
  // 32 published, none, unknown.
  assert.equal(names.length, 34)
})

test('the published names appear at their codes', () => {
  const rows: [string, import('../index.js').Door, string, string, number][] = [
    ['blank', Door.I32, ' ', 'blank', 1],
    ['unexpected character', Door.I32, '12a', 'unexpectedCharacter', 2],
    ['digit expected', Door.F64, '1e', 'digitExpected', 3],
    ['delimiter expected', Door.Date, '2026/01/07', 'delimiterExpected', 4],
    ['trailing text', Door.Time, '15:04:05x', 'trailingText', 5],
    ['no digits', Door.I32, '-', 'noDigits', 6],
    ['sign misplaced', Door.I32, '--1', 'signMisplaced', 7],
    ['parentheses unbalanced', Door.I32, '(1', 'parenthesesUnbalanced', 8],
    ['mark repeated', Door.F64, '1.2.3', 'markRepeated', 9],
    ['group size', Door.I32, '12,34', 'groupSize', 11],
    ['fraction not allowed', Door.I32, '1.5', 'fractionNotAllowed', 14],
    ['wrong length', Door.Date, '2026-1-07', 'wrongLength', 16],
    ['not recognized', Door.Bool, 'perhaps', 'notRecognized', 18],
    ['too large', Door.I8, '128', 'tooLarge', 25],
    ['too small', Door.I8, '-129', 'tooSmall', 26],
    ['negative not allowed', Door.U8, '-1', 'negativeNotAllowed', 24],
    ['too precise', Door.Decimal, `0.${'1'.repeat(29)}`, 'tooPrecise', 27],
    ['date field out of range', Door.Date, '2026-13-07', 'dateFieldOutOfRange', 28],
    ['time field out of range', Door.Time, '25:00:00', 'timeFieldOutOfRange', 29],
    ['not a scalar', Door.Char, 'U+110000', 'notAScalar', 32],
  ]
  for (const [name, door, text, detail, detailCode] of rows) {
    const verdict = api.castDetailed(door, text)
    assert.ok(!verdict.ok, name)
    assert.deepEqual([verdict.fault.detail, verdict.fault.detailCode], [detail, detailCode], name)
    assert.ok((Object.values(api.Detail) as string[]).includes(verdict.fault.detail), name)
  }
})

test('the packed layouts are untouched by the detailed doors', () => {
  assert.equal(NumFormat.invariant().packed.length, 92)
  const declared = Constraints.declare({ door: Door.I32, maximum: '10' })
  assert.ok(declared.ok)
  assert.equal(declared.value.packed.length, 72)
})

test('Detail is open: a name this binding lacks still type-checks, and a switch needs a default', () => {
  // A later release naming code 33 must not break a caller's switch: the type admits any string.
  const later: import('../index.js').Detail = 'aCodeNamedLater'
  const describe = (detail: import('../index.js').Detail): string => {
    switch (detail) {
      case 'blank':
        return 'blank'
      case 'groupSize':
        return 'groups'
      default:
        return `other: ${detail}`
    }
  }
  assert.equal(describe('blank'), 'blank')
  assert.equal(describe(later), 'other: aCodeNamedLater')
  const verdict = api.castDetailed(Door.I32, '12,34')
  assert.ok(!verdict.ok)
  assert.equal(describe(verdict.fault.detail), 'groups')
  // The runtime names are still the literal strings.
  assert.equal(api.Detail.GroupSize, 'groupSize')
  assert.equal(api.Detail.Unknown, 'unknown')
})

test('Constraints.declare names the doors that can carry constraints', () => {
  for (const door of [Door.Bool, Door.Uuid, Door.Date, Door.Char, 99]) {
    const declared = Constraints.declare({ door } as never)
    assert.ok(!declared.ok)
    assert.match(
      declared.message,
      /^door must be a Door that carries constraints: an integer, real or decimal door, or Door\.Text$/,
    )
  }
})
