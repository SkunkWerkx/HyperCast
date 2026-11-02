import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as all from '../index.js'
import { castI32, isAvailable, NumFormat, NumStyles } from '../index.js'

test('named imports work from ESM', () => {
  assert.equal(isAvailable(), true)
  assert.equal(typeof castI32, 'function')
  assert.deepEqual(castI32('42'), { ok: true, value: 42 })
  assert.equal(NumFormat.invariant().flags, NumStyles.All)
})

test('the namespace exposes exactly the public names', () => {
  const keys = Object.keys(all)
    .filter((k) => k !== 'default' && k !== 'module.exports')
    .sort()
  const expected = [
    'nativeVersion',
    'isAvailable',
    'isHyperCastError',
    'optional',
    'castBool',
    'castChar',
    'castI8',
    'castI16',
    'castI32',
    'castI64',
    'castU8',
    'castU16',
    'castU32',
    'castU64',
    'castF32',
    'castF64',
    'castDecimal',
    'castUuid',
    'castTimestamp',
    'castUnix',
    'castExcelSerial',
    'castDate',
    'castDatetime',
    'castTime',
    'castDuration',
    'castDecimalFromNumber',
    'castExcelSerialFromNumber',
    'castExcelTime',
    'castExcelDuration',
    'castChecked',
    'castCheckedFromNumber',
    'castText',
    'castConstraintsCheck',
    'castDeclaredDecimal',
    'NumFormat',
    'DateFormat',
    'Constraints',
    'NumStyles',
    'UnixPrecision',
    'DateOrder',
    'ExcelEpoch',
    'Door',
    'Rounding',
    'Keyword',
    'CastFailure',
  ].sort()
  assert.deepEqual(keys, expected)
})
