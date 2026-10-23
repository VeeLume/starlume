<script lang="ts">
  // Where the ship is sold and rented, from UEX's player-reported prices
  // (the game files have no shops): cheapest first, each with its place
  // and how old its report is. The first few show; the rest fold.
  import type { OfferView, ShipPricesView } from "$lib/bindings";

  let { prices }: { prices: ShipPricesView | null } = $props();

  const SHOWN = 5;
  let all = $state(false);

  const ago = (t: number) => {
    const d = (Date.now() / 1000 - t) / 86400;
    return d < 1 ? "today" : d < 2 ? "yesterday" : `${Math.floor(d)} d ago`;
  };
  const aUEC = (v: number) => Math.round(v).toLocaleString("en-US");
</script>

{#snippet list(title: string, offers: OfferView[], unit: string)}
  <div class="side">
    <h4>{title}</h4>
    {#if offers.length}
      <table>
        <tbody>
          {#each all ? offers : offers.slice(0, SHOWN) as o (o.terminal + o.place)}
            <tr>
              <td class="num">{aUEC(o.price)}<small>{unit}</small></td>
              <td><b>{o.terminal}</b>{#if o.place}<span class="place">{o.place}</span>{/if}</td>
              <td class="age">{ago(o.updated)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="none">No shop known.</p>
    {/if}
  </div>
{/snippet}

<div class="prices">
  {#if !prices || prices.fetchedAt === null}
    <p class="none">No prices yet: they come from UEX once online mode is on.</p>
  {:else}
    {@render list("Buy", prices.buy, "aUEC")}
    {@render list("Rent", prices.rent, "aUEC / day")}
    {#if !all && (prices.buy.length > SHOWN || prices.rent.length > SHOWN)}
      <button type="button" class="btn subtle more" onclick={() => (all = true)}>Show every shop</button>
    {/if}
    <p class="foot">Player-reported prices from UEX; each row's age is its last report.</p>
  {/if}
</div>

<style>
  .prices {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  h4 {
    margin: 0 0 4px;
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: var(--label-transform);
    color: var(--text-faint);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-small);
  }
  td {
    padding: 3px 0;
    color: var(--text-dim);
    vertical-align: baseline;
  }
  td + td {
    padding-left: 10px;
  }
  td b {
    font-weight: var(--weight-medium);
    color: var(--text);
  }
  .place {
    margin-left: 8px;
    font-size: 0.72rem;
    color: var(--text-faint);
  }
  .num {
    font-family: var(--font-mono);
    color: var(--text-strong);
    text-align: right;
    white-space: nowrap;
    width: 1%;
  }
  .num small {
    margin-left: 4px;
    font-size: 0.68rem;
    color: var(--text-faint);
  }
  .age {
    text-align: right;
    white-space: nowrap;
    font-size: 0.72rem;
    color: var(--text-faint);
  }
  .none,
  .foot {
    margin: 0;
    font-size: 0.72rem;
    color: var(--text-faint);
  }
  .more {
    align-self: flex-start;
  }
</style>
