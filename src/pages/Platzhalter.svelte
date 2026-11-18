<script lang="ts">
  let { titel, meilenstein }: { titel: string; meilenstein: number } = $props()
</script>

<h1>{titel}</h1>
<div class="karte grau">
  <p>Dieser Bereich folgt in Meilenstein {meilenstein}.</p>
</div>
