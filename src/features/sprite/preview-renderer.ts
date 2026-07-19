import { defaultFrameRegion } from "./boundary-model";
import {
  createGridCellRects,
  getGridCellSizeSummary,
  isEvenGridLines,
  normalizeGridLines,
} from "./grid-lines";
import type { AutoBoundsResult, FrameBounds, GridLineHit, GridLines, SplitRegion } from "./types";
import { clampNumber } from "./utils";

interface DrawGridPreviewOptions {
  canvas: HTMLCanvasElement;
  placeholder: HTMLElement;
  sheetImage: HTMLImageElement;
  rows: number;
  cols: number;
  region: SplitRegion;
  gridLines: GridLines | null;
  highlightedGridLine: GridLineHit | null;
  autoBounds: AutoBoundsResult | null;
  autoBoundsValid: boolean;
  selectedBoundsIndex: number | null;
  autoTrimMode: string;
}

export function drawGridPreviewScene(options: DrawGridPreviewOptions): string | null {
  const {
    canvas,
    placeholder,
    sheetImage,
    rows,
    cols,
    region,
    gridLines,
    highlightedGridLine,
    autoBounds,
    autoBoundsValid,
    selectedBoundsIndex,
    autoTrimMode,
  } = options;

  const ctx = canvas.getContext("2d");
  const area = canvas.closest<HTMLElement>(".preview-area");
  if (!ctx || !area) return null;

  const padding = 32;
  const maxW = Math.max(240, area.clientWidth - padding);
  const maxH = Math.max(240, area.clientHeight - padding);
  const scale = Math.min(maxW / sheetImage.naturalWidth, maxH / sheetImage.naturalHeight, 1.5);
  const w = Math.max(1, Math.round(sheetImage.naturalWidth * scale));
  const h = Math.max(1, Math.round(sheetImage.naturalHeight * scale));

  canvas.width = w;
  canvas.height = h;
  ctx.clearRect(0, 0, w, h);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(sheetImage, 0, 0, w, h);

  const regionX = region.x * scale;
  const regionY = region.y * scale;
  const regionW = region.width * scale;
  const regionH = region.height * scale;
  const normalizedGridLines = normalizeGridLines(gridLines, rows, cols, region);
  const cellRects = createGridCellRects(rows, cols, region, normalizedGridLines);

  ctx.save();
  drawDimmedOutsideRegion(ctx, w, h, regionX, regionY, regionW, regionH);
  drawGridLines(ctx, {
    rows,
    cols,
    regionX,
    regionY,
    regionW,
    regionH,
    scale,
    lines: normalizedGridLines,
    highlightedGridLine,
  });
  drawFrameLabels(ctx, cellRects, scale);
  drawRegionHandles(ctx, regionX, regionY, regionW, regionH);
  drawAutoBounds(ctx, {
    scale,
    bounds: autoBounds,
    autoBoundsValid,
    selectedBoundsIndex,
    autoTrimMode,
  });
  ctx.restore();

  area.classList.add("has-image");
  placeholder.style.display = "none";

  const cellSizeNote = getGridCellSizeSummary(cellRects);
  const gridLineNote = isEvenGridLines(rows, cols, region, normalizedGridLines) ? "" : " | 非均分";
  const autoNote = autoBounds && autoBoundsValid
    ? ` | 自动: ${autoBounds.frameBounds.length - autoBounds.emptyCount}/${autoBounds.frameBounds.length}${autoBounds.allowExpand ? ` | 扩展: ${autoBounds.expandPixels}px` : ""} | 固定帧: ${autoBounds.fixedWidth}x${autoBounds.fixedHeight}`
    : "";

  return `区域: ${region.width}x${region.height} @ ${region.x},${region.y} | 网格: ${rows}行 x ${cols}列 | 单帧: ${cellSizeNote}${gridLineNote}${autoNote}`;
}

function drawDimmedOutsideRegion(
  ctx: CanvasRenderingContext2D,
  canvasWidth: number,
  canvasHeight: number,
  regionX: number,
  regionY: number,
  regionW: number,
  regionH: number
): void {
  ctx.fillStyle = "rgba(44, 38, 33, 0.42)";
  ctx.fillRect(0, 0, canvasWidth, regionY);
  ctx.fillRect(0, regionY + regionH, canvasWidth, canvasHeight - regionY - regionH);
  ctx.fillRect(0, regionY, regionX, regionH);
  ctx.fillRect(regionX + regionW, regionY, canvasWidth - regionX - regionW, regionH);
}

function drawGridLines(
  ctx: CanvasRenderingContext2D,
  options: {
    rows: number;
    cols: number;
    regionX: number;
    regionY: number;
    regionW: number;
    regionH: number;
    scale: number;
    lines: GridLines;
    highlightedGridLine: GridLineHit | null;
  }
): void {
  const {
    rows,
    cols,
    regionX,
    regionY,
    regionW,
    regionH,
    scale,
    lines,
    highlightedGridLine,
  } = options;

  ctx.strokeStyle = "rgba(198, 97, 63, 0.95)";
  ctx.lineWidth = 1;
  ctx.setLineDash([8, 5]);

  for (let c = 1; c < cols; c++) {
    const x = Math.round(regionX + lines.x[c] * scale) + 0.5;
    drawAdjustableGridLine(
      ctx,
      "x",
      c,
      x,
      regionY,
      x,
      regionY + regionH,
      highlightedGridLine
    );
  }

  for (let r = 1; r < rows; r++) {
    const y = Math.round(regionY + lines.y[r] * scale) + 0.5;
    drawAdjustableGridLine(
      ctx,
      "y",
      r,
      regionX,
      y,
      regionX + regionW,
      y,
      highlightedGridLine
    );
  }

  ctx.setLineDash([]);
  ctx.strokeStyle = "rgba(44, 38, 33, 0.75)";
  ctx.strokeRect(
    regionX + 0.5,
    regionY + 0.5,
    Math.max(1, regionW - 1),
    Math.max(1, regionH - 1)
  );
  ctx.strokeStyle = "rgba(198, 97, 63, 0.95)";
  ctx.lineWidth = 2;
  ctx.strokeRect(
    regionX + 1,
    regionY + 1,
    Math.max(1, regionW - 2),
    Math.max(1, regionH - 2)
  );
}

function drawFrameLabels(
  ctx: CanvasRenderingContext2D,
  cellRects: SplitRegion[],
  scale: number
): void {
  ctx.font = "600 11px sans-serif";
  ctx.textBaseline = "top";
  cellRects.forEach((rect, index) => {
    const frameW = rect.width * scale;
    const frameH = rect.height * scale;
    if (frameW < 34 || frameH < 26) return;

    const label = String(index + 1);
    const x = Math.round(rect.x * scale) + 5;
    const y = Math.round(rect.y * scale) + 5;
    const labelW = Math.max(18, ctx.measureText(label).width + 8);
    ctx.fillStyle = "rgba(250, 248, 245, 0.88)";
    ctx.fillRect(x - 2, y - 2, labelW, 18);
    ctx.fillStyle = "rgba(198, 97, 63, 0.95)";
    ctx.fillText(label, x + 2, y + 1);
  });
}

function drawAdjustableGridLine(
  ctx: CanvasRenderingContext2D,
  axis: "x" | "y",
  lineIndex: number,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  highlightedGridLine: GridLineHit | null
): void {
  const highlighted = highlightedGridLine?.axis === axis &&
    highlightedGridLine.lineIndex === lineIndex;

  ctx.save();
  ctx.strokeStyle = highlighted ? "rgba(255, 214, 142, 0.98)" : "rgba(198, 97, 63, 0.95)";
  ctx.lineWidth = highlighted ? 2 : 1;
  ctx.setLineDash(highlighted ? [] : [8, 5]);
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(x2, y2);
  ctx.stroke();

  if (highlighted) {
    drawGridLineGrip(ctx, axis, (x1 + x2) / 2, (y1 + y2) / 2);
  }
  ctx.restore();
}

function drawGridLineGrip(
  ctx: CanvasRenderingContext2D,
  axis: "x" | "y",
  x: number,
  y: number
): void {
  const width = axis === "x" ? 8 : 26;
  const height = axis === "x" ? 26 : 8;
  const left = Math.round(x - width / 2) + 0.5;
  const top = Math.round(y - height / 2) + 0.5;

  ctx.fillStyle = "rgba(44, 38, 33, 0.92)";
  ctx.strokeStyle = "rgba(255, 214, 142, 0.98)";
  ctx.lineWidth = 1;
  ctx.setLineDash([]);
  ctx.fillRect(left, top, width, height);
  ctx.strokeRect(left, top, width, height);
}

function drawRegionHandles(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number
): void {
  const points = [
    [x, y],
    [x + width / 2, y],
    [x + width, y],
    [x, y + height / 2],
    [x + width, y + height / 2],
    [x, y + height],
    [x + width / 2, y + height],
    [x + width, y + height],
  ];

  ctx.fillStyle = "rgba(250, 248, 245, 0.95)";
  ctx.strokeStyle = "rgba(198, 97, 63, 0.95)";
  ctx.lineWidth = 1;
  points.forEach(([px, py]) => {
    ctx.fillRect(px - 4, py - 4, 8, 8);
    ctx.strokeRect(px - 4.5, py - 4.5, 9, 9);
  });
}

function drawAutoBounds(
  ctx: CanvasRenderingContext2D,
  options: {
    scale: number;
    bounds: AutoBoundsResult | null;
    autoBoundsValid: boolean;
    selectedBoundsIndex: number | null;
    autoTrimMode: string;
  }
): void {
  const { scale, bounds, autoBoundsValid, selectedBoundsIndex, autoTrimMode } = options;
  if (!bounds || !autoBoundsValid) {
    return;
  }

  ctx.save();
  ctx.lineWidth = 1;
  ctx.strokeStyle = "rgba(44, 126, 79, 0.95)";
  ctx.setLineDash([4, 3]);
  bounds.frameBounds.forEach((frame) => {
    if (frame.empty) return;
    ctx.strokeRect(
      frame.x * scale + 0.5,
      frame.y * scale + 0.5,
      Math.max(1, frame.width * scale - 1),
      Math.max(1, frame.height * scale - 1)
    );
  });

  ctx.setLineDash([]);
  bounds.frameBounds.forEach((frame) => {
    if (frame.empty) return;
    drawBoundaryAnchor(ctx, frame, scale, false);
  });

  if (autoTrimMode === "fixed") {
    ctx.strokeStyle = "rgba(48, 91, 142, 0.95)";
    ctx.setLineDash([]);
    bounds.frameBounds.forEach((frame) => {
      ctx.strokeRect(
        (frame.cellX + bounds.fixedOffsetX) * scale + 0.5,
        (frame.cellY + bounds.fixedOffsetY) * scale + 0.5,
        Math.max(1, bounds.fixedWidth * scale - 1),
        Math.max(1, bounds.fixedHeight * scale - 1)
      );
    });
  }

  if (
    selectedBoundsIndex !== null &&
    selectedBoundsIndex >= 0 &&
    selectedBoundsIndex < bounds.frameBounds.length
  ) {
    const frame = bounds.frameBounds[selectedBoundsIndex];
    const highlight = frame.empty
      ? {
        x: frame.cellX,
        y: frame.cellY,
        width: frame.cellWidth,
        height: frame.cellHeight,
      }
      : frame;
    ctx.strokeStyle = "rgba(224, 171, 106, 0.98)";
    ctx.lineWidth = 2;
    ctx.setLineDash([]);
    ctx.strokeRect(
      highlight.x * scale + 1,
      highlight.y * scale + 1,
      Math.max(1, highlight.width * scale - 2),
      Math.max(1, highlight.height * scale - 2)
    );
    drawBoundaryAnchor(ctx, frame, scale, true);
  }
  ctx.restore();
}

function drawBoundaryAnchor(
  ctx: CanvasRenderingContext2D,
  frame: FrameBounds,
  scale: number,
  selected: boolean
): void {
  const region = frame.empty ? defaultFrameRegion(frame) : frame;
  const anchorX = clampNumber(frame.anchorX, region.x, region.x + region.width);
  const x = Math.round(anchorX * scale) + 0.5;
  const top = region.y * scale;
  const bottom = (region.y + region.height) * scale;

  ctx.save();
  ctx.setLineDash([]);
  ctx.lineWidth = selected ? 2 : 1;
  ctx.strokeStyle = selected ? "rgba(255, 214, 142, 0.98)" : "rgba(224, 171, 106, 0.88)";
  ctx.beginPath();
  ctx.moveTo(x, top);
  ctx.lineTo(x, bottom);
  ctx.stroke();

  const tick = selected ? 5 : 4;
  ctx.beginPath();
  ctx.moveTo(x - tick, top + 0.5);
  ctx.lineTo(x + tick, top + 0.5);
  ctx.moveTo(x - tick, bottom - 0.5);
  ctx.lineTo(x + tick, bottom - 0.5);
  ctx.stroke();
  ctx.restore();
}
