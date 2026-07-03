<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type FriendGroup } from "$lib/bindings";
  import { authStore, loadAuth } from "$lib/state/auth.svelte";
  import { notify } from "$lib/state/notifications.svelte";

  let groups = $state<FriendGroup[]>([]);
  let loaded = $state(false);
  let error = $state("");
  let newGroupName = $state("");
  let joinCode = $state("");
  /** group id → freshly minted invite code (shown inline until dismissed). */
  let inviteCodes = $state<Record<string, string>>({});

  const auth = $derived(authStore.current);

  async function refresh() {
    error = "";
    const result = await commands.listGroups();
    if (result.status === "ok") {
      groups = result.data;
      loaded = true;
    } else {
      error = result.error.message;
    }
  }

  async function createGroup() {
    const name = newGroupName.trim();
    if (!name) return;
    error = "";
    const result = await commands.createGroup(name);
    if (result.status === "ok") {
      newGroupName = "";
      await refresh();
    } else {
      error = result.error.message;
    }
  }

  async function join() {
    const code = joinCode.trim();
    if (!code) return;
    error = "";
    const result = await commands.joinGroup(code);
    if (result.status === "ok") {
      joinCode = "";
      notify({ level: "success", title: `Joined ${result.data.name}`, source: "friends" });
      await refresh();
    } else {
      error = result.error.message;
    }
  }

  async function invite(groupId: string) {
    error = "";
    const result = await commands.createInvite(groupId);
    if (result.status === "ok") {
      inviteCodes = { ...inviteCodes, [groupId]: result.data };
      await navigator.clipboard.writeText(result.data).catch(() => {});
    } else {
      error = result.error.message;
    }
  }

  async function leave(group: FriendGroup) {
    error = "";
    const result = await commands.leaveGroup(group.id);
    if (result.status === "ok") {
      await refresh();
    } else {
      error = result.error.message;
    }
  }

  onMount(async () => {
    await loadAuth();
    if (authStore.current?.logged_in) await refresh();
    else loaded = true;
  });
</script>

<h1>Friends</h1>

{#if !auth?.logged_in}
  <p class="dim">Sign in (Settings → Account) to create and join friend groups.</p>
{:else if !loaded}
  <p class="dim">Loading…</p>
{:else}
  <section class="actions">
    <form onsubmit={(e) => { e.preventDefault(); void createGroup(); }}>
      <input type="text" placeholder="New group name" bind:value={newGroupName} maxlength="64" />
      <button type="submit">Create</button>
    </form>
    <form onsubmit={(e) => { e.preventDefault(); void join(); }}>
      <input type="text" placeholder="Invite code" bind:value={joinCode} />
      <button type="submit">Join</button>
    </form>
  </section>

  {#if groups.length === 0}
    <p class="dim">No groups yet — create one, or join with a code from a friend.</p>
  {:else}
    {#each groups as g (g.id)}
      <div class="group">
        <div class="group-head">
          <span class="group-name">{g.name}</span>
          <span class="dim">{g.members.length} member{g.members.length === 1 ? "" : "s"}</span>
          <span class="spacer"></span>
          <button onclick={() => invite(g.id)}>Invite</button>
          <button onclick={() => leave(g)}>Leave</button>
        </div>
        {#if inviteCodes[g.id]}
          <p class="invite">
            Invite code <code>{inviteCodes[g.id]}</code> — copied to clipboard, valid 7 days.
          </p>
        {/if}
        <ul>
          {#each g.members as m (m.username)}
            <li>{m.username}{m.is_owner ? " 👑" : ""}</li>
          {/each}
        </ul>
      </div>
    {/each}
  {/if}
{/if}

{#if error}
  <p class="error">{error}</p>
{/if}

<style>
  .actions {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 20px;
  }

  form {
    display: flex;
    gap: 6px;
  }

  .group {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
    margin-bottom: 12px;
    max-width: 480px;
  }

  .group-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .group-name {
    font-weight: 600;
  }

  .spacer {
    flex: 1;
  }

  .invite {
    font-size: 13px;
    color: var(--text-dim);
  }

  .invite code {
    color: var(--accent);
    font-size: 14px;
  }

  ul {
    margin: 8px 0 0;
    padding-left: 20px;
    color: var(--text-dim);
  }

  .dim {
    color: var(--text-dim);
  }

  .error {
    color: #e06c6c;
  }
</style>
