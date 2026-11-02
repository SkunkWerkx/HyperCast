// The binding's promise: no door throws, whatever it is handed. Every door is called with
// every argument below in every position (and with none), and each call must return a
// well-formed verdict: `ok` true with a value, or `ok` false with a fault whose reason is one
// of the three (or, from a checked door, a violation) whose span is two whole numbers inside
// the input, in its own units.
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { test }: typeof import('node:test') = require('node:test')
const { types }: typeof import('node:util') = require('node:util')
const api = require('../index.js') as typeof import('../index.js')
const { Constraints, DateFormat, DateOrder, Door, NumFormat } = api

type Door = (...args: unknown[]) => unknown

const doors: Record<string, Door> = {
  castBool: api.castBool,
  castChar: api.castChar,
  castI8: api.castI8,
  castI16: api.castI16,
  castI32: api.castI32,
  castI64: api.castI64,
  castU8: api.castU8,
  castU16: api.castU16,
  castU32: api.castU32,
  castU64: api.castU64,
  castF32: api.castF32,
  castF64: api.castF64,
  castDecimal: api.castDecimal,
  castUuid: api.castUuid,
  castTimestamp: api.castTimestamp,
  castUnix: api.castUnix,
  castExcelSerial: api.castExcelSerial,
  castDate: api.castDate,
  castDatetime: api.castDatetime,
  castTime: api.castTime,
  castDuration: api.castDuration,
  castDecimalFromNumber: api.castDecimalFromNumber,
  castExcelSerialFromNumber: api.castExcelSerialFromNumber,
  castExcelTime: api.castExcelTime,
  castExcelDuration: api.castExcelDuration,
  castChecked: api.castChecked,
  castCheckedFromNumber: api.castCheckedFromNumber,
  castText: api.castText,
} as unknown as Record<string, Door>

const dateFormat = DateFormat.declare({ order: DateOrder.DayMonthYear, am: 'vorm.', pm: 'nachm.' })
assert.ok(dateFormat.ok)
const numeric = Constraints.declare({ door: Door.I32, maximum: '40', multipleOf: '2' })
assert.ok(numeric.ok)
const textual = Constraints.declare({
  door: Door.Text,
  trim: true,
  maxLength: 3,
  enum: ['a', '42'],
})
assert.ok(textual.ok)

function detached(): Uint8Array {
  const view = new Uint8Array(8)
  structuredClone(view.buffer, { transfer: [view.buffer] })
  return view
}

// Everything a JavaScript caller could pass, the types' contract broken every way it can be.
const hostile: [string, unknown][] = [
  ['undefined', undefined],
  ['null', null],
  ['true', true],
  ['0', 0],
  ['1', 1],
  ['2', 2],
  ['-1', -1],
  ['1.5', 1.5],
  ['NaN', Number.NaN],
  ['Infinity', Number.POSITIVE_INFINITY],
  ['-0', -0],
  ['2^32', 2 ** 32],
  ['1n', 1n],
  ['symbol', Symbol('x')],
  ['empty string', ''],
  ['whitespace', ' \t\n'],
  ['digits', '42'],
  ['lone surrogate', '\ud800x'],
  ['astral', '😀1'],
  ['NUL', '\u0000'],
  ['a long string', '9'.repeat(1 << 20)],
  ['Uint8Array', new Uint8Array([0x34, 0x32])],
  ['invalid UTF-8', new Uint8Array([0xff, 0xfe, 0x31])],
  ['Buffer', Buffer.from('7')],
  ['empty Uint8Array', new Uint8Array(0)],
  ['subarray', new Uint8Array([0x20, 0x31, 0x32, 0x20]).subarray(1, 3)],
  ['detached Uint8Array', detached()],
  ['Uint8ClampedArray', new Uint8ClampedArray([0x31])],
  ['Int8Array', new Int8Array([0x31])],
  ['Uint16Array', new Uint16Array([0x31])],
  ['DataView', new DataView(new ArrayBuffer(2))],
  ['ArrayBuffer', new ArrayBuffer(2)],
  ['SharedArrayBuffer view', new Uint8Array(new SharedArrayBuffer(2))],
  ['array', [0x31]],
  ['plain object', { decimalSep: '.', groupSep: ',', flags: 0 }],
  ['Date', new Date(0)],
  ['function', () => 1],
  ['String object', new String('1')],
  [
    'Proxy whose get trap throws',
    new Proxy(
      {},
      {
        get() {
          throw new Error('trap')
        },
      },
    ),
  ],
  ['NumFormat', NumFormat.invariant()],
  ['DateFormat', dateFormat.value],
  ['NumFormat.prototype', NumFormat.prototype],
  ['Object.create(NumFormat.prototype)', Object.create(NumFormat.prototype)],
  ['DateOrder member', DateOrder.MonthDayYear],
  ['Constraints at I32', numeric.value],
  ['Constraints at Text', textual.value],
  ['Constraints.prototype', Constraints.prototype],
  ['a ConstraintsSpec', { door: Door.Text, enum: ['\ud800', 1n] }],
  ['an array holding a Proxy', [new Proxy({}, {})]],
]

// A Proxy whose [[GetPrototypeOf]] throws: a trap that throws, or a revoked Proxy. Natively
// no door or declaration reaches the prototype, so these join the hostile arguments there.
// Under the WASI build they throw, a recorded loss: emnapi's handle lookup tests every value
// `instanceof WeakRef`, which runs the trap, and the exception unwinds through the module
// before any Rust runs (README.md, "When no native addon fits").
const revoked = Proxy.revocable({}, {})
revoked.revoke()
const prototypeThrowing: [string, unknown][] = [
  [
    'Proxy whose getPrototypeOf trap throws',
    new Proxy(
      {},
      {
        getPrototypeOf() {
          throw new Error('trap')
        },
      },
    ),
  ],
  ['revoked Proxy', revoked.proxy],
]
const wasi =
  (require('../binding.js') as { __napiBindingTarget: string }).__napiBindingTarget !== 'native'
if (!wasi) hostile.push(...prototypeThrowing)

// node:util's check, not instanceof, which would run the Proxy's getPrototypeOf trap.
const units = (input: unknown): number | undefined => {
  if (typeof input === 'string') return input.length
  if (types.isUint8Array(input)) return input.length
  return undefined
}

function wellFormed(name: string, args: unknown[], verdict: unknown): void {
  const ctx = `${name}(${args.map((a) => typeof a).join(', ')})`
  assert.equal(typeof verdict, 'object', ctx)
  const v = verdict as {
    ok: unknown
    value?: unknown
    fault?: Record<string, unknown>
    violation?: Record<string, unknown>
  }
  assert.equal(typeof v.ok, 'boolean', ctx)
  if (v.ok) {
    assert.ok('value' in v, ctx)
    assert.ok(!('fault' in v) && !('violation' in v), ctx)
    return
  }
  assert.ok(!('value' in v), ctx)
  assert.notEqual('fault' in v, 'violation' in v, ctx)
  const fault = (v.fault ?? v.violation) as { reason?: unknown; offset: unknown; length: unknown }
  if (v.fault) {
    assert.ok(['empty', 'malformed', 'outOfRange'].includes(fault.reason as string), ctx)
  } else {
    assert.equal(typeof (v.violation as { keyword: unknown }).keyword, 'string', ctx)
  }
  assert.ok(Number.isInteger(fault.offset) && (fault.offset as number) >= 0, ctx)
  assert.ok(Number.isInteger(fault.length) && (fault.length as number) >= 0, ctx)
  const n = units(args[0])
  if (n !== undefined) {
    assert.ok((fault.offset as number) + (fault.length as number) <= n, `${ctx}: span past input`)
  }
}

test('no door throws, for any argument in any position', () => {
  let calls = 0
  for (const [name, door] of Object.entries(doors)) {
    const call = (args: unknown[]) => {
      let verdict: unknown
      assert.doesNotThrow(
        () => {
          verdict = door(...args)
        },
        `${name}(${args.map((a) => String(typeof a)).join(', ')}) threw`,
      )
      wellFormed(name, args, verdict)
      calls++
    }
    call([])
    for (const [, first] of hostile) {
      call([first])
      for (const [, second] of hostile) call([first, second])
    }
  }
  assert.ok(calls > 28 * hostile.length * hostile.length)
})

test('the declarations never throw either', () => {
  // A getter or Proxy trap of the caller's that throws is cleared and answered as a refusal.
  const declarations = [NumFormat.declare, DateFormat.declare, Constraints.declare]
  const throwing = new Proxy(
    {},
    {
      get() {
        throw new Error('trap')
      },
    },
  )
  // Each declaration field also takes every hostile value, and a Proxy that throws.
  const specs: [string, unknown][] = [...hostile]
  for (const field of ['door', 'minimum', 'multipleOf', 'rounding', 'maxLength', 'trim', 'enum']) {
    for (const [name, value] of [...hostile, ['throwing Proxy', throwing] as const]) {
      specs.push([`${field}: ${name}`, { door: Door.I32, [field]: value }])
      specs.push([`${field}: ${name} at Text`, { door: Door.Text, [field]: value }])
    }
  }
  specs.push(['enum with a throwing entry', { door: Door.Text, enum: [throwing] }])
  for (const [name, spec] of specs) {
    for (const declare of declarations) {
      let declared: unknown
      assert.doesNotThrow(() => {
        declared = (declare as (spec: unknown) => unknown)(spec)
      }, `${name}: the declaration threw`)
      const d = declared as { ok: unknown; value?: unknown; message?: unknown }
      assert.equal(typeof d.ok, 'boolean')
      if (!d.ok) assert.equal(typeof d.message, 'string')
    }
    let code: unknown
    assert.doesNotThrow(() => {
      code = (api.castConstraintsCheck as (spec: unknown) => unknown)(spec)
    }, `${name}: castConstraintsCheck threw`)
    assert.ok(Number.isInteger(code) && (code as number) >= -1 && (code as number) <= 11, name)
    let converted: unknown
    assert.doesNotThrow(() => {
      converted = (api.castDeclaredDecimal as (text: unknown) => unknown)(spec)
    }, `${name}: castDeclaredDecimal threw`)
    assert.equal(typeof (converted as { ok: unknown }).ok, 'boolean', name)
  }
})

test('a contract violation is malformed with an empty span; absent text is empty', () => {
  const contract = { ok: false, fault: { reason: 'malformed', offset: 0, length: 0 } }
  const empty = { ok: false, fault: { reason: 'empty', offset: 0, length: 0 } }
  const castI32 = api.castI32 as unknown as Door
  const castUnix = api.castUnix as unknown as Door
  const castExcelTime = api.castExcelTime as unknown as Door
  assert.deepEqual(castI32(42), contract)
  assert.deepEqual(castI32(new Uint16Array([0x31])), contract)
  assert.deepEqual(castI32('1', {}), contract)
  assert.deepEqual(castI32('1', dateFormat.value), contract)
  assert.deepEqual(castUnix('1', 9), contract)
  assert.deepEqual(castUnix('1', 1.5), contract)
  assert.deepEqual(castUnix('1'), contract)
  assert.deepEqual(castExcelTime('0.5'), contract)
  assert.deepEqual(castI32(undefined), empty)
  assert.deepEqual(castI32(null), empty)
  assert.deepEqual(castI32(), empty)
  // The contract is read before the text: absent text under a broken declaration is malformed.
  assert.deepEqual(castUnix(null, 9), contract)
})

test('under WASI, a Proxy whose prototype lookup throws still throws (the recorded loss)', {
  skip: !wasi && 'native: these are among the hostile arguments above',
}, () => {
  for (const [name, proxy] of prototypeThrowing) {
    assert.throws(() => api.castBool(proxy as string), Error, name)
    assert.throws(() => api.castI32('1', proxy as never), Error, name)
    assert.throws(() => NumFormat.declare(proxy as never), Error, name)
    // The module is still sound after the exception unwound through it.
    assert.deepEqual(api.castI32('(1,234)'), { ok: true, value: -1234 })
  }
})
