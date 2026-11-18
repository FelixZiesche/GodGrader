<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { api, fehlertext } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import { dollar } from '../lib/format'
  import Dialog from '../components/Dialog.svelte'
  import type { Aufruf, KlausurFortschritt, PipelineEnde, PipelineMeldung, Schaetzung, Uebersicht } from '../lib/types'

  const projekt = $derived(store.projekt)
  const slots = $derived(store.settings?.slots)

  let ue = $state<Uebersicht | null>(null)
  let auswahl = $state<Record<string, boolean>>({})
  let laufAuswahl = $state<string[]>([])
  let fehler = $state<string | null>(null)
  let meldung = $state<string | null>(null)
  let hinweise = $state<string[]>([])
  let schaetzung = $state<Schaetzung | null>(null)
  let startOffen = $state(false)
  let schaetzt = $state(false)
  let stoppt = $state<'anhalten' | 'abbrechen' | null>(null)
  let protokoll = $state<Aufruf[] | null>(null)

  const unlisten: (() => void)[] = []

  const fertig = (k: KlausurFortschritt, zweit: boolean) =>
    k.abschrift === 'fertig' && k.bewertet >= k.aufgaben && (!zweit || k.zweit >= k.aufgaben)

  async function lade() {
    if (!projekt) return
    try {
      ue = await api.pipelineUebersicht(projekt.id)
      // Vorauswahl: alles, was noch nicht fertig und nicht freigegeben ist
      for (const k of ue.klausuren) {
        if (!(k.id in auswahl)) auswahl[k.id] = k.status !== 'freigegeben' && !fertig(k, ue.zweitmeinung)
      }
    } catch (e) {
      fehler = fehlertext(e)
    }
  }

  let ladeTimer: ReturnType<typeof setTimeout> | undefined
  function spaeterLaden() {
    clearTimeout(ladeTimer)
    ladeTimer = setTimeout(lade, 250)
  }

  onMount(async () => {
    await lade()
    try {
      unlisten.push(
        await listen<PipelineMeldung>('pipeline-ereignis', (ev) => {
          if (ev.payload.projekt_id !== projekt?.id) return
          const e = ev.payload.ereignis
          if (e.art === 'kosten' && ue) ue.kosten_gesamt = e.gesamt
          else if (e.art === 'hinweis') hinweise.push(e.text)
          else spaeterLaden()
        }),
      )
      unlisten.push(
        await listen<PipelineEnde>('pipeline-ende', (ev) => {
          if (ev.payload.projekt_id !== projekt?.id) return
          stoppt = null
          const r = ev.payload.ergebnis
          if (ev.payload.fehler) fehler = ev.payload.fehler
          else if (r) {
            const teil = r.fehler ? ` ${r.fehler} Schritte mit Fehler, siehe Tabelle.` : ''
            meldung = {
              fertig: `Verarbeitung abgeschlossen. Kosten dieses Laufs: ${dollar(r.kosten)}.${teil}`,
              angehalten: `Angehalten. Kosten bis hier: ${dollar(r.kosten)}. „Starten“ macht dort weiter.${teil}`,
              abgebrochen: `Abgebrochen. Kosten bis hier: ${dollar(r.kosten)}. „Starten“ macht dort weiter.${teil}`,
              budget: `Budget erreicht, Lauf angehalten. Kosten dieses Laufs: ${dollar(r.kosten)}.${teil}`,
            }[r.ende]
          }
          void lade()
          if (protokoll) void protokollLaden()
        }),
      )
    } catch {
      /* außerhalb von Tauri (Vorschau) */
    }
  })
  onDestroy(() => {
    unlisten.forEach((u) => u())
    clearTimeout(ladeTimer)
  })

  const gewaehlt = $derived(ue ? ue.klausuren.filter((k) => auswahl[k.id] && k.status !== 'freigegeben').map((k) => k.id) : [])
  const alleGewaehlt = $derived(!!ue && ue.klausuren.filter((k) => k.status !== 'freigegeben').every((k) => auswahl[k.id]))
  function alleUmschalten() {
    if (!ue) return
    const an = !alleGewaehlt
    for (const k of ue.klausuren) if (k.status !== 'freigegeben') auswahl[k.id] = an
  }

  const schritteJe = (k: KlausurFortschritt) => 1 + k.aufgaben * (ue?.zweitmeinung ? 2 : 1)
  const erledigtJe = (k: KlausurFortschritt) =>
    (k.abschrift === 'fertig' ? 1 : 0) + Math.min(k.bewertet, k.aufgaben) + (ue?.zweitmeinung ? Math.min(k.zweit, k.aufgaben) : 0)
  const fortschritt = $derived.by(() => {
    if (!ue) return { fertig: 0, gesamt: 0 }
    const ks = ue.klausuren.filter((k) => laufAuswahl.includes(k.id))
    return { fertig: ks.reduce((s, k) => s + erledigtJe(k), 0), gesamt: ks.reduce((s, k) => s + schritteJe(k), 0) }
  })
  const prozent = $derived(fortschritt.gesamt ? Math.round((fortschritt.fertig / fortschritt.gesamt) * 100) : 0)
  const budgetProzent = $derived(ue?.budget ? Math.min(100, (ue.kosten_gesamt / ue.budget) * 100) : 0)
  const ohneSchwaerzen = $derived(!!projekt && !projekt.schwaerzen)

  async function vorbereiten() {
    if (!projekt || !gewaehlt.length) return
    fehler = meldung = null
    schaetzt = true
    try {
      schaetzung = await api.kostenSchaetzen(projekt.id, gewaehlt)
      startOffen = true
    } catch (e) {
      fehler = fehlertext(e)
    } finally {
      schaetzt = false
    }
  }

  async function starten() {
    if (!projekt) return
    startOffen = false
    fehler = meldung = null
    hinweise = []
    try {
      laufAuswahl = [...gewaehlt]
      await api.pipelineStarten(projekt.id, laufAuswahl)
      if (ue) ue.laeuft = true
      spaeterLaden()
    } catch (e) {
      fehler = fehlertext(e)
    }
  }

  async function anhalten() {
    if (!projekt) return
    stoppt = 'anhalten'
    await api.pipelineAnhalten(projekt.id)
  }
  async function abbrechen() {
    if (!projekt) return
    stoppt = 'abbrechen'
    await api.pipelineAbbrechen(projekt.id)
  }

  async function protokollLaden() {
    if (!projekt) return
    protokoll = (await api.aufrufe(projekt.id, null)).reverse()
  }
  const namen = $derived(Object.fromEntries((ue?.klausuren ?? []).map((k) => [k.id, k.bezeichnung])))

  const STATUS: Record<string, string> = { offen: 'offen', vorkorrigiert: 'vorkorrigiert', in_pruefung: 'in Prüfung', freigegeben: 'freigegeben' }
  const restBudget = $derived(schaetzung?.budget != null ? schaetzung.budget - schaetzung.bisher : null)
</script>

<h1>Verarbeitung</h1>

{#if !projekt}
  <div class="karte grau"><p>Bitte zuerst unter „Projekte“ ein Projekt anlegen oder öffnen.</p></div>
{:else if ue}
  {#if fehler}<div class="warnung" role="alert">{fehler}</div>{/if}
  {#each hinweise as h (h)}<div class="warnung">{h}</div>{/each}
  {#if meldung}<div class="karte grau meldung">{meldung}</div>{/if}

  <div class="kopfzeile">
    <div class="karte kosten">
      <div class="zeile zwischen"><strong>Kosten im Projekt</strong><span class="betrag">{dollar(ue.kosten_gesamt)}</span></div>
      {#if ue.budget}
        <div class="balken" class:knapp={budgetProzent >= 80}><div style="width: {budgetProzent}%"></div></div>
        <span class="hinweis">Budget {dollar(ue.budget)}, noch {dollar(Math.max(0, ue.budget - ue.kosten_gesamt))}</span>
      {:else}
        <span class="hinweis">Kein Budget gesetzt. Einstellbar unter „Projekte“ → Bearbeiten.</span>
      {/if}
    </div>
    {#if slots}
      <div class="karte grau slots">
        <div><span class="hinweis">Abschreiben</span> {slots.abschreiben.model}</div>
        <div><span class="hinweis">Bewerten</span> {slots.bewerten.model}</div>
        <div><span class="hinweis">Zweitmeinung</span> {slots.zweitmeinung.enabled ? slots.zweitmeinung.model : 'aus'}</div>
      </div>
    {/if}
  </div>

  {#if !ue.hat_ewh}
    <div class="warnung">Es fehlt noch der Erwartungshorizont. Bitte zuerst unter „Erwartungshorizont“ importieren.</div>
  {:else if !ue.klausuren.length}
    <div class="karte grau"><p>Noch keine Klausuren importiert. Das geht unter „Scans“.</p></div>
  {:else}
    {#if ohneSchwaerzen}
      <p class="hinweis rot">Schwärzen ist für dieses Projekt ausgeschaltet. Die Seiten gehen so, wie sie sind, an das Abschreiben-Modell.</p>
    {/if}

    <div class="aktionen zeile">
      {#if ue.laeuft}
        <div class="lauf">
          <div class="balken"><div style="width: {prozent}%"></div></div>
          <span class="hinweis">{fortschritt.fertig} von {fortschritt.gesamt} Schritten · {prozent} %</span>
        </div>
        <button onclick={anhalten} disabled={!!stoppt}>{stoppt === 'anhalten' ? 'Hält an …' : 'Anhalten'}</button>
        <button onclick={abbrechen} disabled={stoppt === 'abbrechen'}>{stoppt === 'abbrechen' ? 'Bricht ab …' : 'Abbrechen'}</button>
      {:else}
        <button class="primaer" onclick={vorbereiten} disabled={!gewaehlt.length || schaetzt}>
          {schaetzt ? 'Schätzt …' : gewaehlt.length === 1 ? '1 Klausur verarbeiten' : `${gewaehlt.length} Klausuren verarbeiten`}
        </button>
        <span class="hinweis">Fertige Schritte werden übersprungen. Fehlgeschlagene laufen erneut.</span>
      {/if}
    </div>

    <table class="tabelle">
      <thead>
        <tr>
          <th class="w"><input type="checkbox" checked={alleGewaehlt} onchange={alleUmschalten} disabled={ue.laeuft} aria-label="alle auswählen" /></th>
          <th>Klausur</th>
          <th>Seiten</th>
          <th>Abschrift</th>
          <th>Bewertung</th>
          {#if ue.zweitmeinung}<th>Zweitmeinung</th>{/if}
          <th class="r">Kosten</th>
          <th>Status</th>
        </tr>
      </thead>
      <tbody>
        {#each ue.klausuren as k (k.id)}
          <tr class:aktiv={k.laeuft}>
            <td class="w"><input type="checkbox" bind:checked={auswahl[k.id]} disabled={ue.laeuft || k.status === 'freigegeben'} aria-label="{k.bezeichnung} auswählen" /></td>
            <td><strong>{k.bezeichnung}</strong></td>
            <td>{k.seiten}</td>
            <td>
              <span class="schritt {k.abschrift}">{{ offen: 'offen', laeuft: 'läuft …', fertig: 'fertig', fehler: 'Fehler' }[k.abschrift]}</span>
              {#if k.abschrift_hinweis}<span class="hinweis"> · {k.abschrift_hinweis}</span>{/if}
            </td>
            <td><span class="schritt" class:fertig={k.bewertet >= k.aufgaben}>{k.bewertet} / {k.aufgaben}</span></td>
            {#if ue.zweitmeinung}<td><span class="schritt" class:fertig={k.zweit >= k.aufgaben}>{k.zweit} / {k.aufgaben}</span></td>{/if}
            <td class="r">{dollar(k.kosten)}</td>
            <td>{STATUS[k.status]}</td>
          </tr>
          {#if k.fehler.length}
            <tr class="fehlerzeile">
              <td></td>
              <td colspan={ue.zweitmeinung ? 7 : 6}>{#each k.fehler as f (f)}<div>{f}</div>{/each}</td>
            </tr>
          {/if}
        {/each}
      </tbody>
    </table>

    <details class="protokoll" ontoggle={(e) => { if ((e.currentTarget as HTMLDetailsElement).open) void protokollLaden() }}>
      <summary>Kostenprotokoll (jeder Aufruf einzeln)</summary>
      {#if protokoll}
        {#if protokoll.length}
          <table class="tabelle klein">
            <thead><tr><th>Klausur</th><th>Schritt</th><th>Modell</th><th class="r">Tokens ein/aus</th><th class="r">Dauer</th><th class="r">Kosten</th></tr></thead>
            <tbody>
              {#each protokoll as a, i (i)}
                <tr class:fehlerzeile={!a.ok}>
                  <td>{a.klausur_id ? (namen[a.klausur_id] ?? 'gelöscht') : '–'}</td>
                  <td>{a.art}{a.aufgabe ? ` ${a.aufgabe}` : ''}{a.ok ? '' : ' (Fehler)'}</td>
                  <td>{a.modell}</td>
                  <td class="r">{a.tokens_ein.toLocaleString('de-DE')} / {a.tokens_aus.toLocaleString('de-DE')}</td>
                  <td class="r">{(a.dauer_ms / 1000).toLocaleString('de-DE', { maximumFractionDigits: 1 })} s</td>
                  <td class="r">{dollar(a.kosten)}{a.geschaetzt ? '*' : ''}</td>
                </tr>
              {/each}
            </tbody>
          </table>
          <p class="hinweis">* aus der Preisliste berechnet, weil OpenRouter keine Kosten gemeldet hat.</p>
        {:else}
          <p class="hinweis">Noch keine Aufrufe.</p>
        {/if}
      {/if}
    </details>
  {/if}
{/if}

<Dialog titel="Verarbeitung starten?" bind:offen={startOffen}>
  {#if schaetzung}
    {#if schaetzung.aufrufe === 0}
      <p>Für die gewählten Klausuren ist schon alles erledigt.</p>
    {:else}
      <p>
        {schaetzung.klausuren} {schaetzung.klausuren === 1 ? 'Klausur' : 'Klausuren'}, {schaetzung.aufrufe} Anfragen{schaetzung.seitenbilder ? `, ${schaetzung.seitenbilder} Seitenbilder zum Abschreiben` : ''}.
      </p>
      <p class="schaetzung">
        Geschätzte Kosten: <strong>etwa {dollar(schaetzung.kosten)}</strong>
        <span class="hinweis">(zwischen {dollar(schaetzung.kosten_min)} und {dollar(schaetzung.kosten_max)}{schaetzung.aus_erfahrung ? ', nach den bisherigen Aufrufen' : ', nach der Preisliste'})</span>
      </p>
      {#if schaetzung.ohne_preis.length}
        <p class="hinweis rot">Ohne Preisangabe in der Modellliste: {schaetzung.ohne_preis.join(', ')}. Die Schätzung ist daher zu niedrig.</p>
      {/if}
      {#if restBudget != null && schaetzung.kosten > restBudget}
        <p class="hinweis rot">Das restliche Budget ({dollar(Math.max(0, restBudget))}) reicht wahrscheinlich nicht. Der Lauf hält an, sobald es erreicht ist.</p>
      {/if}
      {#if ohneSchwaerzen}
        <p class="hinweis rot">Schwärzen ist aus. Achten Sie darauf, dass auf den Scans keine Namen stehen.</p>
      {/if}
    {/if}
  {/if}
  {#snippet aktionen()}
    <button onclick={() => (startOffen = false)}>Abbrechen</button>
    <button class="primaer" onclick={starten} disabled={!schaetzung?.aufrufe}>Starten</button>
  {/snippet}
</Dialog>

<style>
  .meldung { margin-bottom: 12px; }
  .kopfzeile { display: grid; grid-template-columns: minmax(260px, 360px) 1fr; gap: 12px; margin-bottom: 14px; }
  .kosten { display: flex; flex-direction: column; gap: 6px; }
  .zwischen { justify-content: space-between; }
  .betrag { font-size: 18px; font-weight: 700; }
  .slots { display: flex; flex-direction: column; gap: 3px; font-size: 13px; }
  .slots .hinweis { display: inline-block; width: 100px; }
  .balken { height: 8px; border: 1px solid var(--schwarz); border-radius: var(--radius); overflow: hidden; }
  .balken div { height: 100%; background: var(--schwarz); transition: width 0.3s; }
  .balken.knapp { border-color: var(--akzent); }
  .balken.knapp div { background: var(--akzent); }
  .aktionen { margin: 6px 0 12px; gap: 10px; }
  .lauf { flex: 1; display: flex; align-items: center; gap: 10px; }
  .lauf .balken { flex: 1; }
  .tabelle { width: 100%; border-collapse: collapse; }
  .tabelle th, .tabelle td { text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--grau-2); vertical-align: top; }
  .tabelle th { background: var(--grau-2); font-weight: 600; }
  .tabelle .r { text-align: right; white-space: nowrap; }
  .tabelle .w { width: 28px; }
  .tabelle tr.aktiv td { background: var(--grau-1); }
  .tabelle.klein { font-size: 12px; margin-top: 8px; }
  .schritt { white-space: nowrap; }
  .schritt.fertig { font-weight: 600; }
  .schritt.laeuft { font-style: italic; }
  .schritt.fehler { color: var(--akzent); font-weight: 600; }
  .fehlerzeile td { color: var(--akzent); font-size: 12px; border-bottom-color: var(--grau-2); }
  .protokoll { margin-top: 16px; }
  .protokoll summary { cursor: pointer; font-weight: 600; }
  .schaetzung { font-size: 15px; }
  .rot { color: var(--akzent); }
</style>
