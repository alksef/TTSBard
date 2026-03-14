import { ref } from 'vue'
import { debugLog, debugError, debugWarn, debugInfo } from '../utils/debug'

/**
 * Типы ошибок
 */
export enum ErrorLevel {
  INFO = 'info',
  WARNING = 'warning',
  ERROR = 'error',
  SUCCESS = 'success',
}

/**
 * Интерфейс для сообщения об ошибке
 */
export interface ErrorMessage {
  id: string
  level: ErrorLevel
  message: string
  timestamp: number
  duration?: number // 0 = не скрывать автоматически
}

/**
 * Опции для showError
 */
export interface ShowErrorOptions {
  level?: ErrorLevel
  duration?: number // ms, 0 = не скрывать автоматически
  logToConsole?: boolean
  showNotification?: boolean
}

const DEFAULT_DURATION = 3000 // 3 секунды по умолчанию

// Shared state (singleton) - один экземпляр для всех компонентов
const errors = ref<ErrorMessage[]>([])
let errorIdCounter = 0

/**
 * Composable для унифицированной обработки ошибок
 *
 * Предоставляет единый интерфейс для:
 * - Показа toast notifications
 * - Логирования в консоль
 * - Отслеживания активных ошибок
 */
export function useErrorHandler() {

  /**
   * Добавить ошибку в список
   */
  function addError(message: string, level: ErrorLevel, duration?: number): string {
    const id = `error-${++errorIdCounter}-${Date.now()}`
    const error: ErrorMessage = {
      id,
      level,
      message,
      timestamp: Date.now(),
      duration: duration ?? DEFAULT_DURATION,
    }

    errors.value.push(error)

    // Автоматически удалить ошибку после duration (если не 0)
    if (error.duration && error.duration > 0) {
      setTimeout(() => {
        removeError(id)
      }, error.duration)
    }

    return id
  }

  /**
   * Удалить ошибку из списка
   */
  function removeError(id: string): void {
    const index = errors.value.findIndex(e => e.id === id)
    if (index !== -1) {
      errors.value.splice(index, 1)
    }
  }

  /**
   * Очистить все ошибки
   */
  function clearAllErrors(): void {
    errors.value = []
  }

  /**
   * Показать сообщение об ошибке
   *
   * @param message - текст сообщения
   * @param options - опции отображения
   * @returns id ошибки для возможного ручного удаления
   */
  function showError(message: string, options: ShowErrorOptions = {}): string {
    const {
      level = ErrorLevel.ERROR,
      duration = DEFAULT_DURATION,
      logToConsole = true,
      showNotification = true,
    } = options

    // Логирование в консоль
    if (logToConsole) {
      switch (level) {
        case ErrorLevel.ERROR:
          debugError(`[Error] ${message}`)
          break
        case ErrorLevel.WARNING:
          debugWarn(`[Warning] ${message}`)
          break
        case ErrorLevel.INFO:
          debugInfo(`[Info] ${message}`)
          break
        case ErrorLevel.SUCCESS:
          debugLog(`[Success] ${message}`)
          break
      }
    }

    // Показать в UI
    if (showNotification) {
      return addError(message, level, duration)
    }

    return ''
  }

  /**
   * Обработать ошибку из catch блока
   *
   * @param error - ошибка из catch
   * @param context - контекст (например, "Failed to load settings")
   * @param options - опции отображения
   */
  function handleCaughtError(
    error: unknown,
    context: string,
    options: ShowErrorOptions = {}
  ): string {
    const message = formatError(error, context)
    return showError(message, options)
  }

  /**
   * Форматировать ошибку в строку
   */
  function formatError(error: unknown, context: string): string {
    let errorMsg = ''

    if (error instanceof Error) {
      errorMsg = error.message
    } else if (typeof error === 'string') {
      errorMsg = error
    } else {
      try {
        errorMsg = JSON.stringify(error)
      } catch {
        errorMsg = 'Unknown error'
      }
    }

    return context ? `${context}: ${errorMsg}` : errorMsg
  }

  /**
   * Convenience методы для разных уровней ошибок
   */
  function showInfo(message: string, duration?: number): string {
    return showError(message, { level: ErrorLevel.INFO, duration })
  }

  function showWarning(message: string, duration?: number): string {
    return showError(message, { level: ErrorLevel.WARNING, duration })
  }

  function showSuccess(message: string, duration?: number): string {
    return showError(message, { level: ErrorLevel.SUCCESS, duration })
  }

  return {
    // State
    errors,

    // Methods
    showError,
    showInfo,
    showWarning,
    showSuccess,
    handleCaughtError,
    removeError,
    clearAllErrors,
    formatError,
  }
}
