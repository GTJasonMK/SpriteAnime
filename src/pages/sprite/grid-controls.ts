import type { SpriteElements } from "./dom";
import { normalizeGridSize } from "./utils";

export function updateGridPresetState(
  els: SpriteElements,
  rows?: number,
  cols?: number
): void {
  const safeRows = rows ?? normalizeGridSize(els.rows.value, 3);
  const safeCols = cols ?? normalizeGridSize(els.cols.value, 4);
  els.gridPresets.forEach((button) => {
    const buttonRows = Number.parseInt(button.dataset.rows || "0", 10);
    const buttonCols = Number.parseInt(button.dataset.cols || "0", 10);
    button.classList.toggle("active", buttonRows === safeRows && buttonCols === safeCols);
  });
}
