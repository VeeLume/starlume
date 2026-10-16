<script lang="ts">
  // The scan in the editor: mass and the composition rows. The draft is the
  // store's, so the calculator solves as you type.
  import { Dialog } from "@veelume/ui";
  import type { MiningCatalogView } from "$lib/bindings";
  import RockEditor from "./RockEditor.svelte";

  let { open, onclose, catalog }: { open: boolean; onclose: () => void; catalog: MiningCatalogView } = $props();
</script>

<Dialog {open} {onclose} title="Scan" class="max-w-lg!">
  <div class="body">
    <RockEditor {catalog} />
    <div class="foot"><button class="btn primary" onclick={onclose}>Done</button></div>
  </div>
</Dialog>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: 12px 16px 16px;
    overflow: auto;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
  }
</style>
