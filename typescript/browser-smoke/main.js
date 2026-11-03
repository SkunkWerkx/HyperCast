// The corpus, run against the WASI package in a browser. Each check appends a line to
// `#out`; the last line is `PASS`, or the first failure is `FAIL: <reason>` and nothing
// follows it. `check-dom.mjs` reads that verdict from the DOM Chrome dumps.
//
// `tests/corpus.test.ts` is the source of truth for the corpus replay; it is ported here
// file for file, both forms of every vector (its UTF-8 bytes, with byte spans verbatim, and
// the string, with spans in UTF-16 code units), since the browser has neither `node:test`
// nor `node:assert`. The files come in as text and are parsed with the reviver the Node test
// uses, so an integer past 2^53 keeps its digits.
import booleanText from '../../corpus/boolean.json?raw'
import charText from '../../corpus/char.json?raw'
import constraintsText from '../../corpus/constraints.json?raw'
import dateText from '../../corpus/date.json?raw'
import dateOrderText from '../../corpus/date_order.json?raw'
import datetimeText from '../../corpus/datetime.json?raw'
import decimalText from '../../corpus/decimal.json?raw'
import declarationsText from '../../corpus/declarations.json?raw'
import durationText from '../../corpus/duration.json?raw'
import excelSerialText from '../../corpus/excel_serial.json?raw'
import integerText from '../../corpus/integer.json?raw'
import realText from '../../corpus/real.json?raw'
import timeText from '../../corpus/time.json?raw'
import timestampText from '../../corpus/timestamp.json?raw'
import typedText from '../../corpus/typed.json?raw'
import unixText from '../../corpus/unix.json?raw'
import uuidText from '../../corpus/uuid.json?raw'

const out = document.getElementById('out')
const log = (line) => {
  out.textContent += `${line}\n`
}

const parse = (text) =>
  JSON.parse(text, (_key, value, context) => {
    if (typeof value !== 'number' || Number.isSafeInteger(value)) return value
    if (context?.source === undefined) throw new Error('JSON.parse revivers get no source text')
    return /^-?\d+$/.test(context.source) ? context.source : value
  })

// Structural equality over the plain values a door answers.
function same(a, b) {
  if (Object.is(a, b) || a === b) return true
  if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false
  const keys = Object.keys(a)
  if (keys.length !== Object.keys(b).length) return false
  return keys.every((key) => same(a[key], b[key]))
}

// A double's bits, so 0 and -0 differ: the core never delivers a negative zero.
function bits(value) {
  const view = new DataView(new ArrayBuffer(8))
  view.setFloat64(0, value)
  return view.getBigUint64(0)
}
const sameBits = (a, e) => typeof a === 'number' && typeof e === 'number' && bits(a) === bits(e)

function check(ok, context) {
  if (!ok) throw new Error(context)
}

const encoder = new TextEncoder()
const decoder = new TextDecoder()
const reasons = { empty: 'empty', malformed: 'malformed', out_of_range: 'outOfRange' }

function utf16Span(input, [offset, length]) {
  const bytes = encoder.encode(input)
  const start = decoder.decode(bytes.subarray(0, offset)).length
  const end = decoder.decode(bytes.subarray(0, offset + length)).length
  return [start, end - start]
}

function replay(file, v, door, expected, equal = (a, e) => a === e) {
  const bytes = encoder.encode(v.input)
  for (const [text, units, isBytes] of [
    [bytes, bytes.length, true],
    [v.input, v.input.length, false],
  ]) {
    const ctx = `${file} ${JSON.stringify(v)} as ${isBytes ? 'bytes' : 'string'}`
    const verdict = door(text)
    if (verdict.ok) {
      check(v.expect === 'ok', `${ctx}: unexpectedly cast to ${String(verdict.value)}`)
      check(equal(verdict.value, expected), `${ctx}: ${String(verdict.value)} != ${expected}`)
      continue
    }
    const { reason, offset, length } = verdict.fault
    check(v.expect !== 'ok', `${ctx}: faulted ${reason}`)
    check(reason === reasons[v.expect], `${ctx}: reason ${reason}`)
    check(offset + length <= units, `${ctx}: span ${offset}+${length} past ${units}`)
    if (v.expect === 'empty') check(offset === 0 && length === 0, `${ctx}: empty span`)
    if (v.fault !== undefined) {
      const span = isBytes ? v.fault : utf16Span(v.input, v.fault)
      check(offset === span[0] && length === span[1], `${ctx}: span ${offset}+${length}`)
    }
  }
}

try {
  // The package's `browser` condition resolves to napi's `hypercast.wasip1-browser.js`, which
  // fetches and instantiates the module under top-level await and then exports the bindings
  // as plain named exports. The import is dynamic so that a failure to load lands in `#out`
  // as a FAIL line instead of leaving the page blank.
  const api = await import('@skunkwerkx/hypercast-wasm32-wasip1')
  const { Constraints, DateFormat, Door, NumFormat, Rounding } = api
  log(`loaded: target=${api.__napiBindingTarget}`)

  const formats = new Map()
  const formatOf = (v) => {
    if (v.format === undefined) return undefined
    const key = JSON.stringify(v.format)
    if (!formats.has(key)) {
      const f = v.format
      const declared = NumFormat.declare({
        decimalSep: f.decimal_sep,
        groupSep: f.group_sep,
        flags: f.flags,
        currency: f.currency,
        currencyDecimalSep: f.currency_decimal_sep,
        currencyGroupSep: f.currency_group_sep,
        minusSign: f.minus_sign,
        plusSign: f.plus_sign,
        percent: f.percent,
        digitZero: f.digit_zero,
      })
      check(declared.ok, `${key}: ${declared.message}`)
      formats.set(key, declared.value)
    }
    return formats.get(key)
  }
  const timestampOf = (v) =>
    v.seconds === undefined ? undefined : { seconds: v.seconds, nanos: v.nanos }
  const dateOf = (v) =>
    v.year === undefined ? undefined : { year: v.year, month: v.month, day: v.day }
  const timeOf = (nanosOfDay) => {
    const seconds = Math.floor(nanosOfDay / 1e9)
    return {
      hour: Math.floor(seconds / 3600),
      minute: Math.floor(seconds / 60) % 60,
      second: seconds % 60,
      nanos: nanosOfDay % 1e9,
    }
  }
  const civilOf = (v) =>
    v.year === undefined ? undefined : { ...dateOf(v), ...timeOf(v.nanos_of_day) }
  const integerDoors = {
    i8: api.castI8,
    i16: api.castI16,
    i32: api.castI32,
    i64: api.castI64,
    u8: api.castU8,
    u16: api.castU16,
    u32: api.castU32,
    u64: api.castU64,
  }

  // constraints.json, as tests/corpus.test.ts replays it: every row declared through the
  // public constructor, a text row as bytes and as the string, floats compared by bits.
  const doorOf = {
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
  const roundingOf = {
    strict: Rounding.Strict,
    midpoint: Rounding.Midpoint,
    floor: Rounding.Floor,
    ceiling: Rounding.Ceiling,
  }
  const specOf = (v) => {
    const { rounding, ...rest } = v.constraints
    const spec = { door: doorOf[v.door], ...rest }
    if (rounding !== undefined) spec.rounding = roundingOf[rounding]
    return spec
  }
  const expectedOf = (v) => {
    if (v.door === 'i64' || v.door === 'u64') return BigInt(v.value)
    if (v.door === 'f32') return Math.fround(v.value)
    return v.value
  }
  const judge = (ctx, v, verdict, span, units) => {
    if (verdict.ok) {
      check(v.expect === 'ok', `${ctx}: unexpectedly ${String(verdict.value)}`)
      const expected = expectedOf(v)
      const equal = v.door === 'f32' || v.door === 'f64' ? sameBits : (a, e) => a === e
      check(equal(verdict.value, expected), `${ctx}: ${String(verdict.value)}`)
      check(verdict.adjusted === v.adjusted, `${ctx}: adjusted`)
      check(verdict.enumIndex === (v.enum_index ?? null), `${ctx}: enumIndex`)
      return
    }
    const failed = verdict.violation ?? verdict.fault
    if (verdict.violation) {
      check(v.expect === 'violation', `${ctx}: violated ${failed.keyword}`)
      check(failed.keyword === v.keyword, `${ctx}: keyword ${failed.keyword}`)
    } else {
      check(failed.reason === reasons[v.expect], `${ctx}: reason ${failed.reason}`)
    }
    check(failed.offset + failed.length <= units, `${ctx}: span past ${units}`)
    if (span !== undefined) {
      check(failed.offset === span[0] && failed.length === span[1], `${ctx}: span`)
    }
  }
  const constrained = (v) => {
    const ctx = `constraints.json ${JSON.stringify(v)}`
    const spec = specOf(v)
    const declared = Constraints.declare(spec)
    if (v.expect === 'refused') {
      check(!declared.ok, `${ctx}: declared`)
      check(declared.message.startsWith(`Constraints refused (code ${v.refusal}):`), ctx)
      check(api.castConstraintsCheck(spec) === v.refusal, `${ctx}: check`)
      return
    }
    check(declared.ok, `${ctx}: ${declared.message}`)
    const constraints = declared.value
    if (v.from_f64 !== undefined) {
      const value = typeof v.from_f64 === 'string' ? Number(v.from_f64) : v.from_f64
      judge(ctx, v, api.castCheckedFromNumber(value, constraints), v.fault, 0)
      return
    }
    const format = formatOf(v)
    const door = (text) =>
      v.door === 'text'
        ? api.castText(text, constraints)
        : api.castChecked(text, constraints, format)
    if (v.input_bytes !== undefined) {
      const bytes = new Uint8Array(v.input_bytes)
      judge(`${ctx} as bytes`, v, door(bytes), v.fault, bytes.length)
      return
    }
    const bytes = encoder.encode(v.input)
    judge(`${ctx} as bytes`, v, door(bytes), v.fault, bytes.length)
    const span = v.fault === undefined ? undefined : utf16Span(v.input, v.fault)
    judge(`${ctx} as string`, v, door(v.input), span, v.input.length)
  }
  // declarations.json: the conversion, held to the raw triple its canonical text spells.
  const declaredDecimal = (v) => {
    const ctx = `declarations.json ${JSON.stringify(v)}`
    const text = v.input_bytes === undefined ? v.input : new Uint8Array(v.input_bytes)
    const declared = api.castDeclaredDecimal(text)
    if (v.expect === 'refused') {
      check(!declared.ok, `${ctx}: converted`)
      check(declared.message.startsWith(`Constraints refused (code ${v.refusal}):`), ctx)
      return
    }
    check(declared.ok, `${ctx}: ${declared.message}`)
    const canonical = declared.value
    const negative = canonical.startsWith('-')
    const unsigned = negative ? canonical.slice(1) : canonical
    const point = unsigned.indexOf('.')
    const scale = point < 0 ? 0 : unsigned.length - point - 1
    check(canonical === v.value, `${ctx}: ${canonical}`)
    check(negative === v.negative && scale === v.scale, `${ctx}: sign or scale`)
    check(BigInt(unsigned.replace('.', '')) === BigInt(v.magnitude), `${ctx}: magnitude`)
  }

  const files = [
    ['boolean.json', booleanText, (v) => replay('boolean.json', v, api.castBool, v.value)],
    [
      'char.json',
      charText,
      (v) =>
        replay(
          'char.json',
          v,
          api.castChar,
          v.value,
          (a, code) => [...a].length === 1 && a.codePointAt(0) === code,
        ),
    ],
    [
      'integer.json',
      integerText,
      (v) => {
        const wide = v.type === 'i64' || v.type === 'u64'
        const expected = v.value === undefined ? undefined : wide ? BigInt(v.value) : v.value
        const format = formatOf(v)
        replay('integer.json', v, (text) => integerDoors[v.type](text, format), expected)
      },
    ],
    [
      'real.json',
      realText,
      (v) => {
        const door = v.type === 'f32' ? api.castF32 : api.castF64
        const expected =
          typeof v.value === 'number' && v.type === 'f32' ? Math.fround(v.value) : v.value
        const format = formatOf(v)
        replay('real.json', v, (text) => door(text, format), expected, sameBits)
      },
    ],
    [
      'decimal.json',
      decimalText,
      (v) => {
        const format = formatOf(v)
        replay('decimal.json', v, (text) => api.castDecimal(text, format), v.value)
      },
    ],
    [
      'uuid.json',
      uuidText,
      (v) => {
        const expected =
          typeof v.value === 'string'
            ? v.value.replace(/^(.{8})(.{4})(.{4})(.{4})(.{12})$/, '$1-$2-$3-$4-$5')
            : undefined
        replay('uuid.json', v, api.castUuid, expected)
      },
    ],
    [
      'timestamp.json',
      timestampText,
      (v) => replay('timestamp.json', v, api.castTimestamp, timestampOf(v), same),
    ],
    [
      'unix.json',
      unixText,
      (v) => replay('unix.json', v, (t) => api.castUnix(t, v.precision), timestampOf(v), same),
    ],
    [
      'excel_serial.json',
      excelSerialText,
      (v) =>
        replay(
          'excel_serial.json',
          v,
          (t) => api.castExcelSerial(t, v.epoch),
          timestampOf(v),
          same,
        ),
    ],
    ['date.json', dateText, (v) => replay('date.json', v, api.castDate, dateOf(v), same)],
    [
      'date_order.json',
      dateOrderText,
      (v) => replay('date_order.json', v, (t) => api.castDate(t, v.order), dateOf(v), same),
    ],
    [
      'datetime.json',
      datetimeText,
      (v) => {
        let format = v.order
        if (v.am !== undefined || v.pm !== undefined) {
          const declared = DateFormat.declare({ order: v.order, am: v.am, pm: v.pm })
          check(declared.ok, `datetime.json ${JSON.stringify(v)}: ${declared.message}`)
          format = declared.value
        }
        replay('datetime.json', v, (t) => api.castDatetime(t, format), civilOf(v), same)
      },
    ],
    [
      'time.json',
      timeText,
      (v) =>
        replay(
          'time.json',
          v,
          api.castTime,
          v.nanos === undefined ? undefined : timeOf(v.nanos),
          same,
        ),
    ],
    [
      'duration.json',
      durationText,
      (v) => replay('duration.json', v, api.castDuration, timestampOf(v), same),
    ],
    [
      'typed.json',
      typedText,
      (v) => {
        const view = new DataView(new ArrayBuffer(8))
        view.setBigUint64(0, BigInt(`0x${v.bits}`))
        const value = view.getFloat64(0)
        const doors = {
          decimal: () => [api.castDecimalFromNumber(value), v.value],
          excel_serial: () => [api.castExcelSerialFromNumber(value, v.epoch), civilOf(v)],
          excel_time: () => [
            api.castExcelTime(value),
            v.nanos === undefined ? undefined : timeOf(v.nanos),
          ],
          excel_duration: () => [api.castExcelDuration(value), timestampOf(v)],
        }
        const [verdict, expected] = doors[v.door]()
        const ctx = `typed.json ${JSON.stringify(v)}`
        if (verdict.ok) {
          check(v.expect === 'ok', `${ctx}: unexpectedly ok`)
          check(same(verdict.value, expected), `${ctx}: ${JSON.stringify(verdict.value)}`)
        } else {
          check(verdict.fault.reason === reasons[v.expect], `${ctx}: ${verdict.fault.reason}`)
          check(verdict.fault.offset === 0 && verdict.fault.length === 0, `${ctx}: span`)
        }
      },
    ],
    ['constraints.json', constraintsText, constrained],
    ['declarations.json', declarationsText, declaredDecimal],
  ]
  for (const [name, text, run] of files) {
    const vectors = parse(text)
    for (const v of vectors) run(v)
    log(`${name}: ${vectors.length} vectors`)
  }

  // No door throws for an argument of the wrong type: a contract violation is a verdict.
  const contract = api.castI32(42, {})
  check(
    !contract.ok && contract.fault.reason === 'malformed' && contract.fault.length === 0,
    `castI32(42, {}): ${JSON.stringify(contract)}`,
  )
  const refused = NumFormat.declare({ decimalSep: ',', groupSep: ',', flags: 0 })
  check(!refused.ok && typeof refused.message === 'string', 'NumFormat.declare refusal')
  log('contract: a wrong-typed argument and a refused declaration, neither thrown')

  // castText picks plain or constrained by its argument count, here as in the package: a
  // failed declaration's missing `.value` is a contract violation, never "no rules".
  const missing = Constraints.declare({ door: Door.Text, minLength: 5, maxLength: 2 }).value
  for (const [args, want] of [
    [['x', missing], 'malformed'],
    [['x', null], 'malformed'],
    [['x'], 'ok'],
  ]) {
    const verdict = api.castText(...args)
    const got = verdict.ok ? 'ok' : verdict.fault.reason
    check(got === want, `castText(${args.length} arguments): ${JSON.stringify(verdict)}`)
  }
  log('castText: plain or constrained by argument count')

  log('PASS')
} catch (error) {
  log(`FAIL: ${error?.stack ?? error}`)
}
