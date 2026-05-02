import { createGridCellRects, getGridLinesSignature } from "./grid-lines";
import type { AutoBoundsResult, FrameBounds, SplitRegion } from "./types";
import { clampNumber, getAutoExpandPixels, getMinimumCellDimension, getRegionCenterX } from "./utils";

export function createDefaultAutoBounds(
  rows: number,
  cols: number,
  region: SplitRegion,
  allowExpand: boolean,
  cellRects: SplitRegion[] = createGridCellRects(rows, cols, region, null),
  gridSignature: string = getGridLinesSignature(rows, cols, region, null)
): AutoBoundsResult {
  const minCellWidth = getMinimumCellDimension(cellRects, "width");
  const minCellHeight = getMinimumCellDimension(cellRects, "height");
  const expandPixels = allowExpand ? getAutoExpandPixels(minCellWidth, minCellHeight) : 0;
  const frameBounds: FrameBounds[] = [];

  for (let row = 0; row < rows; row++) {
    for (let col = 0; col < cols; col++) {
      const cell = cellRects[row * cols + col];
      frameBounds.push({
        index: row * cols + col,
        row,
        col,
        cellX: cell.x,
        cellY: cell.y,
        cellWidth: cell.width,
        cellHeight: cell.height,
        x: cell.x,
        y: cell.y,
        width: cell.width,
        height: cell.height,
        anchorX: getRegionCenterX(cell.x, cell.width),
        empty: true,
      });
    }
  }

  return {
    rows,
    cols,
    region: { ...region },
    gridSignature,
    allowExpand,
    expandPixels,
    frameBounds,
    fixedOffsetX: 0,
    fixedOffsetY: 0,
    fixedWidth: minCellWidth,
    fixedHeight: minCellHeight,
    emptyCount: frameBounds.length,
  };
}

export function defaultFrameRegion(frame: FrameBounds): SplitRegion {
  return {
    x: frame.cellX,
    y: frame.cellY,
    width: frame.cellWidth,
    height: frame.cellHeight,
  };
}

export function clampBoundaryEdges(
  leftValue: number,
  topValue: number,
  rightValue: number,
  bottomValue: number,
  limit: SplitRegion
): SplitRegion {
  const limitRight = limit.x + limit.width;
  const limitBottom = limit.y + limit.height;
  const left = clampNumber(Math.round(leftValue), limit.x, limitRight - 1);
  const top = clampNumber(Math.round(topValue), limit.y, limitBottom - 1);
  const right = clampNumber(Math.round(rightValue), left + 1, limitRight);
  const bottom = clampNumber(Math.round(bottomValue), top + 1, limitBottom);

  return {
    x: left,
    y: top,
    width: right - left,
    height: bottom - top,
  };
}

export function recalculateAutoBounds(bounds: AutoBoundsResult): void {
  let emptyCount = 0;
  let minRelX = Number.POSITIVE_INFINITY;
  let minRelY = Number.POSITIVE_INFINITY;
  let maxRelRight = Number.NEGATIVE_INFINITY;
  let maxRelBottom = Number.NEGATIVE_INFINITY;

  bounds.frameBounds.forEach((frame) => {
    if (frame.empty) {
      emptyCount += 1;
      return;
    }
    const relX = frame.x - frame.cellX;
    const relY = frame.y - frame.cellY;
    minRelX = Math.min(minRelX, relX);
    minRelY = Math.min(minRelY, relY);
    maxRelRight = Math.max(maxRelRight, relX + frame.width);
    maxRelBottom = Math.max(maxRelBottom, relY + frame.height);
  });

  const first = bounds.frameBounds[0];
  if (!first || emptyCount === bounds.frameBounds.length) {
    bounds.fixedOffsetX = 0;
    bounds.fixedOffsetY = 0;
    bounds.fixedWidth = first?.cellWidth ?? 1;
    bounds.fixedHeight = first?.cellHeight ?? 1;
    bounds.emptyCount = emptyCount;
    return;
  }

  const padding = 2;
  const fixedOffsetX = Math.floor(minRelX - padding);
  const fixedOffsetY = Math.floor(minRelY - padding);
  const fixedRight = Math.ceil(maxRelRight + padding);
  const fixedBottom = Math.ceil(maxRelBottom + padding);

  bounds.fixedOffsetX = fixedOffsetX;
  bounds.fixedOffsetY = fixedOffsetY;
  bounds.fixedWidth = Math.max(1, fixedRight - fixedOffsetX);
  bounds.fixedHeight = Math.max(1, fixedBottom - fixedOffsetY);
  bounds.emptyCount = emptyCount;
}
