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
