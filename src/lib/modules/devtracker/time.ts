// Post times: relative while recent ("3 h ago"), a date after a week. The
// wording is UI text (English, like the rest); the date follows the user's
// formatting locale.

const rtf = new Intl.RelativeTimeFormat("en", { numeric: "auto", style: "short" });

export function postAge(ms: number, now = Date.now()): string {
  const s = Math.round((ms - now) / 1000);
  const abs = Math.abs(s);
  if (abs < 60) return "just now";
  if (abs < 3600) return rtf.format(Math.round(s / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(s / 3600), "hour");
  if (abs < 7 * 86400) return rtf.format(Math.round(s / 86400), "day");
  return new Date(ms).toLocaleDateString([], { dateStyle: "medium" });
}

/** The day a post belongs to, for the feed's group headers. */
export function dayLabel(ms: number, now = new Date()): string {
  const day = new Date(ms);
  const start = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const diff = Math.round((start(now) - start(day)) / 86400000);
  if (diff === 0) return "Today";
  if (diff === 1) return "Yesterday";
  return day.toLocaleDateString([], { weekday: "long", day: "numeric", month: "long" });
}
