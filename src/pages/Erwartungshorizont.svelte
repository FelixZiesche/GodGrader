<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import { getCurrentWebview } from '@tauri-apps/api/webview'
  import { api, fehlertext } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import AufgabeEditor from '../components/AufgabeEditor.svelte'
  import Dialog from '../components/Dialog.svelte'
  import type { Element, Erwartungshorizont, Warnung } from '../lib/types'

  let ewh = $state<Erwartungshorizont | null>(null)
  let laden = $state(true)
  let importiert = $state(false)
  let fehler = $state<string | null>(null)
  let warnungen = $state<Warnung[]>([])
  let speicherStatus = $state<'gespeichert' | 'speichert' | 'fehler'>('gespeichert')
  let ziehen = $state(false)
  let unlisten: (() => void) | null = null

  const projekt = $derived(store.projekt)
  const gesamt = $derived(ewh ? ewh.aufgaben.reduce((s, a) => s + (Number(a.max_be) || 0), 0) : 0)
  const allgemein = $derived(warnungen.filter((w) => !w.aufgabe))
  const proAufgabe = (nr: string) => warnungen.filter((w) => w.aufgabe === nr).map((w) => w.text)

  async function lade() {
    laden = true
    fehler = null
    ewh = projekt ? await api.getEwh(projekt.id) : null
    laden = false
  }

  onMount(async () => {
    await lade()
    try {
      unlisten = await getCurrentWebview().onDragDropEvent((ev) => {
        if (ev.payload.type === 'over' || ev.payload.type === 'enter') ziehen = true
        else if (ev.payload.type === 'leave') ziehen = false
        else if (ev.payload.type === 'drop') {
          ziehen = false
          const p = ev.payload.paths.find((x) => /\.(docx|pdf)$/i.test(x))
          if (p) void importieren(p)
        }
      })
    } catch {
      /* außerhalb von Tauri (Vorschau) */
    }
  })
  onDestroy(() => unlisten?.())

  async function waehlen() {
    const p = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Erwartungshorizont', extensions: ['docx', 'pdf'] }],
    })
    if (typeof p === 'string') await importieren(p)
  }

  async function importieren(pfad: string) {
    if (!projekt) return
    fehler = null
    laden = true
    try {
      ewh = await api.importEwh(projekt.id, pfad)
      importiert = true
    } catch (e) {
      fehler = fehlertext(e)
    } finally {
      laden = false
    }
  }

  // Automatisch prüfen und speichern
  let timer: ReturnType<typeof setTimeout> | undefined
  let erstesMal = true
  $effect(() => {
    if (!ewh || !projekt) return
    const snap = $state.snapshot(ewh) as Erwartungshorizont
    const pid = projekt.id
    const halbe = projekt.halbe_be
    clearTimeout(timer)
    void api.validateEwh(snap, halbe).then((w) => (warnungen = w))
    if (erstesMal) {
      erstesMal = false
      return
    }
    speicherStatus = 'speichert'
    timer = setTimeout(async () => {
      try {
        await api.saveEwh(pid, snap)
        speicherStatus = 'gespeichert'
      } catch {
        speicherStatus = 'fehler'
      }
    }, 600)
  })

  async function notenNeu() {
    if (!ewh) return
    ewh.notenschluessel = await api.berechneNotenschluessel(ewh.stufe, gesamt)
  }

  function aufgabeNeu() {
    if (!ewh) return
    ewh.aufgaben.push({ nr: String(ewh.aufgaben.length + 1), text: '', max_be: 1, loesung: '', elemente: [], afb: [], hat_raster: false })
  }
  // KI-Hilfe beim Import
  let hilfe = $state<{ index: number; nr: string; elemente: Element[] | null; fehler: string | null } | null>(null)
  let hilfeOffen = $state(false)
  const hilfeSumme = $derived(hilfe?.elemente?.reduce((s, e) => s + e.max_be, 0) ?? 0)
  async function kiHilfeStarten(i: number) {
    if (!ewh || !projekt) return
    const nr = ewh.aufgaben[i].nr
    hilfe = { index: i, nr, elemente: null, fehler: null }
    hilfeOffen = true
    try {
      const el = await api.kiHilfeEwh(projekt.id, $state.snapshot(ewh) as Erwartungshorizont, nr)
      if (hilfe?.nr === nr) hilfe.elemente = el
    } catch (e) {
      if (hilfe?.nr === nr) hilfe.fehler = fehlertext(e)
    }
  }
  function kiHilfeUebernehmen() {
    if (!ewh || !hilfe?.elemente) return
    const a = ewh.aufgaben[hilfe.index]
    if (a && a.nr === hilfe.nr) a.elemente = hilfe.elemente
    hilfeOffen = false
  }
</script>

<div class="kopf">
  <h1>Erwartungshorizont</h1>
  {#if ewh}
    <span class="hinweis">
      {#if speicherStatus === 'speichert'}speichert …{:else if speicherStatus === 'fehler'}<span class="rot">Speichern fehlgeschlagen</span>{:else}automatisch gespeichert{/if}
    </span>
  {/if}
</div>

{#if !projekt}
  <div class="karte grau"><p>Bitte zuerst unter „Projekte“ ein Projekt anlegen oder öffnen.</p></div>
{:else}
  <div class="karte grau import" class:ziehen>
    <div>
      <strong>{ewh ? 'Neu importieren' : 'Erwartungshorizont importieren'}</strong>
      <p class="hinweis" style="margin: 2px 0 0">Word (.docx) oder PDF aus godtest. Datei hierher ziehen oder auswählen.</p>
    </div>
    <button class="primaer" onclick={waehlen} disabled={laden}>{laden ? 'Liest …' : 'Datei wählen'}</button>
  </div>
  {#if fehler}<div class="warnung" style="margin-top: 10px">{fehler}</div>{/if}

  {#if ewh}
    {#if importiert}
      <div class="karte grau" style="margin-top: 10px">
        Import abgeschlossen: {ewh.aufgaben.length} Aufgaben, {ewh.aufgaben.reduce((s, a) => s + a.elemente.length, 0)} Bewertungselemente. Bitte alles prüfen.
      </div>
    {/if}

    <section class="karte" style="margin-top: 14px">
      <h2>Kopf</h2>
      <div class="raster4">
        <label class="feld"><span>Titel</span><input bind:value={ewh.kopf.titel} /></label>
        <label class="feld"><span>Thema</span><input bind:value={ewh.kopf.thema} /></label>
        <label class="feld"><span>{ewh.stufe === 'oberstufe' ? 'Kurs' : 'Klasse'}</span><input bind:value={ewh.kopf.lerngruppe} /></label>
        <label class="feld"><span>Datum</span><input bind:value={ewh.kopf.datum} placeholder="TT.MM.JJJJ" /></label>
        <label class="feld">
          <span>Stufe</span>
          <select bind:value={ewh.stufe}><option value="oberstufe">Oberstufe (NP 0–15)</option><option value="sek1">Sek I (Noten 1–6)</option></select>
        </label>
        <div class="feld"><span>Gesamt</span><strong>{String(gesamt).replace('.', ',')} BE</strong>{#if ewh.gesamt_be_dokument !== null}<small>laut AFB-Übersicht {ewh.gesamt_be_dokument} BE</small>{/if}</div>
      </div>
      {#if projekt.stufe !== ewh.stufe}
        <div class="warnung klein">Die Stufe im Erwartungshorizont weicht vom Projekt ab ({projekt.stufe === 'oberstufe' ? 'Oberstufe' : 'Sek I'}).</div>
      {/if}
    </section>

    {#if allgemein.length || ewh.import_hinweise.length}
      <section class="karte" style="margin-top: 14px; border-color: var(--akzent)">
        <h2>Hinweise</h2>
        <ul>
          {#each ewh.import_hinweise as h (h)}<li>{h}</li>{/each}
          {#each allgemein as w (w.text)}<li>{w.text}</li>{/each}
        </ul>
      </section>
    {:else if warnungen.length === 0}
      <p class="ok" style="margin: 12px 0 0">Alle Prüfungen bestanden: Summen, Raster, AFB und Notenschlüssel passen.</p>
    {/if}

    <h2 style="margin-top: 20px">Aufgaben</h2>
    {#each ewh.aufgaben as _, i (i)}
      <AufgabeEditor
        bind:aufgabe={ewh.aufgaben[i]}
        warnungen={proAufgabe(ewh.aufgaben[i].nr)}
        halbeBe={projekt.halbe_be}
        entfernen={() => ewh!.aufgaben.splice(i, 1)}
        kiHilfe={() => kiHilfeStarten(i)}
      />
    {/each}
    <button onclick={aufgabeNeu}>+ Aufgabe</button>

    <section class="karte" style="margin-top: 20px">
      <div class="zeile" style="justify-content: space-between">
        <h2>Notenschlüssel</h2>
        <div class="zeile">
          <span class="hinweis">{ewh.notenschluessel?.aus_dokument ? 'aus dem Dokument' : ewh.notenschluessel ? 'berechnet' : 'fehlt'}</span>
          <button onclick={notenNeu}>Für {String(gesamt).replace('.', ',')} BE neu berechnen</button>
        </div>
      </div>
      {#if ewh.notenschluessel}
        <div class="noten">
          <table>
            <tbody>
              {#if ewh.stufe === 'oberstufe'}
                <tr><th>Notenpunkte</th>{#each ewh.notenschluessel.stufen as s, i (i)}<td>{s.punkte ?? ''}</td>{/each}</tr>
              {/if}
              <tr><th>Note</th>{#each ewh.notenschluessel.stufen as s, i (i)}<td>{s.note}</td>{/each}</tr>
              <tr><th>ab BE</th>{#each ewh.notenschluessel.stufen as s, i (i)}<td><input type="number" min="0" step="0.5" bind:value={s.ab_be} /></td>{/each}</tr>
            </tbody>
          </table>
        </div>
      {:else}
        <p class="hinweis">Kein Notenschlüssel vorhanden. „Neu berechnen“ erzeugt ihn nach Ihrer Tabelle.</p>
      {/if}
    </section>
  {:else if !laden}
    <p class="hinweis" style="margin-top: 12px">Für dieses Projekt ist noch kein Erwartungshorizont importiert.</p>
  {/if}
{/if}

<style>
  .vorschlag { width: 100%; border-collapse: collapse; margin-bottom: 8px; }
  .vorschlag th, .vorschlag td { text-align: left; padding: 4px 6px; border-bottom: 1px solid var(--grau-2); vertical-align: top; }
  .vorschlag th { background: var(--grau-2); }
  .kopf { display: flex; justify-content: space-between; align-items: baseline; }
  .rot { color: var(--akzent); }
  .import { display: flex; justify-content: space-between; align-items: center; gap: 12px; border: 1px dashed var(--grau-3); }
  .import.ziehen { border-color: var(--schwarz); background: var(--grau-2); }
  .raster4 { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 0 14px; }
  .raster4 input, .raster4 select { width: 100%; }
  .klein { padding: 4px 10px; font-size: 13px; }
  ul { margin: 0; padding-left: 18px; }
  .noten { overflow-x: auto; }
  .noten table { border-collapse: collapse; }
  .noten th { background: var(--grau-2); text-align: left; padding: 4px 8px; white-space: nowrap; }
  .noten td { border: 1px solid var(--grau-3); padding: 2px; text-align: center; min-width: 44px; }
  .noten input { width: 48px; padding: 2px; text-align: center; border: none; }
</style>

<Dialog titel={hilfe ? `KI-Vorschlag für Aufgabe ${hilfe.nr}` : 'KI-Vorschlag'} bind:offen={hilfeOffen}>
  {#if hilfe}
    {#if hilfe.fehler}
      <div class="warnung">{hilfe.fehler}</div>
    {:else if !hilfe.elemente}
      <p class="hinweis">Das Bewerten-Modell arbeitet …</p>
    {:else}
      <table class="vorschlag">
        <thead><tr><th>Element</th><th>BE</th></tr></thead>
        <tbody>
          {#each hilfe.elemente as e, i (i)}
            <tr><td><strong>{e.titel}</strong>{#if e.text}<div class="hinweis">{e.text}</div>{/if}</td><td>{String(e.max_be).replace('.', ',')}</td></tr>
          {/each}
        </tbody>
      </table>
      <p class="hinweis">Summe {String(hilfeSumme).replace('.', ',')} BE. Übernehmen ersetzt die bisherigen Elemente dieser Aufgabe.</p>
    {/if}
  {/if}
  {#snippet aktionen()}
    <button onclick={() => (hilfeOffen = false)}>Verwerfen</button>
    <button class="primaer" onclick={kiHilfeUebernehmen} disabled={!hilfe?.elemente}>Übernehmen</button>
  {/snippet}
</Dialog>
