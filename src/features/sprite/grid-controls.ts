import type { SpriteElements } from "./dom";

export function updateGridPresetState(
  els: SpriteElements,
  rows: number,
  cols: number
): void {
  els.gridPresets.forEach((button) => {
    const buttonRows = Number.parseInt(button.dataset.rows!, 10);
    const buttonCols = Number.parseInt(button.dataset.cols!, 10);
    button.classList.toggle("active", buttonRows === rows && buttonCols === cols);
  });
}
