<script lang="ts">
  // "UEX" in a price credit, opening their site in the browser.
  import { commands } from "$lib/bindings";
</script>

<button type="button" title="uexcorp.space" onclick={() => void commands.openCreditLink("https://uexcorp.space")}>UEX</button>

<style>
  button {
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    color: var(--text-dim);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  button:hover {
    color: var(--text-strong);
  }
</style>
