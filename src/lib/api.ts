import { invoke } from '@tauri-apps/api/core'
import type {
  AppInfo, AppSettings, KeyInfo, KeyStatus, ModelCache, SlotCheck, SlotKind,
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
}
