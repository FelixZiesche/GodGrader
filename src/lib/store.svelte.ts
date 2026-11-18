import { api, fehlertext } from './api'
import type { AppSettings, ModelCache, SlotCheck } from './types'

/** Gemeinsamer Zustand der App (Svelte-5-Runes). */
class AppStore {
  settings = $state<AppSettings | null>(null)
  models = $state<ModelCache | null>(null)
  slotChecks = $state<SlotCheck[]>([])
  modelsLoading = $state(false)
  modelsError = $state<string | null>(null)

  async init() {
    this.settings = await api.getSettings()
    this.models = await api.getModels()
    await this.refreshChecks()
    // Beim Start frisch laden; bei Fehler bleibt der Zwischenspeicher.
    void this.refreshModels()
  }

  async refreshModels() {
    this.modelsLoading = true
    this.modelsError = null
    try {
      this.models = await api.refreshModels()
    } catch (e) {
      this.modelsError = fehlertext(e)
    } finally {
      this.modelsLoading = false
    }
    await this.refreshChecks()
  }

  async refreshChecks() {
    try {
      this.slotChecks = await api.checkSlots()
    } catch {
      this.slotChecks = []
    }
  }

  async save(next: AppSettings) {
    this.settings = await api.saveSettings($state.snapshot(next) as AppSettings)
    await this.refreshChecks()
  }
}

export const store = new AppStore()
