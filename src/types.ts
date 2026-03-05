/**
 * Application events emitted from Rust backend
 */
export type AppEvent =
  | { InterceptionChanged: boolean }
  | { LayoutChanged: InputLayout }
  | { TextReady: string }
  | { TtsStatusChanged: TtsStatus }
  | { TtsError: string }
  | { ShowFloatingWindow: null }
  | { HideFloatingWindow: null }
  | { ShowMainWindow: null }
  | { UpdateFloatingText: string }
  | { UpdateTrayIcon: boolean }
  | { ShowSoundPanelWindow: null }
  | { HideSoundPanelWindow: null }
  | { SoundPanelNoBinding: string }

/**
 * Keyboard layout states
 */
export enum InputLayout {
  English = 'English',
  Russian = 'Russian'
}

/**
 * TTS status states
 */
export type TtsStatus =
  | { Idle: null }
  | { Speaking: null }
  | { Error: string }

/**
 * Sound Panel binding
 */
export interface SoundBinding {
  key: string
  description: string
  filename: string
  original_path?: string
}
