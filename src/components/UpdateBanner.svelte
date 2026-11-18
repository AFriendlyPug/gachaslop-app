<script lang="ts">
  import { api, type UpdateView } from "../lib/api";
  import { localDate } from "../lib/time";

  // A newer version is out. Nothing happens unless the user picks Install (or downloads it themselves).
  let { update, current, offline, dismissible = false }:
    { update: UpdateView; current: string; offline: boolean; dismissible?: boolean } = $props();

  let installing = $state(false);
  let error = $state("");

  async function install() {
    error = "";
    installing = true;
    try {
      await api.installUpdate(); // on success the app closes and the installer takes over
    } catch (e) {
      error = String(e);
      installing = false;
    }
  }

  const size = $derived(update.size ? ` (${(update.size / 1_000_000).toFixed(1)} MB)` : "");
</script>

<section class="card update-banner" aria-label="Update available">
  <div class="row">
    <span class="update-title">Version {update.version} is available</span>
    {#if dismissible && !installing}<button class="link" onclick={() => api.dismissUpdate()}>Not now</button>{/if}
  </div>
  <p class="small secondary">
    You have {current}.{update.published ? ` Released ${localDate(update.published)}.` : ""}
    {installing ? "Downloading and checking the installer. GachaSlop will close and open again when it's done."
      : `Install downloads it${size}, checks it and updates in place; GachaSlop closes and opens again by itself.`}
  </p>
  <div class="buttons">
    <button class="primary" disabled={installing || offline} title={offline ? "Turn off offline mode first" : undefined}
            onclick={install}>{installing ? "Installing…" : "Install update"}</button>
    <button onclick={() => api.openPath("release")}>What's new</button>
  </div>
  {#if error}<p class="error">{error}</p>{/if}
</section>
