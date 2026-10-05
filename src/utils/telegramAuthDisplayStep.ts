import type { TelegramAuthState } from '../composables/useTelegramAuth'

export type TelegramAuthActionStep = 'credentials' | 'code' | 'password'

export type TelegramAuthDisplayStep = 'credentials' | 'code' | 'password' | 'connected'

export function actionStepFromState(state: TelegramAuthState): TelegramAuthActionStep | null {
  switch (state) {
    case 'idle':
      return 'credentials'
    case 'code_required':
      return 'code'
    case 'password_required':
      return 'password'
    default:
      return null
  }
}

export function resolveDisplayStep(
  state: TelegramAuthState,
  rememberedActionStep: TelegramAuthActionStep | null,
): TelegramAuthDisplayStep {
  switch (state) {
    case 'connected':
      return 'connected'
    case 'code_required':
      return 'code'
    case 'password_required':
      return 'password'
    case 'loading':
      return rememberedActionStep ?? 'credentials'
    case 'idle':
    case 'error':
      return 'credentials'
  }
}
