import {
  cursorForGridLine,
  hitTestGridLine,
  moveGridLine,
  sameGridLineHit
} from "../grid-lines";
import {
  clampSplitRegion,
  getCanvasNaturalPoint,
  hitTestRegion,
  isFullRegion,
  regionFromDrag
} from "../region-model";
import type {
  SplitRegion
} from "../types";
import {
  cursorForDragMode,
  normalizeGridSize,
  parseRegionNumber
} from "../utils";

import type { SpritePage } from "../sprite-page";

export const spritePageRegionMethods = {
  invalidateAutoBounds(render: boolean = true): void {
    this.autoBounds = null;
    this.clearSelectedBounds();
    if (render) {
      this.renderGridPreviewFromCurrentImage();
    }
  },

  handleRegionInput(): void {
    if (!this.canRunSpriteAction("editRegion")) return;
    if (!this.sheetImage) return;

    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    const current = this.getValidatedRegion(rows, cols);
    this.splitRegion = clampSplitRegion(
      {
        x: parseRegionNumber(this.els.regionX.value, current.x),
        y: parseRegionNumber(this.els.regionY.value, current.y),
        width: parseRegionNumber(this.els.regionW.value, current.width),
        height: parseRegionNumber(this.els.regionH.value, current.height),
      },
      rows,
      cols,
      this.sheetImage.naturalWidth,
      this.sheetImage.naturalHeight
    );
    this.gridLines = null;
    this.hoveredGridLine = null;
    this.invalidateAutoBounds(false);
    this.syncRegionInputs();
    this.renderGridPreviewFromCurrentImage();
  },

  getRegionPointerContext(event: PointerEvent) {
    const image = this.sheetImage;
    if (!image) return null;
    const point = getCanvasNaturalPoint(
      event,
      this.els.canvas,
      image.naturalWidth,
      image.naturalHeight
    );
    if (!point) return null;
    return {
      image,
      point,
      rows: normalizeGridSize(this.els.rows.value, 3),
      cols: normalizeGridSize(this.els.cols.value, 4),
    };
  },

  handleRegionPointerDown(event: PointerEvent): void {
    if (!this.canRunSpriteAction("editRegion")) return;
    const pointer = this.getRegionPointerContext(event);
    if (!pointer) return;
    const { image, point, rows, cols } = pointer;
    const startRegion = this.getValidatedRegion(rows, cols);
    const gridLines = this.getActiveGridLines(rows, cols, startRegion);
    const hitMode = hitTestRegion(point.x, point.y, this.splitRegion, this.getCanvasDisplayScale());
    const isRegionResizeHandle = Boolean(hitMode && hitMode !== "move");
    if (!isRegionResizeHandle) {
      const gridLineHit = hitTestGridLine(
        point.x,
        point.y,
        rows,
        cols,
        startRegion,
        gridLines,
        this.getCanvasNaturalTolerance()
      );
      if (gridLineHit) {
        this.gridLineDrag = {
          ...gridLineHit,
          pointerId: event.pointerId,
        };
        this.hoveredGridLine = gridLineHit;
        this.els.canvas.setPointerCapture(event.pointerId);
        this.els.canvas.style.cursor = cursorForGridLine(gridLineHit) ?? "default";
        this.drawGridPreview(rows, cols);
        event.preventDefault();
        return;
      }
    }
    if (
      !isRegionResizeHandle &&
      this.handleBoundaryPointerDown(point.x, point.y, rows, cols, startRegion)
    ) {
      event.preventDefault();
      return;
    }
    const mode = hitMode && !(
      hitMode === "move" &&
      isFullRegion(startRegion, image.naturalWidth, image.naturalHeight)
    )
      ? hitMode
      : "new";

    this.regionDrag = {
      mode,
      startX: point.x,
      startY: point.y,
      startRegion,
      pointerId: event.pointerId,
    };
    this.els.canvas.setPointerCapture(event.pointerId);
    event.preventDefault();
  },

  handleRegionPointerMove(event: PointerEvent): void {
    if (!this.canRunSpriteAction("editRegion")) {
      this.els.canvas.style.cursor = "default";
      return;
    }
    const pointer = this.getRegionPointerContext(event);
    if (!pointer) return;
    const { image, point, rows, cols } = pointer;
    const gridDrag = this.gridLineDrag;
    if (gridDrag && gridDrag.pointerId === event.pointerId) {
      const region = this.getValidatedRegion(rows, cols);
      this.gridLines = moveGridLine(
        this.gridLines,
        rows,
        cols,
        region,
        gridDrag.axis,
        gridDrag.lineIndex,
        gridDrag.axis === "x" ? point.x : point.y
      );
      this.hoveredGridLine = {
        axis: gridDrag.axis,
        lineIndex: gridDrag.lineIndex,
      };
      this.invalidateAutoBounds(false);
      this.drawGridPreview(rows, cols);
      this.els.canvas.style.cursor = cursorForGridLine(this.hoveredGridLine) ?? "default";
      event.preventDefault();
      return;
    }

    const drag = this.regionDrag;
    if (!drag) {
      const regionHit = hitTestRegion(point.x, point.y, this.splitRegion, this.getCanvasDisplayScale());
      const isRegionResizeHandle = Boolean(regionHit && regionHit !== "move");
      const region = this.getValidatedRegion(rows, cols);
      const gridLineHit = !isRegionResizeHandle
        ? hitTestGridLine(
          point.x,
          point.y,
          rows,
          cols,
          region,
          this.getActiveGridLines(rows, cols, region),
          this.getCanvasNaturalTolerance()
        )
        : null;
      if (!sameGridLineHit(this.hoveredGridLine, gridLineHit)) {
        this.hoveredGridLine = gridLineHit;
        this.drawGridPreview(rows, cols);
      }
      this.els.canvas.style.cursor = isRegionResizeHandle
        ? cursorForDragMode(regionHit)
        : cursorForGridLine(gridLineHit) ?? cursorForDragMode(regionHit);
      return;
    }

    this.splitRegion = regionFromDrag(
      point.x,
      point.y,
      rows,
      cols,
      image.naturalWidth,
      image.naturalHeight,
      drag
    );
    if (drag.mode !== "move") {
      this.gridLines = null;
      this.hoveredGridLine = null;
    }
    this.invalidateAutoBounds(false);
    this.syncRegionInputs();
    this.renderGridPreviewFromCurrentImage();
    event.preventDefault();
  },

  handleRegionPointerUp(event: PointerEvent): void {
    if (!this.canRunSpriteAction("editRegion")) return;

    if (this.gridLineDrag && this.gridLineDrag.pointerId === event.pointerId) {
      this.gridLineDrag = null;
      this.releaseCanvasPointer(event.pointerId);
      if (!this.sheetImage) {
        this.els.canvas.style.cursor = "default";
        return;
      }
      const rows = normalizeGridSize(this.els.rows.value, 3);
      const cols = normalizeGridSize(this.els.cols.value, 4);
      const point = getCanvasNaturalPoint(
        event,
        this.els.canvas,
        this.sheetImage.naturalWidth,
        this.sheetImage.naturalHeight
      );
      const region = this.getValidatedRegion(rows, cols);
      this.hoveredGridLine = point
        ? hitTestGridLine(
          point.x,
          point.y,
          rows,
          cols,
          region,
          this.getActiveGridLines(rows, cols, region),
          this.getCanvasNaturalTolerance()
        )
        : null;
      this.els.canvas.style.cursor = cursorForGridLine(this.hoveredGridLine) ?? "default";
      this.drawGridPreview(rows, cols);
      return;
    }

    if (!this.regionDrag || this.regionDrag.pointerId !== event.pointerId) return;

    this.regionDrag = null;
    this.releaseCanvasPointer(event.pointerId);
    if (!this.sheetImage) {
      this.els.canvas.style.cursor = "default";
      return;
    }
    const point = getCanvasNaturalPoint(
      event,
      this.els.canvas,
      this.sheetImage.naturalWidth,
      this.sheetImage.naturalHeight
    );
    this.els.canvas.style.cursor = point
      ? cursorForDragMode(hitTestRegion(point.x, point.y, this.splitRegion, this.getCanvasDisplayScale()))
      : "default";
  },

  setFullRegion(render: boolean = true, force: boolean = false): void {
    if (!force && !this.canRunSpriteAction("editRegion") && this.sheetImage) return;
    if (!this.sheetImage) {
      this.splitRegion = null;
      this.syncRegionInputs();
      return;
    }

    this.splitRegion = {
      x: 0,
      y: 0,
      width: this.sheetImage.naturalWidth,
      height: this.sheetImage.naturalHeight,
    };
    this.gridLines = null;
    this.hoveredGridLine = null;
    this.invalidateAutoBounds(false);
    this.syncRegionInputs();
    if (render) {
      this.renderGridPreviewFromCurrentImage();
    }
  },

  getValidatedRegion(rows: number, cols: number): SplitRegion {
    if (!this.sheetImage || !this.splitRegion) {
      throw new Error("图片区域尚未初始化");
    }

    this.splitRegion = clampSplitRegion(
      this.splitRegion,
      rows,
      cols,
      this.sheetImage.naturalWidth,
      this.sheetImage.naturalHeight
    );
    return this.splitRegion;
  },

  syncRegionInputs(): void {
    const enabled = Boolean(this.sheetImage && this.splitRegion) && this.canRunSpriteAction("editRegion");
    [
      this.els.regionX,
      this.els.regionY,
      this.els.regionW,
      this.els.regionH,
    ].forEach((input) => {
      input.disabled = !enabled;
    });
    this.els.regionFull.disabled = !enabled;
    this.els.resetGridLines.disabled = !enabled;
    this.els.autoTrim.disabled = !enabled;
    this.els.autoExpand.disabled = !enabled;
    this.els.autoBgMode.disabled = !enabled;
    this.els.autoTrimMode.disabled = !enabled;
    this.els.autoThreshold.disabled = !enabled;
    this.els.detectBounds.disabled = !enabled;
    this.syncBoundaryControls();

    if (!enabled || !this.splitRegion || !this.sheetImage) {
      this.els.regionX.value = "0";
      this.els.regionY.value = "0";
      this.els.regionW.value = "0";
      this.els.regionH.value = "0";
      return;
    }

    this.els.regionX.max = String(this.sheetImage.naturalWidth - 1);
    this.els.regionY.max = String(this.sheetImage.naturalHeight - 1);
    this.els.regionW.max = String(this.sheetImage.naturalWidth);
    this.els.regionH.max = String(this.sheetImage.naturalHeight);
    this.els.regionX.value = String(this.splitRegion.x);
    this.els.regionY.value = String(this.splitRegion.y);
    this.els.regionW.value = String(this.splitRegion.width);
    this.els.regionH.value = String(this.splitRegion.height);
  },


} satisfies ThisType<SpritePage>;

export type SpritePageRegionMethods = typeof spritePageRegionMethods;
