<script lang="ts">
  import { fileUrl, type FeedView, type GameView, type PickView, type RotationView } from "../lib/api";
  import { ago, lasts, left, localDate, localTime, roughlyUntil } from "../lib/time";
  import GameBadge from "./GameBadge.svelte";

  // One game: its resets, and its banners if it has a feed (built-in games only).
  let { game, feed = null, now, offline }: { game: GameView; feed?: FeedView | null; now: number; offline: boolean } =
    $props();

  // "Blue Archive (Global)" is shown as "Blue Archive" with "Global" underneath
  const title = $derived(game.name.replace(/\s*\(([^)]*)\)$/, ""));
  const server = $derived(game.name.match(/\(([^)]*)\)$/)?.[1] ?? "");

  const live = $derived(feed?.rotations.filter((r) => r.start * 1000 <= now && now < r.end * 1000) ?? []);
  const upcoming = $derived(feed?.rotations.filter((r) => r.start * 1000 > now) ?? []);
  const next = $derived(upcoming[0] ?? null);
  const warn = $derived(Boolean(offline || feed?.lastError));
  const status = $derived.by(() => {
    if (!feed) return "";
    if (offline) return `Offline · updated ${ago(feed.fetchedAt, now)}`;
    if (feed.lastError) return `Couldn't check · updated ${ago(feed.fetchedAt, now)}`;
    return `Updated ${ago(feed.fetchedAt, now)}`;
  });
  const engineTip = (name: string) => `W-Engine: ${name}`;
  // hovering an agent: "Severian · W-Engine: Evil Caught Within"
  const tip = (p: PickView) => [p.name, ...p.tags, ...(p.engine ? [engineTip(p.engine.name)] : [])].join(" · ");
  const PREVIEW = 4; // icons of the next banner shown on the closed dropdown
  // "Hasumi (Swimsuit)" is shown as "Hasumi" with "Swimsuit · Limited" underneath, so names aren't cut off
  const outfit = (name: string) => name.match(/^(.*?)\s*\(([^)]*)\)$/)?.slice(1, 3) ?? [name, ""];
  const below = (p: PickView) => [outfit(p.name)[1], ...p.tags].filter(Boolean).join(" · ");
</script>

{#snippet icon(p: PickView, size: "big" | "mid" | "small" | "engine", hint: string)}
  {#if p.iconPath}
    <img class="pick-icon {size}" src={fileUrl(p.iconPath)} alt={p.name} title={hint} />
  {:else}
    <span class="pick-icon {size} placeholder" title={hint}>{p.name.slice(0, 2)}</span>
  {/if}
{/snippet}

<!-- a banner's characters: icon (with a ZZZ agent's W-Engine on its corner), name, and tags or W-Engine -->
{#snippet picks(rot: RotationView, size: "big" | "mid")}
  <ul class="picks" class:compact={size === "mid"}>
    {#each rot.picks as p (p.name)}
      <li class="pick" title={tip(p)}>
        <span class="pick-slot">
          {@render icon(p, size, tip(p))}
          {#if p.engine}{@render icon(p.engine, "engine", engineTip(p.engine.name))}{/if}
        </span>
        <span class="grow">
          <span class="pick-name ellipsis block">{outfit(p.name)[0]}</span>
          {#if p.engine}
            <span class="pick-sub ellipsis block">{p.engine.name}</span>
          {:else if below(p)}
            <span class="pick-sub ellipsis block">{below(p)}</span>
          {/if}
        </span>
      </li>
    {/each}
    {#each rot.extras as p (p.name)}
      <li class="pick" title={engineTip(p.name)}>
        {@render icon(p, size, engineTip(p.name))}
        <span class="grow">
          <span class="pick-name ellipsis block">{p.name}</span>
          <span class="pick-sub">W-Engine</span>
        </span>
      </li>
    {/each}
  </ul>
{/snippet}

<article class="card game" aria-label={game.name}>
  <header class="game-head">
    <GameBadge {game} />
    <div class="grow">
      <h2 class="game-name">{title}</h2>
      {#if server}<div class="small secondary">{server}</div>{/if}
    </div>
    <div class="reset">
      <div class="label">Daily reset · {localTime(game.nextReset, now)}</div>
      <div class="countdown big">{left(game.nextReset, now)}</div>
    </div>
  </header>

  {#if game.nextWeekly}
    <div class="fact">
      <span class="label">Weekly reset</span>
      <span class="value small">{localTime(game.nextWeekly, now)} · <span class="countdown">{left(game.nextWeekly, now)}</span></span>
    </div>
  {/if}

  {#if feed}
    <section class="banner" aria-label="Banners">
      {#each live as rot (rot.key)}
        <div class="fact">
          <span class="label">
            Current banner
            {#if !rot.confirmed}<span class="pill" title="Projected from the JP schedule, not confirmed yet">Expected</span>{/if}
          </span>
          <span class="value small">Ends in <span class="countdown">{left(rot.end, now)}</span></span>
        </div>
        {@render picks(rot, "big")}
        {#each rot.details as line}<p class="details">{line}</p>{/each}
      {:else}
        <p class="small muted">
          {#if !feed.fetchedAt}
            {offline ? "No banner data yet. Turn off offline mode to fetch it." : "No banner data yet."}
          {:else if feed.nextUnknownFrom}
            Next banners from {localDate(feed.nextUnknownFrom)}. Not announced yet.
          {:else}
            No banner on right now.
          {/if}
        </p>
      {/each}

      {#if next}
        <details class="upcoming">
          <summary>
            <span class="open">Upcoming <span class="count">{upcoming.length}</span></span>
            <span class="next" title={`Next: ${next.picks.map((p) => p.name).join(", ")}`}>
              <span class="mini-picks" aria-hidden="true">
                {#each next.picks.slice(0, PREVIEW) as p (p.name)}{@render icon(p, "small", tip(p))}{/each}
              </span>
              <span class="small">Next {roughlyUntil(next.start, now)}</span>
            </span>
          </summary>
          {#if feed.projected}
            <p class="note">Dates are projected from the JP server's schedule and may move.</p>
          {/if}
          <ol class="timeline">
            {#each upcoming as rot (rot.key)}
              <li>
                <div class="when">
                  <span><strong>{localDate(rot.start)}</strong> <span class="muted">· {lasts(rot.start, rot.end)}</span></span>
                  <span class="small secondary">{roughlyUntil(rot.start, now)}</span>
                </div>
                {@render picks(rot, "mid")}
              </li>
            {/each}
          </ol>
          {#if !warn}<p class="foot">{status}</p>{/if}
        </details>
        {#if warn}<p class="foot warn">{status}</p>{/if}
      {:else}
        <div class="banner-foot">
          <span class="secondary">{live.length && feed.nextUnknownFrom ? `Next from ${localDate(feed.nextUnknownFrom)}` : ""}</span>
          <span class="foot" class:warn>{status}</span>
        </div>
      {/if}
    </section>
  {/if}
</article>
