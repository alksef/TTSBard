import { readFileSync, readdirSync } from 'node:fs'
import { dirname, relative, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const SOURCE_EXTENSIONS = new Set(['.rs', '.ts', '.vue'])

function normalizePath(path) {
  return path.replaceAll('\\', '/')
}

function lineAt(source, offset) {
  return source.slice(0, offset).split('\n').length
}

function sourceFiles(root, directory) {
  const start = resolve(root, directory)
  const files = []

  function visit(current) {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = resolve(current, entry.name)
      if (entry.isDirectory()) {
        visit(path)
      } else if (SOURCE_EXTENSIONS.has(entry.name.slice(entry.name.lastIndexOf('.')))) {
        files.push(path)
      }
    }
  }

  visit(start)
  return files
}

function balancedBlock(source, marker, open, close) {
  const markerOffset = source.indexOf(marker)
  if (markerOffset < 0) throw new Error(`Missing source marker: ${marker}`)
  const start = source.indexOf(open, markerOffset + marker.length)
  if (start < 0) throw new Error(`Missing ${open} after ${marker}`)

  let depth = 0
  for (let offset = start; offset < source.length; offset += 1) {
    if (source[offset] === open) depth += 1
    if (source[offset] === close) depth -= 1
    if (depth === 0) return { text: source.slice(start + 1, offset), offset: start + 1 }
  }
  throw new Error(`Unclosed ${open} after ${marker}`)
}

function stringConstants(files, root, language) {
  const constants = new Map()
  const pattern = language === 'rust'
    ? /\b(?:pub\s+)?const\s+([A-Z][A-Z0-9_]*)\s*:\s*&str\s*=\s*(["'])(.*?)\2/g
    : /\b(?:export\s+)?const\s+([A-Z][A-Z0-9_]*)\s*=\s*(["'])(.*?)\2/g

  for (const path of files) {
    const source = readFileSync(path, 'utf8')
    for (const match of source.matchAll(pattern)) {
      constants.set(match[1], {
        name: match[3],
        file: normalizePath(relative(root, path)),
        line: lineAt(source, match.index),
      })
    }
  }
  return constants
}

function location(root, path, source, offset, name) {
  return {
    name,
    file: normalizePath(relative(root, path)),
    line: lineAt(source, offset),
  }
}

function parseArgument(expression, constants) {
  const value = expression.trim()
  const literal = value.match(/^(["'])(.*?)\1$/s)
  if (literal) return { name: literal[2] }
  if (/^[A-Z][A-Z0-9_]*$/.test(value) && constants.has(value)) {
    return { name: constants.get(value).name }
  }
  const qualified = value.match(/^(?:[a-z_]\w*::)+([A-Z][A-Z0-9_]*)$/)
  if (qualified && constants.has(qualified[1])) {
    return { name: constants.get(qualified[1]).name }
  }
  return { expression: value.replace(/\s+/g, ' ') }
}

function collectFrontendCalls(files, root, constants) {
  const calls = { invoke: [], listen: [], dynamic: [] }
  const pattern = /\b(invoke|listen)(?:\s*<[\s\S]{0,800}?>)?\s*\(\s*([^,\r\n)]+)/g

  for (const path of files) {
    const source = readFileSync(path, 'utf8')
    for (const match of source.matchAll(pattern)) {
      const parsed = parseArgument(match[2], constants)
      const kind = match[1]
      if (parsed.name) {
        calls[kind].push(location(root, path, source, match.index, parsed.name))
      } else {
        calls.dynamic.push({
          kind: `frontend_${kind}`,
          file: normalizePath(relative(root, path)),
          line: lineAt(source, match.index),
          expression: parsed.expression,
        })
      }
    }
  }
  return calls
}

function collectRegisteredCommands(root) {
  const path = resolve(root, 'src-tauri/src/lib.rs')
  const source = readFileSync(path, 'utf8')
  const block = balancedBlock(source, 'tauri::generate_handler!', '[', ']')
  const commands = []

  let cursor = 0
  for (const line of block.text.replace(/\r\n?/g, '\n').split('\n')) {
    const code = line.replace(/\/\/.*$/, '').trim().replace(/,$/, '')
    if (code) {
      const match = code.match(/^(?:[a-zA-Z_][a-zA-Z0-9_]*::)*([a-zA-Z_][a-zA-Z0-9_]*)$/)
      if (!match) throw new Error(`Unsupported generate_handler entry: ${code}`)
      commands.push(location(root, path, source, block.offset + cursor, match[1]))
    }
    cursor += line.length + 1
  }
  return commands
}

function collectBackendEvents(files, root, constants) {
  const events = []
  const dynamic = []
  const emitPattern = /\.emit\s*\(\s*([^,\r\n)]+)/g

  for (const path of files) {
    const source = readFileSync(path, 'utf8')
    for (const match of source.matchAll(emitPattern)) {
      const parsed = parseArgument(match[1], constants)
      if (parsed.name) {
        events.push(location(root, path, source, match.index, parsed.name))
      } else {
        dynamic.push({
          kind: 'backend_emit',
          file: normalizePath(relative(root, path)),
          line: lineAt(source, match.index),
          expression: parsed.expression,
        })
      }
    }
  }

  const eventsPath = resolve(root, 'src-tauri/src/events.rs')
  const eventsSource = readFileSync(eventsPath, 'utf8')
  const block = balancedBlock(eventsSource, 'pub fn to_tauri_event', '{', '}')
  for (const match of block.text.matchAll(/=>\s*"([^"]+)"/g)) {
    events.push(location(root, eventsPath, eventsSource, block.offset + match.index, match[1]))
  }

  return { events, dynamic }
}

function uniqueNames(entries) {
  return [...new Set(entries.map(({ name }) => name))].sort()
}

function allowedDynamic(item, allowlist) {
  return allowlist.some((entry) =>
    entry.kind === item.kind
    && normalizePath(entry.file) === item.file
    && entry.expression === item.expression
    && typeof entry.reason === 'string'
    && entry.reason.trim().length > 0)
}

export function collectInventory(root) {
  const frontendFiles = ['src', 'src-playback', 'src-soundpanel']
    .flatMap((directory) => sourceFiles(root, directory))
  const backendFiles = sourceFiles(root, 'src-tauri/src')
  const frontendConstants = stringConstants(frontendFiles, root, 'typescript')
  const backendConstants = stringConstants(backendFiles, root, 'rust')
  const frontend = collectFrontendCalls(frontendFiles, root, frontendConstants)
  const backend = collectBackendEvents(backendFiles, root, backendConstants)
  const registeredCommands = collectRegisteredCommands(root)

  return {
    registeredCommands,
    frontendInvokes: frontend.invoke,
    backendEvents: backend.events,
    frontendListens: frontend.listen,
    dynamicExpressions: [...frontend.dynamic, ...backend.dynamic],
  }
}

export function validateInventory(inventory, allowlist) {
  const commandNames = new Set(uniqueNames(inventory.registeredCommands))
  const eventNames = new Set(uniqueNames(inventory.backendEvents))
  const errors = []

  for (const item of inventory.frontendInvokes) {
    if (!commandNames.has(item.name)) {
      errors.push(`${item.file}:${item.line}: invoke('${item.name}') has no registered command`)
    }
  }
  for (const item of inventory.frontendListens) {
    if (!eventNames.has(item.name)) {
      errors.push(`${item.file}:${item.line}: listen('${item.name}') has no backend event`)
    }
  }
  for (const item of inventory.dynamicExpressions) {
    if (!allowedDynamic(item, allowlist.dynamicExpressions ?? [])) {
      errors.push(`${item.file}:${item.line}: unresolved ${item.kind} expression '${item.expression}'`)
    }
  }
  return errors
}

export function run(root) {
  const inventory = collectInventory(root)
  const allowlistPath = resolve(root, 'scripts/ipc-contract-allowlist.json')
  const allowlist = JSON.parse(readFileSync(allowlistPath, 'utf8'))
  return { inventory, errors: validateInventory(inventory, allowlist) }
}

const isCli = process.argv[1]
  && import.meta.url === pathToFileURL(resolve(process.argv[1])).href

if (isCli) {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
  const { inventory, errors } = run(root)
  if (process.argv.includes('--json')) {
    process.stdout.write(`${JSON.stringify({ inventory, errors }, null, 2)}\n`)
  } else if (errors.length > 0) {
    console.error(`IPC contract check failed with ${errors.length} error(s):`)
    for (const error of errors) console.error(`- ${error}`)
    process.exitCode = 1
  } else {
    console.log(
      `IPC contracts OK: ${uniqueNames(inventory.registeredCommands).length} commands, `
      + `${uniqueNames(inventory.frontendInvokes).length} invoked, `
      + `${uniqueNames(inventory.backendEvents).length} events, `
      + `${uniqueNames(inventory.frontendListens).length} listened.`,
    )
  }
}
