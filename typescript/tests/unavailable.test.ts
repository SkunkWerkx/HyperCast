// What a consumer gets when neither the native addon nor the WebAssembly fallback loads: the
// package still requires, `isAvailable()` is false, the enums and `optional` are still there,
// and a door throws ERR_HYPERCAST_UNAVAILABLE with the loader's error as its cause. That
// throw is the one this package makes: an install problem, not an input, and no door could
// answer it with a verdict.
const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const { spawnSync }: typeof import('node:child_process') = require('node:child_process')
const {
  copyFileSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
}: typeof import('node:fs') = require('node:fs')
const { tmpdir }: typeof import('node:os') = require('node:os')
const path: typeof import('node:path') = require('node:path')
const { test }: typeof import('node:test') = require('node:test')
const api = require('../index.js') as typeof import('../index.js')
const native = require('../binding.js') as typeof import('../binding.js')

test('the fallback literals equal the native values', () => {
  assert.deepEqual({ ...api.NumStyles }, { ...native.NumStyles })
  assert.deepEqual({ ...api.UnixPrecision }, { ...native.UnixPrecision })
  assert.deepEqual({ ...api.DateOrder }, { ...native.DateOrder })
  assert.deepEqual({ ...api.ExcelEpoch }, { ...native.ExcelEpoch })
  assert.deepEqual({ ...api.Door }, { ...native.Door })
  assert.deepEqual({ ...api.Rounding }, { ...native.Rounding })
  assert.deepEqual({ ...api.Grouping }, { ...native.Grouping })
  assert.deepEqual({ ...api.DateStyles }, { ...native.DateStyles })
  // A string enum is a type in binding.d.ts and an object at runtime.
  assert.deepEqual({ ...api.Detail }, { ...(native as unknown as { Detail: object }).Detail })
  assert.deepEqual(
    { ...api.CastFailure },
    {
      Empty: 'empty',
      Malformed: 'malformed',
      OutOfRange: 'outOfRange',
    },
  )
})

test('isHyperCastError recognises the unavailable error and refuses everything else', () => {
  const error = Object.assign(new Error('x'), { code: 'ERR_HYPERCAST_UNAVAILABLE' })
  assert.equal(api.isHyperCastError(error), true)
  assert.equal(api.isHyperCastError(new Error('x')), false)
  assert.equal(api.isHyperCastError(null), false)
  assert.equal(api.isHyperCastError({ code: 'ERR_HYPERCAST_UNAVAILABLE' }), false)
  assert.equal(api.isHyperCastError(Object.assign(new Error('x'), { code: 42 })), false)
})

// The loader is made to fail in a process of its own (the same runtime, Node or Bun): a
// copy of index.js beside a binding.js that throws, as an install with no addon and no
// fallback leaves it. A preload hook would do on Node, but Bun's require does not consult one.
const SCRIPT = `
const assert = require('node:assert/strict')
const api = require(process.env.HYPERCAST_INDEX)
assert.equal(api.isAvailable(), false)
assert.equal(api.NumStyles.All, 95)
assert.equal(api.DateOrder.DayMonthYear, 3)
assert.equal(api.Door.Text, 18)
assert.equal(api.Door.Char, 23)
assert.equal(api.Grouping.Western, 256)
assert.equal(api.DateStyles.CommonForms, 7)
assert.equal(api.Detail.GroupSize, 'groupSize')
assert.equal(api.Rounding.Midpoint, 1)
assert.equal(api.Keyword.MultipleOf, 'multipleOf')
assert.equal(api.optional({ ok: false, fault: { reason: 'empty', offset: 0, length: 0 } }), null)
for (const call of [
  () => api.castI32('1'),
  () => api.NumFormat.declare({}),
  () => api.nativeVersion(),
  () => api.Constraints.declare({}),
  () => api.castChecked('1'),
  () => api.castText('1'),
  () => api.castConstraintsCheck({}),
  () => api.castDeclaredDecimal('1'),
  () => api.castDetailed(4, '1'),
  () => api.castDetailedFromNumber(19, 1),
  () => api.castCheckedDetailed('1'),
  () => api.castCheckedDetailedFromNumber(1),
  () => api.DateOptions.declare({}),
  () => api.DateOptions.commonForms(),
  () => api.MonthNames.declare([]),
  () => api.MonthNames.fromPacked(new Uint8Array()),
]) {
  let thrown
  try { call() } catch (error) { thrown = error }
  assert.ok(api.isHyperCastError(thrown), String(thrown))
  assert.equal(thrown.code, 'ERR_HYPERCAST_UNAVAILABLE')
  assert.equal(thrown.cause.message, 'no addon here')
}
`

test('with no addon, the package loads and every door throws ERR_HYPERCAST_UNAVAILABLE', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'hypercast-unavailable-'))
  try {
    copyFileSync(path.resolve(__dirname, '..', 'index.js'), path.join(dir, 'index.js'))
    writeFileSync(path.join(dir, 'binding.js'), "throw new Error('no addon here')\n")
    const result = spawnSync(process.execPath, ['-e', SCRIPT], {
      env: { ...process.env, HYPERCAST_INDEX: path.join(dir, 'index.js') },
      encoding: 'utf8',
    })
    assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

// The addon must be this package's. A stub binding.js stands in for a stale platform package or an
// addon NAPI_RS_NATIVE_LIBRARY_PATH names: another core version, or one with no castDetailed.
const PAIRING = `
const assert = require('node:assert/strict')
const api = require(process.env.HYPERCAST_INDEX)
const want = JSON.parse(process.env.HYPERCAST_WANT)
assert.equal(api.isAvailable(), want.available)
if (!want.available) {
  let thrown
  try { api.castI32('1') } catch (error) { thrown = error }
  assert.ok(api.isHyperCastError(thrown), String(thrown))
  assert.match(thrown.cause.message, new RegExp(want.cause))
  assert.equal(api.Grouping.Western, 256)
} else {
  assert.equal(api.nativeVersion(), want.version)
}
`

function pairing(stub: (version: string) => string, want: Record<string, unknown>): void {
  const dir = mkdtempSync(path.join(tmpdir(), 'hypercast-pairing-'))
  try {
    copyFileSync(path.resolve(__dirname, '..', 'index.js'), path.join(dir, 'index.js'))
    copyFileSync(path.resolve(__dirname, '..', 'package.json'), path.join(dir, 'package.json'))
    const version = JSON.parse(
      readFileSync(path.resolve(__dirname, '..', 'package.json'), 'utf8'),
    ).version
    writeFileSync(path.join(dir, 'binding.js'), stub(version))
    const result = spawnSync(process.execPath, ['-e', PAIRING], {
      env: {
        ...process.env,
        HYPERCAST_INDEX: path.join(dir, 'index.js'),
        HYPERCAST_WANT: JSON.stringify({ version, ...want }),
      },
      encoding: 'utf8',
    })
    assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

test('an addon of another core version is treated as not loaded', () => {
  pairing(
    () => "module.exports = { nativeVersion: () => '0.9.0', castDetailed() {}, castI32() {} }\n",
    { available: false, cause: 'the addon is core 0\\.9\\.0, and this package is' },
  )
})

test('an addon with no castDetailed (a 1.0 addon) is treated as not loaded', () => {
  pairing((version) => `module.exports = { nativeVersion: () => '${version}', castI32() {} }\n`, {
    available: false,
    cause: 'no castDetailed',
  })
})

test('an addon that cannot report its version is treated as not loaded', () => {
  pairing(() => 'module.exports = { castDetailed() {} }\n', {
    available: false,
    cause: 'nativeVersion is not a function',
  })
})

test('an addon of this version with castDetailed is the addon', () => {
  pairing(
    (version) =>
      `module.exports = { nativeVersion: () => '${version}', castDetailed() {}, castI32() {} }\n`,
    { available: true },
  )
})
