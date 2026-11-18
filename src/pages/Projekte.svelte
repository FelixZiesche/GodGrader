<script lang="ts">
  import { api, fehlertext } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import Dialog from '../components/Dialog.svelte'
  import type { ProjektMeta } from '../lib/types'

  let { weiter }: { weiter: () => void } = $props()

  const leer = (): ProjektMeta => ({
    id: '', titel: '', lerngruppe: '', datum: new Date().toISOString().slice(0, 10), stufe: 'oberstufe',
    halbe_be: false, schwaerzen: true, budget_usd: null, warnschwelle_prozent: null, erstellt: '', geaendert: '',
  })

  let formOffen = $state(false)
  let form = $state<ProjektMeta>(leer())
  let fehler = $state<string | null>(null)
  let loeschen = $state<ProjektMeta | null>(null)
  let loeschenOffen = $state(false)

  function neu() {
    form = leer()
    fehler = null
    formOffen = true
  }
  function bearbeiten(p: ProjektMeta) {
    form = { ...p }
    fehler = null
    formOffen = true
  }

  async function speichern() {
    fehler = null
    try {
      const m = $state.snapshot(form) as ProjektMeta
      const saved = m.id ? await api.updateProject(m) : await api.createProject(m)
      await store.ladeProjekte()
      store.waehleProjekt(saved.id)
      formOffen = false
      if (!m.id) weiter()
    } catch (e) {
      fehler = fehlertext(e)
    }
  }

  async function wirklichLoeschen() {
    if (!loeschen) return
    await api.deleteProject(loeschen.id)
    loeschenOffen = false
    loeschen = null
    await store.ladeProjekte()
  }

  const datumDe = (iso: string) => (iso ? new Date(iso + 'T00:00').toLocaleDateString('de-DE') : '–')
</script>

<div class="kopf">
  <h1>Projekte</h1>
  <button class="primaer" onclick={neu}>Neues Projekt</button>
</div>
<p class="hinweis">Ein Projekt ist eine Klausur einer Lerngruppe. Alles liegt nur auf diesem Rechner.</p>

{#if store.projekte.length === 0}
  <div class="karte grau"><p>Noch keine Projekte. Legen Sie zuerst ein Projekt an.</p></div>
{:else}
  <div class="liste">
    {#each store.projekte as p (p.id)}
      <div class="karte projekt" class:aktiv={store.projektId === p.id}>
        <div>
          <h2>{p.titel}</h2>
          <p class="hinweis">
            {p.lerngruppe || 'ohne Lerngruppe'} · {datumDe(p.datum)} ·
            {p.stufe === 'oberstufe' ? 'Oberstufe (0–15 NP)' : 'Sek I (Noten 1–6)'}
            {#if p.halbe_be}· halbe BE{/if}
            {#if !p.schwaerzen}· ohne Schwärzen{/if}
          </p>
        </div>
        <div class="zeile">
          {#if store.projektId === p.id}
            <span class="ok">geöffnet</span>
          {:else}
            <button class="primaer" onclick={() => { store.waehleProjekt(p.id); weiter() }}>Öffnen</button>
          {/if}
          <button onclick={() => bearbeiten(p)}>Bearbeiten</button>
          <button class="leise" onclick={() => { loeschen = p; loeschenOffen = true }}>Löschen</button>
        </div>
      </div>
    {/each}
  </div>
{/if}

<Dialog titel={form.id ? 'Projekt bearbeiten' : 'Neues Projekt'} bind:offen={formOffen}>
  <label class="feld"><span>Titel</span><input bind:value={form.titel} placeholder="z. B. 11.1 Klausur Soziale Marktwirtschaft" style="width: 100%" /></label>
  <div class="zwei">
    <label class="feld"><span>Lerngruppe</span><input bind:value={form.lerngruppe} placeholder="z. B. WR_gA" style="width: 100%" /></label>
    <label class="feld"><span>Datum</span><input type="date" bind:value={form.datum} style="width: 100%" /></label>
  </div>
  <div class="feld">
    <span>Stufe</span>
    <label class="zeile"><input type="radio" bind:group={form.stufe} value="oberstufe" /> Oberstufe (Notenpunkte 0–15, Anrede „Sie“)</label>
    <label class="zeile"><input type="radio" bind:group={form.stufe} value="sek1" /> Sek I (Noten 1–6, Anrede „du“)</label>
  </div>
  <label class="feld zeile"><input type="checkbox" bind:checked={form.halbe_be} /> halbe BE erlaubt</label>
  <label class="feld zeile"><input type="checkbox" bind:checked={form.schwaerzen} /> Bereiche (z. B. Namenszeile) vor dem Senden schwärzen</label>
  <div class="zwei">
    <label class="feld">
      <span>Kostenlimit (optional)</span>
      <input type="number" min="0" step="0.5" bind:value={form.budget_usd} placeholder="kein Limit" style="width: 120px" /> $
      <small>Bei Erreichen wird die Verarbeitung angehalten.</small>
    </label>
    <label class="feld">
      <span>Warnschwelle schlechte Noten (optional)</span>
      <input type="number" min="0" max="100" step="1" bind:value={form.warnschwelle_prozent} placeholder="aus" style="width: 90px" /> %
      <small>Für die Statistik, z. B. Genehmigungspflicht.</small>
    </label>
  </div>
  {#if fehler}<div class="warnung">{fehler}</div>{/if}
  {#snippet aktionen()}
    <button onclick={() => (formOffen = false)}>Abbrechen</button>
    <button class="primaer" onclick={speichern}>{form.id ? 'Speichern' : 'Anlegen'}</button>
  {/snippet}
</Dialog>

<Dialog titel="Projekt löschen?" bind:offen={loeschenOffen}>
  <p>
    „{loeschen?.titel}“ wird mit <strong>allen Dateien</strong> gelöscht: Scans, Seitenbilder, Abschriften, Bewertungen und
    Erwartungshorizont. Das lässt sich nicht rückgängig machen.
  </p>
  {#snippet aktionen()}
    <button onclick={() => (loeschenOffen = false)}>Abbrechen</button>
    <button class="primaer" onclick={wirklichLoeschen}>Endgültig löschen</button>
  {/snippet}
</Dialog>

<style>
  .kopf { display: flex; justify-content: space-between; align-items: baseline; }
  .liste { display: flex; flex-direction: column; gap: 10px; max-width: 900px; }
  .projekt { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; }
  .projekt.aktiv { border-width: 2px; }
  .projekt h2 { margin-bottom: 2px; }
  .projekt p { margin: 0; }
  .zwei { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
</style>
