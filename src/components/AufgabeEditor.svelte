<script lang="ts">
  import type { Afb, Aufgabe, Element } from '../lib/types'

  let {
    aufgabe = $bindable(),
    warnungen = [],
    halbeBe = false,
    entfernen,
    kiHilfe,
  }: { aufgabe: Aufgabe; warnungen?: string[]; halbeBe?: boolean; entfernen: () => void; kiHilfe?: () => void } = $props()

  let offen = $state<Record<number, boolean>>({})
  let textOffen = $state(false)
  let loesungOffen = $state(false)

  const summe = $derived(aufgabe.elemente.reduce((s, e) => s + (Number(e.max_be) || 0), 0))
  const passt = $derived(Math.abs(summe - aufgabe.max_be) < 1e-6)
  const schritt = $derived(halbeBe ? 0.5 : 1)
  const vorschau = $derived.by(() => {
    const t = aufgabe.text.replace(/\*/g, '').replace(/\|\s*---[\s|-]*\|/g, ' ').replace(/\s*\|\s*/g, ' · ').replace(/<br>/g, ' ')
    return t.length > 240 ? t.slice(0, 240) + ' …' : t
  })
  const be = (x: number) => String(x).replace('.', ',')

  function neuesElement() {
    aufgabe.elemente.push({ id: '', titel: 'Neues Element', text: '', max_be: 1, art: 'element', afb: aufgabe.afb[0]?.afb ?? null })
  }
  function verschieben(i: number, d: number) {
    const j = i + d
    if (j < 0 || j >= aufgabe.elemente.length) return
    const l = aufgabe.elemente
    ;[l[i], l[j]] = [l[j], l[i]]
  }
  function zusammenlegen(i: number) {
    const a = aufgabe.elemente[i]
    const b = aufgabe.elemente[i + 1]
    if (!b) return
    const neu: Element = {
      ...a,
      titel: `${a.titel} + ${b.titel}`,
      text: [a.text, b.text].filter(Boolean).join('\n\n'),
      max_be: (Number(a.max_be) || 0) + (Number(b.max_be) || 0),
    }
    aufgabe.elemente.splice(i, 2, neu)
  }
  function afbNeu() {
    aufgabe.afb.push({ afb: 'II', be: aufgabe.max_be, zusatz: null })
  }
  const AFBS: Afb[] = ['I', 'II', 'III']
</script>

<section class="karte aufgabe" class:fehler={warnungen.length > 0}>
  <div class="kopfzeile">
    <label class="zeile">Aufgabe <input class="nr" bind:value={aufgabe.nr} /></label>
    <label class="zeile"><input type="number" class="be" min="0" step={schritt} bind:value={aufgabe.max_be} /> BE</label>
    <span class="summe" class:falsch={!passt} title="Summe der Elemente">Σ {be(summe)} / {be(aufgabe.max_be)} BE</span>
    <div class="afb zeile">
      {#each aufgabe.afb as a, i (i)}
        <span class="afbchip">
          AFB
          <select bind:value={a.afb}>{#each AFBS as x (x)}<option value={x}>{x}</option>{/each}</select>
          <input type="number" min="0" step={schritt} bind:value={a.be} /> BE
          <button class="x" title="entfernen" onclick={() => aufgabe.afb.splice(i, 1)}>×</button>
        </span>
      {/each}
      <button class="leise" onclick={afbNeu}>+ AFB</button>
    </div>
    <label class="zeile"><input type="checkbox" bind:checked={aufgabe.hat_raster} /> Raster</label>
    <button class="leise" onclick={entfernen}>Aufgabe löschen</button>
  </div>

  {#each warnungen as w (w)}<div class="warnung klein">{w}</div>{/each}
  {#if warnungen.length && kiHilfe}
    <div class="zeile kihilfe">
      <button onclick={kiHilfe}>KI-Hilfe: Elemente vorschlagen</button>
      <span class="hinweis">Das Bewerten-Modell gliedert die Musterlösung neu. Sie entscheiden, ob Sie den Vorschlag übernehmen.</span>
    </div>
  {/if}

  <button class="aufklapp" onclick={() => (textOffen = !textOffen)}>{textOffen ? '▾' : '▸'} Aufgabentext</button>
  {#if textOffen}
    <textarea rows="4" bind:value={aufgabe.text}></textarea>
  {:else}
    <p class="vorschau">{vorschau}</p>
  {/if}

  <table class="elemente">
    <thead>
      <tr><th class="t-id">Nr.</th><th>Bewertungselement</th><th class="t-be">max. BE</th><th class="t-afb">AFB</th><th class="t-art">Art</th><th class="t-akt"></th></tr>
    </thead>
    <tbody>
      {#each aufgabe.elemente as el, i (i)}
        <tr>
          <td class="t-id">{aufgabe.nr}-{i + 1}</td>
          <td>
            <div class="zeile" style="flex-wrap: nowrap">
              <button class="x" title="Text anzeigen" onclick={() => (offen[i] = !offen[i])}>{offen[i] ? '▾' : '▸'}</button>
              <input class="titel" bind:value={el.titel} />
            </div>
            {#if offen[i]}<textarea rows="5" bind:value={el.text}></textarea>{/if}
          </td>
          <td class="t-be"><input type="number" min="0" step={schritt} bind:value={el.max_be} /></td>
          <td class="t-afb">
            <select bind:value={el.afb}>
              <option value={null}>–</option>
              {#each AFBS as x (x)}<option value={x}>{x}</option>{/each}
            </select>
          </td>
          <td class="t-art">
            <select bind:value={el.art}><option value="element">Element</option><option value="raster">Raster</option></select>
          </td>
          <td class="t-akt">
            <button class="x" title="nach oben" onclick={() => verschieben(i, -1)}>↑</button>
            <button class="x" title="nach unten" onclick={() => verschieben(i, 1)}>↓</button>
            <button class="x" title="mit nächstem zusammenlegen" disabled={i === aufgabe.elemente.length - 1} onclick={() => zusammenlegen(i)}>⊕</button>
            <button class="x" title="löschen" onclick={() => aufgabe.elemente.splice(i, 1)}>×</button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
  <div class="zeile" style="justify-content: space-between; margin-top: 6px">
    <button onclick={neuesElement}>+ Element</button>
    <button class="aufklapp" onclick={() => (loesungOffen = !loesungOffen)}>{loesungOffen ? '▾' : '▸'} vollständige Musterlösung</button>
  </div>
  {#if loesungOffen}<textarea rows="10" bind:value={aufgabe.loesung}></textarea>{/if}
</section>

<style>
  .kihilfe { margin: 6px 0 8px; gap: 10px; }
  .aufgabe { margin-bottom: 14px; }
  .aufgabe.fehler { border-color: var(--akzent); }
  .kopfzeile { display: flex; gap: 14px; align-items: center; flex-wrap: wrap; margin-bottom: 8px; }
  .kopfzeile label { font-weight: 600; }
  .nr { width: 52px; font-weight: 700; }
  .be { width: 64px; }
  .summe { font-variant-numeric: tabular-nums; padding: 2px 8px; border-radius: var(--radius); background: var(--grau-1); }
  .summe.falsch { background: var(--akzent-hell); color: var(--akzent); font-weight: 600; }
  .afbchip { border: 1px solid var(--grau-3); border-radius: var(--radius); padding: 2px 4px 2px 8px; display: inline-flex; gap: 4px; align-items: center; font-size: 13px; }
  .afbchip input { width: 54px; padding: 2px 4px; }
  .afbchip select { padding: 2px; }
  .x { border: none; background: transparent; padding: 2px 5px; }
  .x:hover:not(:disabled) { background: var(--grau-2); }
  .klein { padding: 4px 10px; margin-bottom: 6px; font-size: 13px; }
  .aufklapp { border: none; padding: 2px 0; background: none; font-weight: 600; }
  .vorschau { color: var(--grau-4); font-size: 13px; margin: 2px 0 8px; }
  textarea { width: 100%; font-size: 13px; margin-top: 4px; }
  table.elemente { width: 100%; border-collapse: collapse; margin-top: 8px; }
  .elemente th { background: var(--grau-2); text-align: left; font-weight: 600; padding: 5px 6px; font-size: 13px; }
  .elemente td { border-top: 1px solid var(--grau-2); padding: 4px 6px; vertical-align: top; }
  .titel { width: 100%; }
  .t-id { width: 52px; color: var(--grau-4); font-size: 12px; padding-top: 10px !important; }
  .t-be { width: 74px; }
  .t-be input { width: 64px; }
  .t-afb { width: 64px; }
  .t-art { width: 96px; }
  .t-akt { width: 118px; white-space: nowrap; }
</style>
