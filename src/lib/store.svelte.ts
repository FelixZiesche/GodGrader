import { api, fehlertext } from './api'
import type { AppSettings, ModelCache, ProjektMeta, SlotCheck } from './types'

/** Gemeinsamer Zustand der App (Svelte-5-Runes). */
class AppStore {
  settings = $state<AppSettings | null>(null)
  models = $state<ModelCache | null>(null)
  slotChecks = $state<SlotCheck[]>([])
  modelsLoading = $state(false)
  modelsError = $state<string | null>(null)
  projekte = $state<ProjektMeta[]>([])
  projektId = $state<string | null>(null)

  get projekt(): ProjektMeta | null {
    return this.projekte.find((p) => p.id === this.projektId) ?? null
  }

  async ladeProjekte() {
    this.projekte = await api.listProjects()
    if (this.projektId && !this.projekte.some((p) => p.id === this.projektId)) this.projektId = null
    if (!this.projektId) {
      try {
        const last = localStorage.getItem('projekt')
        if (last && this.projekte.some((p) => p.id === last)) this.projektId = last
      } catch { /* ohne Speicher */ }
    }
  }

  waehleProjekt(id: string | null) {
    this.projektId = id
    try {
      if (id) localStorage.setItem('projekt', id)
      else localStorage.removeItem('projekt')
    } catch { /* ohne Speicher */ }
  }

  async init() {
    this.settings = await api.getSettings()
    this.models = await api.getModels()
    await this.ladeProjekte()
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
