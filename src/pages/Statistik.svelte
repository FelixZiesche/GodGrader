<script lang="ts">
  import { onMount } from 'svelte'
  import { api, fehlertext } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import type { Quote, StatistikAntwort } from '../lib/types'

  const projekt = $derived(store.projekt)

  let daten = $state<StatistikAntwort | null>(null)
  let fehler = $state<string | null>(null)

  onMount(async () => {
    if (!projekt) return
    try {
      daten = await api.statistik(projekt.id)
    } catch (e) {
      fehler = fehlertext(e)
    }
  })

  const st = $derived(daten?.statistik)
  const oberstufe = $derived(st?.stufe === 'oberstufe')
  const einheit = $derived(oberstufe ? 'NP' : '')

  const zahl = (v: number | null | undefined, stellen = 1) =>
    v == null ? '–' : v.toLocaleString('de-DE', { minimumFractionDigits: 0, maximumFractionDigits: stellen })
  const pz = (v: number | null | undefined) => (v == null ? '–' : `${zahl(v)} %`)
  const be = (v: number) => zahl(v, 1)

  const maxAnzahl = $derived(Math.max(1, ...(st?.notenspiegel.map((n) => n.anzahl) ?? [1])))
  const schlechtText = $derived(oberstufe ? 'unter 5 NP' : 'Noten 5 und 6')
</script>

<h1>Statistik</h1>

{#snippet quoten(titel: string, liste: Quote[], mitAufgabe = false)}
  <section class="karte">
    <h2>{titel}</h2>
    <table class="tabelle">
      <thead>
        <tr>
          <th>{mitAufgabe ? 'Element' : 'Bereich'}</th>
          {#if mitAufgabe}<th>Aufgabe</th>{/if}
          <th class="r">BE</th>
          <th class="quote">Lösungsquote</th>
        </tr>
      </thead>
      <tbody>
        {#each liste as q, i (i)}
          <tr>
            <td>{q.name}</td>
            {#if mitAufgabe}<td>{q.zusatz ?? ''}</td>{/if}
            <td class="r">{be(q.erreicht)} / {be(q.moeglich)}</td>
            <td class="quote">
              <div class="qzeile">
                <div class="qbalken"><div style="width: {Math.min(100, q.quote)}%"></div></div>
                <span>{zahl(q.quote, 0)} %</span>
              </div>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
{/snippet}

{#if !projekt}
  <div class="karte grau"><p>Bitte zuerst unter „Projekte“ ein Projekt anlegen oder öffnen.</p></div>
{:else if fehler}
  <div class="warnung" role="alert">{fehler}</div>
{:else if daten && st}
  {#if st.nicht_freigegeben.length}
    <div class="warnung">
      {st.nicht_freigegeben.length === 1
        ? '1 Klausur ist noch nicht freigegeben und zählt'
        : `${st.nicht_freigegeben.length} Klausuren sind noch nicht freigegeben und zählen`} hier nicht mit: {st.nicht_freigegeben.join(', ')}.
    </div>
  {/if}

  {#if st.anzahl === 0}
    <div class="karte grau"><p>Noch keine freigegebene Klausur. Freigeben geht unter „Prüfen“.</p></div>
  {:else}
    <div class="kacheln">
      <div class="karte kachel"><span class="hinweis">Klausuren</span><strong>{st.anzahl}</strong></div>
      <div class="karte kachel">
        <span class="hinweis">Durchschnitt</span>
        <strong>{zahl(st.durchschnitt, oberstufe ? 1 : 2)} {einheit}</strong>
        <span class="hinweis">{pz(st.durchschnitt_prozent)} der BE</span>
      </div>
      <div class="karte kachel"><span class="hinweis">Median</span><strong>{zahl(st.median)} {einheit}</strong></div>
      <div class="karte kachel">
        <span class="hinweis">Spannweite</span>
        <strong>{zahl(st.minimum)} bis {zahl(st.maximum)} {einheit}</strong>
      </div>
      <div class="karte kachel" class:alarm={st.ueber_warnschwelle}>
        <span class="hinweis">{schlechtText}</span>
        <strong>{st.schlecht_anzahl} ({pz(st.schlecht_prozent)})</strong>
        <span class="hinweis">
          {#if st.warnschwelle == null}Keine Warnschwelle gesetzt{:else if st.ueber_warnschwelle}Über der Warnschwelle von {zahl(st.warnschwelle)} %{:else}Warnschwelle {zahl(st.warnschwelle)} %{/if}
        </span>
      </div>
      <div class="karte kachel">
        <span class="hinweis">KI-Vorschläge geändert</span>
        <strong>{pz(st.geaendert_prozent)}</strong>
        <span class="hinweis">{st.geaendert} von {st.mit_vorschlag} Elementen</span>
      </div>
    </div>

    {#if st.ueber_warnschwelle}
      <div class="warnung">
        Der Anteil {oberstufe ? 'der Ergebnisse unter 5 NP' : 'der Noten 5 und 6'} liegt mit {pz(st.schlecht_prozent)} über der
        Warnschwelle von {zahl(st.warnschwelle)} %. Bitte die schulischen Vorgaben zur Genehmigung beachten.
      </div>
    {/if}

    <section class="karte">
      <h2>Notenspiegel</h2>
      <div class="spiegel" role="img" aria-label="Notenspiegel">
        {#each st.notenspiegel as n (n.label)}
          <div class="saeule">
            <span class="anzahl">{n.anzahl || ''}</span>
            <div class="balken" style="height: {(n.anzahl / maxAnzahl) * 100}%"></div>
            <span class="label">{n.label}</span>
          </div>
        {/each}
      </div>
    </section>

    <div class="zwei">
      {@render quoten('Lösungsquote je Aufgabe', st.je_aufgabe)}
      {@render quoten('Lösungsquote je Anforderungsbereich', st.je_afb)}
    </div>

    <section class="karte">
      <h2>Schwächste Elemente</h2>
      <ol class="schwach">
        {#each st.schwaechste as q, i (i)}
          <li><strong>{q.name}</strong> <span class="hinweis">Aufgabe {q.zusatz}, {zahl(q.quote, 0)} % gelöst</span></li>
        {/each}
      </ol>
    </section>

    {@render quoten('Lösungsquote je Element', st.je_element, true)}

    <section class="karte">
      <h2>Notenliste</h2>
      <table class="tabelle">
        <thead>
          <tr>
            <th>{daten.label}</th>
            {#each daten.aufgaben as a (a)}<th class="r">A {a}</th>{/each}
            <th class="r">Summe</th>
            <th class="r">%</th>
            <th class="r">{oberstufe ? 'NP' : 'Note'}</th>
          </tr>
        </thead>
        <tbody>
          {#each daten.zeilen as z (z.id)}
            <tr>
              <td>{z.bezeichnung}</td>
              {#each z.je_aufgabe as w, i (i)}<td class="r">{be(w)}</td>{/each}
              <td class="r"><strong>{be(z.summe)}</strong></td>
              <td class="r">{zahl(z.prozent, 0)}</td>
              <td class="r"><strong>{oberstufe ? (z.punkte ?? '–') : (z.note ?? '–')}</strong></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  {/if}
{/if}

<style>
  section { margin-bottom: 14px; }
  .warnung { margin-bottom: 12px; }
  h2 { font-size: 15px; margin: 0 0 10px; }
  .kacheln { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 10px; margin-bottom: 14px; }
  .kachel { display: flex; flex-direction: column; gap: 3px; }
  .kachel strong { font-size: 20px; }
  .kachel.alarm { border-color: var(--akzent); }
  .kachel.alarm strong { color: var(--akzent); }
  .spiegel { display: flex; align-items: flex-end; gap: 6px; height: 180px; padding-top: 18px; }
  .saeule { flex: 1; height: 100%; display: flex; flex-direction: column; justify-content: flex-end; align-items: stretch; min-width: 0; }
  .saeule .balken { background: var(--grau-4); border-radius: 2px 2px 0 0; min-height: 1px; }
  .saeule .anzahl { text-align: center; font-size: 12px; font-weight: 600; height: 16px; }
  .saeule .label { text-align: center; font-size: 12px; border-top: 1px solid var(--schwarz); padding-top: 3px; }
  .zwei { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
  .zwei section { margin-bottom: 14px; }
  @media (max-width: 900px) { .zwei { grid-template-columns: 1fr; } }
  .tabelle { width: 100%; border-collapse: collapse; }
  .tabelle th, .tabelle td { text-align: left; padding: 5px 8px; border-bottom: 1px solid var(--grau-2); }
  .tabelle th { background: var(--grau-2); font-weight: 600; }
  .tabelle .r { text-align: right; white-space: nowrap; }
  .quote { width: 38%; }
  .qzeile { display: flex; align-items: center; gap: 8px; }
  .qzeile span { width: 42px; text-align: right; font-variant-numeric: tabular-nums; }
  .qbalken { flex: 1; height: 8px; background: var(--grau-2); border-radius: 2px; overflow: hidden; }
  .qbalken div { height: 100%; background: var(--grau-4); }
  .schwach { margin: 0; padding-left: 20px; display: flex; flex-direction: column; gap: 4px; }
</style>
