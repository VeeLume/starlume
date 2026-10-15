// Mission preview helpers — pure, so the split is tested.

/** A patched text split into the game's own part and what the patch
 *  appended. When the patch changed text inside the original (not only
 *  after it), nothing is split off: the whole patched text is `base`. */
export function splitAdded(original: string, patched: string): { base: string; added: string } {
  // Game text writes line breaks as a literal \n; trim those too.
  const head = original.replace(/(\s|\\n)+$/, "");
  if (head && patched.startsWith(head)) {
    const added = patched.slice(head.length).replace(/^(\s|\\n)+/, "");
    return { base: head, added };
  }
  return { base: patched, added: "" };
}

/** An option's in-game tag from its description ("Add [BP] to titles…" →
 *  "[BP]"), for the checkbox's example; `null` when it names none. */
export function gameTag(description: string): string | null {
  return /\[[A-Za-z][A-Za-z !]*\]/.exec(description)?.[0] ?? null;
}
