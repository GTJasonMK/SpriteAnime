import type { SpriteElements } from "./dom";
import { getFileName } from "./utils";

export function addImageSource(
  elements: SpriteElements,
  path: string,
  select = true
): boolean {
  const existing = Array.from(elements.imageSelect.options).find((option) => option.value === path);
  if (existing) {
    if (select) elements.imageSelect.value = path;
    return false;
  }
  const option = document.createElement("option");
  option.value = path;
  option.textContent = getFileName(path);
  elements.imageSelect.appendChild(option);
  if (select) elements.imageSelect.value = path;
  return true;
}
