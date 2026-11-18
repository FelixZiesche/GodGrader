<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import { api, fehlertext } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import { dollar } from '../lib/format'
  import Dialog from '../components/Dialog.svelte'
  import type { ExportBericht, ExportOptionen, FeedbackEintrag } from '../lib/types'

  const projekt = $derived(store.projekt)
  const feedbackModell = $derived(store.settings?.slots.feedback.model ?? '')

  let liste = $state<FeedbackEintrag[]>([])
  let texte = $state<Record<string, string>>({})
  let gespeichert = $state<Record<string, boolean>>({})
  let fehler = $state<string | null>(null)
  let meldung = $state<string | null>(null)
  let erzeugt = $state<string[] | null>(null)
  let ersetzenOffen = $state(false)
  let ersetzenIds = $state<string[]>([])

  const merkKey = $derived(`godgrader.exportordner.${projekt?.id ?? ''}`)
  let ordner = $state('')
  let optionen = $state<ExportOptionen>({ notenliste: true, korrekturboegen: true, sammeldatei: true, feedback: false, statistik: true })
  let exportiert = $state(false)
  let bericht = $state<ExportBericht | null>(null)

  async function lade() {
    if (!projekt) return
    try {
      liste = await api.feedbackListe(projekt.id)
      for (const e of liste) texte[e.klausur_id] = e.feedback?.text ?? ''
    } catch (e) {
      fehler = fehlertext(e)
    }
  }

  onMount(async () => {
    try {
      ordner = localStorage.getItem(merkKey) ?? ''
    } catch {
      /* ohne Speicher */
    }
    await lade()
    if (liste.some((e) => e.feedback)) optionen.feedback = true
  })

  const freigegeben = $derived(liste.filter((e) => e.status === 'freigegeben'))
  const fehlend = $derived(liste.filter((e) => e.status !== 'freigegeben'))
  const ohneFeedback = $derived(freigegeben.filter((e) => !e.feedback))
  const bearbeitet = (e: FeedbackEintrag) => !!e.feedback && e.feedback.ki_text !== '' && e.feedback.text !== e.feedback.ki_text

  async function erzeugen(ids: string[]) {
    if (!projekt || !ids.length) return
    ersetzenOffen = false
    fehler = meldung = null
    erzeugt = ids
    try {
      const r = await api.feedbackErzeugen(projekt.id, ids)
      const teile = [`${r.erzeugt} ${r.erzeugt === 1 ? 'Rückmeldung' : 'Rückmeldungen'} erzeugt, Kosten ${dollar(r.kosten)}.`]
      if (r.budget_erreicht) teile.push('Das Budget ist erreicht, der Rest wurde nicht erzeugt.')
      meldung = teile.join(' ')
      if (r.fehler.length) fehler = r.fehler.join(' · ')
      await lade()
    } catch (e) {
      fehler = fehlertext(e)
    } finally {
      erzeugt = null
    }
  }

  /** Neu erzeugen überschreibt Text. Bei vorhandenem Text erst fragen. */
  function neuErzeugen(ids: string[]) {
    if (ids.some((id) => liste.find((e) => e.klausur_id === id)?.feedback)) {
      ersetzenIds = ids
      ersetzenOffen = true
    } else void erzeugen(ids)
  }

  async function speichern(e: FeedbackEintrag) {
    if (!projekt) return
    const text = texte[e.klausur_id] ?? ''
    if (text === (e.feedback?.text ?? '')) return
    try {
      await api.feedbackSpeichern(projekt.id, e.klausur_id, text)
      gespeichert[e.klausur_id] = true
      setTimeout(() => (gespeichert[e.klausur_id] = false), 1500)
      await lade()
    } catch (err) {
      fehler = fehlertext(err)
    }
  }

  async function ordnerWaehlen() {
    const p = await open({ directory: true, multiple: false, defaultPath: ordner || undefined })
    if (typeof p === 'string') {
      ordner = p
      try {
        localStorage.setItem(merkKey, p)
      } catch {
        /* ohne Speicher */
      }
    }
  }

  const irgendwas = $derived(Object.values(optionen).some(Boolean))

  async function exportieren() {
    if (!projekt || !ordner) return
    fehler = meldung = null
    bericht = null
    exportiert = true
    try {
      bericht = await api.exportieren(projekt.id, ordner, $state.snapshot(optionen))
    } catch (e) {
      fehler = fehlertext(e)
    } finally {
      exportiert = false
    }
  }

  async function ordnerOeffnen() {
    if (!bericht) return
    try {
      await api.ordnerOeffnen(bericht.ordner)
    } catch (e) {
      fehler = fehlertext(e)
    }
  }

  const dateiname = (pfad: string) => pfad.split(/[\\/]/).pop() ?? pfad
</script>

<h1>Export</h1>

{#if !projekt}
  <div class="karte grau"><p>Bitte zuerst unter „Projekte“ ein Projekt anlegen oder öffnen.</p></div>
{:else}
  {#if fehler}<div class="warnung" role="alert">{fehler}</div>{/if}
  {#if meldung}<div class="karte grau meldung">{meldung}</div>{/if}

  {#if fehlend.length}
    <div class="warnung">
      {fehlend.length === 1 ? '1 Klausur ist' : `${fehlend.length} Klausuren sind`} noch nicht freigegeben und
      {fehlend.length === 1 ? 'fehlt' : 'fehlen'} in allen Exporten: {fehlend.map((e) => e.bezeichnung).join(', ')}.
    </div>
  {/if}

  <section class="karte">
    <h2>Dateien exportieren</h2>
    <div class="optionen">
      <label><input type="checkbox" bind:checked={optionen.notenliste} /> Notenliste (Excel und CSV)</label>
      <label><input type="checkbox" bind:checked={optionen.korrekturboegen} /> Korrekturbogen je Klausur (Word)</label>
      <label><input type="checkbox" bind:checked={optionen.sammeldatei} /> Alle Korrekturbögen in einer Datei (Word)</label>
      <label><input type="checkbox" bind:checked={optionen.feedback} /> Rückmeldung je Klausur (Word)</label>
      <label><input type="checkbox" bind:checked={optionen.statistik} /> Statistik (Word)</label>
    </div>
    {#if optionen.feedback && ohneFeedback.length}
      <p class="hinweis">Ohne Rückmeldung und daher ohne Feedback-Datei: {ohneFeedback.map((e) => e.bezeichnung).join(', ')}. Erzeugen geht weiter unten.</p>
    {/if}

    <div class="zeile ordner">
      <input type="text" readonly value={ordner} placeholder="Noch kein Ordner gewählt" aria-label="Exportordner" />
      <button onclick={ordnerWaehlen}>Ordner wählen …</button>
    </div>

    <div class="zeile">
      <button class="primaer" onclick={exportieren} disabled={!ordner || !irgendwas || !freigegeben.length || exportiert}>
        {exportiert ? 'Exportiert …' : 'Exportieren'}
      </button>
      <span class="hinweis">Exportiert werden nur freigegebene Klausuren. Vorhandene Dateien gleichen Namens werden ersetzt.</span>
    </div>

    {#if bericht}
      <div class="karte grau bericht">
        <div class="zeile zwischen">
          <strong>{bericht.dateien.length} {bericht.dateien.length === 1 ? 'Datei' : 'Dateien'} gespeichert</strong>
          <button class="klein" onclick={ordnerOeffnen}>Ordner öffnen</button>
        </div>
        <ul>
          {#each bericht.dateien as d (d)}<li>{dateiname(d)}</li>{/each}
        </ul>
        {#if bericht.fehlend.length}<p class="hinweis">Nicht enthalten, weil nicht freigegeben: {bericht.fehlend.join(', ')}.</p>{/if}
        {#if bericht.ohne_feedback.length}<p class="hinweis">Ohne Rückmeldung: {bericht.ohne_feedback.join(', ')}.</p>{/if}
      </div>
    {/if}
  </section>

  <section class="karte">
    <div class="zeile zwischen">
      <h2>Rückmeldungen</h2>
      <div class="zeile">
        <button class="primaer" onclick={() => erzeugen(ohneFeedback.map((e) => e.klausur_id))} disabled={!ohneFeedback.length || !!erzeugt || !feedbackModell}>
          {erzeugt ? 'Erzeugt …' : ohneFeedback.length ? `Fehlende erzeugen (${ohneFeedback.length})` : 'Alle haben eine Rückmeldung'}
        </button>
        {#if freigegeben.length && !ohneFeedback.length}
          <button onclick={() => neuErzeugen(freigegeben.map((e) => e.klausur_id))} disabled={!!erzeugt || !feedbackModell}>Alle neu erzeugen</button>
        {/if}
      </div>
    </div>
    <p class="hinweis">
      Optional. Die KI schreibt aus den finalen BE, den Begründungen und der Abschrift je 2 bis 3 Stärken und Entwicklungsfelder
      und einen Tipp. Nur für freigegebene Klausuren. Name oder Pseudonym gehen nicht mit. Der Text ist frei bearbeitbar und
      wird beim Verlassen des Feldes gespeichert.
      {#if !feedbackModell}<strong>Slot „Feedback“ hat noch kein Modell (siehe „Modelle“).</strong>{/if}
    </p>

    {#if !freigegeben.length}
      <p class="hinweis">Noch keine freigegebene Klausur.</p>
    {:else}
      <div class="feedbacks">
        {#each freigegeben as e (e.klausur_id)}
          <div class="fb">
            <div class="zeile zwischen">
              <strong>{e.bezeichnung}</strong>
              <span class="zeile">
                {#if gespeichert[e.klausur_id]}<span class="hinweis">gespeichert</span>{/if}
                {#if bearbeitet(e)}<span class="hinweis">bearbeitet</span>{/if}
                {#if erzeugt?.includes(e.klausur_id)}<span class="hinweis">wird erzeugt …</span>{/if}
                <button class="klein" onclick={() => neuErzeugen([e.klausur_id])} disabled={!!erzeugt || !feedbackModell}>
                  {e.feedback ? 'Neu erzeugen' : 'Erzeugen'}
                </button>
              </span>
            </div>
            <textarea
              rows={e.feedback || texte[e.klausur_id] ? 7 : 2}
              bind:value={texte[e.klausur_id]}
              onblur={() => speichern(e)}
              placeholder="Noch keine Rückmeldung. Sie können sie erzeugen lassen oder selbst schreiben."
              aria-label="Rückmeldung für {e.bezeichnung}"
            ></textarea>
          </div>
        {/each}
      </div>
    {/if}
  </section>
{/if}

<Dialog titel="Rückmeldungen ersetzen?" bind:offen={ersetzenOffen}>
  <p>Vorhandene Rückmeldungen werden durch neue ersetzt. Eigene Änderungen daran gehen verloren.</p>
  {#snippet aktionen()}
    <button onclick={() => (ersetzenOffen = false)}>Abbrechen</button>
    <button class="primaer" onclick={() => erzeugen(ersetzenIds)}>Neu erzeugen</button>
  {/snippet}
</Dialog>

<style>
  section { margin-bottom: 14px; }
  .warnung { margin-bottom: 12px; }
  h2 { font-size: 15px; margin: 0; }
  .zwischen { justify-content: space-between; }
  .meldung { margin-bottom: 12px; }
  .feedbacks { display: flex; flex-direction: column; gap: 12px; margin-top: 10px; }
  .fb { display: flex; flex-direction: column; gap: 6px; }
  .fb textarea { width: 100%; font-family: inherit; resize: vertical; line-height: 1.45; }
  button.klein { padding: 3px 9px; font-size: 13px; }
  .optionen { display: flex; flex-direction: column; gap: 6px; margin: 10px 0; }
  .ordner { margin: 10px 0; }
  .ordner input { flex: 1; min-width: 240px; }
  .bericht { margin-top: 12px; }
  .bericht ul { margin: 8px 0; padding-left: 20px; font-size: 13px; }
</style>
