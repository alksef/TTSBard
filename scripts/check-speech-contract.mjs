import { readFileSync, existsSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import ts from 'typescript'
import {
  checkSettingsParity,
  findExportedType,
  createDiskProgram,
  createSyntheticProgram,
  describeType,
} from './check-settings-parity.mjs'

const __root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const SPEECH_TS = resolve(__root, 'src/ipc/speech.ts')
const PLAYBACK_TS = resolve(__root, 'src-playback/speechQueue.ts')
const FIXTURE_DIR = resolve(__root, 'scripts/contract-fixtures/speech')

// ---------------------------------------------------------------------------
// Error code contract check (parameterized for testability)
// ---------------------------------------------------------------------------

function checkSpeechErrorContract(opts = {}) {
  const errors = []

  let fixture
  if (opts.fixtureData) {
    fixture = opts.fixtureData
  } else {
    const fixturePath = opts.fixturePath || resolve(FIXTURE_DIR, 'speech-errors.json')
    if (!existsSync(fixturePath)) {
      return { errors: [`speech-errors.json fixture missing at ${fixturePath}`] }
    }
    fixture = JSON.parse(readFileSync(fixturePath, 'utf8'))
  }

  if (!fixture.codes || !Array.isArray(fixture.codes)) {
    return { errors: ['speech-errors.json missing "codes" array'] }
  }
  if (!fixture.envelope || typeof fixture.envelope !== 'object') {
    return { errors: ['speech-errors.json missing "envelope" object'] }
  }

  for (const fixtureEntry of fixture.codes) {
    if (typeof fixtureEntry.code !== 'string' || typeof fixtureEntry.retryable !== 'boolean') {
      errors.push(
        `[Rust] fixture entry has malformed code/retryable shape: ${JSON.stringify(fixtureEntry)}`
      )
    }
  }
  if (errors.length > 0) return { errors }

  let program
  let sourceFileName
  if (opts.sourceText) {
    program = createSyntheticProgram(opts.sourceText, [])
    sourceFileName = '/_synthetic.ts'
  } else {
    const sourcePath = opts.sourcePath || SPEECH_TS
    sourceFileName = opts.sourcePath || SPEECH_TS
    program = createDiskProgram([sourceFileName])
  }

  const checker = program.getTypeChecker()

  const speechErrorCodeType = findExportedType(program, sourceFileName, 'SpeechErrorCode')
  if (!speechErrorCodeType) {
    return { errors: ["export 'SpeechErrorCode' not found in TypeScript source"] }
  }

  const speechErrorMetaType = findExportedType(program, sourceFileName, 'SPEECH_ERROR_META')
  if (!speechErrorMetaType) {
    return { errors: ["export 'SPEECH_ERROR_META' not found in TypeScript source"] }
  }

  const tsCodeSet = extractStringLiteralUnion(checker, speechErrorCodeType)
  if (!tsCodeSet) {
    return { errors: ["'SpeechErrorCode' is not a string literal union"] }
  }

  const tsMetaProps = extractObjectPropertiesFromMeta(checker, speechErrorMetaType)
  if (!tsMetaProps) {
    return { errors: ["'SPEECH_ERROR_META' is not an object type"] }
  }

  const fixtureCodes = new Set(fixture.codes.map(c => c.code))
  const tsCodes = new Set(tsCodeSet)

  for (const code of fixtureCodes) {
    if (!tsCodes.has(code)) {
      errors.push(`[TS] code '${code}' is in Rust fixture but not in TypeScript SpeechErrorCode`)
    }
  }
  for (const code of tsCodes) {
    if (!fixtureCodes.has(code)) {
      errors.push(`[TS] code '${code}' is in TypeScript SpeechErrorCode but not in Rust fixture`)
    }
  }

  if (errors.length > 0) return { errors }

  for (const fixtureEntry of fixture.codes) {
    const { code, retryable } = fixtureEntry
    const tsProp = tsMetaProps.get(code)
    if (!tsProp) {
      errors.push(`[TS] code '${code}' in Rust fixture is not a property of SPEECH_ERROR_META`)
      continue
    }

    const propDesc = tsProp.type
    if (propDesc.kind !== 'object') {
      errors.push(`[TS] SPEECH_ERROR_META['${code}'] is not an object type (kind=${propDesc.kind})`)
      continue
    }

    const retryProp = propDesc.properties.get('retryable')
    if (!retryProp) {
      errors.push(`[TS] SPEECH_ERROR_META['${code}'] missing 'retryable' property`)
      continue
    }

    if (retryProp.type.kind !== 'literal' || retryProp.type.literalType !== 'boolean') {
      errors.push(
        `[TS] SPEECH_ERROR_META['${code}'].retryable is not a boolean literal (kind=${retryProp.type.kind})`
      )
      continue
    }

    if (retryProp.type.value !== retryable) {
      errors.push(
        `[TS] retryability mismatch for '${code}': Rust=${retryable} TS=${retryProp.type.value}`
      )
    }
  }

  for (const [code] of tsMetaProps) {
    if (!fixtureCodes.has(code)) {
      errors.push(`[TS] SPEECH_ERROR_META has property '${code}' not in Rust fixture`)
    }
  }

  const envelopeResult = checkSettingsParity({
    populatedFixture: fixture.envelope,
    omitNullFixture: fixture.envelope,
    rootExportName: 'SpeechCommandErrorDto',
    sourcePath: opts.sourcePath || SPEECH_TS,
    sourceText: opts.sourceText,
  })
  errors.push(...envelopeResult.errors.map(error => `[envelope] ${error}`))

  return { errors }
}

// ---------------------------------------------------------------------------
// Event payload contract check (parameterized for testability)
// ---------------------------------------------------------------------------

function checkSpeechEventContract(opts = {}) {
  let populated
  let empty

  if (opts.populatedFixture && opts.emptyFixture) {
    populated = opts.populatedFixture
    empty = opts.emptyFixture
  } else {
    const populatedPath =
      opts.populatedFixturePath || resolve(FIXTURE_DIR, 'speech-queue-populated.json')
    const emptyPath =
      opts.emptyFixturePath || resolve(FIXTURE_DIR, 'speech-queue-empty.json')

    if (!existsSync(populatedPath) || !existsSync(emptyPath)) {
      return {
        errors: ['Speech queue fixtures missing — regenerate with Rust fixture test'],
      }
    }

    populated = JSON.parse(readFileSync(populatedPath, 'utf8'))
    empty = JSON.parse(readFileSync(emptyPath, 'utf8'))
  }

  return checkSettingsParity({
    populatedFixture: populated,
    omitNullFixture: empty,
    rootExportName: 'SpeechQueueStateDto',
    sourcePath: opts.sourcePath || PLAYBACK_TS,
    sourceText: opts.sourceText || undefined,
  })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function extractStringLiteralUnion(checker, unionType) {
  if (!unionType.isUnion()) return null
  const literals = []
  for (const t of unionType.types) {
    if (t.flags & ts.TypeFlags.StringLiteral) {
      literals.push(t.value)
    } else {
      return null
    }
  }
  return literals
}

function extractObjectPropertiesFromMeta(checker, objType) {
  const props = objType.getProperties()
  const result = new Map()
  for (const prop of props) {
    const name = prop.getName()
    const decls = prop.getDeclarations()
    if (decls && decls.length > 0) {
      const decl = decls[0]
      if (ts.isIndexSignatureDeclaration(decl)) continue
    }
    const propType = checker.getTypeOfSymbol(prop)
    const desc = describeType(checker, propType, new Set(), name)
    result.set(name, { type: desc })
  }
  return result
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const isCli =
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href

if (isCli) {
  let allErrors = []

  const errorResult = checkSpeechErrorContract()
  if (errorResult.errors.length > 0) {
    allErrors.push(...errorResult.errors.map(e => `[errors] ${e}`))
  }

  const eventResult = checkSpeechEventContract()
  if (eventResult.errors.length > 0) {
    allErrors.push(...eventResult.errors.map(e => `[event] ${e}`))
  }

  if (allErrors.length > 0) {
    console.error(`Speech contract check failed with ${allErrors.length} error(s):`)
    for (const err of allErrors) {
      console.error(`  - ${err}`)
    }
    process.exitCode = 1
  } else {
    console.log('Speech contract OK')
  }
}

export { checkSpeechErrorContract, checkSpeechEventContract }
