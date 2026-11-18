<script lang="ts">
  import type { Snippet } from 'svelte'

  let {
    titel,
    offen = $bindable(false),
    children,
    aktionen,
  }: { titel: string; offen: boolean; children: Snippet; aktionen?: Snippet } = $props()

  function taste(e: KeyboardEvent) {
    if (offen && e.key === 'Escape') offen = false
  }
</script>

<svelte:window onkeydown={taste} />

{#if offen}
  <div class="schleier" role="presentation" onclick={(e) => e.target === e.currentTarget && (offen = false)}>
    <div class="karte dialog" role="dialog" aria-modal="true" aria-label={titel}>
      <h2>{titel}</h2>
      {@render children()}
      {#if aktionen}
        <div class="zeile aktionen">{@render aktionen()}</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .schleier { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.35); display: grid; place-items: center; z-index: 60; }
  .dialog { width: min(560px, calc(100vw - 32px)); max-height: calc(100vh - 64px); overflow: auto; }
  .aktionen { justify-content: flex-end; margin-top: 16px; }
</style>
