import { describe, expect, it } from 'vitest'

import { isValidDomain } from './hostsFn'

describe('isValidDomain', () => {
  it.each(['github.com', 'a-b.example.co', 'raw.githubusercontent.com', '1.2.3.4.com'])(
    'accepts %s',
    (s) => {
      expect(isValidDomain(s)).toBe(true)
    },
  )

  it.each([
    '',
    '   ',
    'github',
    'github.com.',
    'https://github.com',
    'github.com/x',
    'github.com:443',
    'a..b',
    '.a.com',
    '-a.com',
    'a-.com',
    '192.168.1.1',
    'a b.com',
    `${'a'.repeat(64)}.com`,
    `${'a'.repeat(250)}.com`,
  ])('rejects %s', (s) => {
    expect(isValidDomain(s)).toBe(false)
  })
})
