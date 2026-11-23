<script lang="ts">
  // Plan and Bookmark, the two ways anything goes into Plans (decision
  // 01a2017f, design pass 8): two fixed icons at the end of a row or a side
  // panel's header, the same on every page. Plan: work on it now (an active
  // plan). Bookmark: keep it for later (a parked plan, no steps). The lit
  // icon says which it is and opens it on Plans; the other one moves it
  // across. Both keep their size whatever the state, so nothing moves.
  // Hidden while the Plans module is off.
  import { goto } from "$app/navigation";
  import { Bookmark, Target } from "lucide-svelte";
  import type { Plan } from "$lib/bindings";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { loadPlans, parkPlan, plansStore } from "$lib/state/plans.svelte";

  let {
    plan,
    add,
    what = "",
    blocked = "",
    markOff = "",
    small = false,
  }: {
    /** The plan it already has, planned or bookmarked. */
    plan: Plan | undefined;
    /** Create it: a plan, or `parked` a bookmark. */
    add: (parked: boolean) => Promise<boolean>;
    /** What a click plans, for the tooltips ("Contractor", "CBH-3 Helmet"). */
    what?: string;
    /** Why it cannot be planned yet; set, both icons are off with it as
     *  the tooltip. */
    blocked?: string;
    /** Why it cannot be a bookmark (a craft inside a project); set, the
     *  bookmark is off with it as the tooltip. */
    markOff?: string;
    small?: boolean;
  } = $props();

  const on = $derived(settingsStore.current?.enabled_modules.includes("plans") ?? false);
  $effect(() => {
    if (on && plansStore.plans === null) void loadPlans();
  });

  const planned = $derived(!!plan && !plan.parked);
  const marked = $derived(!!plan && plan.parked);
  const named = $derived(what ? ` ${what}` : "");
  let busy = $state(false);

  const open = (tab: "active" | "bookmarks") => void goto(`/plans?tab=${tab}&plan=${encodeURIComponent(plan!.id)}`);

  async function run(next: () => Promise<boolean>) {
    busy = true;
    await next();
    busy = false;
  }

  function onPlan() {
    if (planned) open("active");
    else void run(() => (plan ? parkPlan(plan.id, false) : add(false)));
  }

  function onMark() {
    if (marked) open("bookmarks");
    else void run(() => (plan ? parkPlan(plan.id, true) : add(true)));
  }

  const planTip = $derived(
    blocked && !plan ? blocked : planned ? "Planned: open it in Plans" : marked ? `Plan${named}, from your bookmark` : `Plan${named}`,
  );
  const markTip = $derived(
    markOff ? markOff : blocked && !plan ? blocked : marked ? "Bookmarked: open your bookmarks" : planned ? "Move it to your bookmarks" : `Bookmark${named}`,
  );
  const off = $derived(busy || (!!blocked && !plan));
</script>

{#if on}
  <span class="pair" class:small>
    <button type="button" class="ib plan" class:lit={planned} disabled={off} title={planTip} aria-label={planTip} aria-pressed={planned} onclick={onPlan}>
      <Target size={small ? 14 : 15} />
    </button>
    <button type="button" class="ib mark" class:lit={marked} disabled={off || !!markOff} title={markTip} aria-label={markTip} aria-pressed={marked} onclick={onMark}>
      <Bookmark size={small ? 14 : 15} fill={marked ? "currentColor" : "none"} />
    </button>
  </span>
{/if}

<style>
  .pair {
    display: inline-flex;
    flex: 0 0 auto;
    gap: 4px;
  }
  .ib {
    all: unset;
    box-sizing: border-box;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 1px solid transparent;
    border-radius: var(--radius);
    color: var(--text-faint);
    cursor: pointer;
  }
  .small .ib {
    width: 24px;
    height: 24px;
  }
  .ib:hover:not(:disabled) {
    border-color: var(--border);
    background: var(--bg-sunk);
    color: var(--text);
  }
  .ib:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .ib:disabled {
    cursor: default;
    opacity: 0.45;
  }
  .plan.lit {
    color: var(--accent);
    border-color: var(--accent-line);
    background: var(--accent-fill);
  }
  .mark.lit {
    color: var(--hue-sand);
    border-color: color-mix(in srgb, var(--hue-sand) 45%, transparent);
    background: color-mix(in srgb, var(--hue-sand) 14%, transparent);
  }
</style>
