<script lang="ts">
  import { fileUrl, type FeedView, type GameView, type PickView } from "../lib/api";
  import { ago, left, localDate, localTime, until } from "../lib/time";
  import GameBadge from "./GameBadge.svelte";

  // One game: its resets, and its banners if it has a feed (built-in games only).
  let { game, feed = null, now, offline }: { game: GameView; feed?: FeedView | null; now: number; offline: boolean } =
    $props();

  const live = $derived(feed?.rotations.filter((r) => r.start * 1000 <= now && now < r.end * 1000) ?? []);
  const upcoming = $derived(feed?.rotations.filter((r) => r.start * 1000 > now) ?? []);
  const status = $derived.by(() => {
    if (!feed) return "";
    const parts = [`Banners updated ${ago(feed.fetchedAt, now)}`];
    if (offline) parts.push("offline");
    else if (feed.lastError) parts.push("couldn't check");
    return parts.join(" · ");
  });
  const label = (p: PickView) => (p.marks ? `${p.name} ${p.marks}` : p.name);
  const first = (name: string) => name.split(" (")[0]; // "Iroha (Swimsuit)" under a 40px icon: "Iroha"
</script>

{#snippet icon(p: PickView, size: "big" | "small")}
  {#if p.iconPath}
    <img class="pick {size}" src={fileUrl(p.iconPath)} alt={p.name} title={label(p)} />
  {:else}
    <span class="pick {size} placeholder" title={label(p)}>{p.name.slice(0, 2)}</span>
  {/if}
{/snippet}

<article class="card game">
  <header class="row">
    <GameBadge {game} />
    <div class="grow">
      <div class="name">{game.name}</div>
      {#if game.nextWeekly}
        <div class="muted small">Weekly {until(game.nextWeekly, now)} · {localTime(game.nextWeekly, now)}</div>
      {/if}
    </div>
    <div class="when" title="Daily reset">
      <div class="countdown big">{left(game.nextReset, now)}</div>
      <div class="muted small">{localTime(game.nextReset, now)}</div>
    </div>
  </header>

  {#if feed}
    {#each live as rot (rot.key)}
      <div class="banner">
        <div class="picks">
          {#each rot.picks as p (p.name)}
            <figure>
              {@render icon(p, "big")}
              <figcaption title={label(p)}>{first(p.name)}{p.marks ? ` ${p.marks}` : ""}</figcaption>
            </figure>
          {/each}
          {#each rot.extras as p (p.name)}
            <figure class="extra">{@render icon(p, "small")}</figure>
          {/each}
        </div>
        <div class="ends">
          <div class="muted small">Ends in</div>
          <div class="countdown">{left(rot.end, now)}</div>
          {#if !rot.confirmed}<span class="pill" title="Projected from the JP schedule; not confirmed yet">Expected</span>{/if}
        </div>
      </div>
      {#each rot.details as line}<div class="muted small">{line}</div>{/each}
    {:else}
      {#if !feed.fetchedAt}
        <p class="muted small">{offline ? "No banner data yet. Turn off offline mode to fetch it." : "No banner data yet."}</p>
      {/if}
    {/each}

    {#if upcoming.length}
      <details class="upcoming">
        <summary>Upcoming ({upcoming.length})</summary>
        <ul>
          {#each upcoming as rot (rot.key)}
            <li>
              <div class="mini-picks">{#each rot.picks as p (p.name)}{@render icon(p, "small")}{/each}</div>
              <div class="grow">
                <div class="small">{rot.picks.map(label).join(", ")}</div>
                <div class="muted small">{localDate(rot.start)} · <span class="countdown">{until(rot.start, now)}</span></div>
              </div>
              {#if feed.projected && !rot.confirmed}<span class="pill" title="Projected from the JP schedule">Expected</span>{/if}
            </li>
          {/each}
        </ul>
      </details>
    {:else if feed.nextUnknownFrom}
      <p class="muted small">Next banners from {localDate(feed.nextUnknownFrom)}. Characters not announced yet.</p>
    {/if}

    <p class="foot" class:warn={offline || feed.lastError} title={feed.legend || undefined}>{status}</p>
  {/if}
</article>
