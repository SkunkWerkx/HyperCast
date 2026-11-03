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
const { Constraints, DateFormat, Door, NumFormat, Rounding } = api
type Verdict<T> = import('../index.js').Verdict<T>
type CheckedVerdict<T> = import('../index.js').CheckedVerdict<T>
type ConstraintsSpec = import('../index.js').ConstraintsSpec
type Text = import('../index.js').Text

// One corpus record. Each file uses a subset of these fields; JSON.parse does not check them.
interface Vector {
  input: string
  expect: 'ok' | 'empty' | 'malformed' | 'out_of_range' | 'violation' | 'refused'
  fault?: [number, number]
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

// Runs `door` on the vector's bytes and on its string, and holds both verdicts to it.
function replay<T>(
  file: string,
  v: Vector,
  door: (text: Text) => Verdict<T>,
  expected: unknown,
  same: (actual: T, expected: unknown) => boolean = (a, e) => a === e,
): void {
  const forms: [Text, number, boolean][] = [
    [encoder.encode(v.input), encoder.encode(v.input).length, true],
    [v.input, v.input.length, false],
  ]
  for (const [text, units, bytes] of forms) {
    const ctx = `${file} ${JSON.stringify(v)} as ${bytes ? 'bytes' : 'string'}`
    const verdict = door(text)
    if (verdict.ok) {
      assert.equal(v.expect, 'ok', `${ctx}: unexpectedly cast to ${String(verdict.value)}`)
      assert.ok(same(verdict.value, expected), `${ctx}: ${String(verdict.value)} != ${expected}`)
      continue
    }
    const { reason, offset, length } = verdict.fault
    assert.notEqual(v.expect, 'ok', `${ctx}: faulted ${reason}`)
    assert.equal(reason, reasons[v.expect as keyof typeof reasons], ctx)
    assert.ok(offset + length <= units, `${ctx}: span ${offset}+${length} past ${units}`)
    if (v.expect === 'empty') assert.deepEqual([offset, length], [0, 0], ctx)
    if (v.fault !== undefined) {
      const span = bytes ? v.fault : utf16Span(v.input, v.fault)
      assert.deepEqual([offset, length], span, `${ctx}: span`)
    }
  }
}

const formats = new Map<string, InstanceType<typeof NumFormat>>()
function formatOf(v: Vector): InstanceType<typeof NumFormat> | undefined {
  if (v.format === undefined) return undefined
  const key = JSON.stringify(v.format)
  const cached = formats.get(key)
  if (cached) return cached
  const f = v.format
  const declared = NumFormat.declare({
    decimalSep: f.decimal_sep as string,
    groupSep: f.group_sep as string,
    flags: f.flags as number,
    currency: f.currency as string | undefined,
    currencyDecimalSep: f.currency_decimal_sep as string | undefined,
    currencyGroupSep: f.currency_group_sep as string | undefined,
    minusSign: f.minus_sign as string | undefined,
    plusSign: f.plus_sign as string | undefined,
    percent: f.percent as string | undefined,
    digitZero: f.digit_zero as string | undefined,
  })
  assert.ok(declared.ok, `${key}: ${declared.ok ? '' : declared.message}`)
  formats.set(key, declared.value)
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
]

test('every corpus file is replayed', () => {
  const present = readdirSync(CORPUS).filter((name) => name.endsWith('.json'))
  assert.deepEqual([...present].sort(), [...files].sort())
})

test('boolean.json', () => {
  for (const v of load('boolean.json')) replay('boolean.json', v, api.castBool, v.value)
})

test('char.json', () => {
  // The vector pins the code point; the door answers the one-character string.
  for (const v of load('char.json')) {
    replay('char.json', v, api.castChar, v.value, (actual, code) => {
      return [...actual].length === 1 && actual.codePointAt(0) === code
    })
  }
})

test('integer.json', () => {
  const doors = {
    i8: api.castI8,
    i16: api.castI16,
    i32: api.castI32,
    i64: api.castI64,
    u8: api.castU8,
    u16: api.castU16,
    u32: api.castU32,
    u64: api.castU64,
  } as const
  for (const v of load('integer.json')) {
    const type = v.type as keyof typeof doors
    const door = doors[type] as (
      text: Text,
      format?: InstanceType<typeof NumFormat>,
    ) => Verdict<number | bigint>
    const format = formatOf(v)
    // The 64-bit doors answer a bigint, exact over the whole range.
    const wide = type === 'i64' || type === 'u64'
    const expected = v.value === undefined ? undefined : wide ? BigInt(v.value as string) : v.value
    replay('integer.json', v, (text) => door(text, format), expected)
  }
})

test('real.json', () => {
  for (const v of load('real.json')) {
    const door = v.type === 'f32' ? api.castF32 : api.castF64
    const format = formatOf(v)
    const expected =
      typeof v.value === 'number' && v.type === 'f32' ? Math.fround(v.value) : v.value
    replay('real.json', v, (text) => door(text, format), expected, sameBits)
  }
})

test('decimal.json', () => {
  // The canonical text pins sign, digits and scale at once: "1.10" can only come back "1.1".
  for (const v of load('decimal.json')) {
    const format = formatOf(v)
    replay('decimal.json', v, (text) => api.castDecimal(text, format), v.value)
  }
})

test('uuid.json', () => {
  for (const v of load('uuid.json')) {
    const expected =
      typeof v.value === 'string'
        ? v.value.replace(/^(.{8})(.{4})(.{4})(.{4})(.{12})$/, '$1-$2-$3-$4-$5')
        : undefined
    replay('uuid.json', v, api.castUuid, expected)
  }
})

test('timestamp.json', () => {
  for (const v of load('timestamp.json')) {
    replay('timestamp.json', v, api.castTimestamp, timestampOf(v), deep)
  }
})

test('unix.json', () => {
  for (const v of load('unix.json')) {
    const door = (text: Text) => api.castUnix(text, v.precision as number)
    replay('unix.json', v, door, timestampOf(v), deep)
  }
})

test('excel_serial.json', () => {
  for (const v of load('excel_serial.json')) {
    const door = (text: Text) => api.castExcelSerial(text, v.epoch as number)
    replay('excel_serial.json', v, door, timestampOf(v), deep)
  }
})

test('date.json', () => {
  for (const v of load('date.json')) replay('date.json', v, api.castDate, dateOf(v), deep)
})

test('date_order.json', () => {
  for (const v of load('date_order.json')) {
    const door = (text: Text) => api.castDate(text, v.order as number)
    replay('date_order.json', v, door, dateOf(v), deep)
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
    replay('datetime.json', v, (text) => api.castDatetime(text, format), civilOf(v), deep)
  }
})

test('time.json', () => {
  for (const v of load('time.json')) {
    const expected = v.nanos === undefined ? undefined : timeOf(v.nanos)
    replay('time.json', v, api.castTime, expected, deep)
  }
})

test('duration.json', () => {
  for (const v of load('duration.json')) {
    replay('duration.json', v, api.castDuration, timestampOf(v), deep)
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
    let expected: unknown
    if (door === 'decimal') {
      verdict = api.castDecimalFromNumber(value)
      expected = v.value
    } else if (door === 'excel_serial') {
      verdict = api.castExcelSerialFromNumber(value, v.epoch as number)
      expected = civilOf(v)
    } else if (door === 'excel_time') {
      verdict = api.castExcelTime(value)
      expected = v.nanos === undefined ? undefined : timeOf(v.nanos)
    } else if (door === 'excel_duration') {
      verdict = api.castExcelDuration(value)
      expected = timestampOf(v)
    } else {
      throw new Error(`typed: unknown door ${door}`)
    }
    const ctx = `typed.json ${JSON.stringify(v)}`
    if (verdict.ok) {
      assert.equal(v.expect, 'ok', ctx)
      assert.deepEqual(verdict.value, expected, ctx)
    } else {
      assert.equal(verdict.fault.reason, reasons[v.expect as keyof typeof reasons], ctx)
      assert.deepEqual([verdict.fault.offset, verdict.fault.length], [0, 0], `${ctx}: span`)
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
  verdict: CheckedVerdict<unknown>,
  span: [number, number] | undefined,
  units: number,
): void {
  if (verdict.ok) {
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
    assert.equal(keyword, v.keyword, ctx)
    assert.ok(offset + length <= units, `${ctx}: span ${offset}+${length} past ${units}`)
    if (span !== undefined) assert.deepEqual([offset, length], span, `${ctx}: span`)
    return
  }
  const { reason, offset, length } = verdict.fault
  assert.equal(reason, reasons[v.expect as keyof typeof reasons], `${ctx}: ${reason}`)
  assert.ok(offset + length <= units, `${ctx}: span ${offset}+${length} past ${units}`)
  if (span !== undefined) assert.deepEqual([offset, length], span, `${ctx}: span`)
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
      judge(ctx, v, api.castCheckedFromNumber(value, constraints), v.fault, 0)
      typed++
      replayed++
      continue
    }
    const format = formatOf(v)
    const door = (text: Text) =>
      v.door === 'text'
        ? api.castText(text, constraints)
        : api.castChecked(text, constraints, format)
    if (v.input_bytes !== undefined) {
      // Bytes no string can spell (invalid UTF-8): the byte form alone.
      const bytes = new Uint8Array(v.input_bytes)
      judge(`${ctx} as bytes`, v, door(bytes), v.fault, bytes.length)
      replayed++
      continue
    }
    const bytes = encoder.encode(v.input)
    judge(`${ctx} as bytes`, v, door(bytes), v.fault, bytes.length)
    const span = v.fault === undefined ? undefined : utf16Span(v.input, v.fault)
    judge(`${ctx} as string`, v, door(v.input), span, v.input.length)
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
