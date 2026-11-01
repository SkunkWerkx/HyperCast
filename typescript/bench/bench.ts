// The doors against the parsers JavaScript ships, on the same input. The built-ins guess
// where a door declares (Number.parseInt reads '12x' as 12), so this measures the cost of the
// verdict, not a like-for-like contract. No figure from it is quoted anywhere until it has
// been run and dated on a named machine.
const { bench, do_not_optimize, group, run, summary }: typeof import('mitata') = require('mitata')
const api = require('../index.js') as typeof import('../index.js')
const { castBool, castDecimal, castF64, castI32, castTimestamp, castUuid } = api

const bytes = new TextEncoder().encode('1234567')

group('i32', () => {
  summary(() => {
    bench("castI32('1234567')", () => do_not_optimize(castI32('1234567')))
    bench('castI32(bytes)', () => do_not_optimize(castI32(bytes)))
    bench("Number.parseInt('1234567', 10)", () => do_not_optimize(Number.parseInt('1234567', 10)))
  })
})

group('f64', () => {
  summary(() => {
    bench("castF64('1,234.5')", () => do_not_optimize(castF64('1,234.5')))
    bench("Number.parseFloat('1234.5')", () => do_not_optimize(Number.parseFloat('1234.5')))
  })
})

group('timestamp', () => {
  summary(() => {
    bench('castTimestamp(rfc3339)', () =>
      do_not_optimize(castTimestamp('2026-01-02T15:04:05.123456789Z')),
    )
    bench('Date.parse(rfc3339)', () => do_not_optimize(Date.parse('2026-01-02T15:04:05.123Z')))
  })
})

group('other doors', () => {
  bench("castBool('yes')", () => do_not_optimize(castBool('yes')))
  bench("castDecimal('-1234.50')", () => do_not_optimize(castDecimal('-1234.50')))
  bench('castUuid(braced)', () =>
    do_not_optimize(castUuid('{01020304-0506-0708-090a-0b0c0d0e0f10}')),
  )
  bench("castI32('x') (a fault)", () => do_not_optimize(castI32('x')))
})

run()
