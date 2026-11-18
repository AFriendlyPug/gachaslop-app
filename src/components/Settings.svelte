<script lang="ts">
  import { api, type Diagnostics, type GameView, type Sample, type Settings, type Snapshot } from "../lib/api";
  import { ago, resetText, until } from "../lib/time";
  import GameBadge from "./GameBadge.svelte";

  let { snap, now, onEdit }: { snap: Snapshot; now: number; onEdit: (game: GameView | null) => void } = $props();
  let error = $state("");

  async function save(change: Partial<Settings>) {
    error = "";
    try {
      await api.saveSettings({ ...snap.settings, ...change });
    } catch (e) {
      error = String(e);
    }
  }

  function toggleGame(key: string, on: boolean) {
    const others = snap.settings.disabledGames.filter((k) => k !== key);
    save({ disabledGames: on ? others : [...others, key] });
  }

  let confirming = $state<string | null>(null); // a native confirm box would take focus and hide the popup

  async function remove(game: GameView) {
    if (confirming !== game.key) {
      confirming = game.key;
      setTimeout(() => { if (confirming === game.key) confirming = null; }, 4000);
      return;
    }
    confirming = null;
    await api.deleteGame(game.key);
  }

  // ---- Testing: try notifications, catch-up and the tray icon without waiting for them ----
  const SAMPLES: [Sample, string][] = [
    ["reset", "Reset"], ["weekly", "Weekly reset"], ["headsUp", "Heads-up"],
    ["bannerLive", "New banner"], ["bannerEnding", "Banner ending"], ["summary", "Summary"],
  ];
  let testMessage = $state("");
  let diag = $state<Diagnostics | null>(null);

  async function check() {
    diag = await api.diagnostics();
  }

  async function tryIt(action: () => Promise<string | void>, done = "") {
    try {
      testMessage = (await action()) || done;
    } catch (e) {
      testMessage = String(e);
    }
    setTimeout(check, 1500); // the widget changes on its next tick
  }

  const widgetText = (d: Diagnostics) =>
    d.widget.strips
      ? `on ${d.widget.strips} taskbar${d.widget.strips === 1 ? "" : "s"}${d.widget.trayShown ? ", tray icon also shown" : ""}`
      : d.widget.trayShown ? "not shown, using the tray icon" : "not shown";
</script>

{#snippet toggle(name: string, hint: string, checked: boolean, onchange: (on: boolean) => void)}
  <label class="setting">
    <span class="grow">
      <span class="setting-name block">{name}</span>
      <span class="setting-hint block">{hint}</span>
    </span>
    <input type="checkbox" class="switch" {checked} onchange={(e) => onchange(e.currentTarget.checked)} />
  </label>
{/snippet}

<section class="group">
  <h2>Games</h2>
  <ul class="list">
    {#each snap.games as game (game.key)}
      <li>
        <GameBadge {game} small />
        <div class="grow">
          <div class="setting-name">{game.name}</div>
          <div class="setting-hint">{resetText(game.hour, game.minute, game.weeklyDay, now)}{game.feed ? ". Shows banners." : ""}</div>
          {#if !game.builtin}
            <div class="row left">
              <button class="link" onclick={() => onEdit(game)}>Edit</button>
              <button class="link danger" onclick={() => remove(game)}>{confirming === game.key ? "Sure? Click again" : "Delete"}</button>
            </div>
          {/if}
        </div>
        <input type="checkbox" class="switch" aria-label={`Show ${game.name}`} checked={game.enabled}
               onchange={(e) => toggleGame(game.key, e.currentTarget.checked)} />
      </li>
    {/each}
  </ul>
  <button class="wide" onclick={() => onEdit(null)}>+ Add a game</button>
</section>

<section class="group">
  <h2>Notifications</h2>
  <p class="small muted">Cards appear above the taskbar clock for a few seconds. While a full-screen game is open they wait until you're back.</p>
  {@render toggle("Resets", "When a game's daily or weekly reset happens.", snap.settings.resetToasts,
                  (on) => save({ resetToasts: on }))}
  <label class="setting">
    <span class="grow">
      <span class="setting-name block">Heads-up before resets</span>
      <span class="setting-hint block">Minutes before each reset. 0 turns it off.</span>
    </span>
    <input type="number" min="0" max="720" step="5" value={snap.settings.headsupMinutes} aria-label="Minutes before resets"
           onchange={(e) => save({ headsupMinutes: Math.max(0, Math.min(720, Number(e.currentTarget.value) || 0)) })} />
  </label>
  {@render toggle("Banners", "When a new banner starts, and a day before one ends.", snap.settings.bannerToasts,
                  (on) => save({ bannerToasts: on }))}
  {@render toggle("Catch-up", "Show what you missed while the app was closed or the PC was asleep.",
                  snap.settings.catchup, (on) => save({ catchup: on }))}
</section>

<section class="group">
  <h2>App</h2>
  {@render toggle("Offline mode", "Make no network requests. Stored banners and icons still show.",
                  snap.settings.offline, (on) => save({ offline: on }))}
  {@render toggle("Start with Windows", "Starts on the taskbar when you sign in.", snap.settings.autostart,
                  (on) => save({ autostart: on }))}
  {#if error}<p class="error">{error}</p>{/if}
</section>

<section class="group">
  <h2>Network</h2>
  <div class="card small">
    <p>Requests this month: <strong>{snap.requestsThisMonth}</strong></p>
    {#each snap.feeds as feed (feed.key)}
      <p class="secondary">
        {feed.name}: checked {ago(feed.checkedAt, now)}{!feed.enabled ? " (game switched off)"
          : feed.nextCheck && !snap.settings.offline ? `, next check ${until(feed.nextCheck, now)}` : ""}
      </p>
    {/each}
  </div>
</section>

<section class="group">
  <h2>Data sources</h2>
  <div class="card small secondary">
    {#each snap.feeds as feed (feed.key)}<p>{feed.name}: {feed.credit}</p>{/each}
    <p>Game icons: App Store icons by NEXON Korea and COGNOSPHERE.</p>
  </div>
</section>

<details class="testing" ontoggle={(e) => e.currentTarget.open && check()}>
  <summary>Testing</summary>
  <p class="small muted">Try things now instead of waiting for them. Nothing here uses the network.</p>

  <h3>Notification cards</h3>
  <p class="small muted">Made from your games and stored banners, and marked [Test]. They don't count as shown.</p>
  <div class="buttons">
    {#each SAMPLES as [sample, text] (sample)}
      <button onclick={() => tryIt(() => api.testNotification(sample))}>{text}</button>
    {/each}
  </div>

  <h3>Catch-up</h3>
  <p class="small muted">Shows everything due now as if you'd just come back: its cards (following the settings above),
    the "While you were away" list and the red dot on the widget.</p>
  <div class="buttons">
    <button onclick={() => tryIt(api.replayCatchup, "Replaying. Go back to see the list.")}>Replay catch-up</button>
  </div>

  <h3>Taskbar</h3>
  {@render toggle("Use the tray icon instead", "Try the fallback used when the taskbar auto-hides. Until the app quits.",
                  diag?.widget.trayOnly ?? false, (on) => tryIt(() => api.setTrayOnly(on)))}
  {#if diag}
    <p class="small secondary">Widget: {widgetText(diag)}. Notifications remembered: {diag.remembered}.
      Banner icons stored: {diag.icons}.</p>
  {/if}

  <h3>Files</h3>
  <div class="buttons">
    <button onclick={() => tryIt(() => api.openPath("data"))}>Open data folder</button>
    <button onclick={() => tryIt(() => api.openPath("log"))}>Open log</button>
  </div>
  {#if testMessage}<p class="small" role="status">{testMessage}</p>{/if}
</details>

<button class="wide danger" onclick={() => api.quit()}>Quit GachaSlop</button>
