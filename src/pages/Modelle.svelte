<script lang="ts">
  import { api, fehlertext } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import { datum, kontext, preis } from '../lib/format'
  import { SLOTS, type ModelInfo, type SlotKind } from '../lib/types'

  // ---- Filter ----
  let suche = $state('')
  let nurBild = $state(false)
  let anbieter = $state('')
  let maxPreis = $state<number | null>(null)
  let sortierung = $state<'name' | 'preis' | 'neu'>('name')

  const modelle = $derived(store.models?.models ?? [])
  const anbieterListe = $derived([...new Set(modelle.map((m) => m.provider))].sort())
  const gefiltert = $derived.by(() => {
    const q = suche.trim().toLowerCase()
    let l = modelle.filter(
      (m) =>
        (!q || m.name.toLowerCase().includes(q) || m.id.toLowerCase().includes(q)) &&
        (!nurBild || m.reads_images) &&
        (!anbieter || m.provider === anbieter) &&
        (maxPreis === null || (m.price_in_per_m !== null && m.price_in_per_m <= maxPreis)),
    )
    if (sortierung === 'preis') l = [...l].sort((a, b) => (a.price_in_per_m ?? 1e9) - (b.price_in_per_m ?? 1e9))
    if (sortierung === 'neu') l = [...l].sort((a, b) => b.created - a.created)
    return l
  })
  const byId = $derived(new Map(modelle.map((m) => [m.id, m])))

  // ---- Drag & Drop (Zeiger-Ereignisse, damit das Datei-Drop von Tauri unberührt bleibt) ----
  let gezogen = $state<ModelInfo | null>(null)
  let ausgewaehlt = $state<ModelInfo | null>(null)
  let geist = $state({ x: 0, y: 0 })
  let ueberSlot = $state<SlotKind | null>(null)
  let start: { x: number; y: number; m: ModelInfo } | null = null
  let meldung = $state<{ text: string; fehler: boolean } | null>(null)

  function slotUnter(x: number, y: number): SlotKind | null {
    const el = document.elementFromPoint(x, y)?.closest('[data-slot]') as HTMLElement | null
    return (el?.dataset.slot as SlotKind) ?? null
  }

  function down(e: PointerEvent, m: ModelInfo) {
    if (e.button !== 0) return
    start = { x: e.clientX, y: e.clientY, m }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up, { once: true })
  }
  function move(e: PointerEvent) {
    if (!start) return
    if (!gezogen && Math.hypot(e.clientX - start.x, e.clientY - start.y) > 6) gezogen = start.m
    if (gezogen) {
      geist = { x: e.clientX, y: e.clientY }
      ueberSlot = slotUnter(e.clientX, e.clientY)
    }
  }
  async function up(e: PointerEvent) {
    window.removeEventListener('pointermove', move)
    const m = gezogen
    const ziel = m ? slotUnter(e.clientX, e.clientY) : null
    gezogen = null
    ueberSlot = null
    if (!m && start) {
      // einfacher Klick: Karte auswählen, dann Slot anklicken
      ausgewaehlt = ausgewaehlt?.id === start.m.id ? null : start.m
    }
    start = null
    if (m && ziel) await zuweisen(ziel, m.id)
  }

  async function zuweisen(kind: SlotKind, id: string) {
    meldung = null
    try {
      await api.validateSlotDrop(kind, id)
    } catch (e) {
      meldung = { text: fehlertext(e), fehler: true }
      return
    }
    const s = $state.snapshot(store.settings!)
    s.slots[kind].model = id
    await store.save(s)
    ausgewaehlt = null
    meldung = { text: `${byId.get(id)?.name ?? id} → ${SLOTS.find((x) => x.kind === kind)!.titel}`, fehler: false }
  }

  async function slotKlick(kind: SlotKind) {
    if (ausgewaehlt) await zuweisen(kind, ausgewaehlt.id)
  }

  async function speichern() {
    if (store.settings) await store.save(store.settings)
  }

  async function standard() {
    store.settings = await api.resetSlots()
    await store.refreshChecks()
    meldung = { text: 'Standardmodelle wiederhergestellt.', fehler: false }
  }

  const check = (k: SlotKind) => store.slotChecks.find((c) => c.kind === k)
</script>

<div class="kopf">
  <h1>Modelle</h1>
  <div class="zeile">
    <span class="hinweis">
      {#if store.modelsLoading}Lade Modellliste …{:else if store.models}Stand: {datum(store.models.fetched_at)} · {modelle.length} Modelle{/if}
    </span>
    <button onclick={() => store.refreshModels()} disabled={store.modelsLoading}>Modellliste aktualisieren</button>
  </div>
</div>

{#if store.modelsError}
  <div class="warnung" style="margin-bottom: 12px">
    Modellliste konnte nicht geladen werden: {store.modelsError}
    {#if store.models}Es wird der zwischengespeicherte Stand verwendet.{/if}
  </div>
{/if}
{#if meldung}
  <div class={meldung.fehler ? 'warnung' : 'karte grau'} style="margin-bottom: 12px; padding: 8px 12px">{meldung.text}</div>
{/if}

<div class="spalten">
  <!-- Bibliothek -->
  <section class="bibliothek">
    <div class="filter">
      <input type="search" placeholder="Modell suchen …" bind:value={suche} />
      <div class="zeile">
        <label class="zeile"><input type="checkbox" bind:checked={nurBild} /> nur Bildmodelle</label>
        <select bind:value={anbieter}>
          <option value="">alle Anbieter</option>
          {#each anbieterListe as a (a)}<option value={a}>{a}</option>{/each}
        </select>
      </div>
      <div class="zeile">
        <label class="zeile">
          Eingabe bis
          <input type="number" min="0" step="0.5" style="width: 80px" bind:value={maxPreis} placeholder="∞" />
          $ / 1 Mio.
        </label>
        <select bind:value={sortierung}>
          <option value="name">nach Name</option>
          <option value="preis">nach Preis</option>
          <option value="neu">neueste zuerst</option>
        </select>
      </div>
      <p class="hinweis">Karte auf einen Slot ziehen oder anklicken und dann den Slot anklicken.</p>
    </div>
    <div class="liste">
      {#each gefiltert as m (m.id)}
        <div
          class="modellkarte"
          class:auswahl={ausgewaehlt?.id === m.id}
          role="button"
          tabindex="0"
          onpointerdown={(e) => down(e, m)}
          onkeydown={(e) => e.key === 'Enter' && (ausgewaehlt = m)}
        >
          <div class="zeile" style="justify-content: space-between; flex-wrap: nowrap">
            <strong class="name">{m.name}</strong>
            {#if m.reads_images}<span class="marke-bild" title="kann Bilder lesen">Bild</span>{/if}
          </div>
          <div class="hinweis id">{m.id}</div>
          <div class="werte">
            <span>Ein {preis(m.price_in_per_m)}</span>
            <span>Aus {preis(m.price_out_per_m)}</span>
            <span>Kontext {kontext(m.context_length)}</span>
          </div>
        </div>
      {:else}
        <p class="hinweis">{modelle.length ? 'Keine Treffer.' : 'Noch keine Modellliste geladen.'}</p>
      {/each}
    </div>
  </section>

  <!-- Slots -->
  <section class="slots">
    {#if store.settings}
      {#each SLOTS as s (s.kind)}
        {@const cfg = store.settings.slots[s.kind]}
        {@const m = byId.get(cfg.model)}
        {@const c = check(s.kind)}
        {@const rot = c && !c.available && (s.kind !== 'zweitmeinung' || cfg.enabled)}
        <div
          class="slot karte"
          class:rot
          class:drueber={ueberSlot === s.kind}
          class:wartet={!!ausgewaehlt}
          class:aus={!cfg.enabled}
          data-slot={s.kind}
          role="button"
          tabindex="0"
          onclick={() => slotKlick(s.kind)}
          onkeydown={(e) => e.key === 'Enter' && slotKlick(s.kind)}
        >
          <div class="zeile" style="justify-content: space-between">
            <h2>{s.nr} · {s.titel}</h2>
            {#if s.kind === 'zweitmeinung'}
              <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
              <label class="zeile" onclick={(e) => e.stopPropagation()}>
                <input type="checkbox" bind:checked={cfg.enabled} onchange={speichern} /> aktiv
              </label>
            {/if}
          </div>
          <p class="hinweis">{s.aufgabe}</p>
          <div class="belegt">
            <strong>{m?.name ?? cfg.model}</strong>
            <div class="hinweis id">{cfg.model}</div>
            {#if m}
              <div class="werte">
                <span>Ein {preis(m.price_in_per_m)}</span><span>Aus {preis(m.price_out_per_m)}</span>
                {#if m.reads_images}<span>liest Bilder</span>{/if}
                {#if m.supports_structured}<span>JSON-Schema</span>{/if}
              </div>
            {/if}
          </div>
          {#if rot}
            <div class="warnung" style="margin-top: 8px">
              {c!.problem}
              {#if c!.suggestion}
                <div class="zeile" style="margin-top: 6px">
                  Vorschlag: <strong>{byId.get(c!.suggestion)?.name ?? c!.suggestion}</strong>
                  <button onclick={(e) => { e.stopPropagation(); zuweisen(s.kind, c!.suggestion!) }}>Übernehmen</button>
                </div>
              {/if}
            </div>
          {/if}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="optionen" onclick={(e) => e.stopPropagation()}>
            <label>
              Reasoning
              <select bind:value={cfg.reasoning} onchange={speichern} disabled={!!m && !m.supports_reasoning}>
                <option value="standard">Standard</option>
                <option value="niedrig">niedrig</option>
                <option value="mittel">mittel</option>
                <option value="hoch">hoch</option>
              </select>
            </label>
            <label>
              Temperatur
              <input type="number" min="0" max="2" step="0.1" bind:value={cfg.temperature} onchange={speichern}
                disabled={!!m && !m.supports_temperature} style="width: 70px" />
            </label>
            <label>
              max. Tokens
              <input type="number" min="256" step="256" bind:value={cfg.max_tokens} onchange={speichern} style="width: 90px" />
            </label>
          </div>
        </div>
      {/each}
      <div class="zeile"><button onclick={standard}>Standard wiederherstellen</button></div>
    {/if}
  </section>
</div>

{#if gezogen}
  <div class="geist" style="left: {geist.x + 12}px; top: {geist.y + 12}px">{gezogen.name}</div>
{/if}

<style>
  .kopf { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; flex-wrap: wrap; }
  .spalten { display: grid; grid-template-columns: minmax(320px, 420px) 1fr; gap: 20px; align-items: start; }
  .bibliothek { border: 1px solid var(--schwarz); border-radius: var(--radius); display: flex; flex-direction: column; max-height: calc(100vh - 150px); }
  .filter { padding: 12px; border-bottom: 1px solid var(--grau-2); display: flex; flex-direction: column; gap: 8px; }
  .filter input[type='search'] { width: 100%; }
  .filter .hinweis { margin: 0; }
  .liste { overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 6px; }
  .modellkarte {
    border: 1px solid var(--grau-3); border-radius: var(--radius); padding: 8px 10px;
    cursor: grab; user-select: none; touch-action: none; background: var(--weiss);
  }
  .modellkarte:hover { border-color: var(--schwarz); }
  .modellkarte.auswahl { border: 2px solid var(--schwarz); background: var(--grau-1); }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .id { font-size: 12px; overflow-wrap: anywhere; }
  .werte { display: flex; gap: 10px; flex-wrap: wrap; font-size: 12px; margin-top: 4px; }
  .marke-bild { font-size: 11px; border: 1px solid var(--schwarz); border-radius: 3px; padding: 0 5px; flex-shrink: 0; }
  .slots { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 14px; }
  .slots > .zeile { grid-column: 1 / -1; }
  .slot { cursor: default; }
  .slot.wartet { cursor: pointer; border-style: dashed; }
  .slot.drueber { background: var(--grau-1); border-width: 2px; }
  .slot.rot { border-color: var(--akzent); }
  .slot.aus { opacity: 0.6; }
  .belegt { border-top: 1px solid var(--grau-2); padding-top: 8px; }
  .optionen { display: flex; gap: 10px; flex-wrap: wrap; margin-top: 10px; font-size: 12px; }
  .optionen label { display: flex; flex-direction: column; gap: 2px; }
  .geist {
    position: fixed; pointer-events: none; z-index: 100; background: var(--schwarz); color: var(--weiss);
    padding: 6px 10px; border-radius: var(--radius); font-size: 13px; box-shadow: 0 2px 6px rgba(0,0,0,.25);
  }
</style>
