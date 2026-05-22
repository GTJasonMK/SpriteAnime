import { queryAll, queryOptional } from "../utils/dom";

export type AppTab = "generator" | "video-sprite" | "sprite";

export const PREPARE_SPRITE_FROM_GENERATOR_EVENT =
  "spriteanimte:prepare-sprite-from-generator";

const TAB_BUTTON_SELECTOR = ".tab-button";
const PAGE_SELECTOR = ".page";

export function getTabButtons(): HTMLButtonElement[] {
  return queryAll<HTMLButtonElement>(TAB_BUTTON_SELECTOR);
}

export function getPages(): HTMLElement[] {
  return queryAll<HTMLElement>(PAGE_SELECTOR);
}

export function getTabButton(tab: AppTab): HTMLButtonElement | null {
  return queryOptional<HTMLButtonElement>(`${TAB_BUTTON_SELECTOR}[data-tab="${tab}"]`);
}

export function getPageForTab(tab: string | undefined): HTMLElement | null {
  if (!tab) return null;
  return queryOptional<HTMLElement>(`#page-${tab}`);
}

export function clickTab(tab: AppTab): void {
  getTabButton(tab)?.click();
}

export function dispatchPrepareSpriteFromGenerator(): void {
  document.dispatchEvent(new CustomEvent(PREPARE_SPRITE_FROM_GENERATOR_EVENT));
}

export function onPrepareSpriteFromGenerator(handler: () => void): void {
  document.addEventListener(PREPARE_SPRITE_FROM_GENERATOR_EVENT, handler);
}
