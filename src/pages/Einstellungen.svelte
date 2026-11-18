<script lang="ts">
  import { onMount } from 'svelte'
  import { api, fehlertext } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import type { AppInfo, KeyInfo, KeyStatus } from '../lib/types'

  let keyStatus = $state<KeyStatus | null>(null)
  let neuerKey = $state('')
  let keyMeldung = $state<string | null>(null)
  let keyFehler = $state<string | null>(null)
  let pruefung = $state<'idle' | 'laeuft'>('idle')
  let verbindung = $state<KeyInfo | null>(null)
  let info = $state<AppInfo | null>(null)
  let gespeichert = $state(false)

  onMount(async () => {
    keyStatus = await api.apiKeyStatus()
    info = await api.appInfo()
  })

  async function keySpeichern() {
    keyFehler = keyMeldung = null
    try {
      keyStatus = await api.setApiKey(neuerKey)
      neuerKey = ''
      keyMeldung = 'Schlüssel im Schlüsselbund gespeichert.'
    } catch (e) {
      keyFehler = fehlertext(e)
    }
  }

  async function keyLoeschen() {
    keyFehler = keyMeldung = null
    try {
      keyStatus = await api.deleteApiKey()
      verbindung = null
      keyMeldung = 'Schlüssel entfernt.'
    } catch (e) {
      keyFehler = fehlertext(e)
    }
  }

  async function verbindungPruefen() {
    keyFehler = keyMeldung = null
    verbindung = null
    pruefung = 'laeuft'
    try {
      verbindung = await api.checkConnection()
    } catch (e) {
      keyFehler = fehlertext(e)
    } finally {
      pruefung = 'idle'
    }
  }

  async function allgemeinSpeichern() {
    if (!store.settings) return
    await store.save(store.settings)
    gespeichert = true
    setTimeout(() => (gespeichert = false), 1800)
  }

  const usd = (v: number | null) =>
    v === null ? '–' : v.toLocaleString('de-DE', { minimumFractionDigits: 2, maximumFractionDigits: 4 }) + ' $'
</script>

<h1>Einstellungen</h1>

<div class="raster">
  <section class="karte">
    <h2>OpenRouter-API-Schlüssel</h2>
    <p class="hinweis">
      Der Schlüssel wird nur im Schlüsselbund des Betriebssystems gespeichert (Windows:
      Anmeldeinformationsverwaltung), nie in Dateien.
    </p>
    {#if keyStatus}
      <p>
        Status:
        {#if keyStatus.present}<span class="ok">hinterlegt</span> ({keyStatus.masked}){:else}nicht hinterlegt{/if}
      </p>
      {#if keyStatus.error}<div class="warnung">{keyStatus.error}</div>{/if}
    {/if}
    <label class="feld">
      <span>{keyStatus?.present ? 'Neuen Schlüssel eintragen' : 'Schlüssel eintragen'}</span>
      <input type="password" autocomplete="off" placeholder="sk-or-…" bind:value={neuerKey} style="width: 100%" />
    </label>
    <div class="zeile">
      <button class="primaer" disabled={!neuerKey.trim()} onclick={keySpeichern}>Speichern</button>
      <button disabled={!keyStatus?.present || pruefung === 'laeuft'} onclick={verbindungPruefen}>
        {pruefung === 'laeuft' ? 'Prüfe …' : 'Verbindung prüfen'}
      </button>
      {#if keyStatus?.present}<button class="leise" onclick={keyLoeschen}>Schlüssel entfernen</button>{/if}
    </div>
    {#if keyMeldung}<p class="ok" style="margin-top: 10px">{keyMeldung}</p>{/if}
    {#if keyFehler}<div class="warnung" style="margin-top: 10px">{keyFehler}</div>{/if}
    {#if verbindung}
      <div class="karte grau" style="margin-top: 10px">
        <p class="ok">Verbindung funktioniert.</p>
        <p>Verbraucht: {usd(verbindung.usage)} · Limit: {verbindung.limit === null ? 'keins' : usd(verbindung.limit)}</p>
      </div>
    {/if}
  </section>

  {#if store.settings}
    <section class="karte">
      <h2>Datenschutz und Verarbeitung</h2>
      <p class="hinweis">Jede Anfrage verbietet den Anbietern grundsätzlich, Daten zu speichern oder zum Training zu nutzen.</p>
      <label class="feld zeile">
        <input type="checkbox" bind:checked={store.settings.zdr_only} />
        Nur Anbieter ohne Datenspeicherung (ZDR)
      </label>
      <label class="feld">
        <span>Gleichzeitige Anfragen</span>
        <input type="number" min="1" max="16" bind:value={store.settings.parallelism} style="width: 90px" />
        <small>Standard 3. Mehr ist schneller, löst aber eher Ratenlimits aus.</small>
      </label>
      <label class="feld">
        <span>Wiederholungen bei Fehlern</span>
        <input type="number" min="0" max="10" bind:value={store.settings.max_retries} style="width: 90px" />
        <small>Bei Überlastung (429) oder Serverfehlern, mit wachsender Wartezeit.</small>
      </label>
      <div class="zeile">
        <button class="primaer" onclick={allgemeinSpeichern}>Speichern</button>
        {#if gespeichert}<span class="ok">Gespeichert</span>{/if}
      </div>
    </section>
  {/if}

  <section class="karte grau">
    <h2>Über GodGrader</h2>
    {#if info}
      <p>Version {info.version}</p>
      <p class="hinweis">Datenordner: {info.data_dir}</p>
    {/if}
    <p class="hinweis">Keine Telemetrie, keine Cloud-Synchronisation, keine automatischen Updates. Netzwerkzugriff nur zu OpenRouter.</p>
  </section>
</div>

<style>
  .raster { display: grid; grid-template-columns: repeat(auto-fill, minmax(380px, 1fr)); gap: 16px; align-items: start; }
</style>
