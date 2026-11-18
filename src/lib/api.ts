import { invoke } from '@tauri-apps/api/core'
import type {
  AppInfo, AppSettings, Erwartungshorizont, KeyInfo, KeyStatus, ModelCache, Notenschluessel,
  ProjektMeta, SlotCheck, SlotKind, Stufe, Warnung,
  ImportAuftrag, KlausurInfo, PdfInfo, Schwaerzung,
  Aufruf, Element, PromptArt, ProtokollEintrag, PruefDaten, PromptVorlage, Schaetzung, Uebersicht,
} from './types'

/** Fehler aus Rust kommen als deutscher Text. */
export function fehlertext(e: unknown): string {
  if (typeof e === 'string') return e
  if (e instanceof Error) return e.message
  return String(e)
}

export const api = {
  appInfo: () => invoke<AppInfo>('app_info'),
  getSettings: () => invoke<AppSettings>('get_settings'),
  saveSettings: (settings: AppSettings) => invoke<AppSettings>('save_settings', { settings }),
  ackHinweis: () => invoke<void>('ack_hinweis'),
  apiKeyStatus: () => invoke<KeyStatus>('api_key_status'),
  setApiKey: (key: string) => invoke<KeyStatus>('set_api_key', { key }),
  deleteApiKey: () => invoke<KeyStatus>('delete_api_key'),
  checkConnection: () => invoke<KeyInfo>('check_connection'),
  getModels: () => invoke<ModelCache | null>('get_models'),
  refreshModels: () => invoke<ModelCache>('refresh_models'),
  checkSlots: () => invoke<SlotCheck[]>('check_slots'),
  validateSlotDrop: (kind: SlotKind, modelId: string) =>
    invoke<void>('validate_slot_drop', { kind, modelId }),
  resetSlots: () => invoke<AppSettings>('reset_slots'),

  listProjects: () => invoke<ProjektMeta[]>('list_projects'),
  createProject: (meta: ProjektMeta) => invoke<ProjektMeta>('create_project', { meta }),
  updateProject: (meta: ProjektMeta) => invoke<ProjektMeta>('update_project', { meta }),
  deleteProject: (id: string) => invoke<void>('delete_project', { id }),

  getEwh: (projectId: string) => invoke<Erwartungshorizont | null>('get_ewh', { projectId }),
  importEwh: (projectId: string, path: string) => invoke<Erwartungshorizont>('import_ewh', { projectId, path }),
  saveEwh: (projectId: string, ewh: Erwartungshorizont) => invoke<void>('save_ewh', { projectId, ewh }),
  validateEwh: (ewh: Erwartungshorizont, halbeBe: boolean) => invoke<Warnung[]>('validate_ewh', { ewh, halbeBe }),
  berechneNotenschluessel: (stufe: Stufe, gesamt: number) =>
    invoke<Notenschluessel>('berechne_notenschluessel', { stufe, gesamt }),

  pdfInfo: (path: string) => invoke<PdfInfo>('pdf_info', { path }),
  pdfMiniaturen: (path: string, breite: number) => invoke<string[]>('pdf_miniaturen', { path, breite }),
  pdfSeitenvorschau: (path: string, seite: number, breite: number) =>
    invoke<string>('pdf_seitenvorschau', { path, seite, breite }),
  klausuren: (projectId: string) => invoke<KlausurInfo[]>('klausuren', { projectId }),
  scansImportieren: (projectId: string, auftraege: ImportAuftrag[], teilen: boolean) =>
    invoke<KlausurInfo[]>('scans_importieren', { projectId, auftraege, optionen: { teilen } }),
  seitenNeuErzeugen: (projectId: string) => invoke<void>('seiten_neu_erzeugen', { projectId }),
  klausurUmbenennen: (projectId: string, id: string, bezeichnung: string) =>
    invoke<void>('klausur_umbenennen', { projectId, id, bezeichnung }),
  klausurLoeschen: (projectId: string, id: string) => invoke<void>('klausur_loeschen', { projectId, id }),
  seitenMiniatur: (projectId: string, seiteId: string, breite: number) =>
    invoke<string>('seiten_miniatur', { projectId, seiteId, breite }),
  beispielseite: (projectId: string, breite: number) => invoke<string | null>('beispielseite', { projectId, breite }),
  getSchwaerzung: (projectId: string) => invoke<Schwaerzung[]>('get_schwaerzung', { projectId }),
  setSchwaerzung: (projectId: string, schwaerzung: Schwaerzung[]) =>
    invoke<void>('set_schwaerzung', { projectId, schwaerzung }),
  getPseudonyme: (projectId: string) => invoke<string[]>('get_pseudonyme', { projectId }),
  setPseudonyme: (projectId: string, liste: string[]) => invoke<void>('set_pseudonyme', { projectId, liste }),
  pseudonymeImportieren: (projectId: string, path: string) =>
    invoke<string[]>('pseudonyme_importieren', { projectId, path }),
  bezeichnungVorschlaege: (projectId: string, dateinamen: string[]) =>
    invoke<string[]>('bezeichnung_vorschlaege', { projectId, dateinamen }),

  pipelineUebersicht: (projectId: string) => invoke<Uebersicht>('pipeline_uebersicht', { projectId }),
  kostenSchaetzen: (projectId: string, klausurIds: string[]) =>
    invoke<Schaetzung>('kosten_schaetzen', { projectId, klausurIds }),
  pipelineStarten: (projectId: string, klausurIds: string[]) =>
    invoke<void>('pipeline_starten', { projectId, klausurIds }),
  pipelineAnhalten: (projectId: string) => invoke<void>('pipeline_anhalten', { projectId }),
  pipelineAbbrechen: (projectId: string) => invoke<void>('pipeline_abbrechen', { projectId }),
  aufrufe: (projectId: string, klausurId: string | null) => invoke<Aufruf[]>('aufrufe', { projectId, klausurId }),
  promptsLesen: () => invoke<PromptVorlage[]>('prompts_lesen'),
  promptSpeichern: (art: PromptArt, text: string) => invoke<void>('prompt_speichern', { art, text }),
  promptZuruecksetzen: (art: PromptArt) => invoke<void>('prompt_zuruecksetzen', { art }),
  kiHilfeEwh: (projectId: string, ewh: Erwartungshorizont, nr: string) =>
    invoke<Element[]>('ki_hilfe_ewh', { projectId, ewh, nr }),

  pruefdaten: (projectId: string, klausurId: string) => invoke<PruefDaten>('pruefdaten', { projectId, klausurId }),
  beSetzen: (projectId: string, klausurId: string, elementId: string, be: number, bestaetigt: boolean) =>
    invoke<PruefDaten>('be_setzen', { projectId, klausurId, elementId, be, bestaetigt }),
  bestaetigungAufheben: (projectId: string, klausurId: string, elementId: string) =>
    invoke<PruefDaten>('bestaetigung_aufheben', { projectId, klausurId, elementId }),
  freigeben: (projectId: string, klausurId: string) => invoke<PruefDaten>('freigeben', { projectId, klausurId }),
  freigabeZuruecknehmen: (projectId: string, klausurId: string) =>
    invoke<PruefDaten>('freigabe_zuruecknehmen', { projectId, klausurId }),
  abschriftSpeichern: (projectId: string, klausurId: string, aufgabe: string, text: string) =>
    invoke<void>('abschrift_speichern', { projectId, klausurId, aufgabe, text }),
  protokoll: (projectId: string, klausurId: string) => invoke<ProtokollEintrag[]>('protokoll', { projectId, klausurId }),
  seitenbild: (projectId: string, seiteId: string) => invoke<ArrayBuffer>('seitenbild', { projectId, seiteId }),
  aufgabeNeuBewerten: (projectId: string, klausurId: string, aufgabe: string) =>
    invoke<PruefDaten>('aufgabe_neu_bewerten', { projectId, klausurId, aufgabe }),
}
