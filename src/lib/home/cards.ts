// The cards Home can show: the shell's own, then each enabled module's
// (registry order). Module cards come and go with their module (module
// rule 2); the arrangement is the user's (`layout.ts`).

import type { HomeCard } from "$lib/modules/types";
import { moduleRegistry } from "$lib/modules/registry";
import GameStatusCard from "$lib/components/home/GameStatusCard.svelte";

/** The shell's cards — always available, whatever modules are enabled. */
const shellCards: HomeCard[] = [
  { id: "game-status", title: "Game status", component: GameStatusCard, wide: true },
];

export interface AvailableCard extends HomeCard {
  key: string;
  defaultVisible: boolean;
  /** Who contributes it, for customize mode ("Text Patching"). */
  ownerName: string;
}

export function availableCards(enabledModules: string[]): AvailableCard[] {
  const owned = (owner: string, ownerName: string) => (c: HomeCard) => ({
    ...c,
    key: `${owner}:${c.id}`,
    defaultVisible: c.defaultVisible ?? true,
    ownerName,
  });
  return [
    ...shellCards.map(owned("shell", "Starlume")),
    ...moduleRegistry
      .filter((d) => enabledModules.includes(d.id))
      .flatMap((d) => (d.homeCards ?? []).map(owned(d.id, d.name))),
  ];
}
