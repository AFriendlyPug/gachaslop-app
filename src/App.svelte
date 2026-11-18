<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, type GameView, type Snapshot } from "./lib/api";
  import { ago } from "./lib/time";
  import GameCard from "./components/GameCard.svelte";
  import Settings from "./components/Settings.svelte";
  import GameEditor from "./components/GameEditor.svelte";
  import Notifications from "./components/Notifications.svelte";
  import UpdateBanner from "./components/UpdateBanner.svelte";

  type View = { name: "home" } | { name: "settings" } | { name: "notifications" } | { name: "edit"; game: GameView | null };

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

  // ---- height: fits what's shown, unless the user dragged the top edge to a height of their own ----
  let header: HTMLElement;
  let content: HTMLElement;
  let lastFit = 0;

  // The window fits the home screen's sections, with the upcoming-banner dropdowns counted closed: opening
  // one scrolls the cards instead of growing the window. Every other view keeps that height and scrolls.
  function fit() {
    if (!content || !header || view.name !== "home") return;
    const body = content.parentElement!;
    const padding = parseFloat(getComputedStyle(body).paddingTop) + parseFloat(getComputedStyle(body).paddingBottom);
    const status = body.previousElementSibling?.classList.contains("status-line") ? (body.previousElementSibling as HTMLElement).offsetHeight : 0;
    const dropdowns = [...content.querySelectorAll<HTMLDetailsElement>("details.upcoming[open]")]
      .reduce((sum, d) => sum + d.offsetHeight - (d.querySelector("summary") as HTMLElement).offsetHeight, 0);
    const height = Math.ceil(header.offsetHeight + status + content.offsetHeight - dropdowns + padding + 2); // + border
    if (Math.abs(height - lastFit) >= 1) {
      lastFit = height;
      api.fitPopup(height);
    }
  }

  $effect(() => {
    if (!content) return;
    const watch = new ResizeObserver(fit);
    watch.observe(content);
    watch.observe(header);
    return () => watch.disconnect();
  });

  // the tray menu's Settings: asked before this window existed, or told while it's open
  const openPending = () => api.takeView().then((v) => { if (v) view = { name: v }; });

  onMount(() => {
    load();
    openPending();
    // countdowns tick only while the popup is on screen
    const timer = setInterval(() => { if (!document.hidden) now = Date.now(); }, 1000);
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") api.hide(); };
    window.addEventListener("keydown", onKey);
    const unlisten = [
      listen("state-changed", load),
      listen("open-settings", openPending),
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
  // the home screen lists new notifications in full (a few, so the games still fit); once read, just the latest
  const NEW_SHOWN = 3;
  const fresh = $derived(snap?.notices.filter((n) => n.new) ?? []);
  const gameName = (key: string) => snap?.games.find((g) => g.key === key)?.short ?? key;
  const title = $derived({ home: "GachaSlop", settings: "Settings", notifications: "Notifications",
    edit: view.name === "edit" && view.game ? "Edit game" : "Add a game" }[view.name]);
</script>

<main class="panel">
  <header class="bar" bind:this={header}>
    {#if view.name !== "home"}
      <button class="back" onclick={() => (view = view.name === "edit" ? { name: "settings" } : { name: "home" })}>
        ‹ {view.name === "edit" ? "Settings" : "Back"}
      </button>
    {/if}
    <h1>{title}</h1>
    {#if view.name === "home"}
      {#if snap?.settings.offline}<span class="pill warn" title="No network requests are made">Offline</span>{/if}
      <div class="actions">
        <button class="icon bell" title="Notifications" aria-label={`Notifications${snap?.newNotices ? `, ${snap.newNotices} new` : ""}`}
                onclick={() => (view = { name: "notifications" })}>
          <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true" fill="none" stroke="currentColor"
               stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15z" /><path d="M10 20.5a2 2 0 0 0 4 0" />
          </svg>
          {#if snap?.newNotices}<span class="count-badge">{snap.newNotices}</span>{/if}
        </button>
        <button class="icon" title="Refresh banners" aria-label="Refresh banners"
                disabled={snap?.settings.offline} onclick={refresh}>⟳</button>
        <button class="icon" title="Settings" aria-label="Settings" onclick={() => (view = { name: "settings" })}>⚙</button>
        <button class="icon" title="Close (Esc)" aria-label="Close" onclick={() => api.hide()}>✕</button>
      </div>
    {/if}
  </header>

  {#if message}<p class="status-line" role="status">{message}</p>{/if}

  <div class="body">
    <div class="content" bind:this={content}>
    {#if !snap}
      <p class="muted">Loading…</p>
    {:else if view.name === "home"}
      {#if snap.update && !snap.update.dismissed}
        <UpdateBanner update={snap.update} current={snap.version} offline={snap.settings.offline} dismissible />
      {/if}
      {#if fresh.length}
        <button class="card notice-home" onclick={() => (view = { name: "notifications" })}>
          <span class="row">
            <span class="notice-head">Notifications <span class="pill">{fresh.length} new</span></span>
            <span class="see-all">See all ›</span>
          </span>
          <ul>
            {#each fresh.slice(0, NEW_SHOWN) as n (n.at + n.title + n.added)}
              <li>
                <span class="pill">{gameName(n.game)}</span>
                <span class="grow ellipsis">{n.title}</span>
                {#if n.missed}<span class="tiny secondary">missed</span>{/if}
                <span class="tiny muted">{ago(n.at, now)}</span>
              </li>
            {/each}
          </ul>
          {#if fresh.length > NEW_SHOWN}
            <span class="small secondary">{fresh.length - NEW_SHOWN} more new in the list</span>
          {/if}
        </button>
      {:else if snap.notices.length}
        {@const latest = snap.notices[0]}
        <button class="card notice-home compact" onclick={() => (view = { name: "notifications" })}>
          <span class="notice-head">Notifications</span>
          <span class="grow ellipsis secondary small">{latest.title}</span>
          <span class="tiny muted">{ago(latest.at, now)}</span>
          <span class="see-all">›</span>
        </button>
      {/if}

      {#each cards as game (game.key)}
        <GameCard {game} feed={feedFor(game)} {now} offline={snap.settings.offline} />
      {:else}
        <p class="muted">All games are switched off. Turn some on in Settings.</p>
      {/each}
    {:else if view.name === "notifications"}
      <Notifications {snap} {now} />
    {:else if view.name === "settings"}
      <Settings {snap} {now} onEdit={(game) => (view = { name: "edit", game })} />
    {:else}
      <GameEditor game={view.game} onDone={() => (view = { name: "settings" })} />
    {/if}
    </div>
  </div>
</main>
