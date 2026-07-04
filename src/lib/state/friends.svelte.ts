// Friends + groups store (docs/frontend.md rules 1–2): server-backed
// community state cached across navigation. The page renders the cache
// instantly and refreshes silently (mount, focus, slow poll) — no push
// channel yet, the v2 plan is "polling, not realtime".

import { commands, type FriendGroup, type FriendUser } from "$lib/bindings";
import { authStore } from "./auth.svelte";

let _friends = $state<FriendUser[]>([]);
let _groups = $state<FriendGroup[]>([]);
let _loaded = $state(false);
let _error = $state("");

export const friendsStore = {
  get friends() {
    return _friends;
  },
  get groups() {
    return _groups;
  },
  get loaded() {
    return _loaded;
  },
  get error() {
    return _error;
  },
};

/** Refresh both lists. Silent no-op when signed out. */
export async function refreshFriends(): Promise<void> {
  if (!authStore.current?.logged_in) {
    _loaded = true;
    return;
  }
  const [friendsResult, groupsResult] = await Promise.all([
    commands.listFriends(),
    commands.listGroups(),
  ]);
  if (friendsResult.status === "ok") {
    _friends = friendsResult.data;
    _error = "";
  } else {
    _error = friendsResult.error.message;
  }
  if (groupsResult.status === "ok") _groups = groupsResult.data;
  else _error = groupsResult.error.message;
  _loaded = true;
}

/** Mint a 7-day multi-use friend code. Returns the code, or null on error. */
export async function mintFriendCode(): Promise<string | null> {
  const result = await commands.createFriendInvite();
  if (result.status === "ok") {
    _error = "";
    return result.data;
  }
  _error = result.error.message;
  return null;
}

/** Redeem a friend code (mutual add). True on success. */
export async function addFriend(code: string): Promise<boolean> {
  const result = await commands.addFriend(code);
  if (result.status === "ok") {
    _friends = result.data;
    _error = "";
    return true;
  }
  _error = result.error.message;
  return false;
}

export async function removeFriend(userId: string): Promise<void> {
  const result = await commands.removeFriend(userId);
  if (result.status === "ok") {
    _friends = result.data;
    _error = "";
  } else {
    _error = result.error.message;
  }
}

export async function createGroup(name: string): Promise<boolean> {
  const result = await commands.createGroup(name);
  if (result.status === "ok") {
    _error = "";
    await refreshFriends();
    return true;
  }
  _error = result.error.message;
  return false;
}

/** Join by invite code. Returns the joined group, or null on error. */
export async function joinGroup(code: string): Promise<FriendGroup | null> {
  const result = await commands.joinGroup(code);
  if (result.status === "ok") {
    _error = "";
    await refreshFriends();
    return result.data;
  }
  _error = result.error.message;
  return null;
}

/** Mint a group invite code. Returns the code, or null on error. */
export async function createInvite(groupId: string): Promise<string | null> {
  const result = await commands.createInvite(groupId);
  if (result.status === "ok") {
    _error = "";
    return result.data;
  }
  _error = result.error.message;
  return null;
}

export async function leaveGroup(groupId: string): Promise<void> {
  const result = await commands.leaveGroup(groupId);
  if (result.status === "ok") {
    _error = "";
    await refreshFriends();
  } else {
    _error = result.error.message;
  }
}
