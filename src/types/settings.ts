/**
 * Application settings types
 *
 * These types match the Rust DTOs for unified settings loading.
 */

import type { SoundBinding } from '../types'

// ============================================================================
// Hotkey Settings Types
// ============================================================================

export type HotkeyModifier = 'ctrl' | 'shift' | 'alt' | 'super'

export interface HotkeyDto {
  modifiers: HotkeyModifier[]
  key: string
}

export interface HotkeySettingsDto {
  main_window: HotkeyDto
  sound_panel: HotkeyDto
}

// ============================================================================
// TTS Settings Types
// ============================================================================

// Rust enum uses #[serde(rename_all = "lowercase")]
// So JSON returns: "openai", "silero", "local"
export const TtsProviderType = {
  OpenAi: 'openai',
  Silero: 'silero',
  Local: 'local'
} as const

export type TtsProviderType = (typeof TtsProviderType)[keyof typeof TtsProviderType]

export interface OpenAiSettingsDto {
  api_key?: string
  voice: string
  proxy_host?: string
  proxy_port?: number
  use_proxy?: boolean
}

export interface LocalTtsSettingsDto {
  url: string
}

export interface TelegramTtsSettingsDto {
  api_id?: number
  proxy_mode?: string
}

export interface Socks5SettingsDto {
  proxy_url?: string
}

export interface MtProxySettingsDto {
  host?: string
  port: number
  secret?: string
  dc_id?: number
}

export interface NetworkSettingsDto {
  proxy: Socks5SettingsDto
  mtproxy: MtProxySettingsDto
}

export interface TtsSettingsDto {
  provider: TtsProviderType
  openai: OpenAiSettingsDto
  local: LocalTtsSettingsDto
  telegram: TelegramTtsSettingsDto
  network: NetworkSettingsDto
}

// ============================================================================
// Legacy Proxy Settings Types (deprecated, use NetworkSettingsDto)
// ============================================================================

export type ProxyTypeDto = 'Socks5' | 'Socks4' | 'Http'

export interface ProxySettingsDto {
  proxy_url?: string
  proxy_type: ProxyTypeDto
}

// ============================================================================
// WebView Settings Types
// ============================================================================

export interface WebViewSettingsDto {
  enabled: boolean
  start_on_boot: boolean
  port: number
  bind_address: string
  access_token?: string
  upnp_enabled: boolean
}

// ============================================================================
// Twitch Settings Types
// ============================================================================

export interface TwitchSettingsDto {
  enabled: boolean
  username: string
  token: string
  channel: string
  start_on_boot: boolean
}

// ============================================================================
// Audio Settings Types
// ============================================================================

export interface AudioSettingsDto {
  speaker_device?: string
  speaker_enabled: boolean
  speaker_volume: number
  virtual_mic_device?: string
  virtual_mic_volume: number
}

// ============================================================================
// Logging Settings Types
// ============================================================================

export interface LoggingSettingsDto {
  enabled: boolean
  level: string
  module_levels: Record<string, string>
}

// ============================================================================
// Windows Settings Types
// ============================================================================

export interface WindowPositionDto {
  x?: number
  y?: number
}

export interface SoundPanelWindowSettingsDto {
  x?: number
  y?: number
  opacity: number
  bg_color: string
  clickthrough: boolean
}

export interface GlobalSettingsDto {
  exclude_from_capture: boolean
}

export interface WindowsSettingsDto {
  global: GlobalSettingsDto
  main: WindowPositionDto
  soundpanel: SoundPanelWindowSettingsDto
}

// ============================================================================
// General Settings Types
// ============================================================================

export type Theme = 'dark' | 'light'

export interface GeneralSettingsDto {
  hotkey_enabled: boolean
  interception_enabled: boolean
  enter_closes_disabled: boolean
  theme?: Theme
}

// ============================================================================
// Preprocessor Settings Types
// ============================================================================

export interface PreprocessorSettingsDto {
  enabled: boolean
  replacements_count: number
}

// ============================================================================
// Editor Settings Types
// ============================================================================

export interface EditorSettingsDto {
  quick: boolean
  ai: boolean
}

// ============================================================================
// AI Settings Types
// ============================================================================

export const AiProviderType = {
  OpenAi: 'openai',
  ZAi: 'zai'  // Z.ai (capital Z)
} as const

export type AiProviderType = (typeof AiProviderType)[keyof typeof AiProviderType]

export interface AiOpenAiSettingsDto {
  api_key?: string
  use_proxy?: boolean
  model?: string
}

export interface AiZAiSettingsDto {
  url?: string
  api_key?: string
  model: string
}

// Z.ai (Anthropic-compatible AI provider)

export interface AiSettingsDto {
  provider: AiProviderType
  openai: AiOpenAiSettingsDto
  zai: AiZAiSettingsDto
  prompt: string
}

// ============================================================================
// Main App Settings DTO
// ============================================================================

/**
 * All application settings in a single DTO
 * This is the response from get_all_app_settings command
 */
export interface AppSettingsDto {
  tts: TtsSettingsDto
  webview: WebViewSettingsDto
  twitch: TwitchSettingsDto
  windows: WindowsSettingsDto
  audio: AudioSettingsDto
  general: GeneralSettingsDto
  logging: LoggingSettingsDto
  preprocessor: PreprocessorSettingsDto
  soundpanel_bindings: SoundBinding[]
  editor: EditorSettingsDto
  ai: AiSettingsDto
  hotkeys: HotkeySettingsDto
}

// ============================================================================
// Tauri Command Types
// ============================================================================

export type AppSettingsCommand = 'get_all_app_settings' | 'is_backend_ready' | 'confirm_backend_ready'

// ============================================================================
// Injection Key
// ============================================================================

import { InjectionKey, Ref } from 'vue'

export interface AppSettingsContext {
  settings: Ref<AppSettingsDto | null>
  isLoading: Ref<boolean>
  error: Ref<string | null>
  reload: () => Promise<void>
  cleanup?: () => void
}

export const APP_SETTINGS_KEY: InjectionKey<AppSettingsContext> = Symbol('app-settings')
