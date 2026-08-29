<script lang="ts">
  // The settings section on the kit scaffold — the kit owns the three-state
  // responsive list-detail layout; this file owns only the category data.
  import { Settings } from "@veelume/ui";
  import { settingsCategories } from "$lib/settingsNav";

  let { children } = $props();
</script>

<Settings.Root categories={settingsCategories}>
  {@render children()}
</Settings.Root>
