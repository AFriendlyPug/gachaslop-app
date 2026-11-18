<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { api, fileUrl, type GameInput, type GameView, type ResetPreview, type Zone } from "../lib/api";
  import { toIconPng } from "../lib/icon";
  import { clock, localDate, until, utcToLocal, WEEKDAYS } from "../lib/time";
  import GameBadge from "./GameBadge.svelte";

  let { game, onDone }: { game: GameView | null; onDone: () => void } = $props();

  // the form starts from the game being edited (its times are UTC), or a blank one
  const start = untrack(() => game);
  const pad = (n: number) => String(n).padStart(2, "0");
  let name = $state(start?.name ?? "");
  let time = $state(start ? `${pad(start.hour)}:${pad(start.minute)}` : "04:00");
  let weekly = $state<string>(start?.weeklyDay == null ? "" : String(start.weeklyDay));
  let zone = $state<string>(start?.zone ?? ""); // "" = UTC
  let zones = $state<Zone[]>([]);
  onMount(() => api.timeZones().then((z) => (zones = z)));
  const zoneName = $derived(zones.find((z) => z.id === zone)?.name ?? zone);
  const where = $derived(zone ? "in that time zone" : "UTC");
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
    zone: zone || null,
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

  // any image becomes a 128px PNG (see lib/icon.ts), then is staged until the game is saved
  async function useImage(image: Blob) {
    staged = await api.stageGameIcon(await toIconPng(image));
    clearIcon = false;
  }

  async function picked() {
    await api.setDialogOpen(false);
    const file = picker.files?.[0];
    if (!file) return;
    error = "";
    try {
      await useImage(file);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  let link = $state("");
  let fetching = $state(false);

  async function download() {
    error = "";
    fetching = true;
    try {
      await useImage(new Blob([await api.fetchIcon(link)]));
      link = "";
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      fetching = false;
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
      <input bind:this={picker} type="file" accept="image/*" hidden
             onchange={picked} oncancel={() => api.setDialogOpen(false)} />
    </div>
    <div class="row left">
      <input class="grow" type="url" bind:value={link} placeholder="or paste an image link (https://…)"
             aria-label="Image link" onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); if (link) download(); } }} />
      <button type="button" disabled={!link.trim() || fetching} onclick={download}>{fetching ? "Getting…" : "Get"}</button>
    </div>
    <span class="muted small">Any image (PNG, JPG, WebP, GIF…). It's made into a square icon and a copy is kept, so it
      works offline.</span>
  </div>

  <label>
    <span>Time zone of the reset</span>
    <select bind:value={zone}>
      <option value="">UTC (no daylight saving)</option>
      {#each zones as z (z.id)}<option value={z.id}>{z.name}</option>{/each}
    </select>
  </label>

  <div class="row two">
    <label>
      <span>Daily reset ({where})</span>
      <input type="time" bind:value={time} required />
    </label>
    <label>
      <span>Weekly reset ({where === "UTC" ? "UTC day" : "day there"})</span>
      <select bind:value={weekly}>
        <option value="">None</option>
        {#each WEEKDAYS as day, i (day)}<option value={String(i)}>{day}</option>{/each}
      </select>
    </label>
  </div>

  {#if preview}
    <p class="card small">
      {#if zone}
        {pad(input.hour)}:{pad(input.minute)} in {zoneName} is <strong>{clock(preview.nextReset)}</strong> your time
        today, and follows its daylight saving.<br />
      {:else}
        {pad(input.hour)}:{pad(input.minute)} UTC is <strong>{utcToLocal(input.hour, input.minute, now)}</strong> your time.<br />
      {/if}
      Next reset: {localDate(preview.nextReset)}, {until(preview.nextReset, now)}
      {#if preview.nextWeekly}<br />Next weekly: {localDate(preview.nextWeekly)}, {until(preview.nextWeekly, now)}{/if}
    </p>
  {/if}
  <p class="muted small">If the game resets at a set time on its server, pick the server's time zone and the reset follows its
    daylight saving by itself. Otherwise leave it on UTC.</p>

  {#if error}<p class="error">{error}</p>{/if}

  <div class="row right">
    <button type="button" onclick={onDone}>Cancel</button>
    <button type="submit" class="primary" disabled={saving}>{start ? "Save" : "Add game"}</button>
  </div>
</form>
