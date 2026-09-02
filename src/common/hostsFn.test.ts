import { describe, expect, it } from 'vitest'

import { extractDomain, isValidDomain } from './hostsFn'

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

describe('extractDomain', () => {
  it.each([
    ['dblp.org', 'dblp.org'],
    ['https://dblp.org/', 'dblp.org'],
    ['https://dblp.org/search?q=x', 'dblp.org'],
    ['http://user:pass@github.com:8080/a/b', 'github.com'],
    ['dblp.org/search?q=x', 'dblp.org'],
    ['github.com:443', 'github.com'],
    ['dblp.org.', 'dblp.org'],
  ])('extracts %j → %j', (input, expected) => {
    expect(extractDomain(input)).toBe(expected)
  })

  it.each(['', '   ', 'https://', 'bad domain!', '192.168.1.1/x', 'not a domain'])(
    'returns null for %j',
    (input) => {
      expect(extractDomain(input)).toBeNull()
    },
  )
})
