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
const { DateFormat, NumFormat } = api
type Verdict<T> = import('../index.js').Verdict<T>
type Text = import('../index.js').Text

// One corpus record. Each file uses a subset of these fields; JSON.parse does not check them.
interface Vector {
  input: string
  expect: 'ok' | 'empty' | 'malformed' | 'out_of_range'
  fault?: [number, number]
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
    replay('real.json', v, (text) => door(text, format), expected)
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
