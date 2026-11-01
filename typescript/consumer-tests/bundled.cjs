// The public-API half of smoke.cjs, for the bundler scenario: a self-contained bundle is
// judged on what it does, not on which file it loaded, so binding.js is never inspected. The
// scenario runs it under NAPI_RS_FORCE_WASI=error, under which the loader never consults the
// native addon, so a pass here is a pass on the WASI module.
'use strict'
const fs = require('node:fs')
const path = require('node:path')
const api = require('@skunkwerkx/hypercast')

function fail(message) {
  process.stdout.write(`FAIL: ${message}\n`)
  process.exit(1)
}

if (!api.isAvailable()) fail('isAvailable() is false')
process.stdout.write(`available=true version=${api.nativeVersion()}\n`)
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b)
const accounting = api.castI32('(1,234)')
if (!same(accounting, { ok: true, value: -1234 })) fail(`castI32: ${JSON.stringify(accounting)}`)
const corpus = process.env.HYPERCAST_CORPUS
if (!corpus) fail('HYPERCAST_CORPUS is not set')
const load = (name) => JSON.parse(fs.readFileSync(path.join(corpus, name), 'utf8'))
const stamp = load('timestamp.json').find((v) => v.expect === 'ok')
const instant = api.castTimestamp(stamp.input)
if (!same(instant, { ok: true, value: { seconds: stamp.seconds, nanos: stamp.nanos } }))
  fail(`timestamp vector ${stamp.input}: ${JSON.stringify(instant)}`)
process.stdout.write('checks: i32 timestamp-vector OK\n')
