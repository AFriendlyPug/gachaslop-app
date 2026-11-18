<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { api, type GameView, type Notice, type Snapshot } from "../lib/api";
  import { clock, dayLabel } from "../lib/time";
  import GameBadge from "./GameBadge.svelte";

  // Every notification that came in, newest first, grouped by the day it happened.
  let { snap, now }: { snap: Snapshot; now: number } = $props();

  const key = (n: Notice) => `${n.at}|${n.added}|${n.game}|${n.title}`;
  // opening the list marks everything read: remember what was new so it stays highlighted while it's open
  const fresh = new Set(untrack(() => snap.notices.filter((n) => n.new).map(key)));
  onMount(() => { if (fresh.size) api.markNoticesRead(); });

  // filter by game: the games that have notifications in the list
  let only = $state<string | null>(null);
  const gamesListed = $derived([...new Set(snap.notices.map((n) => n.game))]);
  const shown = $derived(only ? snap.notices.filter((n) => n.game === only) : snap.notices);

  const groups = $derived.by(() => {
    const out: { day: string; notices: Notice[] }[] = [];
    for (const n of shown) {
      const day = dayLabel(n.at, now);
      if (out.at(-1)?.day !== day) out.push({ day, notices: [] });
      out.at(-1)!.notices.push(n);
    }
    return out;
  });
  const gameFor = (n: Notice): GameView | null => snap.games.find((g) => g.key === n.game) ?? null;

  let confirming = $state(false); // a native confirm box would take focus and hide the popup
  function clearAll() {
    if (!confirming) {
      confirming = true;
      setTimeout(() => (confirming = false), 4000);
      return;
    }
    confirming = false;
    api.clearNotices();
  }
</script>

{#if snap.notices.length}
  {#if gamesListed.length > 1}
    <div class="chips" role="group" aria-label="Show notifications from">
      <button class="chip" aria-pressed={only === null} onclick={() => (only = null)}>All</button>
      {#each gamesListed as key (key)}
        <button class="chip" aria-pressed={only === key} onclick={() => (only = key)}>
          {snap.games.find((g) => g.key === key)?.short ?? key}
        </button>
      {/each}
    </div>
  {/if}
  {#each groups as group (group.day)}
    <section class="group" aria-label={group.day}>
      <h2>{group.day}</h2>
      <ul class="notices">
        {#each group.notices as n (key(n))}
          {@const game = gameFor(n)}
          <li class:fresh={fresh.has(key(n))}>
            {#if game}<GameBadge {game} small />{:else}<span class="badge small" aria-hidden="true">?</span>{/if}
            <div class="grow">
              <div class="notice-title">{n.title}</div>
              {#if n.body}<div class="notice-body">{n.body}</div>{/if}
            </div>
            <div class="notice-when">
              <span class="small secondary">{clock(n.at)}</span>
              {#if n.missed}<span class="pill" title="Happened while the app was closed or the PC was asleep">Missed</span>{/if}
              {#if fresh.has(key(n))}<span class="pill new">New</span>{/if}
            </div>
          </li>
        {/each}
      </ul>
    </section>
  {/each}
  <p class="small muted">Kept for 30 days.</p>
  <button class="wide danger" onclick={clearAll}>{confirming ? "Sure? Click again to clear" : "Clear all"}</button>
{:else}
  <div class="card empty">
    <p class="setting-name">No notifications yet</p>
    <p class="small muted">Resets, heads-ups and banners you're notified about will be listed here, newest first.</p>
  </div>
{/if}
