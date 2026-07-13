export interface PhraseEntry {
  id: string
  text: string
  count: number
  last_used: number
  provider?: string
  voice?: string
  cache_key?: string
}
