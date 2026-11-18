import { invoke } from '@tauri-apps/api/core'
import type {
  AppInfo, AppSettings, Erwartungshorizont, KeyInfo, KeyStatus, ModelCache, Notenschluessel,
  ProjektMeta, SlotCheck, SlotKind, Stufe, Warnung,
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
}
