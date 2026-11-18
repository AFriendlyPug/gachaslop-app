<script lang="ts">
  import { builtinIcon, fileUrl, type GameView } from "../lib/api";

  let { game, src = null, small = false }: { game: GameView; src?: string | null; small?: boolean } = $props();
  // a picked-but-unsaved image, the game's own icon, or the letters as a fallback
  const url = $derived(src ?? (game.builtin ? builtinIcon(game.key) : fileUrl(game.iconPath)));
  const initials = $derived(game.short.length <= 3 ? game.short : game.short.slice(0, 2).toUpperCase());
</script>

{#if url}
  <img class="badge" class:small src={url} alt="" />
{:else}
  <span class="badge" class:small style:background={game.color} aria-hidden="true">{initials}</span>
{/if}
