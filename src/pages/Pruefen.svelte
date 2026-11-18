<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte'
  import { api, fehlertext } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import Seitenansicht from '../components/Seitenansicht.svelte'
  import type { BewertungZeile, Element, Erwartungshorizont, KlausurInfo, ProtokollEintrag, PruefDaten } from '../lib/types'

  type Eintrag = { nr: string; el: Element; b: BewertungZeile | undefined }

  const projekt = $derived(store.projekt)
  const halbe = $derived(!!projekt?.halbe_be)

  let ewh = $state<Erwartungshorizont | null>(null)
  let klausuren = $state<KlausurInfo[]>([])
  let kid = $state<string | null>(null)
  let daten = $state<PruefDaten | null>(null)
  let auswahl = $state<string | null>(null)
  let nurUnsicher = $state(false)
  let seite = $state(0)
  let fehler = $state<string | null>(null)
  let meldung = $state<string | null>(null)
  let neuBewertet = $state<string | null>(null)
  let laden = $state(true)
  let protokoll = $state<ProtokollEintrag[] | null>(null)

  // Abschrift der gewählten Aufgabe
  let abschriftNr = $state<string | null>(null)
  let abschriftText = $state('')
  let abschriftGeaendert = $state(false)
  let bearbeitet = $state<Record<string, boolean>>({})

  const STATUS: Record<string, string> = { offen: 'offen', vorkorrigiert: 'vorkorrigiert', in_pruefung: 'in Prüfung', freigegeben: 'freigegeben' }
  const be = (x: number | null | undefined) => (x == null ? '–' : String(x).replace('.', ','))

  const freigegeben = $derived(daten?.klausur.status === 'freigegeben')
  const alle = $derived<Eintrag[]>(
    ewh && daten
      ? ewh.aufgaben.flatMap((a) => a.elemente.map((el) => ({ nr: a.nr, el, b: daten!.bewertungen.find((b) => b.element_id === el.id) })))
      : [],
  )
  const abweichung = (b: BewertungZeile | undefined) => !!b && b.zweit_be != null && b.ki_be != null && Math.abs(b.zweit_be - b.ki_be) >= 1
  const istUnsicher = (e: Eintrag) => !e.b || e.b.ki_be == null || e.b.unsicher || e.b.sicherheit === 'niedrig' || abweichung(e.b)
  const sichtbar = $derived(nurUnsicher ? alle.filter(istUnsicher) : alle)
  const anzahlUnsicher = $derived(alle.filter(istUnsicher).length)
  const gewaehlt = $derived(alle.find((e) => e.el.id === auswahl) ?? null)
  const wert = (e: Eintrag) => e.b?.final_be ?? e.b?.ki_be ?? null
  const aufgabe = $derived(ewh?.aufgaben.find((a) => a.nr === gewaehlt?.nr) ?? null)
  const abschrift = $derived(daten?.abschriften.find((a) => a.aufgabe === gewaehlt?.nr) ?? null)
  const nichtZugeordnet = $derived(daten?.abschriften.find((a) => a.aufgabe === '') ?? null)
  const ki = $derived(daten?.ergebnis)
  const kIndex = $derived(klausuren.findIndex((k) => k.id === kid))

  function schritteFuer(nr: string) {
    return daten?.schritte.filter((s) => s.aufgabe === nr && s.art !== 'abschreiben') ?? []
  }
  function aufgabeSumme(nr: string) {
    return alle.filter((e) => e.nr === nr).reduce((s, e) => s + (wert(e) ?? 0), 0)
  }

  // ---------- Laden ----------

  async function start() {
    if (!projekt) return
    laden = true
    try {
      ewh = await api.getEwh(projekt.id)
      klausuren = await api.klausuren(projekt.id)
      let gemerkt: string | null = null
      try {
        gemerkt = localStorage.getItem(`pruefen-${projekt.id}`)
      } catch {
        /* egal */
      }
      const erste = klausuren.find((k) => k.status === 'vorkorrigiert' || k.status === 'in_pruefung') ?? klausuren[0]
      await waehle(klausuren.some((k) => k.id === gemerkt) ? gemerkt! : (erste?.id ?? null))
    } catch (e) {
      fehler = fehlertext(e)
    } finally {
      laden = false
    }
  }

  async function waehle(id: string | null) {
    await abschriftSichern()
    kid = id
    daten = null
    protokoll = null
    meldung = neuBewertet = null
    bearbeitet = {}
    abschriftNr = null
    if (!id || !projekt) return
    try {
      localStorage.setItem(`pruefen-${projekt.id}`, id)
    } catch {
      /* egal */
    }
    daten = await api.pruefdaten(projekt.id, id)
    seite = 0
    const erstes = (nurUnsicher ? alle.filter(istUnsicher) : alle).find((e) => !e.b?.bestaetigt) ?? alle[0]
    auswahl = erstes?.el.id ?? null
  }

  function uebernehmen(d: PruefDaten) {
    daten = d
    const k = klausuren.find((x) => x.id === d.klausur.id)
    if (k) k.status = d.klausur.status
  }

  onMount(() => {
    void start()
    window.addEventListener('keydown', taste)
  })
  onDestroy(() => {
    window.removeEventListener('keydown', taste)
    void abschriftSichern()
  })

  // ---------- Auswahl, Seite, Abschrift ----------

  $effect(() => {
    const nr = gewaehlt?.nr ?? null
    if (nr === abschriftNr) return
    void abschriftSichern().then(() => {
      abschriftNr = nr
      abschriftText = daten?.abschriften.find((a) => a.aufgabe === nr)?.text ?? ''
      abschriftGeaendert = false
      // zur ersten Seite der Antwort springen
      const n = daten?.abschriften.find((a) => a.aufgabe === nr)?.seiten[0]
      if (n && daten) {
        const quellen = [...new Set(daten.klausur.seiten.map((s) => s.quelle_seite))].sort((a, b) => a - b)
        const i = daten.klausur.seiten.findIndex((s) => s.quelle_seite === quellen[n - 1])
        if (i >= 0) seite = i
      }
    })
  })

  async function abschriftSichern() {
    if (!abschriftGeaendert || !projekt || !kid || abschriftNr == null) return
    const nr = abschriftNr
    abschriftGeaendert = false
    try {
      await api.abschriftSpeichern(projekt.id, kid, nr, abschriftText)
      const z = daten?.abschriften.find((a) => a.aufgabe === nr)
      if (z) z.text = abschriftText
      bearbeitet[nr] = true
    } catch (e) {
      fehler = fehlertext(e)
    }
  }

  async function neuBewerten() {
    if (!projekt || !kid || !gewaehlt) return
    await abschriftSichern()
    const nr = gewaehlt.nr
    neuBewertet = nr
    fehler = null
    try {
      uebernehmen(await api.aufgabeNeuBewerten(projekt.id, kid, nr))
      bearbeitet[nr] = false
      meldung = `Aufgabe ${nr} wurde neu bewertet. Bitte die Elemente erneut bestätigen.`
    } catch (e) {
      fehler = fehlertext(e)
    } finally {
      neuBewertet = null
    }
  }

  async function auswaehlen(id: string) {
    auswahl = id
    await tick()
    document.getElementById(`el-${id}`)?.scrollIntoView({ block: 'nearest' })
  }

  function weiter(d: number) {
    const i = sichtbar.findIndex((e) => e.el.id === auswahl)
    const j = Math.min(sichtbar.length - 1, Math.max(0, (i < 0 ? 0 : i + d)))
    if (sichtbar[j]) void auswaehlen(sichtbar[j].el.id)
  }

  // ---------- BE setzen ----------

  async function setzen(e: Eintrag, wertNeu: number, bestaetigt: boolean) {
    if (!projekt || !kid || freigegeben) return
    fehler = null
    try {
      uebernehmen(await api.beSetzen(projekt.id, kid, e.el.id, wertNeu, bestaetigt))
      if (protokoll) protokoll = await api.protokoll(projekt.id, kid)
    } catch (err) {
      fehler = fehlertext(err)
    }
  }

  async function bestaetigen(e: Eintrag) {
    const w = wert(e)
    if (w == null) {
      fehler = `Für ${e.el.titel || e.el.id} gibt es noch keinen Wert. Bitte zuerst BE wählen.`
      return
    }
    await setzen(e, w, true)
    // zum nächsten unbestätigten Element
    const i = sichtbar.findIndex((x) => x.el.id === e.el.id)
    const naechstes = [...sichtbar.slice(i + 1), ...sichtbar.slice(0, i)].find((x) => !x.b?.bestaetigt)
    if (naechstes) void auswaehlen(naechstes.el.id)
  }

  async function aufheben(e: Eintrag) {
    if (!projekt || !kid) return
    uebernehmen(await api.bestaetigungAufheben(projekt.id, kid, e.el.id))
  }

  async function aufgabeBestaetigen(nr: string) {
    for (const e of alle.filter((x) => x.nr === nr && !x.b?.bestaetigt)) {
      const w = wert(e)
      if (w != null) await setzen(e, w, true)
    }
  }

  function stufen(max: number): number[] {
    const s = halbe ? 0.5 : 1
    const l: number[] = []
    for (let v = 0; v <= max + 1e-9; v += s) l.push(Math.round(v * 2) / 2)
    return l
  }

  function taste(ev: KeyboardEvent) {
    const t = ev.target as HTMLElement | null
    if (t && (t.tagName === 'TEXTAREA' || t.tagName === 'INPUT' || t.tagName === 'SELECT' || t.isContentEditable)) return
    if (ev.ctrlKey || ev.metaKey || ev.altKey) return
    const e = gewaehlt
    if (ev.key === 'j' || ev.key === 'J') {
      weiter(1)
    } else if (ev.key === 'k' || ev.key === 'K') {
      weiter(-1)
    } else if (ev.key === 'Enter' && e) {
      void bestaetigen(e)
    } else if (/^[0-9]$/.test(ev.key) && e) {
      const v = Number(ev.key)
      if (v <= e.el.max_be) void setzen(e, v, false)
      else fehler = `Höchstens ${be(e.el.max_be)} BE für dieses Element.`
    } else if ((ev.key === ',' || ev.key === '.') && e && halbe) {
      const v = Math.floor(wert(e) ?? 0) + 0.5
      if (v <= e.el.max_be) void setzen(e, v, false)
    } else {
      return
    }
    ev.preventDefault()
  }

  // ---------- Freigabe ----------

  async function freigeben() {
    if (!projekt || !kid) return
    fehler = null
    try {
      uebernehmen(await api.freigeben(projekt.id, kid))
      meldung = 'Freigegeben. Diese Klausur erscheint jetzt in den Exporten.'
    } catch (e) {
      fehler = fehlertext(e)
    }
  }
  async function zuruecknehmen() {
    if (!projekt || !kid) return
    uebernehmen(await api.freigabeZuruecknehmen(projekt.id, kid))
    meldung = null
  }

  async function protokollUmschalten() {
    if (!projekt || !kid) return
    protokoll = protokoll ? null : await api.protokoll(projekt.id, kid)
  }
  const titel = (id: string) => alle.find((e) => e.el.id === id)?.el.titel ?? id
</script>

{#if !projekt}
  <h1>Prüfen</h1>
  <div class="karte grau"><p>Bitte zuerst unter „Projekte“ ein Projekt anlegen oder öffnen.</p></div>
{:else if laden}
  <h1>Prüfen</h1>
  <p class="hinweis">Lade …</p>
{:else if !ewh}
  <h1>Prüfen</h1>
  <div class="warnung">Es fehlt noch der Erwartungshorizont.</div>
{:else if !klausuren.length}
  <h1>Prüfen</h1>
  <div class="karte grau"><p>Noch keine Klausuren. Erst unter „Scans“ importieren und unter „Verarbeitung“ vorkorrigieren lassen.</p></div>
{:else}
  <div class="pruefen">
    <header class="kopf">
      <div class="zeile wahl">
        <button onclick={() => waehle(klausuren[kIndex - 1]?.id ?? null)} disabled={kIndex <= 0} aria-label="vorige Klausur">◀</button>
        <select value={kid} onchange={(e) => waehle(e.currentTarget.value)} aria-label="Klausur">
          {#each klausuren as k (k.id)}<option value={k.id}>{k.bezeichnung} ({STATUS[k.status]})</option>{/each}
        </select>
        <button onclick={() => waehle(klausuren[kIndex + 1]?.id ?? null)} disabled={kIndex >= klausuren.length - 1} aria-label="nächste Klausur">▶</button>
        {#if daten}<span class="status {daten.klausur.status}">{STATUS[daten.klausur.status]}</span>{/if}
      </div>
      {#if ki}
        <div class="ergebnis">
          <strong>{be(ki.summe)} / {be(ki.gesamt)} BE</strong>
          <span>{ki.prozent.toLocaleString('de-DE', { maximumFractionDigits: 1 })} %</span>
          <strong>{ewh.stufe === 'oberstufe' ? `${ki.punkte ?? '–'} NP` : `Note ${ki.note ?? '–'}`}</strong>
          <span class="hinweis">{ki.bestaetigt} von {ki.elemente} bestätigt</span>
        </div>
      {/if}
      <div class="zeile">
        <label class="zeile filter"><input type="checkbox" bind:checked={nurUnsicher} /> nur Unsicheres ({anzahlUnsicher})</label>
        {#if freigegeben}
          <button onclick={zuruecknehmen}>Freigabe zurücknehmen</button>
        {:else}
          <button class="primaer" onclick={freigeben} disabled={!ki || ki.bestaetigt < ki.elemente} title={ki && ki.bestaetigt < ki.elemente ? 'Erst alle Elemente bestätigen' : ''}>Freigeben</button>
        {/if}
      </div>
    </header>

    {#if fehler}<div class="warnung" role="alert">{fehler} <button class="leise" onclick={() => (fehler = null)}>ausblenden</button></div>{/if}
    {#if meldung}<div class="karte grau meldung">{meldung}{#if freigegeben && kIndex < klausuren.length - 1} <button class="leise" onclick={() => waehle(klausuren[kIndex + 1].id)}>nächste Klausur</button>{/if}</div>{/if}

    {#if daten}
      <div class="spalten">
        <!-- links: Seitenbild -->
        <section class="spalte bild">
          <Seitenansicht projektId={projekt.id} seiten={daten.klausur.seiten} bind:aktiv={seite} />
        </section>

        <!-- Mitte: Abschrift -->
        <section class="spalte mitte">
          {#if gewaehlt && aufgabe}
            <div class="zeile zwischen">
              <h2>Abschrift Aufgabe {aufgabe.nr}</h2>
              {#if abschrift && abschrift.text !== abschrift.ki_text}<span class="hinweis">von Ihnen geändert</span>{/if}
            </div>
            <details class="aufgabentext">
              <summary>Aufgabentext</summary>
              <p>{aufgabe.text}</p>
            </details>
            <textarea
              bind:value={abschriftText}
              oninput={() => (abschriftGeaendert = true)}
              onblur={abschriftSichern}
              disabled={freigegeben}
              aria-label="Abschrift Aufgabe {aufgabe.nr}"
              placeholder="Keine Antwort abgeschrieben."
            ></textarea>
            {#if abschrift?.unsicher.length}
              <div class="stellen">
                <span class="hinweis">Unsichere Stellen:</span>
                {#each abschrift.unsicher as u, i (i)}<span class="chip">{u}</span>{/each}
              </div>
            {/if}
            <div class="zeile">
              <button class:primaer={bearbeitet[aufgabe.nr] || abschriftGeaendert} onclick={neuBewerten} disabled={!!neuBewertet || freigegeben}>
                {neuBewertet === aufgabe.nr ? 'Bewertet neu …' : 'Diese Aufgabe neu bewerten'}
              </button>
              {#if bearbeitet[aufgabe.nr]}<span class="hinweis">Abschrift geändert. Neu bewerten setzt die Bestätigungen dieser Aufgabe zurück.</span>{/if}
            </div>
            {#if nichtZugeordnet}
              <details class="nz">
                <summary>Nicht zugeordneter Text</summary>
                <pre>{nichtZugeordnet.text}</pre>
              </details>
            {/if}
          {:else}
            <p class="hinweis">Element rechts auswählen.</p>
          {/if}
        </section>

        <!-- rechts: Bewertungselemente -->
        <section class="spalte rechts">
          <p class="hinweis tasten">J/K: nächstes/voriges · 0–9: BE{halbe ? ' · Komma: +0,5' : ''} · Enter: bestätigen</p>
          {#each ewh.aufgaben as a (a.nr)}
            {@const eintraege = sichtbar.filter((e) => e.nr === a.nr)}
            {#if eintraege.length}
              <div class="aufgabe">
                <div class="zeile zwischen akopf">
                  <h3>Aufgabe {a.nr}</h3>
                  <span>{be(aufgabeSumme(a.nr))} / {be(a.max_be)} BE</span>
                </div>
                {#each schritteFuer(a.nr) as s (s.art)}
                  {#if s.fehler}<div class="warnung klein">{s.art === 'zweitmeinung' ? 'Zweitmeinung' : 'Bewertung'}: {s.fehler}</div>{/if}
                  {#if s.hinweis && s.art === 'bewerten'}<div class="hinweisbox">{s.hinweis}</div>{/if}
                {/each}
                {#each eintraege as e (e.el.id)}
                  <div
                    id="el-{e.el.id}"
                    class="element"
                    class:gewaehlt={e.el.id === auswahl}
                    class:ok={e.b?.bestaetigt}
                    class:unsicher={istUnsicher(e)}
                    role="button"
                    tabindex="0"
                    onclick={() => (auswahl = e.el.id)}
                    onkeydown={(ev) => ev.key === ' ' && (auswahl = e.el.id)}
                  >
                    <div class="zeile zwischen">
                      <strong>{e.el.titel || e.el.id}{e.el.art === 'raster' ? ' (Raster)' : ''}</strong>
                      <span class="hinweis">max. {be(e.el.max_be)}</span>
                    </div>
                    {#if e.b?.ki_be != null}
                      <div class="ki">
                        KI: <strong>{be(e.b.ki_be)}</strong>
                        <span class="sicher {e.b.sicherheit}">{e.b.sicherheit}</span>
                        {#if e.b.unsicher}<span class="sicher niedrig">hängt an unsicherer Stelle</span>{/if}
                        {#if e.b.zweit_be != null}
                          <span class:abw={abweichung(e.b)}>· Zweitmeinung: {be(e.b.zweit_be)}</span>
                        {/if}
                      </div>
                      {#if e.b.begruendung}<p class="begr">{e.b.begruendung}</p>{/if}
                      {#if e.b.beleg}<p class="beleg">„{e.b.beleg}“</p>{/if}
                      {#if abweichung(e.b) && e.b.zweit_begruendung}<p class="begr abw">Zweitmeinung: {e.b.zweit_begruendung}</p>{/if}
                    {:else}
                      <p class="hinweis">Kein KI-Vorschlag. Bitte selbst bewerten.</p>
                    {/if}
                    <div class="zeile wahlbe">
                      {#each stufen(e.el.max_be) as v (v)}
                        <button
                          class="bew"
                          class:aktiv={wert(e) === v}
                          class:final={e.b?.final_be === v}
                          disabled={freigegeben}
                          onclick={(ev) => { ev.stopPropagation(); auswahl = e.el.id; void setzen(e, v, false) }}
                        >{be(v)}</button>
                      {/each}
                      <span class="luecke"></span>
                      {#if e.b?.bestaetigt}
                        <span class="okmarke">✓ bestätigt</span>
                        {#if !freigegeben}<button class="leise" onclick={(ev) => { ev.stopPropagation(); void aufheben(e) }}>aufheben</button>{/if}
                      {:else}
                        <button class="primaer" disabled={freigegeben || wert(e) == null} onclick={(ev) => { ev.stopPropagation(); auswahl = e.el.id; void bestaetigen(e) }}>✓ bestätigen</button>
                      {/if}
                    </div>
                  </div>
                {/each}
                {#if !freigegeben && eintraege.some((e) => !e.b?.bestaetigt && wert(e) != null)}
                  <button class="leise" onclick={() => aufgabeBestaetigen(a.nr)}>alle in Aufgabe {a.nr} bestätigen</button>
                {/if}
              </div>
            {/if}
          {/each}
          {#if nurUnsicher && !sichtbar.length}<p class="hinweis">Nichts Unsicheres in dieser Klausur.</p>{/if}

          <button class="leise" onclick={protokollUmschalten}>{protokoll ? 'Änderungsprotokoll ausblenden' : 'Änderungsprotokoll anzeigen'}</button>
          {#if protokoll}
            {#if protokoll.length}
              <table class="protokoll">
                <thead><tr><th>Element</th><th>KI</th><th>vorher</th><th>neu</th><th></th></tr></thead>
                <tbody>
                  {#each protokoll as p, i (i)}
                    <tr><td>{titel(p.element_id)}</td><td>{be(p.ki_be)}</td><td>{be(p.vorher)}</td><td>{be(p.nachher)}</td><td>{p.bestaetigt ? '✓' : ''}</td></tr>
                  {/each}
                </tbody>
              </table>
            {:else}
              <p class="hinweis">Noch keine Änderungen.</p>
            {/if}
          {/if}
        </section>
      </div>
    {/if}
  </div>
{/if}

<style>
  .pruefen { display: flex; flex-direction: column; height: calc(100vh - 44px); min-height: 480px; }
  .kopf { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; padding-bottom: 10px; border-bottom: 1px solid var(--schwarz); margin-bottom: 10px; }
  .wahl select { min-width: 220px; }
  .status { font-size: 12px; border: 1px solid var(--schwarz); border-radius: 10px; padding: 1px 8px; }
  .status.freigegeben { background: var(--schwarz); color: var(--weiss); }
  .ergebnis { display: flex; gap: 14px; align-items: baseline; font-size: 15px; }
  .filter { gap: 6px; }
  .meldung { margin-bottom: 8px; }
  .zwischen { justify-content: space-between; }
  .spalten { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(300px, 1.15fr) minmax(260px, 0.9fr) minmax(320px, 1fr); gap: 14px; }
  .spalte { min-height: 0; overflow: auto; }
  .bild { overflow: hidden; display: flex; flex-direction: column; }
  .mitte { display: flex; flex-direction: column; gap: 8px; }
  .mitte h2 { margin: 0; }
  .mitte textarea { flex: 1; min-height: 240px; resize: none; font-size: 14px; line-height: 1.55; }
  .aufgabentext summary, .nz summary { cursor: pointer; color: var(--grau-4); font-size: 13px; }
  .aufgabentext p { white-space: pre-wrap; font-size: 13px; margin: 4px 0 0; }
  .nz pre { white-space: pre-wrap; font-family: inherit; font-size: 13px; background: var(--grau-1); padding: 8px; }
  .stellen { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; }
  .chip { font-size: 12px; border: 1px solid var(--akzent); color: var(--akzent); border-radius: 10px; padding: 0 7px; }
  .rechts { padding-right: 4px; }
  .tasten { margin: 0 0 8px; font-size: 12px; }
  .aufgabe { margin-bottom: 14px; }
  .akopf { border-bottom: 1px solid var(--schwarz); padding-bottom: 3px; margin-bottom: 6px; }
  .akopf h3 { margin: 0; }
  .hinweisbox { font-size: 12px; background: var(--grau-1); padding: 5px 8px; margin-bottom: 6px; white-space: pre-wrap; }
  .warnung.klein { font-size: 12px; padding: 4px 8px; margin-bottom: 6px; }
  .element { border: 1px solid var(--grau-3); border-radius: var(--radius); padding: 8px 10px; margin-bottom: 6px; cursor: pointer; }
  .element.unsicher { border-left: 4px solid var(--akzent); }
  .element.ok { background: var(--grau-1); }
  .element.gewaehlt { border-color: var(--schwarz); box-shadow: 0 0 0 1px var(--schwarz); }
  .ki { font-size: 13px; margin-top: 3px; display: flex; gap: 6px; flex-wrap: wrap; align-items: baseline; }
  .sicher { font-size: 11px; border: 1px solid var(--grau-3); border-radius: 8px; padding: 0 6px; }
  .sicher.niedrig { border-color: var(--akzent); color: var(--akzent); }
  .abw { color: var(--akzent); font-weight: 600; }
  .begr { font-size: 13px; margin: 4px 0 0; }
  .beleg { font-size: 13px; margin: 3px 0 0; font-style: italic; color: var(--grau-4); }
  .wahlbe { margin-top: 6px; gap: 4px; flex-wrap: wrap; }
  .bew { min-width: 34px; padding: 3px 6px; }
  .bew.aktiv { background: var(--grau-2); }
  .bew.final { background: var(--schwarz); color: var(--weiss); }
  .luecke { flex: 1; }
  .okmarke { font-weight: 600; font-size: 13px; }
  .protokoll { width: 100%; border-collapse: collapse; font-size: 12px; margin-top: 6px; }
  .protokoll th, .protokoll td { text-align: left; padding: 3px 5px; border-bottom: 1px solid var(--grau-2); }
  .protokoll th { background: var(--grau-2); }
</style>
