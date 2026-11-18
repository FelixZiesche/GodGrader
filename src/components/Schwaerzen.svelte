<script lang="ts">
  import type { Schwaerzung } from '../lib/types'

  let {
    bild,
    bereiche = $bindable(),
    geaendert,
  }: { bild: string; bereiche: Schwaerzung[]; geaendert: () => void } = $props()

  let flaeche: HTMLDivElement
  let neu = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null)

  function pos(e: PointerEvent) {
    const r = flaeche.getBoundingClientRect()
    return { x: Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)), y: Math.min(1, Math.max(0, (e.clientY - r.top) / r.height)) }
  }
  function down(e: PointerEvent) {
    if (e.button !== 0) return
    const p = pos(e)
    neu = { x0: p.x, y0: p.y, x1: p.x, y1: p.y }
    flaeche.setPointerCapture(e.pointerId)
  }
  function move(e: PointerEvent) {
    if (!neu) return
    const p = pos(e)
    neu.x1 = p.x
    neu.y1 = p.y
  }
  function up() {
    if (!neu) return
    const x = Math.min(neu.x0, neu.x1), y = Math.min(neu.y0, neu.y1)
    const w = Math.abs(neu.x1 - neu.x0), h = Math.abs(neu.y1 - neu.y0)
    neu = null
    if (w < 0.01 || h < 0.005) return
    bereiche.push({ bereich: { x, y, w, h }, seiten: { art: 'seiten', liste: [1] } })
    geaendert()
  }

  function auswahlText(s: Schwaerzung): string {
    return s.seiten.art === 'alle' ? 'alle' : s.seiten.liste.join(', ')
  }
  function setzeAuswahl(i: number, wert: string) {
    const v = wert.trim().toLowerCase()
    if (v === 'alle' || v === '') {
      bereiche[i].seiten = { art: 'alle' }
    } else {
      const liste = v.split(/[,; ]+/).map((x) => parseInt(x, 10)).filter((n) => n > 0)
      bereiche[i].seiten = liste.length ? { art: 'seiten', liste } : { art: 'alle' }
    }
    geaendert()
  }
  const pct = (v: number) => `${(v * 100).toFixed(3)}%`
</script>

<div class="werkzeug">
  <div
    class="flaeche"
    bind:this={flaeche}
    role="application"
    aria-label="Bereiche zum Schwärzen aufziehen"
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
  >
    <img src={bild} alt="Beispielseite" draggable="false" />
    {#each bereiche as s, i (i)}
      <div class="rechteck" style="left:{pct(s.bereich.x)};top:{pct(s.bereich.y)};width:{pct(s.bereich.w)};height:{pct(s.bereich.h)}">
        <span>{i + 1}</span>
      </div>
    {/each}
    {#if neu}
      <div class="rechteck neu" style="left:{pct(Math.min(neu.x0, neu.x1))};top:{pct(Math.min(neu.y0, neu.y1))};width:{pct(Math.abs(neu.x1 - neu.x0))};height:{pct(Math.abs(neu.y1 - neu.y0))}"></div>
    {/if}
  </div>
  <div class="liste">
    <p class="hinweis">Rechtecke mit der Maus aufziehen, z. B. über die Namenszeile. Seiten: „alle“ oder Nummern innerhalb jeder Klausur, z. B. „1“ oder „1, 3“.</p>
    {#each bereiche as s, i (i)}
      <div class="zeile eintrag">
        <strong>{i + 1}</strong>
        <label class="zeile">Seiten <input value={auswahlText(s)} onchange={(e) => setzeAuswahl(i, e.currentTarget.value)} style="width: 90px" /></label>
        <button class="leise" onclick={() => { bereiche.splice(i, 1); geaendert() }}>entfernen</button>
      </div>
    {:else}
      <p class="hinweis">Noch keine Bereiche.</p>
    {/each}
  </div>
</div>

<style>
  .werkzeug { display: grid; grid-template-columns: minmax(260px, 420px) 1fr; gap: 16px; align-items: start; }
  .flaeche { position: relative; border: 1px solid var(--schwarz); cursor: crosshair; user-select: none; touch-action: none; line-height: 0; }
  .flaeche img { width: 100%; display: block; pointer-events: none; }
  .rechteck { position: absolute; background: rgba(0, 0, 0, 0.85); outline: 1px solid var(--weiss); }
  .rechteck span { position: absolute; left: 3px; top: 2px; color: var(--weiss); font-size: 11px; line-height: 1; }
  .rechteck.neu { background: rgba(0, 0, 0, 0.4); outline: 1px dashed var(--schwarz); }
  .eintrag { padding: 4px 0; border-bottom: 1px solid var(--grau-2); }
</style>
