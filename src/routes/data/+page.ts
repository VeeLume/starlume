// /data has no page of its own: install and cache management lives in
// Settings → Game data, so the Catalogs entry opens straight to content.
import { redirect } from "@sveltejs/kit";

export function load() {
  redirect(307, "/data/items");
}
