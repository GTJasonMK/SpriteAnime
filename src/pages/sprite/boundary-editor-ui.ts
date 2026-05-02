import {
  clampBoundaryEdges,
  defaultFrameRegion,
} from "./boundary-model";
import type { SpriteElements } from "./dom";
import type { FrameBounds, SplitRegion } from "./types";
import { clampNumber } from "./utils";

interface BoundaryControlState {
  hasFrames: boolean;
  hasImage: boolean;
  shouldShowEditor: boolean;
  hasSelection: boolean;
  selectedFrame: FrameBounds | null;
}

export function setBoundaryEditorInputs(
  els: SpriteElements,
  frame: FrameBounds,
  limit: SplitRegion
): void {
  const editable = frame.empty ? defaultFrameRegion(frame) : frame;
  const limitRight = limit.x + limit.width;
  const limitBottom = limit.y + limit.height;

  els.boundaryLeft.value = String(Math.round(editable.x));
  els.boundaryTop.value = String(Math.round(editable.y));
  els.boundaryRight.value = String(Math.round(editable.x + editable.width));
  els.boundaryBottom.value = String(Math.round(editable.y + editable.height));
  els.boundaryAnchorX.value = String(
    Math.round(clampNumber(frame.anchorX - editable.x, 0, editable.width))
  );

  els.boundaryLeft.min = String(limit.x);
  els.boundaryLeft.max = String(limitRight - 1);
  els.boundaryTop.min = String(limit.y);
  els.boundaryTop.max = String(limitBottom - 1);
  els.boundaryRight.min = String(limit.x + 1);
  els.boundaryRight.max = String(limitRight);
  els.boundaryBottom.min = String(limit.y + 1);
  els.boundaryBottom.max = String(limitBottom);
  updateBoundaryAnchorInputConstraints(els, editable);
}

export function readBoundaryEditorValues(
  els: SpriteElements,
  frame: FrameBounds,
  fallback: SplitRegion,
  limit: SplitRegion,
  strict: boolean
): { region: SplitRegion; anchorX: number } | null {
  const readEdge = (input: HTMLInputElement, fallbackValue: number): number | null => {
    const parsed = Number.parseInt(input.value, 10);
    if (!Number.isFinite(parsed)) {
      return strict ? null : fallbackValue;
    }
    return parsed;
  };
  const left = readEdge(els.boundaryLeft, fallback.x);
  const top = readEdge(els.boundaryTop, fallback.y);
  const right = readEdge(els.boundaryRight, fallback.x + fallback.width);
  const bottom = readEdge(els.boundaryBottom, fallback.y + fallback.height);

  if (left === null || top === null || right === null || bottom === null) {
    return null;
  }

  const region = clampBoundaryEdges(left, top, right, bottom, limit);
  const fallbackLocalAnchor = clampNumber(frame.anchorX - fallback.x, 0, fallback.width);
  const anchorLocal = readEdge(els.boundaryAnchorX, fallbackLocalAnchor);
  if (anchorLocal === null) {
    return null;
  }

  return {
    region,
    anchorX: region.x + clampNumber(Math.round(anchorLocal), 0, region.width),
  };
}

export function updateBoundaryAnchorInputConstraints(
  els: SpriteElements,
  region: SplitRegion
): void {
  els.boundaryAnchorX.min = "0";
  els.boundaryAnchorX.max = String(Math.max(1, Math.round(region.width)));
}

export function syncBoundaryControls(
  els: SpriteElements,
  state: BoundaryControlState
): void {
  const { hasFrames, hasImage, shouldShowEditor, hasSelection, selectedFrame } = state;
  const editorInputs = [
    els.boundaryLeft,
    els.boundaryTop,
    els.boundaryRight,
    els.boundaryBottom,
    els.boundaryAnchorX,
  ];

  els.boundaryAdd.disabled = !hasImage;
  els.boundaryEdit.disabled = !hasSelection;
  els.boundaryDelete.disabled = !selectedFrame || selectedFrame.empty;
  els.boundaryEditorDelete.disabled = !selectedFrame || selectedFrame.empty;
  els.boundaryApply.disabled = !hasSelection;
  els.boundaryCancel.disabled = !hasSelection;
  els.boundaryClose.disabled = !hasSelection;
  els.boundaryAnchorCenter.disabled = !hasSelection;
  editorInputs.forEach((input) => {
    input.disabled = !hasSelection;
  });
  els.spriteSidebar.classList.toggle("has-frames", hasFrames);
  els.spriteSidebar.classList.toggle("pre-split", !hasFrames);
  els.boundaryPanel.style.display = shouldShowEditor ? "flex" : "none";
  els.boundaryPanel.classList.toggle("active", hasSelection);

  if (!hasSelection || !shouldShowEditor) {
    els.boundaryTitle.textContent = "边界框";
    editorInputs.forEach((input) => {
      input.value = "";
    });
  }
}
