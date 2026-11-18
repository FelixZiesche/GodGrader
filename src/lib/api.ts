import { invoke } from '@tauri-apps/api/core'
import type {
  AppInfo, AppSettings, Erwartungshorizont, KeyInfo, KeyStatus, ModelCache, Notenschluessel,
  ProjektMeta, SlotCheck, SlotKind, Stufe, Warnung,
  ImportAuftrag, KlausurInfo, PdfInfo, Schwaerzung,
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
}
