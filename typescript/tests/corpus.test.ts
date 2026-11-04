// Replays the shared conformance corpus (corpus/*.json at the repository root), the files the
// Rust core and every other binding replay. Each vector runs twice: as the UTF-8 bytes of its
// input in a Uint8Array, whose fault spans are byte offsets and must equal the vector's
// verbatim, and as the string itself, whose spans are UTF-16 code units and must equal the
// vector's span re-expressed in them. A vector that fails here is a break in the polyglot
// contract, not just a failing test.
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { existsSync, readdirSync, readFileSync } = require('node:fs') as typeof import('node:fs')
const path: typeof import('node:path') = require('node:path')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const {
  Constraints,
  DateFormat,
  DateOptions,
  DateOrder,
  DateStyles,
  Door,
  Grouping,
  MonthNames,
  NumFormat,
  Rounding,
} = api
type Verdict<T> = import('../index.js').Verdict<T>
type DetailedVerdict<T> = import('../index.js').DetailedVerdict<T>
type DetailedCheckedVerdict<T> = import('../index.js').DetailedCheckedVerdict<T>
type CheckedVerdict<T> = import('../index.js').CheckedVerdict<T>
type ConstraintsSpec = import('../index.js').ConstraintsSpec
type NumFormatSpec = import('../index.js').NumFormatSpec
type Text = import('../index.js').Text

// One corpus record. Each file uses a subset of these fields; JSON.parse does not check them.
interface Vector {
  input: string
  expect: 'ok' | 'empty' | 'malformed' | 'out_of_range' | 'violation' | 'refused'
  kind?: 'white_space' | 'num_format' | 'day_periods' | 'date_options' | 'month_names'
  rule?:
    | 'length'
    | 'digit'
    | 'equal_separators'
    | 'white_space'
    | 'designator_edge'
    | 'currency_edge'
    | 'grouping_policy'
    | 'unknown_flag'
    | 'flags'
    | 'pivot'
    | 'layout'
    | 'month'
    | 'utf8'
    | 'name_length'
    | 'character'
    | 'month_missing'
    | 'duplicate'
  fault?: [number, number]
  detail?: string
  options?: { flags?: string[]; pivot?: number; months?: [number, string][] }
  flag_bits?: number
  pivot?: number
  months?: [number, string][]
  append_bytes?: number[]
  flags?: string[]
  constraints?: Record<string, unknown>
  input_bytes?: number[]
  from_f64?: number | string
  adjusted?: boolean
  enum_index?: number
  keyword?: string
  refusal?: number
  magnitude?: string
  scale?: number
  negative?: boolean
  type?: string
  door?: string
  bits?: string
  value?: unknown
  format?: Record<string, string | number>
  order?: number
  epoch?: number
  precision?: number
  am?: string
  pm?: string
  year?: number
  month?: number
  day?: number
  nanos_of_day?: number
  nanos?: number
  seconds?: number
}

function corpusDirectory(): string {
  const start = __dirname
  let dir = start
  for (;;) {
    const candidate = path.join(dir, 'corpus')
    if (existsSync(candidate)) return candidate
    const parent = path.dirname(dir)
    if (parent === dir) throw new Error(`corpus directory not found above ${start}`)
    dir = parent
  }
}
// HYPERCAST_CORPUS points the replay at another copy (the negative control corrupts one).
const CORPUS = process.env.HYPERCAST_CORPUS ?? corpusDirectory()

// An integer literal past 2^53 keeps its exact digits (as a string, for the test to make a
// bigint of): JSON.parse alone would round 18446744073709551615 to a double.
function load(name: string): Vector[] {
  return JSON.parse(
    readFileSync(path.join(CORPUS, name), 'utf8'),
    (_key, value, context?: { source?: string }) => {
      if (typeof value !== 'number' || Number.isSafeInteger(value)) return value
      if (context?.source === undefined) {
        throw new Error('this runtime gives JSON.parse revivers no source text')
      }
      return /^-?\d+$/.test(context.source) ? context.source : value
    },
  )
}

const encoder = new TextEncoder()
const decoder = new TextDecoder()
const reasons = { empty: 'empty', malformed: 'malformed', out_of_range: 'outOfRange' } as const

// A byte span over the UTF-8 of `input`, in UTF-16 code units: what a string door reports.
function utf16Span(input: string, [offset, length]: [number, number]): [number, number] {
  const bytes = encoder.encode(input)
  const start = decoder.decode(bytes.subarray(0, offset)).length
  const end = decoder.decode(bytes.subarray(0, offset + length)).length
  return [start, end - start]
}

// The detail codes the spec publishes, by the corpus's spelling. They are written out here, not
// read from the binding, so a renumbered or renamed code fails a row.
const DETAIL_CODES: Record<string, number> = {
  blank: 1,
  unexpected_character: 2,
  digit_expected: 3,
  delimiter_expected: 4,
  trailing_text: 5,
  no_digits: 6,
  sign_misplaced: 7,
  parentheses_unbalanced: 8,
  mark_repeated: 9,
  group_misplaced: 10,
  group_size: 11,
  group_in_fraction: 12,
  ambiguous_separator: 13,
  fraction_not_allowed: 14,
  field_width: 15,
  wrong_length: 16,
  component_missing: 17,
  not_recognized: 18,
  unknown_unit: 19,
  invalid_utf8: 20,
  too_long: 21,
  not_finite: 22,
  not_whole: 23,
  negative_not_allowed: 24,
  too_large: 25,
  too_small: 26,
  too_precise: 27,
  date_field_out_of_range: 28,
  time_field_out_of_range: 29,
  outside_window: 30,
  nonexistent_date: 31,
  not_a_scalar: 32,
}
const camel = (snake: string): string =>
  snake.replace(/_([a-z])/g, (_match, c: string) => c.toUpperCase())

// A failure row names its detail, and a detailed fault carries it by name and by number.
function holdsDetail(ctx: string, v: Vector, fault: { detail: string; detailCode: number }): void {
  assert.ok(v.detail !== undefined, `${ctx}: a failure row names no detail`)
  assert.ok(v.detail in DETAIL_CODES, `${ctx}: unknown detail ${v.detail}`)
  assert.equal(fault.detail, camel(v.detail), `${ctx}: detail`)
  assert.equal(fault.detailCode, DETAIL_CODES[v.detail], `${ctx}: detailCode`)
}

// Runs `door` and its detailed twin on the vector's bytes and on its string, and holds all
// the verdicts to it: the twin has the same value, or the same reason and span plus the row's
// detail, and the plain fault has no detail (the 1.0 shape).
function replay<T>(
  file: string,
  v: Vector,
  door: (text: Text) => Verdict<T>,
  expected: unknown,
  same: (actual: T, expected: unknown) => boolean,
  twin: (text: Text) => DetailedVerdict<T>,
): void {
  const forms: [Text, number, boolean][] = [
    [encoder.encode(v.input), encoder.encode(v.input).length, true],
    [v.input, v.input.length, false],
  ]
  for (const [text, units, bytes] of forms) {
    const ctx = `${file} ${JSON.stringify(v)} as ${bytes ? 'bytes' : 'string'}`
    const verdict = door(text)
    const detailed = twin(text)
    if (verdict.ok) {
      assert.equal(v.expect, 'ok', `${ctx}: unexpectedly cast to ${String(verdict.value)}`)
      assert.ok(same(verdict.value, expected), `${ctx}: ${String(verdict.value)} != ${expected}`)
      assert.equal(v.detail, undefined, `${ctx}: a row that parses names a detail`)
      assert.ok(detailed.ok, `${ctx}: the detailed twin faulted`)
      assert.ok(same(detailed.value, expected), `${ctx}: twin ${String(detailed.value)}`)
      continue
    }
    const { reason, offset, length } = verdict.fault
    assert.deepEqual(Object.keys(verdict.fault), ['reason', 'offset', 'length'], ctx)
    assert.notEqual(v.expect, 'ok', `${ctx}: faulted ${reason}`)
    assert.equal(reason, reasons[v.expect as keyof typeof reasons], ctx)
    assert.ok(offset + length <= units, `${ctx}: span ${offset}+${length} past ${units}`)
    if (v.expect === 'empty') assert.deepEqual([offset, length], [0, 0], ctx)
    if (v.fault !== undefined) {
      const span = bytes ? v.fault : utf16Span(v.input, v.fault)
      assert.deepEqual([offset, length], span, `${ctx}: span`)
    }
    assert.ok(!detailed.ok, `${ctx}: the detailed twin parsed`)
    const fault = detailed.fault
    assert.deepEqual([fault.reason, fault.offset, fault.length], [reason, offset, length], ctx)
    holdsDetail(ctx, v, fault)
  }
}

// A vector's format as a caller writes it, for the public constructor. A row that declares no
// `grouping` is legacy (the core's meaning); a policy a row's own `flags` carry is declared by
// name, except the one no name spells, which stays in `flags` to be refused.
const groupings = {
  legacy: Grouping.Legacy,
  western: Grouping.Western,
  indian: Grouping.Indian,
} as const
function formatSpec(f: Record<string, string | number>): NumFormatSpec {
  let flags = f.flags as number
  let grouping: number | undefined
  if (f.grouping !== undefined) {
    grouping = groupings[f.grouping as keyof typeof groupings]
  } else if ((flags & 0x300) === 0) {
    grouping = Grouping.Legacy
  } else if ((flags & 0x300) !== 0x300) {
    grouping = flags & 0x300
    flags &= ~0x300
  }
  return {
    decimalSep: f.decimal_sep as string,
    groupSep: f.group_sep as string,
    flags,
    grouping: grouping as NumFormatSpec['grouping'],
    currency: f.currency as string | undefined,
    currencyDecimalSep: f.currency_decimal_sep as string | undefined,
    currencyGroupSep: f.currency_group_sep as string | undefined,
    minusSign: f.minus_sign as string | undefined,
    plusSign: f.plus_sign as string | undefined,
    percent: f.percent as string | undefined,
    digitZero: f.digit_zero as string | undefined,
  }
}

// A row with no format is the invariant separators under legacy grouping, the core's meaning.
const LEGACY_INVARIANT: InstanceType<typeof NumFormat> = (() => {
  const declared = NumFormat.declare({
    decimalSep: '.',
    groupSep: ',',
    flags: api.NumStyles.All,
    grouping: Grouping.Legacy,
  })
  assert.ok(declared.ok)
  return declared.value
})()

const formats = new Map<string, InstanceType<typeof NumFormat>>()
function formatOf(v: Vector): InstanceType<typeof NumFormat> {
  if (v.format === undefined) return LEGACY_INVARIANT
  const key = JSON.stringify(v.format)
  const cached = formats.get(key)
  if (cached) return cached
  const declared = NumFormat.declare(formatSpec(v.format))
  assert.ok(declared.ok, `${key}: ${declared.ok ? '' : declared.message}`)
  formats.set(key, declared.value)
  return declared.value
}

// The date options and month names a row declares, built through the public constructors.
const styleBits: Record<string, number> = {
  compact_date: DateStyles.CompactDate,
  space_separator: DateStyles.SpaceSeparator,
  hour_offset: DateStyles.HourOffset,
  common_forms: DateStyles.CommonForms,
}
const styleOf = (names: string[] = []): number =>
  names.reduce((bits, name) => bits | (styleBits[name] ?? Number.NaN), 0)
function monthNamesOf(months: [number, string][]): InstanceType<typeof MonthNames> {
  const declared = MonthNames.declare(months)
  assert.ok(declared.ok, `${JSON.stringify(months)}: ${declared.ok ? '' : declared.message}`)
  return declared.value
}
function optionsOf(v: Vector): InstanceType<typeof DateOptions> | undefined {
  if (v.options === undefined) return undefined
  const declared = DateOptions.declare({
    flags: styleOf(v.options.flags),
    pivot: v.options.pivot,
    months: v.options.months === undefined ? undefined : monthNamesOf(v.options.months),
  })
  assert.ok(declared.ok, `${JSON.stringify(v.options)}: ${declared.ok ? '' : declared.message}`)
  return declared.value
}

// A double's bits, so 0 and -0 differ: the core never delivers a negative zero, and a binding
// that made one of its own would pass a === comparison.
function bits(value: number): bigint {
  const view = new DataView(new ArrayBuffer(8))
  view.setFloat64(0, value)
  return view.getBigUint64(0)
}
const sameBits = (a: unknown, e: unknown): boolean =>
  typeof a === 'number' && typeof e === 'number' && bits(a) === bits(e)

const deep = (a: unknown, e: unknown): boolean => {
  try {
    assert.deepEqual(a, e)
    return true
  } catch {
    return false
  }
}
const timestampOf = (v: Vector) =>
  v.seconds === undefined ? undefined : { seconds: v.seconds, nanos: v.nanos }
const dateOf = (v: Vector) =>
  v.year === undefined ? undefined : { year: v.year, month: v.month, day: v.day }
function timeOf(nanosOfDay: number) {
  const seconds = Math.floor(nanosOfDay / 1e9)
  return {
    hour: Math.floor(seconds / 3600),
    minute: Math.floor(seconds / 60) % 60,
    second: seconds % 60,
    nanos: nanosOfDay % 1e9,
  }
}
const civilOf = (v: Vector) =>
  v.year === undefined ? undefined : { ...dateOf(v), ...timeOf(v.nanos_of_day as number) }

const files = [
  'boolean.json',
  'char.json',
  'integer.json',
  'real.json',
  'decimal.json',
  'uuid.json',
  'timestamp.json',
  'unix.json',
  'excel_serial.json',
  'date.json',
  'date_order.json',
  'datetime.json',
  'time.json',
  'duration.json',
  'typed.json',
  'constraints.json',
  'declarations.json',
  'formats.json',
]

test('every corpus file is replayed', () => {
  const present = readdirSync(CORPUS).filter((name) => name.endsWith('.json'))
  assert.deepEqual([...present].sort(), [...files].sort())
})

const strictEqual = (a: unknown, e: unknown): boolean => a === e

test('boolean.json', () => {
  for (const v of load('boolean.json')) {
    const twin = (text: Text) => api.castDetailed(Door.Bool, text) as DetailedVerdict<boolean>
    replay('boolean.json', v, api.castBool, v.value, strictEqual, twin)
  }
})

test('char.json', () => {
  // The vector pins the code point; the door answers the one-character string.
  for (const v of load('char.json')) {
    const twin = (text: Text) => api.castDetailed(Door.Char, text) as DetailedVerdict<string>
    replay(
      'char.json',
      v,
      api.castChar,
      v.value,
      (actual, code) => [...actual].length === 1 && actual.codePointAt(0) === code,
      twin,
    )
  }
})

test('integer.json', () => {
  const doors = {
    i8: [api.castI8, Door.I8],
    i16: [api.castI16, Door.I16],
    i32: [api.castI32, Door.I32],
    i64: [api.castI64, Door.I64],
    u8: [api.castU8, Door.U8],
    u16: [api.castU16, Door.U16],
    u32: [api.castU32, Door.U32],
    u64: [api.castU64, Door.U64],
  } as const
  for (const v of load('integer.json')) {
    const type = v.type as keyof typeof doors
    const [plain, code] = doors[type] as unknown as [
      (text: Text, format?: InstanceType<typeof NumFormat>) => Verdict<number | bigint>,
      import('../index.js').Door,
    ]
    const format = formatOf(v)
    // The 64-bit doors answer a bigint, exact over the whole range.
    const wide = type === 'i64' || type === 'u64'
    const expected = v.value === undefined ? undefined : wide ? BigInt(v.value as string) : v.value
    const twin = (text: Text) =>
      api.castDetailed(code, text, format) as DetailedVerdict<number | bigint>
    replay('integer.json', v, (text) => plain(text, format), expected, strictEqual, twin)
  }
})

test('real.json', () => {
  for (const v of load('real.json')) {
    const [door, code] = v.type === 'f32' ? [api.castF32, Door.F32] : [api.castF64, Door.F64]
    const format = formatOf(v)
    const expected =
      typeof v.value === 'number' && v.type === 'f32' ? Math.fround(v.value) : v.value
    const twin = (text: Text) => api.castDetailed(code, text, format) as DetailedVerdict<number>
    replay('real.json', v, (text) => door(text, format), expected, sameBits, twin)
  }
})

test('decimal.json', () => {
  // The canonical text pins sign, digits and scale at once: "1.10" can only come back "1.1".
  for (const v of load('decimal.json')) {
    const format = formatOf(v)
    const twin = (text: Text) =>
      api.castDetailed(Door.Decimal, text, format) as DetailedVerdict<string>
    replay('decimal.json', v, (text) => api.castDecimal(text, format), v.value, strictEqual, twin)
  }
})

test('uuid.json', () => {
  for (const v of load('uuid.json')) {
    const expected =
      typeof v.value === 'string'
        ? v.value.replace(/^(.{8})(.{4})(.{4})(.{4})(.{12})$/, '$1-$2-$3-$4-$5')
        : undefined
    const twin = (text: Text) => api.castDetailed(Door.Uuid, text) as DetailedVerdict<string>
    replay('uuid.json', v, api.castUuid, expected, strictEqual, twin)
  }
})

test('timestamp.json', () => {
  // A row with `options` runs through the date options its declaration builds, and only
  // through them: the plain door with options and its detailed twin.
  for (const v of load('timestamp.json')) {
    const options = optionsOf(v)
    const twin = (text: Text) =>
      api.castDetailed(Door.Timestamp, text, undefined, options) as DetailedVerdict<
        import('../index.js').Timestamp
      >
    replay(
      'timestamp.json',
      v,
      (text) => api.castTimestamp(text, options),
      timestampOf(v),
      deep,
      twin,
    )
  }
})

test('unix.json', () => {
  for (const v of load('unix.json')) {
    const door = (text: Text) => api.castUnix(text, v.precision as number)
    const twin = (text: Text) =>
      api.castDetailed(Door.Unix, text, v.precision as number) as DetailedVerdict<
        import('../index.js').Timestamp
      >
    replay('unix.json', v, door, timestampOf(v), deep, twin)
  }
})

test('excel_serial.json', () => {
  for (const v of load('excel_serial.json')) {
    const door = (text: Text) => api.castExcelSerial(text, v.epoch as number)
    const twin = (text: Text) =>
      api.castDetailed(Door.ExcelSerial, text, v.epoch as number) as DetailedVerdict<
        import('../index.js').Timestamp
      >
    replay('excel_serial.json', v, door, timestampOf(v), deep, twin)
  }
})

test('date.json', () => {
  for (const v of load('date.json')) {
    const options = optionsOf(v)
    const twin = (text: Text) =>
      api.castDetailed(Door.Date, text, undefined, options) as DetailedVerdict<
        import('../index.js').CalendarDate
      >
    replay('date.json', v, (text) => api.castDate(text, undefined, options), dateOf(v), deep, twin)
  }
})

test('date_order.json', () => {
  for (const v of load('date_order.json')) {
    const options = optionsOf(v)
    const door = (text: Text) => api.castDate(text, v.order as number, options)
    const twin = (text: Text) =>
      api.castDetailed(Door.DateOrdered, text, v.order as number, options) as DetailedVerdict<
        import('../index.js').CalendarDate
      >
    replay('date_order.json', v, door, dateOf(v), deep, twin)
  }
})

test('datetime.json', () => {
  for (const v of load('datetime.json')) {
    // Declared day periods ride in a DateFormat; a vector without them reads English only.
    let format: number | InstanceType<typeof DateFormat> = v.order as number
    if (v.am !== undefined || v.pm !== undefined) {
      const declared = DateFormat.declare({ order: v.order as number, am: v.am, pm: v.pm })
      assert.ok(declared.ok, JSON.stringify(v))
      format = declared.value
    }
    const options = optionsOf(v)
    const twin = (text: Text) =>
      api.castDetailed(Door.DateTime, text, format, options) as DetailedVerdict<
        import('../index.js').CivilDateTime
      >
    replay(
      'datetime.json',
      v,
      (text) => api.castDatetime(text, format, options),
      civilOf(v),
      deep,
      twin,
    )
  }
})

test('time.json', () => {
  for (const v of load('time.json')) {
    const expected = v.nanos === undefined ? undefined : timeOf(v.nanos)
    const twin = (text: Text) =>
      api.castDetailed(Door.Time, text) as DetailedVerdict<import('../index.js').TimeOfDay>
    replay('time.json', v, api.castTime, expected, deep, twin)
  }
})

test('duration.json', () => {
  for (const v of load('duration.json')) {
    const twin = (text: Text) =>
      api.castDetailed(Door.Duration, text) as DetailedVerdict<import('../index.js').Duration>
    replay('duration.json', v, api.castDuration, timestampOf(v), deep, twin)
  }
})

test('typed.json', () => {
  // Each double is named by its bits, which rebuild it exactly (NaN and the infinities
  // included, which JSON cannot spell). A typed door's fault has no span.
  const seen = new Set<string>()
  for (const v of load('typed.json')) {
    const view = new DataView(new ArrayBuffer(8))
    view.setBigUint64(0, BigInt(`0x${v.bits}`))
    const value = view.getFloat64(0)
    const door = v.door as string
    seen.add(door)
    let verdict: Verdict<unknown>
    let twin: DetailedVerdict<unknown>
    let expected: unknown
    if (door === 'decimal') {
      verdict = api.castDecimalFromNumber(value)
      twin = api.castDetailedFromNumber(Door.Decimal, value)
      expected = v.value
    } else if (door === 'excel_serial') {
      verdict = api.castExcelSerialFromNumber(value, v.epoch as number)
      twin = api.castDetailedFromNumber(Door.ExcelSerial, value, v.epoch as number)
      expected = civilOf(v)
    } else if (door === 'excel_time') {
      verdict = api.castExcelTime(value)
      twin = api.castDetailedFromNumber(Door.Time, value)
      expected = v.nanos === undefined ? undefined : timeOf(v.nanos)
    } else if (door === 'excel_duration') {
      verdict = api.castExcelDuration(value)
      twin = api.castDetailedFromNumber(Door.Duration, value)
      expected = timestampOf(v)
    } else {
      throw new Error(`typed: unknown door ${door}`)
    }
    const ctx = `typed.json ${JSON.stringify(v)}`
    if (verdict.ok) {
      assert.equal(v.expect, 'ok', ctx)
      assert.deepEqual(verdict.value, expected, ctx)
      assert.deepEqual(twin, verdict, `${ctx}: twin`)
      assert.equal(v.detail, undefined, `${ctx}: a row that parses names a detail`)
    } else {
      assert.equal(verdict.fault.reason, reasons[v.expect as keyof typeof reasons], ctx)
      assert.deepEqual([verdict.fault.offset, verdict.fault.length], [0, 0], `${ctx}: span`)
      assert.ok(!twin.ok, `${ctx}: the detailed twin parsed`)
      const fault = twin.fault
      assert.deepEqual(
        [fault.reason, fault.offset, fault.length],
        [verdict.fault.reason, 0, 0],
        ctx,
      )
      holdsDetail(ctx, v, fault)
    }
  }
  assert.deepEqual([...seen].sort(), ['decimal', 'excel_duration', 'excel_serial', 'excel_time'])
})

// The corpus's door names, as the binding's Door members.
const doorOf: Record<string, number> = {
  i8: Door.I8,
  i16: Door.I16,
  i32: Door.I32,
  i64: Door.I64,
  u8: Door.U8,
  u16: Door.U16,
  u32: Door.U32,
  u64: Door.U64,
  f32: Door.F32,
  f64: Door.F64,
  decimal: Door.Decimal,
  text: Door.Text,
}
const roundingOf: Record<string, number> = {
  strict: Rounding.Strict,
  midpoint: Rounding.Midpoint,
  floor: Rounding.Floor,
  ceiling: Rounding.Ceiling,
}

// The vector's declaration as a caller writes it, for the public constructor: every bound,
// step and entry the decimal string the corpus spells it as.
function specOf(v: Vector): ConstraintsSpec {
  const c = v.constraints ?? {}
  const spec: Record<string, unknown> = { door: doorOf[v.door as string] }
  for (const key of [
    'minimum',
    'exclusiveMinimum',
    'maximum',
    'exclusiveMaximum',
    'multipleOf',
    'minLength',
    'maxLength',
    'trim',
    'enum',
  ]) {
    if (key in c) spec[key] = c[key]
  }
  if (c.rounding !== undefined) spec.rounding = roundingOf[c.rounding as string]
  return spec as unknown as ConstraintsSpec
}

// What a checked success at the vector's door is: a bigint at the 64-bit doors, the single
// widened at f32, the canonical text at decimal, the caller's entry (or text) at text.
function expectedValue(v: Vector): unknown {
  if (v.door === 'i64' || v.door === 'u64') return BigInt(v.value as string | number)
  if (v.door === 'f32') return Math.fround(v.value as number)
  return v.value
}
const sameChecked = (door: string) => (a: unknown, e: unknown) =>
  door === 'f32' || door === 'f64' ? sameBits(a, e) : a === e

// Holds one checked verdict to the vector, its span already in the form's units.
function judge(
  ctx: string,
  v: Vector,
  verdict: CheckedVerdict<unknown> | DetailedCheckedVerdict<unknown>,
  span: [number, number] | undefined,
  units: number,
  detailed: boolean,
): void {
  if (verdict.ok) {
    assert.equal(v.detail, undefined, `${ctx}: a row that parses names a detail`)
    assert.equal(v.expect, 'ok', `${ctx}: unexpectedly ${String(verdict.value)}`)
    const expected = expectedValue(v)
    assert.ok(
      sameChecked(v.door as string)(verdict.value, expected),
      `${ctx}: ${String(verdict.value)} != ${String(expected)}`,
    )
    assert.equal(verdict.adjusted, v.adjusted, `${ctx}: adjusted`)
    assert.equal(verdict.enumIndex, v.enum_index ?? null, `${ctx}: enumIndex`)
    return
  }
  if ('violation' in verdict) {
    assert.equal(v.expect, 'violation', `${ctx}: violated ${verdict.violation.keyword}`)
    const { keyword, offset, length } = verdict.violation
    // A violation names a rule the value broke, not a failure to read it: no detail.
    assert.equal(v.detail, undefined, `${ctx}: a violation names a detail`)
    assert.equal(keyword, v.keyword, ctx)
    assert.ok(offset + length <= units, `${ctx}: span ${offset}+${length} past ${units}`)
    if (span !== undefined) assert.deepEqual([offset, length], span, `${ctx}: span`)
    return
  }
  const { reason, offset, length } = verdict.fault
  assert.equal(reason, reasons[v.expect as keyof typeof reasons], `${ctx}: ${reason}`)
  assert.ok(offset + length <= units, `${ctx}: span ${offset}+${length} past ${units}`)
  if (span !== undefined) assert.deepEqual([offset, length], span, `${ctx}: span`)
  if (detailed) holdsDetail(ctx, v, verdict.fault as { detail: string; detailCode: number })
  else assert.deepEqual(Object.keys(verdict.fault), ['reason', 'offset', 'length'], ctx)
}

test('constraints.json', () => {
  // Every row is declared through the public constructor, so the corpus runs the binding's own
  // conversion of each bound, step and entry. A text row runs as bytes and as the string.
  let refusals = 0
  let typed = 0
  let replayed = 0
  const vectors = load('constraints.json')
  for (const v of vectors) {
    const ctx = `constraints.json ${JSON.stringify(v)}`
    const spec = specOf(v)
    const declared = Constraints.declare(spec)
    if (v.expect === 'refused') {
      assert.ok(!declared.ok, `${ctx}: declared`)
      assert.equal(declared.message.split(':')[0], `Constraints refused (code ${v.refusal})`, ctx)
      assert.equal(api.castConstraintsCheck(spec), v.refusal, ctx)
      refusals++
      replayed++
      continue
    }
    assert.ok(declared.ok, `${ctx}: ${declared.ok ? '' : declared.message}`)
    assert.equal(api.castConstraintsCheck(spec), 0, ctx)
    const constraints = declared.value
    if (v.from_f64 !== undefined) {
      const value = typeof v.from_f64 === 'string' ? Number(v.from_f64) : v.from_f64
      judge(ctx, v, api.castCheckedFromNumber(value, constraints), v.fault, 0, false)
      const twin = api.castCheckedDetailedFromNumber(value, constraints)
      judge(`${ctx} detailed`, v, twin, v.fault, 0, true)
      typed++
      replayed++
      continue
    }
    const format = formatOf(v)
    const door = (text: Text) =>
      v.door === 'text'
        ? api.castText(text, constraints)
        : api.castChecked(text, constraints, format)
    // The detailed door takes text constraints too, and reads the same format.
    const twin = (text: Text) => api.castCheckedDetailed(text, constraints, format)
    if (v.input_bytes !== undefined) {
      // Bytes no string can spell (invalid UTF-8): the byte form alone.
      const bytes = new Uint8Array(v.input_bytes)
      judge(`${ctx} as bytes`, v, door(bytes), v.fault, bytes.length, false)
      judge(`${ctx} as bytes detailed`, v, twin(bytes), v.fault, bytes.length, true)
      replayed++
      continue
    }
    const bytes = encoder.encode(v.input)
    judge(`${ctx} as bytes`, v, door(bytes), v.fault, bytes.length, false)
    judge(`${ctx} as bytes detailed`, v, twin(bytes), v.fault, bytes.length, true)
    const span = v.fault === undefined ? undefined : utf16Span(v.input, v.fault)
    judge(`${ctx} as string`, v, door(v.input), span, v.input.length, false)
    judge(`${ctx} as string detailed`, v, twin(v.input), span, v.input.length, true)
    replayed++
  }
  assert.ok(refusals > 0 && typed > 0)
  // Every row was replayed: the count is the file's, never a literal.
  assert.equal(replayed, vectors.length)
})

test('declarations.json', () => {
  // The conversion every bound crosses, held to the raw triple: the canonical text spells the
  // sign, the digits and the scale, so each is read back from it and compared on its own.
  let replayed = 0
  const vectors = load('declarations.json')
  for (const v of vectors) {
    const ctx = `declarations.json ${JSON.stringify(v)}`
    const text: Text = v.input_bytes === undefined ? v.input : new Uint8Array(v.input_bytes)
    const declared = api.castDeclaredDecimal(text)
    if (v.expect === 'refused') {
      assert.ok(!declared.ok, `${ctx}: converted`)
      assert.equal(declared.message.split(':')[0], `Constraints refused (code ${v.refusal})`, ctx)
      replayed++
      continue
    }
    assert.ok(declared.ok, `${ctx}: ${declared.ok ? '' : declared.message}`)
    const canonical = declared.value
    assert.equal(canonical, v.value, ctx)
    const negative = canonical.startsWith('-')
    const unsigned = negative ? canonical.slice(1) : canonical
    const point = unsigned.indexOf('.')
    const scale = point < 0 ? 0 : unsigned.length - point - 1
    const magnitude = BigInt(unsigned.replace('.', ''))
    assert.equal(negative, v.negative, `${ctx}: negative`)
    assert.equal(scale, v.scale, `${ctx}: scale`)
    assert.equal(magnitude, BigInt(v.magnitude as string), `${ctx}: magnitude`)
    replayed++
  }
  // Every row was replayed: the count is the file's, never a literal.
  assert.equal(replayed, vectors.length)
})

// The message this binding gives for each rule a declaration can break. The length and digit
// rules share one message ("1 to 8 UTF-8 bytes with no ASCII digit"); the others each have
// their own, so the rule that won is the one named.
const ruleMessages = {
  length: /must be 1 to (8|16) UTF-8 bytes with no ASCII digit; got /,
  digit: /must be 1 to (8|16) UTF-8 bytes with no ASCII digit; got /,
  equal_separators: /must differ; both are /,
  white_space: /must not contain white space; got /,
  designator_edge: /must not have white space at either end; got /,
  currency_edge: /must not start or end with white space; got /,
  grouping_policy: /flags must not carry a grouping policy; declare it as grouping; got /,
  unknown_flag: /flags holds a bit no NumStyles member names; got /,
  flags: /Date options flags must be DateStyles members combined; got /,
  pivot: /Date options pivot must be 0 \(none\) or a year from 100 to 9999; got /,
  layout: /Month names must be packed entries: a month byte/,
  month: /Month names must each name a month, a whole number from 1 to 12/,
  utf8: /Month names must be whole code points, not surrogate halves or bytes that are not UTF-8/,
  name_length: /Month names must each be 1 to 64 UTF-8 bytes/,
  character: /Month names must hold no ASCII digit, slash, hyphen or white space/,
  month_missing: /Month names must name every month from 1 to 12 at least once/,
  duplicate: /Month names must differ once case-folded, and two fold to the same spelling/,
}

// Holds a refused declaration to the message of the rule that won, and to no other's.
function holdsRule(ctx: string, message: string, rule: string): void {
  assert.match(message, ruleMessages[rule as keyof typeof ruleMessages], ctx)
  const same = (r: string) => r === 'length' || r === 'digit'
  for (const [other, pattern] of Object.entries(ruleMessages)) {
    if (other === rule || (same(other) && same(rule))) continue
    assert.doesNotMatch(message, pattern, `${ctx}: reads as ${other}`)
  }
}

// A vocabulary's packed layout as the spec writes it: a month byte, a little-endian u32 byte
// length, the UTF-8; then whatever raw bytes the row appends.
function packMonths(months: [number, string][], append: number[] = []): Uint8Array {
  const out: number[] = []
  for (const [month, name] of months) {
    const bytes = encoder.encode(name)
    out.push(month, bytes.length & 255, (bytes.length >> 8) & 255, 0, 0, ...bytes)
  }
  return Uint8Array.from([...out, ...append])
}

test('formats.json', () => {
  // The white_space rows pin a predicate; this binding calls the core's own `is_white_space`
  // rather than restating it, and the Rust replayer holds that predicate to them. The
  // declaration rows go through the public constructors, and each refusal is held to the
  // message of the rule that wins. A vocabulary is declared both ways, from its entries and
  // from the bytes the row spells, and must be judged alike.
  const counts = {
    white_space: 0,
    num_format: 0,
    day_periods: 0,
    date_options: 0,
    month_names: 0,
  }
  const vectors = load('formats.json')
  for (const v of vectors) {
    const ctx = `formats.json ${JSON.stringify(v)}`
    const kind = v.kind as keyof typeof counts
    counts[kind]++
    if (kind === 'white_space') continue
    const declarations: { ok: boolean; message?: string; value?: unknown }[] = []
    if (kind === 'num_format') {
      declarations.push(NumFormat.declare(formatSpec(v.format as Record<string, string | number>)))
    } else if (kind === 'day_periods') {
      declarations.push(DateFormat.declare({ order: DateOrder.DayMonthYear, am: v.am, pm: v.pm }))
    } else if (kind === 'date_options') {
      declarations.push(
        DateOptions.declare({ flags: v.flag_bits ?? styleOf(v.flags), pivot: v.pivot }),
      )
    } else {
      const months = v.months as [number, string][]
      const bytes = packMonths(months, v.append_bytes)
      declarations.push(MonthNames.fromPacked(bytes))
      if (v.append_bytes === undefined) {
        const built = MonthNames.declare(months)
        declarations.push(built)
        if (built.ok) assert.deepEqual(built.value.packed, bytes, `${ctx}: packs back`)
      }
    }
    for (const declared of declarations) {
      if (v.expect === 'ok') {
        assert.ok(declared.ok, `${ctx}: ${declared.message ?? ''}`)
        continue
      }
      assert.ok(!declared.ok, `${ctx}: declared`)
      holdsRule(ctx, declared.message as string, v.rule as string)
    }
  }
  for (const [kind, count] of Object.entries(counts)) assert.ok(count > 0, kind)
  assert.equal(
    Object.values(counts).reduce((total, count) => total + count, 0),
    vectors.length,
  )
})
