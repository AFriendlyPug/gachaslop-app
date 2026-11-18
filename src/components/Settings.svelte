<script lang="ts">
  import { api, type GameView, type Settings, type Snapshot } from "../lib/api";
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
</script>

<section>
  <h2>Games</h2>
  <ul class="games">
    {#each snap.games as game (game.key)}
      <li>
        <GameBadge {game} />
        <div class="grow">
          <div class="name">{game.name}</div>
          <div class="muted small">{resetText(game.hour, game.minute, game.weeklyDay, now)}{game.feed ? " · banners" : ""}</div>
        </div>
        {#if !game.builtin}
          <button class="link" onclick={() => onEdit(game)}>Edit</button>
          <button class="link danger" onclick={() => remove(game)}>{confirming === game.key ? "Sure?" : "Delete"}</button>
        {/if}
        <input type="checkbox" class="switch" aria-label={`Track ${game.name}`} checked={game.enabled}
               onchange={(e) => toggleGame(game.key, e.currentTarget.checked)} />
      </li>
    {/each}
  </ul>
  <button class="wide" onclick={() => onEdit(null)}>+ Add a game</button>
</section>

<section>
  <h2>Notifications</h2>
  <label class="setting">
    <span>Reset notifications</span>
    <input type="checkbox" class="switch" checked={snap.settings.resetToasts}
           onchange={(e) => save({ resetToasts: e.currentTarget.checked })} />
  </label>
  <label class="setting">
    <span>Heads-up before resets <span class="muted small">(minutes, 0 = off)</span></span>
    <input type="number" min="0" max="720" step="5" value={snap.settings.headsupMinutes}
           onchange={(e) => save({ headsupMinutes: Math.max(0, Math.min(720, Number(e.currentTarget.value) || 0)) })} />
  </label>
  <label class="setting">
    <span>Banner notifications</span>
    <input type="checkbox" class="switch" checked={snap.settings.bannerToasts}
           onchange={(e) => save({ bannerToasts: e.currentTarget.checked })} />
  </label>
  <label class="setting">
    <span>Catch-up notifications on load <span class="muted small">(what you missed while closed)</span></span>
    <input type="checkbox" class="switch" checked={snap.settings.catchup}
           onchange={(e) => save({ catchup: e.currentTarget.checked })} />
  </label>
</section>

<section>
  <h2>App</h2>
  <label class="setting">
    <span>Offline mode <span class="muted small">(no network requests)</span></span>
    <input type="checkbox" class="switch" checked={snap.settings.offline}
           onchange={(e) => save({ offline: e.currentTarget.checked })} />
  </label>
  <label class="setting">
    <span>Start with Windows</span>
    <input type="checkbox" class="switch" checked={snap.settings.autostart}
           onchange={(e) => save({ autostart: e.currentTarget.checked })} />
  </label>
  {#if error}<p class="error">{error}</p>{/if}
</section>

<section>
  <h2>Network</h2>
  <p class="small">Requests this month: <strong>{snap.requestsThisMonth}</strong></p>
  {#each snap.feeds as feed (feed.key)}
    <p class="small muted">
      {feed.name}: last checked {ago(feed.checkedAt, now)}{!feed.enabled ? " (game switched off)"
        : feed.nextCheck && !snap.settings.offline ? `, next check ${until(feed.nextCheck, now)}` : ""}
    </p>
  {/each}
</section>

<section>
  <h2>Data sources</h2>
  {#each snap.feeds as feed (feed.key)}
    <p class="small muted">{feed.name}: {feed.credit}{feed.legend ? ` (${feed.legend})` : ""}</p>
  {/each}
  <p class="small muted">Game icons: App Store icons by NEXON Korea and COGNOSPHERE.</p>
</section>

<button class="wide danger" onclick={() => api.quit()}>Quit GachaSlop</button>
