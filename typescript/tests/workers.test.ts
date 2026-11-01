const assert: typeof import('node:assert/strict') = require('node:assert/strict')
const path: typeof import('node:path') = require('node:path')
const { test }: typeof import('node:test') = require('node:test')
const { Worker }: typeof import('node:worker_threads') = require('node:worker_threads')
const { castI32 } = require('../index.js') as typeof import('../index.js')

// Each worker loads the binding again (under WASI, its own module instance), and casts on its
// own while the main thread keeps casting too.
test('a worker thread casts with an addon of its own', async () => {
  const entry = path.resolve(__dirname, '../index.js')
  const script = `
    const { parentPort } = require('node:worker_threads')
    const { castI32 } = require(${JSON.stringify(entry)})
    parentPort.postMessage([castI32('(1,234)'), castI32('12x')])
  `
  const fromWorker = await new Promise<unknown>((resolve, reject) => {
    const worker = new Worker(script, { eval: true })
    worker.once('message', resolve)
    worker.once('error', reject)
  })
  assert.deepEqual(fromWorker, [
    { ok: true, value: -1234 },
    { ok: false, fault: { reason: 'malformed', offset: 2, length: 1 } },
  ])
  assert.deepEqual(castI32('7'), { ok: true, value: 7 })
})
