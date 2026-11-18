<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, type GameView, type Snapshot } from "./lib/api";
  import { ago, localDate } from "./lib/time";
  import GameCard from "./components/GameCard.svelte";
  import Settings from "./components/Settings.svelte";
  import GameEditor from "./components/GameEditor.svelte";

  type View = { name: "home" } | { name: "settings" } | { name: "edit"; game: GameView | null };

  let snap = $state<Snapshot | null>(null);
  let view = $state<View>({ name: "home" });
  let now = $state(Date.now());
  let message = $state("");

  async function load() {
    snap = await api.snapshot();
    now = Date.now();
  }

  async function refresh() {
    message = await api.refreshBanners();
    setTimeout(() => (message = ""), 4000);
  }

  onMount(() => {
    load();
    // countdowns tick only while the popup is on screen
    const timer = setInterval(() => { if (!document.hidden) now = Date.now(); }, 1000);
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") api.hide(); };
    window.addEventListener("keydown", onKey);
    const unlisten = [
      listen("state-changed", load),
      listen("open-settings", () => (view = { name: "settings" })),
    ];
    return () => {
      clearInterval(timer);
      window.removeEventListener("keydown", onKey);
      unlisten.forEach((p) => p.then((off) => off()));
    };
  });

  // one card per switched-on game, soonest reset first
  const cards = $derived((snap?.games.filter((g) => g.enabled) ?? []).sort((a, b) => a.nextReset - b.nextReset));
  const feedFor = (game: GameView) => snap?.feeds.find((f) => f.key === game.feed) ?? null;
  const MISSED_SHOWN = 3; // keep the card short so the games still fit
  const gameName = (key: string) => snap?.games.find((g) => g.key === key)?.short ?? key;
  const title = $derived(view.name === "home" ? "GachaSlop" : view.name === "settings" ? "Settings"
    : view.game ? "Edit game" : "Add a game");
</script>

<main class="panel">
  <header class="bar">
    {#if view.name !== "home"}
      <button class="back" onclick={() => (view = view.name === "edit" ? { name: "settings" } : { name: "home" })}>
        ‹ {view.name === "edit" ? "Settings" : "Back"}
      </button>
    {/if}
    <h1>{title}</h1>
    {#if view.name === "home"}
      {#if snap?.settings.offline}<span class="pill warn" title="No network requests are made">Offline</span>{/if}
      <div class="actions">
        <button class="icon" title="Refresh banners" aria-label="Refresh banners"
                disabled={snap?.settings.offline} onclick={refresh}>⟳</button>
        <button class="icon" title="Settings" aria-label="Settings" onclick={() => (view = { name: "settings" })}>⚙</button>
        <button class="icon" title="Close (Esc)" aria-label="Close" onclick={() => api.hide()}>✕</button>
      </div>
    {/if}
  </header>

  {#if message}<p class="status-line" role="status">{message}</p>{/if}

  <div class="body">
    {#if !snap}
      <p class="muted">Loading…</p>
    {:else if view.name === "home"}
      {#if snap.missed.length}
        <section class="card notice" aria-label="Missed while you were away">
          <div class="row">
            <h2>While you were away</h2>
            <button class="link" onclick={() => api.dismissMissed()}>Dismiss</button>
          </div>
          {#if snap.missedSince}<p class="small muted">Since {localDate(snap.missedSince)}</p>{/if}
          <ul>
            {#each snap.missed.slice(-MISSED_SHOWN).reverse() as m (m.at + m.text)}
              <li>
                <span class="pill">{gameName(m.game)}</span>
                <span class="grow">{m.text}</span>
                <span class="muted tiny">{ago(m.at, now)}</span>
              </li>
            {/each}
          </ul>
          {#if snap.missed.length > MISSED_SHOWN}
            <p class="small muted">and {snap.missed.length - MISSED_SHOWN} more</p>
          {/if}
        </section>
      {/if}

      {#each cards as game (game.key)}
        <GameCard {game} feed={feedFor(game)} {now} offline={snap.settings.offline} />
      {:else}
        <p class="muted">All games are switched off. Turn some on in Settings.</p>
      {/each}
    {:else if view.name === "settings"}
      <Settings {snap} {now} onEdit={(game) => (view = { name: "edit", game })} />
    {:else}
      <GameEditor game={view.game} onDone={() => (view = { name: "settings" })} />
    {/if}
  </div>
</main>
