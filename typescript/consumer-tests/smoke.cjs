// What a consumer sees: require('@skunkwerkx/hypercast'), ask whether anything loaded, and if so use it.
// Prints one line, `available=<bool> target=<native|wasm32-wasip1|none> [version=<v>]`, which
// run.sh and container/consumer.sh assert on. When nothing loaded, the first door must throw
// ERR_HYPERCAST_UNAVAILABLE and its cause chain is printed one per line, so the negative
// controls can see that both the native attempts and the WASI one were made. binding.js is
// consulted for the target only after isAvailable() says true: index.js is what tolerates a
// failing binding.js, and this script never requires it when nothing loaded.
'use strict'
const fs = require('node:fs')
const path = require('node:path')
const api = require('@skunkwerkx/hypercast')

function fail(message) {
  process.stdout.write(`FAIL: ${message}\n`)
  process.exit(1)
}

if (!api.isAvailable()) {
  process.stdout.write('available=false target=none\n')
  let thrown
  try {
    api.castI32('1')
  } catch (error) {
    thrown = error
  }
  if (!thrown) fail('castI32() did not throw with nothing loaded')
  if (thrown.code !== 'ERR_HYPERCAST_UNAVAILABLE')
    fail(`castI32() threw ${thrown.code}, not ERR_HYPERCAST_UNAVAILABLE`)
  for (let cause = thrown.cause; cause; cause = cause.cause) {
    process.stdout.write(`cause: ${cause.constructor.name}: ${cause.message.split('\n')[0]}\n`)
  }
  process.exit(0)
}

const binding = require(
  path.join(path.dirname(require.resolve('@skunkwerkx/hypercast')), 'binding.js'),
)
process.stdout.write(
  `available=true target=${binding.__napiBindingTarget} version=${api.nativeVersion()}\n`,
)

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b)
const accounting = api.castI32('(1,234)')
if (!same(accounting, { ok: true, value: -1234 })) fail(`castI32: ${JSON.stringify(accounting)}`)
const wide = api.castU64('18446744073709551615')
if (!(wide.ok && wide.value === 2n ** 64n - 1n)) fail('castU64 of the u64 maximum')
const contract = api.castI32(42)
if (!same(contract, { ok: false, fault: { reason: 'malformed', offset: 0, length: 0 } }))
  fail(`castI32(42) is not a contract verdict: ${JSON.stringify(contract)}`)

const corpus = process.env.HYPERCAST_CORPUS
if (!corpus) fail('HYPERCAST_CORPUS is not set')
const load = (name) => JSON.parse(fs.readFileSync(path.join(corpus, name), 'utf8'))
const stamp = load('timestamp.json').find((v) => v.expect === 'ok')
const instant = api.castTimestamp(stamp.input)
if (!same(instant, { ok: true, value: { seconds: stamp.seconds, nanos: stamp.nanos } }))
  fail(`timestamp vector ${stamp.input}: ${JSON.stringify(instant)}`)
const spanned = load('integer.json').find((v) => v.fault && v.expect === 'malformed' && !v.format)
const bytes = api.castI32(Buffer.from(spanned.input), undefined)
if (bytes.ok || !same([bytes.fault.offset, bytes.fault.length], spanned.fault))
  fail(`integer vector ${spanned.input}: ${JSON.stringify(bytes)}`)
process.stdout.write('checks: i32 u64 contract timestamp-vector integer-vector OK\n')
