export interface PhraseEntry {
  id: string
  provider_text: string
  insert_text: string
  count: number
  last_used: number
  provider?: string
  voice?: string
  cache_key?: string
}
