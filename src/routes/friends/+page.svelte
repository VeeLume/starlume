<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { commands, type FriendGroup, type FriendUser } from "$lib/bindings";
  import { authStore, loadAuth } from "$lib/state/auth.svelte";
  import { notify } from "$lib/state/notifications.svelte";

  // No push channel yet (v2 plan is "polling, not realtime"), so a friend
  // adding you back — or joining a group — wouldn't show until re-navigation.
  // Refresh on window focus (instant when you tab back) plus a slow poll while
  // the page is open. Both are silent no-ops when signed out.
  const POLL_MS = 20_000;
  let pollTimer: ReturnType<typeof setInterval> | undefined;

  function quietRefresh() {
    if (authStore.current?.logged_in) void refresh();
  }

  let friends = $state<FriendUser[]>([]);
  let groups = $state<FriendGroup[]>([]);
  let loaded = $state(false);
  let error = $state("");
  let friendCode = $state("");
  let myFriendCode = $state("");
  let newGroupName = $state("");
  let joinCode = $state("");
  /** group id → freshly minted invite code (shown inline until dismissed). */
  let inviteCodes = $state<Record<string, string>>({});

  const auth = $derived(authStore.current);

  async function refresh() {
    error = "";
    const [friendsResult, groupsResult] = await Promise.all([
      commands.listFriends(),
      commands.listGroups(),
    ]);
    if (friendsResult.status === "ok") friends = friendsResult.data;
    else error = friendsResult.error.message;
    if (groupsResult.status === "ok") groups = groupsResult.data;
    else error = groupsResult.error.message;
    loaded = true;
  }

  async function mintFriendCode() {
    error = "";
    const result = await commands.createFriendInvite();
    if (result.status === "ok") {
      myFriendCode = result.data;
      await navigator.clipboard.writeText(result.data).catch(() => {});
    } else {
      error = result.error.message;
    }
  }

  async function addFriend() {
    const code = friendCode.trim();
    if (!code) return;
    error = "";
    const result = await commands.addFriend(code);
    if (result.status === "ok") {
      friends = result.data;
      friendCode = "";
      notify({ level: "success", title: "Friend added", source: "friends" });
    } else {
      error = result.error.message;
    }
  }

  async function removeFriend(friend: FriendUser) {
    error = "";
    const result = await commands.removeFriend(friend.user_id);
    if (result.status === "ok") friends = result.data;
    else error = result.error.message;
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
    pollTimer = setInterval(quietRefresh, POLL_MS);
  });
  onDestroy(() => clearInterval(pollTimer));
</script>

<svelte:window onfocus={quietRefresh} />

<h1>Friends</h1>

{#if !auth?.logged_in}
  <p class="dim">Sign in (Settings → Account) to add friends and create groups.</p>
{:else if !loaded}
  <p class="dim">Loading…</p>
{:else}
  <section class="actions">
    <button onclick={mintFriendCode}>My friend code</button>
    <form onsubmit={(e) => { e.preventDefault(); void addFriend(); }}>
      <input type="text" placeholder="Friend code" bind:value={friendCode} />
      <button type="submit">Add friend</button>
    </form>
  </section>
  {#if myFriendCode}
    <p class="invite">
      Your friend code: <code>{myFriendCode}</code> — copied to clipboard, valid 7 days.
    </p>
  {/if}

  {#if friends.length === 0}
    <p class="dim">No friends yet — swap friend codes to connect.</p>
  {:else}
    <ul class="friend-list">
      {#each friends as f (f.user_id)}
        <li>
          {f.username}
          <button class="small" onclick={() => removeFriend(f)}>Remove</button>
        </li>
      {/each}
    </ul>
  {/if}

  <h2>Groups</h2>
  <p class="dim hint">For more than one friend at once — shared visibility for a whole circle.</p>

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

  .friend-list {
    list-style: none;
    margin: 0 0 20px;
    padding: 0;
    max-width: 320px;
  }

  .friend-list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 0;
    border-bottom: 1px solid var(--border);
  }

  .small {
    font-size: 12px;
    padding: 3px 10px;
  }

  h2 {
    font-size: 16px;
    margin: 24px 0 2px;
  }

  .hint {
    margin: 0 0 12px;
    font-size: 13px;
  }

  .dim {
    color: var(--text-dim);
  }

  .error {
    color: #e06c6c;
  }
</style>
