<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebview } from '@tauri-apps/api/webview'
  import { api, fehlertext } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import Dialog from '../components/Dialog.svelte'
  import Schwaerzen from '../components/Schwaerzen.svelte'
  import type { Fortschritt, ImportAuftrag, KlausurInfo, Schwaerzung } from '../lib/types'

  type Datei = { pfad: string; name: string; seiten: number; bezeichnung: string }
  type Stapel = { pfad: string; name: string; seiten: number }

  const STATUS: Record<string, string> = {
    offen: 'offen',
    vorkorrigiert: 'vorkorrigiert',
    in_pruefung: 'in Prüfung',
    freigegeben: 'freigegeben',
  }

  const projekt = $derived(store.projekt)

  let klausuren = $state<KlausurInfo[]>([])
  let miniaturen = $state<Record<string, string>>({})
  let pseudonyme = $state<string[]>([])
  let schwaerzung = $state<Schwaerzung[]>([])
  let schwaerzungGeaendert = $state(false)
  let beispiel = $state<string | null>(null)

  let modus = $state<'einzeln' | 'stapel'>('einzeln')
  let dateien = $state<Datei[]>([])
  let stapel = $state<Stapel | null>(null)
  let stapelBilder = $state<string[]>([])
  let proKlausur = $state(4)
  let trenn = $state<boolean[]>([])
  let stapelNamen = $state<string[]>([])
  let teilen = $state(false)

  let fortschritt = $state<Fortschritt | null>(null)
  let arbeitet = $state(false)
  let fehler = $state<string | null>(null)
  let meldung = $state<string | null>(null)
  let ziehen = $state(false)
  let loeschen = $state<KlausurInfo | null>(null)
  let loeschenOffen = $state(false)

  const unlisten: (() => void)[] = []

  // ---------- Laden ----------

  async function lade() {
    if (!projekt) return
    const pid = projekt.id
    const [ks, ps, sw] = await Promise.all([api.klausuren(pid), api.getPseudonyme(pid), api.getSchwaerzung(pid)])
    klausuren = ks
    pseudonyme = ps
    schwaerzung = sw
    schwaerzungGeaendert = false
    void ladeMiniaturen()
    await ladeBeispiel()
  }

  async function ladeMiniaturen() {
    if (!projekt) return
    const pid = projekt.id
    for (const k of klausuren) {
      const s = k.seiten[0]
      if (!s || miniaturen[s.id]) continue
      try {
        miniaturen[s.id] = await api.seitenMiniatur(pid, s.id, 110)
      } catch {
        /* Bild fehlt: nur ohne Vorschau anzeigen */
      }
    }
  }

  /** Vorlage für das Schwärzen: ausgewähltes PDF, sonst erste importierte Klausur. */
  async function ladeBeispiel() {
    if (!projekt?.schwaerzen) return
    try {
      const quelle = modus === 'stapel' ? stapel?.pfad : dateien[0]?.pfad
      beispiel = quelle ? await api.pdfSeitenvorschau(quelle, 0, 900) : await api.beispielseite(projekt.id, 900)
    } catch (e) {
      beispiel = null
      fehler = fehlertext(e)
    }
  }

  onMount(async () => {
    try {
      await lade()
    } catch (e) {
      fehler = fehlertext(e)
    }
    try {
      unlisten.push(await listen<Fortschritt>('scan-fortschritt', (ev) => (fortschritt = ev.payload)))
      unlisten.push(
        await getCurrentWebview().onDragDropEvent((ev) => {
          const t = ev.payload.type
          if (t === 'over' || t === 'enter') ziehen = true
          else if (t === 'leave') ziehen = false
          else if (t === 'drop') {
            ziehen = false
            void abgelegt(ev.payload.paths)
          }
        }),
      )
    } catch {
      /* außerhalb von Tauri (Vorschau) */
    }
  })
  onDestroy(() => unlisten.forEach((u) => u()))

  async function abgelegt(pfade: string[]) {
    const liste = pfade.find((p) => /\.(csv|xlsx|xls|ods|txt)$/i.test(p))
    if (liste) await listeImportieren(liste)
    const pdfs = pfade.filter((p) => /\.pdf$/i.test(p))
    if (!pdfs.length) return
    if (modus === 'stapel') await stapelSetzen(pdfs[0])
    else await dateienHinzu(pdfs)
  }

  // ---------- Pseudonymliste (optional) ----------

  async function listeWaehlen() {
    const p = await open({ multiple: false, filters: [{ name: 'Pseudonymliste', extensions: ['csv', 'xlsx', 'xls', 'ods', 'txt'] }] })
    if (typeof p === 'string') await listeImportieren(p)
  }
  async function listeImportieren(pfad: string) {
    if (!projekt) return
    fehler = null
    try {
      pseudonyme = await api.pseudonymeImportieren(projekt.id, pfad)
      meldung = `${pseudonyme.length} Pseudonyme übernommen.`
      if (dateien.length) await vorschlaegeNeu()
    } catch (e) {
      fehler = fehlertext(e)
    }
  }
  async function listeLeeren() {
    if (!projekt) return
    await api.setPseudonyme(projekt.id, [])
    pseudonyme = []
  }

  const vergeben = $derived(new Set(klausuren.map((k) => k.bezeichnung.trim().toLowerCase())))
  const frei = $derived(pseudonyme.filter((p) => !vergeben.has(p.trim().toLowerCase())))

  // ---------- Schwärzen ----------

  let swTimer: ReturnType<typeof setTimeout> | undefined
  function swGeaendert() {
    if (!projekt) return
    const pid = projekt.id
    clearTimeout(swTimer)
    swTimer = setTimeout(async () => {
      try {
        await api.setSchwaerzung(pid, $state.snapshot(schwaerzung) as Schwaerzung[])
        if (klausuren.length) schwaerzungGeaendert = true
      } catch (e) {
        fehler = fehlertext(e)
      }
    }, 400)
  }

  async function neuErzeugen() {
    if (!projekt) return
    arbeitet = true
    fehler = null
    try {
      await api.seitenNeuErzeugen(projekt.id)
      miniaturen = {}
      schwaerzungGeaendert = false
      meldung = 'Alle Seitenbilder wurden neu erzeugt.'
      void ladeMiniaturen()
    } catch (e) {
      fehler = fehlertext(e)
    } finally {
      arbeitet = false
      fortschritt = null
    }
  }

  // ---------- Ein PDF pro Schüler ----------

  async function dateienWaehlen() {
    const p = await open({ multiple: true, filters: [{ name: 'PDF', extensions: ['pdf'] }] })
    if (Array.isArray(p)) await dateienHinzu(p)
    else if (typeof p === 'string') await dateienHinzu([p])
  }

  async function dateienHinzu(pfade: string[]) {
    if (!projekt) return
    fehler = null
    const neu: Datei[] = []
    for (const pfad of pfade) {
      if (dateien.some((d) => d.pfad === pfad)) continue
      try {
        const info = await api.pdfInfo(pfad)
        neu.push({ pfad, name: info.name, seiten: info.seiten, bezeichnung: '' })
      } catch (e) {
        fehler = `${pfad}: ${fehlertext(e)}`
      }
    }
    const war = dateien.length
    dateien.push(...neu)
    await vorschlaegeNeu()
    if (!war && dateien.length) await ladeBeispiel()
  }

  async function vorschlaegeNeu() {
    if (!projekt || !dateien.length) return
    const v = await api.bezeichnungVorschlaege(projekt.id, dateien.map((d) => d.name))
    dateien.forEach((d, i) => {
      if (!d.bezeichnung.trim()) d.bezeichnung = v[i] ?? ''
    })
  }

  // ---------- Stapel-PDF ----------

  async function stapelWaehlen() {
    const p = await open({ multiple: false, filters: [{ name: 'PDF', extensions: ['pdf'] }] })
    if (typeof p === 'string') await stapelSetzen(p)
  }

  async function stapelSetzen(pfad: string) {
    fehler = null
    arbeitet = true
    try {
      const info = await api.pdfInfo(pfad)
      stapel = { pfad, name: info.name, seiten: info.seiten }
      stapelBilder = []
      trennNachZahl()
      await ladeBeispiel()
      stapelBilder = await api.pdfMiniaturen(pfad, 120)
    } catch (e) {
      fehler = fehlertext(e)
      stapel = null
    } finally {
      arbeitet = false
    }
  }

  function trennNachZahl() {
    if (!stapel) return
    const n = Math.max(1, Math.floor(proKlausur) || 1)
    trenn = Array.from({ length: stapel.seiten }, (_, i) => i % n === 0)
    namenAnpassen()
  }

  function trennUmschalten(i: number) {
    if (i <= 0) return
    trenn[i] = !trenn[i]
    namenAnpassen()
  }

  /** Seitenbereiche der Klausuren im Stapel (0-basiert). */
  const gruppen = $derived.by(() => {
    const g: number[][] = []
    trenn.forEach((t, i) => {
      if (t || !g.length) g.push([i])
      else g[g.length - 1].push(i)
    })
    return g
  })

  function namenAnpassen() {
    const anzahl = trenn.filter((t, i) => t || i === 0).length
    const n = [...stapelNamen]
    while (n.length < anzahl) n.push(`Klausur ${klausuren.length + n.length + 1}`)
    stapelNamen = n.slice(0, anzahl)
  }

  const ungleich = $derived(gruppen.length > 1 && new Set(gruppen.map((g) => g.length)).size > 1)

  // ---------- Import ----------

  const auftraege = $derived.by((): ImportAuftrag[] => {
    if (modus === 'einzeln') {
      return dateien.map((d) => ({ bezeichnung: d.bezeichnung, pdf: d.pfad, seiten: Array.from({ length: d.seiten }, (_, i) => i) }))
    }
    if (!stapel) return []
    const pfad = stapel.pfad
    return gruppen.map((g, i) => ({ bezeichnung: stapelNamen[i] ?? '', pdf: pfad, seiten: g }))
  })

  const doppelt = $derived.by(() => {
    const gesehen = new Set(vergeben)
    const d = new Set<string>()
    for (const a of auftraege) {
      const k = a.bezeichnung.trim().toLowerCase()
      if (!k) continue
      if (gesehen.has(k)) d.add(a.bezeichnung.trim())
      gesehen.add(k)
    }
    return [...d]
  })
  const leer = $derived(auftraege.some((a) => !a.bezeichnung.trim()))

  async function importieren() {
    if (!projekt || !auftraege.length) return
    arbeitet = true
    fehler = null
    meldung = null
    fortschritt = { fertig: 0, gesamt: 1, text: 'Starte …' }
    try {
      const n = auftraege.length
      klausuren = await api.scansImportieren(projekt.id, $state.snapshot(auftraege) as ImportAuftrag[], teilen)
      meldung = n === 1 ? '1 Klausur importiert.' : `${n} Klausuren importiert.`
      dateien = []
      stapel = null
      stapelBilder = []
      trenn = []
      stapelNamen = []
      schwaerzungGeaendert = false
      void ladeMiniaturen()
      await ladeBeispiel()
    } catch (e) {
      fehler = fehlertext(e)
    } finally {
      arbeitet = false
      fortschritt = null
    }
  }

  // ---------- Klausurliste ----------

  async function umbenennen(k: KlausurInfo, wert: string) {
    if (!projekt) return
    const v = wert.trim()
    if (!v || v === k.bezeichnung) return
    try {
      await api.klausurUmbenennen(projekt.id, k.id, v)
      k.bezeichnung = v
    } catch (e) {
      fehler = fehlertext(e)
    }
  }

  async function loeschenBestaetigt() {
    if (!projekt || !loeschen) return
    const id = loeschen.id
    loeschenOffen = false
    try {
      await api.klausurLoeschen(projekt.id, id)
      klausuren = klausuren.filter((k) => k.id !== id)
      if (!klausuren.length) await ladeBeispiel()
    } catch (e) {
      fehler = fehlertext(e)
    }
  }

  const seitenGesamt = $derived(klausuren.reduce((s, k) => s + k.seiten.length, 0))
  const prozent = $derived(fortschritt && fortschritt.gesamt ? Math.round((fortschritt.fertig / fortschritt.gesamt) * 100) : 0)
</script>

<datalist id="pseudonymliste">
  {#each frei as p (p)}<option value={p}></option>{/each}
</datalist>

<h1>Scans</h1>

{#if !projekt}
  <div class="karte grau"><p>Bitte zuerst unter „Projekte“ ein Projekt anlegen oder öffnen.</p></div>
{:else}
  {#if fehler}<div class="warnung" role="alert">{fehler}</div>{/if}
  {#if meldung}<div class="karte grau meldung">{meldung} <button class="leise" onclick={() => (meldung = null)}>ausblenden</button></div>{/if}

  <!-- 1. Pseudonymliste -->
  <section class="karte">
    <div class="kopf">
      <h2>Pseudonymliste <span class="optional">optional</span></h2>
      <div class="zeile">
        {#if pseudonyme.length}<button class="leise" onclick={listeLeeren}>Liste entfernen</button>{/if}
        <button onclick={listeWaehlen}>{pseudonyme.length ? 'Andere Liste' : 'CSV/XLSX importieren'}</button>
      </div>
    </div>
    {#if pseudonyme.length}
      <p class="hinweis">{pseudonyme.length} Pseudonyme, davon {frei.length} noch frei. Sie werden beim Import vorgeschlagen.</p>
      <div class="chips">
        {#each pseudonyme as p (p)}<span class="chip" class:vergeben={vergeben.has(p.trim().toLowerCase())}>{p}</span>{/each}
      </div>
    {:else}
      <p class="hinweis">Ohne Liste geben Sie jeder Klausur einfach eine eigene Bezeichnung. Bezeichnungen werden nie an die KI geschickt.</p>
    {/if}
  </section>

  <!-- 2. Schwärzen -->
  <section class="karte">
    <div class="kopf">
      <h2>Schwärzen</h2>
      {#if projekt.schwaerzen && schwaerzungGeaendert && klausuren.length}
        <button class="primaer" onclick={neuErzeugen} disabled={arbeitet}>Seiten neu erzeugen</button>
      {/if}
    </div>
    {#if !projekt.schwaerzen}
      <p class="hinweis">
        Für dieses Projekt ist das Schwärzen ausgeschaltet. Achten Sie darauf, dass auf den Scans keine Namen stehen,
        denn die Seiten gehen so an die KI. Einschalten lässt es sich unter „Projekte“ → Bearbeiten.
      </p>
    {:else if beispiel}
      {#if schwaerzungGeaendert}
        <p class="hinweis">Die Bereiche haben sich geändert. „Seiten neu erzeugen“ wendet sie auf alle importierten Klausuren an.</p>
      {/if}
      <Schwaerzen bild={beispiel} bind:bereiche={schwaerzung} geaendert={swGeaendert} />
    {:else}
      <p class="hinweis">Wählen Sie unten ein PDF aus. Dann erscheint hier die erste Seite, und Sie ziehen Rechtecke über die Namenszeile.</p>
    {/if}
  </section>

  <!-- 3. Import -->
  <section class="karte" class:ziehen>
    <div class="kopf">
      <h2>Klausuren importieren</h2>
      <div class="umschalter" role="tablist">
        <button role="tab" aria-selected={modus === 'einzeln'} class:aktiv={modus === 'einzeln'} onclick={() => { modus = 'einzeln'; void ladeBeispiel() }}>Ein PDF pro Schüler</button>
        <button role="tab" aria-selected={modus === 'stapel'} class:aktiv={modus === 'stapel'} onclick={() => { modus = 'stapel'; void ladeBeispiel() }}>Stapel-PDF</button>
      </div>
    </div>

    {#if modus === 'einzeln'}
      <div class="ablage">
        <p>PDFs hierher ziehen oder <button onclick={dateienWaehlen} disabled={arbeitet}>PDFs wählen</button></p>
        <p class="hinweis">Die Bezeichnung wird aus dem Dateinamen vorgeschlagen{pseudonyme.length ? ' und mit der Pseudonymliste abgeglichen' : ''}.</p>
      </div>
      {#if dateien.length}
        <table class="tabelle">
          <thead><tr><th>Datei</th><th>Seiten</th><th>Bezeichnung</th><th></th></tr></thead>
          <tbody>
            {#each dateien as d, i (d.pfad)}
              <tr>
                <td class="datei" title={d.pfad}>{d.name}</td>
                <td>{d.seiten}</td>
                <td><input bind:value={d.bezeichnung} list="pseudonymliste" placeholder="z. B. Edler Kranich" aria-label="Bezeichnung für {d.name}" /></td>
                <td><button class="leise" onclick={() => dateien.splice(i, 1)}>entfernen</button></td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {:else}
      <div class="ablage">
        <p>Stapel-PDF hierher ziehen oder <button onclick={stapelWaehlen} disabled={arbeitet}>PDF wählen</button></p>
        {#if stapel}
          <div class="zeile">
            <strong>{stapel.name}</strong><span class="hinweis">{stapel.seiten} Seiten</span>
            <label class="zeile">Seiten pro Klausur
              <input type="number" min="1" max={stapel.seiten} bind:value={proKlausur} onchange={trennNachZahl} style="width: 70px" />
            </label>
            <span class="hinweis">{gruppen.length} Klausuren</span>
          </div>
        {/if}
      </div>
      {#if stapel}
        <p class="hinweis">Klick zwischen zwei Seiten setzt oder entfernt eine Trennstelle, etwa wenn jemand eine Seite mehr hat.</p>
        {#if ungleich}<p class="hinweis rot">Die Klausuren haben unterschiedlich viele Seiten. Bitte kurz prüfen.</p>{/if}
        <div class="stapel">
          {#each gruppen as g, gi (g[0])}
            <div class="gruppe">
              <div class="gruppenkopf">
                <span class="nr">{gi + 1}</span>
                <input bind:value={stapelNamen[gi]} list="pseudonymliste" aria-label="Bezeichnung Klausur {gi + 1}" />
                <span class="hinweis">S. {g[0] + 1}{g.length > 1 ? `–${g[g.length - 1] + 1}` : ''}</span>
              </div>
              <div class="seitenreihe">
                {#each g as s, si (s)}
                  {#if si > 0}
                    <button class="fuge" title="Hier trennen" aria-label="Vor Seite {s + 1} trennen" onclick={() => trennUmschalten(s)}></button>
                  {/if}
                  <figure class="mini">
                    {#if stapelBilder[s]}<img src={stapelBilder[s]} alt="Seite {s + 1}" />{:else}<div class="platz"></div>{/if}
                    <figcaption>{s + 1}</figcaption>
                  </figure>
                {/each}
              </div>
            </div>
            {#if gi < gruppen.length - 1}
              <button class="trennlinie" title="Trennstelle entfernen" aria-label="Trennstelle vor Seite {gruppen[gi + 1][0] + 1} entfernen" onclick={() => trennUmschalten(gruppen[gi + 1][0])}>
                <span>Trennstelle entfernen</span>
              </button>
            {/if}
          {/each}
        </div>
      {/if}
    {/if}

    {#if auftraege.length}
      <div class="fuss">
        <label class="zeile check">
          <input type="checkbox" bind:checked={teilen} />
          Jede Seite in obere und untere Hälfte teilen (für sehr dicht beschriebene Seiten)
        </label>
        {#if doppelt.length}<p class="hinweis rot">Doppelte Bezeichnung: {doppelt.join(', ')}</p>{/if}
        {#if projekt.schwaerzen && !schwaerzung.length}<p class="hinweis rot">Noch kein Bereich zum Schwärzen festgelegt.</p>{/if}
        <div class="zeile">
          <button class="primaer" onclick={importieren} disabled={arbeitet || leer}>
            {auftraege.length === 1 ? '1 Klausur importieren' : `${auftraege.length} Klausuren importieren`}
          </button>
          {#if leer}<span class="hinweis">Jede Klausur braucht eine Bezeichnung.</span>{/if}
        </div>
      </div>
    {/if}
  </section>

  {#if fortschritt}
    <div class="fortschritt" role="status">
      <div class="balken"><div style="width: {prozent}%"></div></div>
      <span class="hinweis">{fortschritt.text} ({prozent} %)</span>
    </div>
  {/if}

  <!-- 4. Importierte Klausuren -->
  <section>
    <h2>Importierte Klausuren <span class="hinweis">{klausuren.length} Klausuren, {seitenGesamt} Seitenbilder</span></h2>
    {#if klausuren.length}
      <div class="klausuren">
        {#each klausuren as k (k.id)}
          <div class="klausur">
            {#if k.seiten[0] && miniaturen[k.seiten[0].id]}
              <img src={miniaturen[k.seiten[0].id]} alt="erste Seite" />
            {:else}
              <div class="platz"></div>
            {/if}
            <div class="infos">
              <input value={k.bezeichnung} list="pseudonymliste" onchange={(e) => umbenennen(k, e.currentTarget.value)} aria-label="Bezeichnung" />
              <span class="hinweis">{k.seiten.length} Seitenbilder · {STATUS[k.status]}</span>
              <span class="hinweis datei" title={k.quelle}>{k.quelle}</span>
              <button class="leise" onclick={() => { loeschen = k; loeschenOffen = true }}>löschen</button>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <p class="hinweis">Noch keine Klausuren importiert.</p>
    {/if}
  </section>
{/if}

<Dialog titel="Klausur löschen?" bind:offen={loeschenOffen}>
  <p>„{loeschen?.bezeichnung}“ wird mit allen Seitenbildern aus dem Projekt entfernt.</p>
  {#snippet aktionen()}
    <button onclick={() => (loeschenOffen = false)}>Abbrechen</button>
    <button class="primaer" onclick={loeschenBestaetigt}>Löschen</button>
  {/snippet}
</Dialog>

<style>
  section { margin-bottom: 18px; }
  .kopf { display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-bottom: 8px; flex-wrap: wrap; }
  .kopf h2 { margin: 0; }
  .optional { font-weight: 400; font-size: 12px; color: var(--grau-4); border: 1px solid var(--grau-3); border-radius: 10px; padding: 1px 7px; margin-left: 6px; }
  .meldung { margin-bottom: 12px; display: flex; justify-content: space-between; align-items: center; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; max-height: 96px; overflow: auto; }
  .chip { font-size: 12px; border: 1px solid var(--grau-3); border-radius: 10px; padding: 1px 8px; }
  .chip.vergeben { background: var(--grau-2); color: var(--grau-4); text-decoration: line-through; }
  .umschalter { display: inline-flex; }
  .umschalter button { border-radius: 0; }
  .umschalter button:first-child { border-radius: var(--radius) 0 0 var(--radius); }
  .umschalter button:last-child { border-radius: 0 var(--radius) var(--radius) 0; border-left: none; }
  .umschalter button.aktiv { background: var(--schwarz); color: var(--weiss); }
  .ablage { border: 1px dashed var(--grau-3); border-radius: var(--radius); padding: 12px 14px; margin-bottom: 10px; }
  .ablage p { margin: 0 0 4px; }
  section.ziehen .ablage { border-color: var(--schwarz); background: var(--grau-1); }
  .tabelle { width: 100%; border-collapse: collapse; }
  .tabelle th, .tabelle td { text-align: left; padding: 4px 6px; border-bottom: 1px solid var(--grau-2); }
  .tabelle th { font-weight: 600; background: var(--grau-2); }
  .tabelle input { width: 100%; }
  .datei { max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .stapel { display: flex; flex-direction: column; gap: 0; }
  .gruppe { padding: 8px 0; }
  .gruppenkopf { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; }
  .gruppenkopf input { width: 220px; }
  .nr { font-weight: 700; min-width: 22px; }
  .seitenreihe { display: flex; align-items: stretch; flex-wrap: wrap; row-gap: 6px; }
  .mini { margin: 0; width: 84px; text-align: center; }
  .mini img, .mini .platz { width: 84px; height: 119px; object-fit: contain; border: 1px solid var(--grau-3); display: block; background: var(--weiss); }
  .mini figcaption { font-size: 11px; color: var(--grau-4); }
  .fuge { width: 14px; border: none; border-radius: 0; padding: 0; background: transparent; cursor: col-resize; position: relative; }
  .fuge:hover::after { content: ''; position: absolute; left: 6px; top: 0; bottom: 14px; width: 2px; background: var(--schwarz); }
  .trennlinie { width: 100%; border: none; border-top: 2px solid var(--schwarz); border-radius: 0; padding: 0; height: 12px; background: transparent; position: relative; }
  .trennlinie span { display: none; position: absolute; right: 0; top: -10px; font-size: 11px; background: var(--weiss); padding: 0 4px; }
  .trennlinie:hover span { display: inline; }
  .trennlinie:hover { border-top-style: dashed; }
  .fuss { border-top: 1px solid var(--grau-2); margin-top: 10px; padding-top: 10px; display: flex; flex-direction: column; gap: 8px; }
  .check { gap: 6px; }
  .fortschritt { display: flex; align-items: center; gap: 10px; margin: -6px 0 18px; }
  .balken { flex: 1; height: 8px; border: 1px solid var(--schwarz); border-radius: var(--radius); overflow: hidden; }
  .balken div { height: 100%; background: var(--schwarz); transition: width 0.2s; }
  .klausuren { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 10px; }
  .klausur { display: flex; gap: 10px; border: 1px solid var(--grau-3); border-radius: var(--radius); padding: 8px; }
  .klausur img, .klausur .platz { width: 70px; height: 99px; object-fit: cover; object-position: top; border: 1px solid var(--grau-2); flex: none; background: var(--grau-1); }
  .infos { display: flex; flex-direction: column; gap: 3px; min-width: 0; align-items: flex-start; }
  .infos input { width: 100%; }
  .rot { color: var(--akzent); }
</style>
