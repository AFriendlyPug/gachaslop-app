<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { api, fileUrl, type GameInput, type GameView, type ResetPreview } from "../lib/api";
  import { localDate, until, utcToLocal, WEEKDAYS } from "../lib/time";
  import GameBadge from "./GameBadge.svelte";

  let { game, onDone }: { game: GameView | null; onDone: () => void } = $props();

  // the form starts from the game being edited (its times are UTC), or a blank one
  const start = untrack(() => game);
  const pad = (n: number) => String(n).padStart(2, "0");
  let name = $state(start?.name ?? "");
  let time = $state(start ? `${pad(start.hour)}:${pad(start.minute)}` : "04:00");
  let weekly = $state<string>(start?.weeklyDay == null ? "" : String(start.weeklyDay));
  let staged = $state<string | null>(null); // path of a picked image, not saved yet
  let clearIcon = $state(false);
  let preview = $state<ResetPreview | null>(null);
  let error = $state("");
  let saving = $state(false);
  let saved = false;
  let now = $state(Date.now());
  let picker: HTMLInputElement;

  const input = $derived<GameInput>({
    name,
    hour: Number(time.split(":")[0] ?? 0),
    minute: Number(time.split(":")[1] ?? 0),
    weeklyDay: weekly === "" ? null : Number(weekly),
  });

  // what the badge shows: a picked image, nothing (after Remove), or the saved icon
  const badge = $derived<GameView>({
    ...(start ?? ({ key: "new", color: "#8e8e93", builtin: false } as GameView)),
    name: name || "New game",
    short: name || "?",
    iconPath: clearIcon ? null : (start?.iconPath ?? null),
  });

  $effect(() => {
    const current = input;
    api.previewReset(current).then((p) => { preview = p; now = Date.now(); }).catch(() => (preview = null));
  });

  // a picked image that's never saved is deleted
  onDestroy(() => { if (staged && !saved) api.discardStaged(); });

  async function choose() {
    error = "";
    await api.setDialogOpen(true); // the picker takes focus: don't let the popup hide
    picker.value = "";
    picker.click();
  }

  async function picked() {
    await api.setDialogOpen(false);
    const file = picker.files?.[0];
    if (!file) return;
    try {
      staged = await api.stageGameIcon(new Uint8Array(await file.arrayBuffer()));
      clearIcon = false;
    } catch (e) {
      error = String(e);
    }
  }

  async function save() {
    error = "";
    saving = true;
    try {
      if (start) await api.updateGame(start.key, input, staged, clearIcon && !staged);
      else await api.addGame(input, staged);
      saved = true;
      onDone();
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<form class="editor" onsubmit={(e) => { e.preventDefault(); save(); }}>
  <label>
    <span>Name</span>
    <input bind:value={name} maxlength="40" placeholder="e.g. Honkai: Star Rail (EU)" required />
  </label>

  <div class="field">
    <span>Icon</span>
    <div class="row left">
      <GameBadge game={badge} src={fileUrl(staged)} />
      <button type="button" onclick={choose}>Choose image…</button>
      {#if staged || (start?.iconPath && !clearIcon)}
        <button type="button" class="link" onclick={() => { staged = null; clearIcon = true; api.discardStaged(); }}>Remove</button>
      {/if}
      <input bind:this={picker} type="file" accept=".png,.jpg,.jpeg,.webp" hidden
             onchange={picked} oncancel={() => api.setDialogOpen(false)} />
    </div>
    <span class="muted small">PNG, JPG or WebP, up to 1 MB. A copy is kept, so it works offline.</span>
  </div>

  <div class="row two">
    <label>
      <span>Daily reset (UTC)</span>
      <input type="time" bind:value={time} required />
    </label>
    <label>
      <span>Weekly reset (UTC day)</span>
      <select bind:value={weekly}>
        <option value="">None</option>
        {#each WEEKDAYS as day, i (day)}<option value={String(i)}>{day}</option>{/each}
      </select>
    </label>
  </div>

  {#if preview}
    <p class="card small">
      {pad(input.hour)}:{pad(input.minute)} UTC is <strong>{utcToLocal(input.hour, input.minute, now)}</strong> your time.<br />
      Next reset: {localDate(preview.nextReset)}, {until(preview.nextReset, now)}
      {#if preview.nextWeekly}<br />Next weekly: {localDate(preview.nextWeekly)}, {until(preview.nextWeekly, now)}{/if}
    </p>
  {/if}
  <p class="muted small">If the game's reset moves with daylight saving, update its UTC time when the clocks change.</p>

  {#if error}<p class="error">{error}</p>{/if}

  <div class="row right">
    <button type="button" onclick={onDone}>Cancel</button>
    <button type="submit" class="primary" disabled={saving}>{start ? "Save" : "Add game"}</button>
  </div>
</form>
