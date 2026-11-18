// Spiegel der Rust-Typen aus godgrader-core (serde-Namen).

export type SlotKind = 'abschreiben' | 'bewerten' | 'feedback' | 'zweitmeinung'
export type ReasoningEffort = 'standard' | 'niedrig' | 'mittel' | 'hoch'

export interface SlotConfig {
  model: string
  enabled: boolean
  temperature: number
  max_tokens: number | null
  reasoning: ReasoningEffort
}

export type SlotSet = Record<SlotKind, SlotConfig>

export interface AppSettings {
  slots: SlotSet
  parallelism: number
  zdr_only: boolean
  max_retries: number
  hinweis_bestaetigt: boolean
}

export interface ModelInfo {
  id: string
  name: string
  provider: string
  created: number
  context_length: number | null
  price_in_per_m: number | null
  price_out_per_m: number | null
  reads_images: boolean
  supports_reasoning: boolean
  supports_structured: boolean
  supports_temperature: boolean
  max_completion_tokens: number | null
}

export interface ModelCache {
  fetched_at: number
  models: ModelInfo[]
}

export interface SlotCheck {
  kind: SlotKind
  model: string
  available: boolean
  problem: string | null
  suggestion: string | null
}

export interface KeyStatus {
  present: boolean
  masked: string | null
  error: string | null
}

export interface KeyInfo {
  label: string | null
  usage: number | null
  limit: number | null
  limit_remaining: number | null
  is_free_tier: boolean | null
}

export interface AppInfo {
  version: string
  data_dir: string
}

export const SLOTS: { kind: SlotKind; nr: number; titel: string; aufgabe: string }[] = [
  { kind: 'abschreiben', nr: 1, titel: 'Abschreiben', aufgabe: 'Handschrift → Text. Muss Bilder lesen können.' },
  { kind: 'bewerten', nr: 2, titel: 'Bewerten', aufgabe: 'Abschrift + Erwartungshorizont → BE-Vorschläge.' },
  { kind: 'feedback', nr: 3, titel: 'Feedback', aufgabe: 'Rückmeldetext pro Schüler.' },
  { kind: 'zweitmeinung', nr: 4, titel: 'Zweitmeinung', aufgabe: 'Unabhängige zweite Bewertung (optional).' },
]
