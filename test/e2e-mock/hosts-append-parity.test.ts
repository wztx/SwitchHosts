import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'

// The Playwright Tauri mock reimplements `make_append_content` from
// src-tauri/src/hosts_apply/write.rs in JavaScript so the renderer can be
// driven without a Rust backend. Nothing forces the two to agree: the e2e
// suite never seeds a hosts file that has a section, so the mock's tail and
// empty-anchor branches are unreachable there, and a change on the Rust side
// would leave the mock silently stale.
//
// Both are therefore asserted against the same fixture. Update the fixture
// when the behaviour intentionally changes, and both sides must follow.

const here = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(here, '../..')
const mockPath = path.join(repoRoot, 'e2e/support/tauri-mock.js')
const fixturePath = path.join(repoRoot, 'test/fixtures/hosts_append_cases.json')

type FixtureCase = {
  name: string
  why: string
  previous: string
  applies: string[]
  expected: string
}

type MockWindow = {
  __SWITCHHOSTS_E2E__: { state: { systemHosts: string; configs: { write_mode: string } } }
  __TAURI_INTERNALS__: { invoke: (cmd: string, args: { args: unknown[] }) => Promise<unknown> }
}

const scope = globalThis as unknown as Record<string, unknown>
let mockWindow: MockWindow

beforeAll(() => {
  // The mock is a browser IIFE that hangs its API off `window`; give it just
  // enough of a DOM to evaluate under vitest's node environment.
  scope.window = {
    location: { search: '' },
    addEventListener: () => {},
    removeEventListener: () => {},
  }
  scope.document = { addEventListener: () => {}, removeEventListener: () => {} }

  new Function(fs.readFileSync(mockPath, 'utf8'))()

  mockWindow = scope.window as MockWindow
})

afterAll(() => {
  delete scope.window
  delete scope.document
})

const applyAll = async (previous: string, payloads: string[]) => {
  const { state } = mockWindow.__SWITCHHOSTS_E2E__
  state.systemHosts = previous
  state.configs.write_mode = 'append'

  for (const payload of payloads) {
    await mockWindow.__TAURI_INTERNALS__.invoke('apply_hosts_selection', {
      args: [payload],
    })
  }

  return state.systemHosts
}

describe('tauri mock append mode', () => {
  const cases: FixtureCase[] = JSON.parse(fs.readFileSync(fixturePath, 'utf8')).cases

  it('has cases to check', () => {
    expect(cases.length).toBeGreaterThan(0)
  })

  it.each(cases)('$name', async ({ previous, applies, expected }) => {
    expect(await applyAll(previous, applies)).toBe(expected)
  })
})
