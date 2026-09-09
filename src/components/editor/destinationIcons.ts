import type { Component } from 'vue'
import { Volume2, Globe, Twitch } from 'lucide-vue-next'

export type Destination = 'voice' | 'webview' | 'twitch'

/**
 * Single source of truth for destination icons shared by the editor route
 * selector and the incoming route selector. Both must render the same
 * vocabulary: voice → Volume2, webview → Globe, twitch → Twitch.
 */
export const destinationIcons: Record<Destination, Component> = {
  voice: Volume2,
  webview: Globe,
  twitch: Twitch,
}
