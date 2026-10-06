/**
 * Классификация стабильного закодированного сообщения об отказе запуска
 * внутренних серверов (WebView и входящего сервера).
 *
 * Backend не передаёт пользователю OS-текст: занятый порт кодируется как
 * `port_in_use:<port>` (порт захватывается в момент bind из
 * `ErrorKind::AddrInUse`), любой другой отказ — как `server_start_failed`.
 * UI разбирает это сообщение и показывает локализованный тост и баннер;
 * порт берётся только из закодированного сообщения, а не из черновика формы.
 */

export type ServerStartErrorKind = 'port_in_use' | 'generic'

export interface ParsedServerStartError {
  kind: ServerStartErrorKind
  /** Порт последней неудачной попытки; только для `kind === 'port_in_use'`. */
  port?: number
}

const PORT_IN_USE_PREFIX = 'port_in_use:'

/**
 * Разобрать сообщение статуса `error` внутреннего сервера.
 *
 * Неизвестные, пустые и некорректные значения сводятся к `generic`, поэтому
 * произвольный backend-текст никогда не становится UI-состоянием.
 */
export function parseServerStartError(message: string | undefined): ParsedServerStartError {
  if (typeof message === 'string' && message.startsWith(PORT_IN_USE_PREFIX)) {
    const port = Number(message.slice(PORT_IN_USE_PREFIX.length))
    if (/^\d+$/.test(message.slice(PORT_IN_USE_PREFIX.length)) && Number.isInteger(port) && port > 0 && port <= 65535) {
      return { kind: 'port_in_use', port }
    }
  }
  return { kind: 'generic' }
}
