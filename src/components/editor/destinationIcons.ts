import type { Component } from 'vue'
import { Volume2, Globe, Twitch, MessageSquareText } from 'lucide-vue-next'

export type Destination = 'voice' | 'webview' | 'twitch' | 'vrchat'

/**
 * Single source of truth for destination icons shared by the editor route
 * selector and the incoming route selector. Both must render the same
 * vocabulary: voice → Volume2, webview → Globe, twitch → Twitch,
 * vrchat → MessageSquareText.
 */
export const destinationIcons: Record<Destination, Component> = {
  voice: Volume2,
  webview: Globe,
  twitch: Twitch,
  vrchat: MessageSquareText,
}
