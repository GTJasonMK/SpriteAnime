import {
  detectContentBounds,
  detectExpandedFrameBounds,
  estimateBackgroundColor,
} from "./bounds-detection";
import { createGridCellRects, getGridLinesSignature } from "./grid-lines";
import type { AutoBoundsResult, FrameBounds, SplitRegion } from "./types";
import { clampNumber, getAutoExpandPixels, getMinimumCellDimension, getRegionCenterX } from "./utils";

interface ImageDataReader {
  getImageData(sx: number, sy: number, sw: number, sh: number): ImageData;
}

export interface DetectFrameBoundsCoreOptions {
  imageWidth: number;
  imageHeight: number;
  rows: number;
  cols: number;
  region: SplitRegion;
  bgMode: string;
  threshold: number;
  allowExpand: boolean;
  reader: ImageDataReader;
  cellRects?: SplitRegion[];
  gridSignature?: string;
}

export function detectFrameBoundsFromImageData(
  options: DetectFrameBoundsCoreOptions
): AutoBoundsResult {
  const { imageWidth, imageHeight, rows, cols, region, bgMode, threshold, allowExpand, reader } = options;
  const cellRects = validateCellRects(
    options.cellRects ?? createGridCellRects(rows, cols, region, null),
    rows,
    cols
  );
  const minCellWidth = getMinimumCellDimension(cellRects, "width");
  const minCellHeight = getMinimumCellDimension(cellRects, "height");
  if (minCellWidth < 1 || minCellHeight < 1) {
    throw new Error("网格太密，单帧尺寸无效");
  }

  const expandPixels = allowExpand ? getAutoExpandPixels(minCellWidth, minCellHeight) : 0;
  let expandedFrameBounds: Array<SplitRegion | null> | null = null;
  if (allowExpand) {
    const fullImageData = reader.getImageData(0, 0, imageWidth, imageHeight);
    const regionImageData = reader.getImageData(region.x, region.y, region.width, region.height);
    const expandedBgColor = bgMode === "white"
      ? { r: 255, g: 255, b: 255, a: 255 }
      : estimateBackgroundColor(regionImageData.data, region.width, region.height);
    expandedFrameBounds = detectExpandedFrameBounds(
      fullImageData.data,
      fullImageData.width,
      fullImageData.height,
      cellRects,
      expandedBgColor,
      threshold,
      expandPixels
    );
  }

  const frameBounds: FrameBounds[] = [];
  let emptyCount = 0;
  let minRelX = Number.POSITIVE_INFINITY;
  let minRelY = Number.POSITIVE_INFINITY;
  let maxRelRight = Number.NEGATIVE_INFINITY;
  let maxRelBottom = Number.NEGATIVE_INFINITY;

  for (let row = 0; row < rows; row++) {
    for (let col = 0; col < cols; col++) {
      const index = row * cols + col;
      const cell = cellRects[index];
      let detectedOriginX = 0;
      let detectedOriginY = 0;
      let detected = expandedFrameBounds ? expandedFrameBounds[index] : null;

      if (!expandedFrameBounds) {
        const imageData = reader.getImageData(cell.x, cell.y, cell.width, cell.height);
        const bgColor = bgMode === "white"
          ? { r: 255, g: 255, b: 255, a: 255 }
          : estimateBackgroundColor(imageData.data, cell.width, cell.height);
        detected = detectContentBounds(imageData.data, cell.width, cell.height, bgColor, threshold);
        detectedOriginX = cell.x;
        detectedOriginY = cell.y;
      }

      if (!detected) {
        emptyCount += 1;
        frameBounds.push({
          index,
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
        continue;
      }

      const detectedX = detectedOriginX + detected.x;
      const detectedY = detectedOriginY + detected.y;
      const relX = detectedX - cell.x;
      const relY = detectedY - cell.y;
      const relRight = relX + detected.width;
      const relBottom = relY + detected.height;

      minRelX = Math.min(minRelX, relX);
      minRelY = Math.min(minRelY, relY);
      maxRelRight = Math.max(maxRelRight, relRight);
      maxRelBottom = Math.max(maxRelBottom, relBottom);

      frameBounds.push({
        index,
        row,
        col,
        cellX: cell.x,
        cellY: cell.y,
        cellWidth: cell.width,
        cellHeight: cell.height,
        x: detectedX,
        y: detectedY,
        width: detected.width,
        height: detected.height,
        anchorX: getRegionCenterX(detectedX, detected.width),
        empty: false,
      });
    }
  }

  if (emptyCount === frameBounds.length) {
    minRelX = 0;
    minRelY = 0;
    maxRelRight = minCellWidth;
    maxRelBottom = minCellHeight;
  }

  const padding = 2;
  const minOffsetX = allowExpand ? -expandPixels : 0;
  const minOffsetY = allowExpand ? -expandPixels : 0;
  const maxRight = allowExpand ? minCellWidth + expandPixels : minCellWidth;
  const maxBottom = allowExpand ? minCellHeight + expandPixels : minCellHeight;
  const fixedOffsetX = clampNumber(minRelX - padding, minOffsetX, Math.max(minOffsetX, minCellWidth - 1));
  const fixedOffsetY = clampNumber(minRelY - padding, minOffsetY, Math.max(minOffsetY, minCellHeight - 1));
  const fixedRight = clampNumber(maxRelRight + padding, fixedOffsetX + 1, maxRight);
  const fixedBottom = clampNumber(maxRelBottom + padding, fixedOffsetY + 1, maxBottom);

  return {
    rows,
    cols,
    region: { ...region },
    gridSignature: options.gridSignature ?? getGridLinesSignature(rows, cols, region, null),
    allowExpand,
    expandPixels,
    frameBounds,
    fixedOffsetX,
    fixedOffsetY,
    fixedWidth: fixedRight - fixedOffsetX,
    fixedHeight: fixedBottom - fixedOffsetY,
    emptyCount,
  };
}

function validateCellRects(cellRects: SplitRegion[], rows: number, cols: number): SplitRegion[] {
  if (cellRects.length !== rows * cols) {
    throw new Error("网格单元数量与行列设置不一致");
  }
  return cellRects;
}
