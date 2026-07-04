<script lang="ts">
  // Renders Star Citizen mission/flavour text: converts literal `\n` escapes
  // to real line breaks and the DCB `<EMn>…</EMn>` emphasis tags to styled
  // runs — EM0–EM2 plain, EM3 underline, EM4 in-text link blue.
  interface Run {
    text: string;
    em: number | null;
  }

  let { text }: { text: string } = $props();

  function normalize(raw: string): string {
    return raw
      .replace(/\\r\\n/g, "\n")
      .replace(/\\n/g, "\n")
      .replace(/\\r/g, "\n");
  }

  function parse(raw: string): Run[] {
    const s = normalize(raw);
    const runs: Run[] = [];
    const re = /<EM(\d)>([\s\S]*?)<\/EM\d>/g;
    let last = 0;
    let m: RegExpExecArray | null;
    while ((m = re.exec(s)) !== null) {
      if (m.index > last) runs.push({ text: s.slice(last, m.index), em: null });
      runs.push({ text: m[2], em: Number(m[1]) });
      last = re.lastIndex;
    }
    if (last < s.length) runs.push({ text: s.slice(last), em: null });
    // Drop any stray/unpaired EM tags left in plain runs.
    return runs.map((r) => (r.em == null ? { ...r, text: r.text.replace(/<\/?EM\d>/g, "") } : r));
  }

  const runs = $derived(parse(text));
</script>

<!-- prettier-ignore -->
<span class="richtext">{#each runs as r, i (i)}{#if r.em === 3}<span class="u">{r.text}</span>{:else if r.em === 4}<span class="link">{r.text}</span>{:else}{r.text}{/if}{/each}</span>

<style>
  .richtext {
    white-space: pre-wrap;
  }
  .u {
    text-decoration: underline;
  }
  .link {
    color: var(--em-link);
  }
</style>
