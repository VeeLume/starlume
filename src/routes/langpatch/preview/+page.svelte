<script lang="ts">
  // Mission text preview: what the current patch config does to each
  // mission's title + description, patched next to the original, without
  // writing anything. Longest-added first, so the list doubles as the
  // "what makes descriptions long" view.
  //
  // The list is page-local and recomputed per visit (frontend rule 1 — this
  // is derived output, not a long-lived dataset): it depends on the patcher
  // options, so a cached copy would go stale on every option change.
  import { onMount } from "svelte";
  import {
    Surface,
    Expand,
    createBrowseState,
    createExpansion,
    type FacetDef,
    type Status,
  } from "@veelume/ui";
  import { commands, type MissionPreviewView, type PreviewTextView } from "$lib/bindings";
  import { ensureChannel } from "$lib/state/data.svelte";
  import RichText from "$lib/components/catalog/RichText.svelte";

  type PreviewRow = {
    key: string;
    /** Plain original title — the row label and search text. */
    title: string;
    p: MissionPreviewView;
    /** Panel lines the patch adds to the description. */
    added: number;
    /** Panel lines of the patched description. */
    lines: number;
    changed: boolean;
  };

  /** Estimated characters per line of the contracts panel's Details column
   *  (its longest line in 4.10 screenshots). Wrapping is approximate. */
  const PANEL_CHARS = 62;

  /** Literal `\n` escapes → real breaks, EM markup dropped. */
  function plain(text: string): string {
    return text.replace(/\\r\\n|\\n|\\r/g, "\n").replace(/<\/?EM\d>/g, "");
  }

  /** Lines the text takes in the panel, wrapped at PANEL_CHARS. */
  function panelLines(text: string): number {
    if (!text) return 0;
    return plain(text)
      .split("\n")
      .reduce((n, line) => n + Math.max(1, Math.ceil(line.length / PANEL_CHARS)), 0);
  }

  const isChanged = (t: PreviewTextView) => t.patched !== t.original;

  let previews = $state<MissionPreviewView[]>([]);
  let error = $state("");
  let loading = $state(true);
  let channel = $state<string | null>(null);

  const browse = createBrowseState({
    q: { kind: "text" },
    show: { kind: "one", default: "changed" },
    sort: { kind: "one", default: "added", narrows: false },
  });

  // Accordion state is page-local, never the URL (the kit rule).
  const expanded = createExpansion("one");

  const facets: FacetDef<PreviewRow>[] = [
    {
      id: "show",
      label: "Show",
      mode: "one",
      options: [
        { value: "changed", label: "Patched missions", test: (r) => r.changed },
        { value: "", label: "All missions" },
      ],
    },
  ];

  const descriptor = $derived({
    sources: () => previews,
    derive: (ps: MissionPreviewView[]): PreviewRow[] =>
      ps.map((p) => {
        const lines = panelLines(p.description.patched);
        return {
          key: `${p.title.key ?? ""}|${p.description.key ?? ""}`,
          title:
            plain(p.title.original).trim() || p.title.key || p.description.key || "(untitled)",
          p,
          added: lines - panelLines(p.description.original),
          lines,
          changed: isChanged(p.title) || isChanged(p.description),
        };
      }),
    searchIn: (r: PreviewRow) => [r.title, r.p.title.key ?? "", r.p.description.key ?? ""],
    facets,
    sorts: [
      {
        value: "added",
        label: "Most added",
        compare: (a: PreviewRow, b: PreviewRow) => b.added - a.added,
      },
      {
        value: "lines",
        label: "Longest",
        compare: (a: PreviewRow, b: PreviewRow) => b.lines - a.lines,
      },
      {
        value: "name",
        label: "Name",
        compare: (a: PreviewRow, b: PreviewRow) => a.title.localeCompare(b.title),
      },
    ],
  });

  const status = $derived<Status>(
    error ? "error" : loading && previews.length === 0 ? "loading" : "ready",
  );

  onMount(() => {
    void (async () => {
      channel = await ensureChannel();
      if (!channel) {
        loading = false;
        return;
      }
      const r = await commands.langpatchPreview(channel);
      loading = false;
      if (r.status === "ok") previews = r.data;
      else error = r.error.message;
    })();
  });
</script>

<div class="flex h-full min-h-0 flex-col gap-3">
  <div>
    <h1>Mission text preview</h1>
    <p class="muted">
      What the current patch settings do to each mission's title and description — English
      base text, language pack not applied. Line counts assume the contracts panel's width
      (about {PANEL_CHARS} characters), so they are estimates.
      <a href="/langpatch">Back to Text Patching</a>
    </p>
  </div>

  {#if !loading && !channel}
    <p class="dim">
      No game data loaded — go to <a href="/data">Overview</a> and press Load first.
    </p>
  {:else}
    {#if error}
      <p class="error">{error}</p>
    {/if}
    <Surface.Root {descriptor} {browse} class="min-h-0 flex-1">
      <Surface.List {status}>
        {#snippet row(r: PreviewRow)}
          <Expand.Row
            title={r.title}
            open={expanded.has(r.key)}
            ontoggle={() => expanded.toggle(r.key)}
          >
            {#snippet meta()}
              {#if isChanged(r.p.title)}<span class="badge" title="Title is patched">title</span
                >{/if}
            {/snippet}
            {#snippet right()}
              <span class="metric" title="Panel lines added / total">
                {r.added > 0 ? `+${r.added}` : r.added} · {r.lines} lines
              </span>
            {/snippet}

            <Expand.Cols>
              {#snippet main()}
                {@render panel("Patched", r.p.title.patched, r.p.description.patched)}
              {/snippet}
              {#snippet side()}
                {@render panel("Original", r.p.title.original, r.p.description.original)}
              {/snippet}
            </Expand.Cols>
            <p class="mono dim keys">{r.p.title.key} · {r.p.description.key}</p>
          </Expand.Row>
        {/snippet}
      </Surface.List>
    </Surface.Root>
  {/if}
</div>

{#snippet panel(label: string, title: string, description: string)}
  <div class="preview">
    <h5>{label}</h5>
    <div class="panel">
      <p class="panel-title"><RichText text={title} /></p>
      <p><RichText text={description} /></p>
    </div>
  </div>
{/snippet}

<style>
  /* The panel mimics the contracts screen's Details column: its width in
   * characters, so wrapping looks like the game's. */
  .panel {
    max-width: 62ch;
    padding: var(--space-3);
    border: 1px solid var(--border-soft);
    border-radius: var(--radius-md);
    font-size: var(--text-base);
  }
  .panel-title {
    font-weight: var(--weight-medium);
    margin-bottom: var(--space-2);
  }
  .keys {
    margin-top: var(--space-2);
    font-size: var(--text-mono);
  }
</style>
