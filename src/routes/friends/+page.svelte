<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import type { FriendGroup, FriendUser } from "$lib/bindings";
  import { authStore, loadAuth } from "$lib/state/auth.svelte";
  import { Button } from "@veelume/ui";
  import { notify } from "$lib/state/notifications.svelte";
  import {
    friendsStore,
    refreshFriends,
    mintFriendCode,
    addFriend,
    removeFriend,
    createGroup,
    joinGroup,
    createInvite,
    leaveGroup,
  } from "$lib/state/friends.svelte";

  // Data lives in friendsStore (renders instantly on revisit); this page
  // only owns input fields + freshly minted codes. No push channel yet
  // (v2 plan is "polling, not realtime"), so refresh on focus + slow poll.
  const POLL_MS = 20_000;
  let pollTimer: ReturnType<typeof setInterval> | undefined;

  function quietRefresh() {
    if (authStore.current?.logged_in) void refreshFriends();
  }

  let friendCode = $state("");
  let myFriendCode = $state("");
  let newGroupName = $state("");
  let joinCode = $state("");
  /** group id → freshly minted invite code (shown inline until dismissed). */
  let inviteCodes = $state<Record<string, string>>({});

  const auth = $derived(authStore.current);

  async function mint() {
    const code = await mintFriendCode();
    if (code) {
      myFriendCode = code;
      await navigator.clipboard.writeText(code).catch(() => {});
    }
  }

  async function add() {
    const code = friendCode.trim();
    if (!code) return;
    if (await addFriend(code)) {
      friendCode = "";
      notify({ level: "success", title: "Friend added", source: "friends" });
    }
  }

  async function remove(friend: FriendUser) {
    await removeFriend(friend.user_id);
  }

  async function create() {
    const name = newGroupName.trim();
    if (!name) return;
    if (await createGroup(name)) newGroupName = "";
  }

  async function join() {
    const code = joinCode.trim();
    if (!code) return;
    const group = await joinGroup(code);
    if (group) {
      joinCode = "";
      notify({ level: "success", title: `Joined ${group.name}`, source: "friends" });
    }
  }

  async function invite(groupId: string) {
    const code = await createInvite(groupId);
    if (code) {
      inviteCodes = { ...inviteCodes, [groupId]: code };
      await navigator.clipboard.writeText(code).catch(() => {});
    }
  }

  async function leave(group: FriendGroup) {
    await leaveGroup(group.id);
  }

  onMount(() => {
    void (async () => {
      await loadAuth();
      void refreshFriends();
    })();
    pollTimer = setInterval(quietRefresh, POLL_MS);
  });
  onDestroy(() => clearInterval(pollTimer));
</script>

<svelte:window onfocus={quietRefresh} />

<h1>Friends</h1>

{#if !auth?.logged_in}
  <p class="dim">Sign in (Settings → Account) to add friends and create groups.</p>
{:else if !friendsStore.loaded}
  <p class="dim">Loading…</p>
{:else}
  <section class="actions">
    <Button variant="outline" onclick={mint}>My friend code</Button>
    <form onsubmit={(e) => { e.preventDefault(); void add(); }}>
      <input class="input" type="text" placeholder="Friend code" bind:value={friendCode} />
      <Button type="submit">Add friend</Button>
    </form>
  </section>
  {#if myFriendCode}
    <p class="invite">
      Your friend code: <code>{myFriendCode}</code> — copied to clipboard, valid 7 days.
    </p>
  {/if}

  {#if friendsStore.friends.length === 0}
    <p class="dim">No friends yet — swap friend codes to connect.</p>
  {:else}
    <ul class="friend-list">
      {#each friendsStore.friends as f (f.user_id)}
        <li>
          {f.username}
          <Button variant="ghost" onclick={() => remove(f)}>Remove</Button>
        </li>
      {/each}
    </ul>
  {/if}

  <h2>Groups</h2>
  <p class="dim hint">For more than one friend at once — shared visibility for a whole circle.</p>

  <section class="actions">
    <form onsubmit={(e) => { e.preventDefault(); void create(); }}>
      <input class="input" type="text" placeholder="New group name" bind:value={newGroupName} maxlength="64" />
      <Button type="submit">Create</Button>
    </form>
    <form onsubmit={(e) => { e.preventDefault(); void join(); }}>
      <input class="input" type="text" placeholder="Invite code" bind:value={joinCode} />
      <Button type="submit">Join</Button>
    </form>
  </section>

  {#if friendsStore.groups.length === 0}
    <p class="dim">No groups yet — create one, or join with a code from a friend.</p>
  {:else}
    {#each friendsStore.groups as g (g.id)}
      <div class="card group">
        <div class="group-head">
          <span class="group-name">{g.name}</span>
          <span class="dim">{g.members.length} member{g.members.length === 1 ? "" : "s"}</span>
          <span class="spacer"></span>
          <Button variant="outline" onclick={() => invite(g.id)}>Invite</Button>
          <Button variant="ghost" onclick={() => leave(g)}>Leave</Button>
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

{#if friendsStore.error}
  <p class="error">{friendsStore.error}</p>
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


  h2 {
    font-size: 16px;
    margin: 24px 0 2px;
  }

  .hint {
    margin: 0 0 12px;
    font-size: 13px;
  }
</style>
