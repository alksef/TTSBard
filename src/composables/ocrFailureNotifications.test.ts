import { describe, it, expect, beforeAll } from 'vitest'
import {
  convertOcrOneShotFailure,
  OCR_FAILURE_REASONS,
  type OcrFailureReason,
  type OcrFailureSeverity,
} from './ocrFailureNotifications'
import { i18n } from '../i18n'
import ruCatalog from '../../locales/ru.json'

beforeAll(() => {
  i18n.global.setLocaleMessage('ru', (ruCatalog as { messages: Record<string, string> }).messages)
  ;(i18n.global.locale as unknown as { value: string }).value = 'ru'
})

const EXPECTED_PRESENTATION: Record<OcrFailureReason, { severity: OcrFailureSeverity; message: string }> = {
  emptyResult: {
    severity: 'warning',
    message: 'Текст в выбранной области не найден',
  },
  noMonitors: {
    severity: 'error',
    message: 'Не найдено ни одного монитора для захвата экрана',
  },
  captureFailed: {
    severity: 'error',
    message: 'Не удалось захватить изображение экрана',
  },
  overlayOpenFailed: {
    severity: 'error',
    message: 'Не удалось открыть окно выбора области',
  },
  overlayHideFailed: {
    severity: 'error',
    message: 'Не удалось закрыть окно выбора области',
  },
  runtimeUnavailable: {
    severity: 'error',
    message: 'Распознавание текста сейчас недоступно',
  },
  recognitionFailed: {
    severity: 'error',
    message: 'Не удалось распознать текст в выбранной области',
  },
  intakeFailed: {
    severity: 'error',
    message: 'Не удалось передать распознанный текст во входящие',
  },
}

describe('convertOcrOneShotFailure', () => {
  it('covers every allowlisted reason with its fixed message and severity', () => {
    for (const reason of OCR_FAILURE_REASONS) {
      const notification = convertOcrOneShotFailure({ reason })
      expect(notification).not.toBeNull()
      expect(notification).toMatchObject({
        reason,
        severity: EXPECTED_PRESENTATION[reason].severity,
        message: EXPECTED_PRESENTATION[reason].message,
      })
    }
  })

  it('maps only emptyResult to the warning severity', () => {
    for (const reason of OCR_FAILURE_REASONS) {
      const notification = convertOcrOneShotFailure({ reason })
      const expected = reason === 'emptyResult' ? 'warning' : 'error'
      expect(notification?.severity).toBe(expected)
    }
  })

  it('never returns an arbitrary raw string as a reason', () => {
    expect(
      convertOcrOneShotFailure({ reason: 'panic in onnx at C:/packs/det.onnx' }),
    ).toBeNull()
    expect(convertOcrOneShotFailure({ reason: 'worker died: boom' })).toBeNull()
  })

  it('never leaks extra payload fields into the message', () => {
    const notification = convertOcrOneShotFailure({
      reason: 'recognitionFailed',
      internal: 'detailed backend error path',
    })
    expect(notification?.message).toBe(EXPECTED_PRESENTATION.recognitionFailed.message)
    expect(notification?.message).not.toContain('detailed backend error path')
    expect(notification?.message).not.toContain('recognitionFailed')
  })

  it('rejects payloads that are not plain objects', () => {
    expect(convertOcrOneShotFailure(null)).toBeNull()
    expect(convertOcrOneShotFailure(undefined)).toBeNull()
    expect(convertOcrOneShotFailure('emptyResult')).toBeNull()
    expect(convertOcrOneShotFailure(['emptyResult'])).toBeNull()
    expect(convertOcrOneShotFailure(42)).toBeNull()
  })

  it('rejects missing, non-string and unknown reasons', () => {
    expect(convertOcrOneShotFailure({})).toBeNull()
    expect(convertOcrOneShotFailure({ reason: 42 })).toBeNull()
    expect(convertOcrOneShotFailure({ reason: null })).toBeNull()
    expect(convertOcrOneShotFailure({ reason: '' })).toBeNull()
    expect(convertOcrOneShotFailure({ reason: 'unknownFailure' })).toBeNull()
  })
})
