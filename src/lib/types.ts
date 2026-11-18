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

// ---------- Projekte und Erwartungshorizont ----------

export type Stufe = 'oberstufe' | 'sek1'
export type Afb = 'I' | 'II' | 'III'
export type ElementArt = 'element' | 'raster'

export interface ProjektMeta {
  id: string
  titel: string
  lerngruppe: string
  datum: string
  stufe: Stufe
  halbe_be: boolean
  schwaerzen: boolean
  budget_usd: number | null
  warnschwelle_prozent: number | null
  erstellt: string
  geaendert: string
}

export interface Element {
  id: string
  titel: string
  text: string
  max_be: number
  art: ElementArt
  afb: Afb | null
}

export interface AfbAnteil {
  afb: Afb
  be: number
  zusatz: string | null
}

export interface Aufgabe {
  nr: string
  text: string
  max_be: number
  loesung: string
  elemente: Element[]
  afb: AfbAnteil[]
  hat_raster: boolean
}

export interface Kopf {
  titel: string
  schule: string | null
  schuljahr: string | null
  thema: string | null
  lerngruppe: string | null
  datum: string | null
  gleichwertig_hinweis: boolean
}

export interface Notenstufe {
  punkte: number | null
  note: string
  ab_be: number
}

export interface Notenschluessel {
  stufen: Notenstufe[]
  aus_dokument: boolean
}

export interface Erwartungshorizont {
  kopf: Kopf
  stufe: Stufe
  aufgaben: Aufgabe[]
  gesamt_be_dokument: number | null
  notenschluessel: Notenschluessel | null
  import_hinweise: string[]
}

export interface Warnung {
  aufgabe: string | null
  text: string
}

// ---------- Scans ----------

export type KlausurStatus = 'offen' | 'vorkorrigiert' | 'in_pruefung' | 'freigegeben'

export interface SeiteInfo {
  id: string
  nr: number
  halbteil: 'oben' | 'unten' | null
  quelle_seite: number
}

export interface KlausurInfo {
  id: string
  bezeichnung: string
  status: KlausurStatus
  quelle: string
  seiten: SeiteInfo[]
}

export interface Bereich { x: number; y: number; w: number; h: number }

export type SeitenAuswahl = { art: 'alle' } | { art: 'seiten'; liste: number[] }

export interface Schwaerzung { bereich: Bereich; seiten: SeitenAuswahl }

export interface ImportAuftrag { bezeichnung: string; pdf: string; seiten: number[] }

export interface Fortschritt { fertig: number; gesamt: number; text: string }

export interface PdfInfo { seiten: number; name: string }

// ---------- KI-Verarbeitung ----------

export type SchrittStatus = 'offen' | 'laeuft' | 'fertig' | 'fehler'
export type SchrittArt = 'abschreiben' | 'bewerten' | 'zweitmeinung'

export interface KlausurFortschritt {
  id: string
  bezeichnung: string
  status: KlausurStatus
  seiten: number
  abschrift: SchrittStatus
  abschrift_hinweis: string | null
  aufgaben: number
  bewertet: number
  zweit: number
  laeuft: boolean
  fehler: string[]
  kosten: number
}

export interface Uebersicht {
  klausuren: KlausurFortschritt[]
  kosten_gesamt: number
  budget: number | null
  zweitmeinung: boolean
  hat_ewh: boolean
  laeuft: boolean
}

export interface Schaetzung {
  klausuren: number
  seitenbilder: number
  aufrufe: number
  kosten: number
  kosten_min: number
  kosten_max: number
  aus_erfahrung: boolean
  ohne_preis: string[]
  bisher: number
  budget: number | null
}

export type Ereignis =
  | { art: 'schritt'; klausur_id: string; schritt: SchrittArt; aufgabe: string; status: SchrittStatus; fehler: string | null }
  | { art: 'kosten'; gesamt: number }
  | { art: 'klausur'; klausur_id: string; status: KlausurStatus }
  | { art: 'hinweis'; text: string }

export interface LaufErgebnis {
  ende: 'fertig' | 'angehalten' | 'abgebrochen' | 'budget'
  fehler: number
  kosten: number
}

export interface PipelineMeldung { projekt_id: string; ereignis: Ereignis }
export interface PipelineEnde { projekt_id: string; ergebnis: LaufErgebnis | null; fehler: string | null }

export type PromptArt = 'abschreiben' | 'bewerten' | 'feedback' | 'ewh_hilfe'

export interface PromptVorlage {
  art: PromptArt
  titel: string
  text: string
  ist_standard: boolean
  platzhalter: string[]
}

export interface Aufruf {
  klausur_id: string | null
  art: string
  aufgabe: string | null
  modell: string
  tokens_ein: number
  tokens_aus: number
  kosten: number
  geschaetzt: boolean
  dauer_ms: number
  ok: boolean
}

// ---------- Prüfansicht ----------

export interface AbschriftZeile {
  aufgabe: string
  text: string
  ki_text: string
  unsicher: string[]
  seiten: number[]
}

export interface BewertungZeile {
  element_id: string
  aufgabe: string
  max_be: number
  ki_be: number | null
  begruendung: string
  beleg: string
  sicherheit: 'hoch' | 'mittel' | 'niedrig' | string
  unsicher: boolean
  zweit_be: number | null
  zweit_begruendung: string | null
  final_be: number | null
  bestaetigt: boolean
}

export interface Schritt {
  klausur_id: string
  art: SchrittArt
  aufgabe: string
  status: SchrittStatus
  modell: string | null
  fehler: string | null
  hinweis: string | null
  aktualisiert: string
}

export interface Ergebnis {
  summe: number
  summe_final: number
  gesamt: number
  prozent: number
  punkte: number | null
  note: string | null
  elemente: number
  bestaetigt: number
  geaendert: number
}

export interface PruefDaten {
  klausur: KlausurInfo
  abschriften: AbschriftZeile[]
  bewertungen: BewertungZeile[]
  schritte: Schritt[]
  ergebnis: Ergebnis
}

export interface ProtokollEintrag {
  element_id: string
  ki_be: number | null
  vorher: number | null
  nachher: number | null
  bestaetigt: boolean
  zeit: string
}
