<script lang="ts">
  import { onDestroy } from 'svelte'
  import { api } from '../lib/api'
  import type { SeiteInfo } from '../lib/types'

  let {
    projektId,
    seiten,
    aktiv = $bindable(0),
  }: { projektId: string; seiten: SeiteInfo[]; aktiv: number } = $props()

  const urls = new Map<string, string>()
  let bild = $state<string | null>(null)
  let laedt = $state(false)
  let zoom = $state(1)
  let flaeche = $state<HTMLDivElement>()

  // Seitennummer im Sinne der Abschrift („Seite 2“, „Seite 2, untere Hälfte“)
  const quellen = $derived([...new Set(seiten.map((s) => s.quelle_seite))].sort((a, b) => a - b))
  const label = (s: SeiteInfo) => {
    const n = quellen.indexOf(s.quelle_seite) + 1
    return s.halbteil ? `${n} ${s.halbteil === 'oben' ? 'oben' : 'unten'}` : String(n)
  }

  $effect(() => {
    const s = seiten[aktiv]
    if (!s) {
      bild = null
      return
    }
    const vorhanden = urls.get(s.id)
    if (vorhanden) {
      bild = vorhanden
      return
    }
    laedt = true
    api
      .seitenbild(projektId, s.id)
      .then((buf) => {
        const u = URL.createObjectURL(new Blob([buf], { type: 'image/jpeg' }))
        urls.set(s.id, u)
        if (seiten[aktiv]?.id === s.id) bild = u
      })
      .catch(() => (bild = null))
      .finally(() => (laedt = false))
  })
  onDestroy(() => urls.forEach((u) => URL.revokeObjectURL(u)))

  function zoomen(f: number) {
    zoom = Math.min(4, Math.max(0.5, Math.round(zoom * f * 100) / 100))
  }
  function rad(e: WheelEvent) {
    if (!e.ctrlKey) return
    e.preventDefault()
    zoomen(e.deltaY < 0 ? 1.15 : 1 / 1.15)
  }

  // Verschieben mit gedrückter Maustaste
  let ziehen: { x: number; y: number; l: number; t: number } | null = null
  function runter(e: PointerEvent) {
    if (!flaeche || e.button !== 0) return
    ziehen = { x: e.clientX, y: e.clientY, l: flaeche.scrollLeft, t: flaeche.scrollTop }
    flaeche.setPointerCapture(e.pointerId)
  }
  function bewegen(e: PointerEvent) {
    if (!ziehen || !flaeche) return
    flaeche.scrollLeft = ziehen.l - (e.clientX - ziehen.x)
    flaeche.scrollTop = ziehen.t - (e.clientY - ziehen.y)
  }
  function hoch() {
    ziehen = null
  }
</script>

<div class="ansicht">
  <div class="leiste">
    <div class="seiten">
      {#each seiten as s, i (s.id)}
        <button class:aktiv={i === aktiv} onclick={() => (aktiv = i)} title="Seite {label(s)}">{label(s)}</button>
      {/each}
    </div>
    <div class="zoom">
      <button onclick={() => zoomen(1 / 1.25)} aria-label="verkleinern">−</button>
      <button onclick={() => (zoom = 1)} title="an Breite anpassen">{Math.round(zoom * 100)} %</button>
      <button onclick={() => zoomen(1.25)} aria-label="vergrößern">+</button>
    </div>
  </div>
  <div
    class="flaeche"
    bind:this={flaeche}
    onwheel={rad}
    onpointerdown={runter}
    onpointermove={bewegen}
    onpointerup={hoch}
    role="img"
    aria-label="Seitenbild"
  >
    {#if bild}
      <img src={bild} alt="Seite {aktiv + 1}" style="width: {zoom * 100}%" draggable="false" />
    {:else}
      <p class="hinweis">{laedt ? 'Lade Seite …' : 'Kein Seitenbild.'}</p>
    {/if}
  </div>
</div>

<style>
  .ansicht { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .leiste { display: flex; justify-content: space-between; gap: 8px; padding-bottom: 6px; flex-wrap: wrap; }
  .seiten, .zoom { display: flex; gap: 3px; flex-wrap: wrap; }
  .leiste button { padding: 2px 8px; font-size: 12px; }
  .seiten button.aktiv { background: var(--schwarz); color: var(--weiss); }
  .flaeche { flex: 1; min-height: 0; overflow: auto; border: 1px solid var(--grau-3); background: var(--grau-1); cursor: grab; }
  .flaeche:active { cursor: grabbing; }
  .flaeche img { display: block; max-width: none; user-select: none; }
  .flaeche p { padding: 12px; }
</style>
