import type { SpriteWorkspaceSnapshot } from "../../workspace/types";
import type { SpritePage } from "./sprite-page";
import { normalizeGridLines } from "./grid-lines";
import { clampSplitRegion } from "./region-model";
import { sameRegion } from "./utils";

export const spriteWorkspaceMethods = {
  createWorkspaceSnapshot(): SpriteWorkspaceSnapshot {
    const frameState = this.frameController.getWorkspaceState();
    return {
      sheetImagePath: this.sheetImagePath,
      rows: this.els.rows.value,
      cols: this.els.cols.value,
      splitRegion: this.splitRegion ? { ...this.splitRegion } : null,
      gridLines: this.gridLines ? { x: [...this.gridLines.x], y: [...this.gridLines.y] } : null,
      autoTrim: this.els.autoTrim.checked,
      autoExpand: this.els.autoExpand.checked,
      autoBackgroundMode: this.els.autoBgMode.value,
      autoTrimMode: this.els.autoTrimMode.value,
      autoThreshold: this.els.autoThreshold.value,
      autoBounds: this.autoBounds ? structuredClone(this.autoBounds) : null,
      selectedBoundsIndex: this.selectedBoundsIndex,
      boundaryEditOriginal: this.boundaryEditOriginal
        ? structuredClone(this.boundaryEditOriginal)
        : null,
      boundaryEditorOpen: this.els.boundaryPanel.classList.contains("active"),
      framesLoaded: this.frameController.hasFrames(),
      selectedFrameIndices: frameState.selectedIndices,
      currentFramePosition: frameState.currentFramePosition,
      playbackFps: frameState.fps,
      playbackScale: frameState.scale,
    };
  },

  async restoreWorkspaceSnapshot(snapshot: SpriteWorkspaceSnapshot): Promise<void> {
    const rows = requireGridSize(snapshot.rows, "序列帧行数");
    const cols = requireGridSize(snapshot.cols, "序列帧列数");
    this.setGridSize(rows, cols, false);
    this.els.autoTrim.checked = snapshot.autoTrim;
    this.els.autoExpand.checked = snapshot.autoExpand;
    setSelectValue(this.els.autoBgMode, snapshot.autoBackgroundMode, "自动边界背景模式");
    setSelectValue(this.els.autoTrimMode, snapshot.autoTrimMode, "自动边界裁切模式");
    this.els.autoThreshold.value = snapshot.autoThreshold;
    this.syncAutoBoundaryOptions();

    if (!snapshot.sheetImagePath) {
      if (snapshot.splitRegion || snapshot.framesLoaded || snapshot.autoBounds) {
        throw new Error("序列帧工作区缺少源图片，但包含依赖源图片的编辑状态");
      }
      this.frameController.restoreWorkspaceState({
        selectedIndices: [],
        currentFramePosition: 0,
        fps: snapshot.playbackFps,
        scale: snapshot.playbackScale,
      });
      return;
    }

    this.addImageSource(snapshot.sheetImagePath);
    this.els.imageSelect.value = snapshot.sheetImagePath;
    const loaded = await this.loadGridPreview(snapshot.sheetImagePath);
    if (!loaded || !this.sheetImage) throw new Error("恢复序列帧源图片失败");
    if (!snapshot.splitRegion) throw new Error("序列帧工作区缺少拆分区域");
    const clamped = clampSplitRegion(
      snapshot.splitRegion,
      rows,
      cols,
      this.sheetImage.naturalWidth,
      this.sheetImage.naturalHeight
    );
    if (!sameRegion(clamped, snapshot.splitRegion)) {
      throw new Error("序列帧拆分区域超出源图片范围");
    }
    this.splitRegion = { ...snapshot.splitRegion };
    validateGridLines(snapshot.gridLines, rows, cols, this.splitRegion);
    this.gridLines = snapshot.gridLines
      ? { x: [...snapshot.gridLines.x], y: [...snapshot.gridLines.y] }
      : null;
    this.autoBounds = snapshot.autoBounds ? structuredClone(snapshot.autoBounds) : null;
    this.selectedBoundsIndex = null;
    this.boundaryEditOriginal = null;
    this.syncRegionInputs();
    this.drawGridPreview(rows, cols);

    if (this.autoBounds && !this.isAutoBoundsValid(rows, cols, this.splitRegion)) {
      throw new Error("序列帧自动边界与当前网格不一致");
    }
    if (snapshot.framesLoaded) {
      await this.handleLoadSplit();
      if (!this.frameController.hasFrames()) {
        throw new Error("恢复序列帧拆分结果失败");
      }
      this.frameController.restoreWorkspaceState({
        selectedIndices: snapshot.selectedFrameIndices,
        currentFramePosition: snapshot.currentFramePosition,
        fps: snapshot.playbackFps,
        scale: snapshot.playbackScale,
      });
      return;
    }

    this.frameController.restoreWorkspaceState({
      selectedIndices: [],
      currentFramePosition: 0,
      fps: snapshot.playbackFps,
      scale: snapshot.playbackScale,
    });
    restoreBoundaryEditor(this, snapshot);
    this.settleWorkflowState();
    this.drawGridPreview(rows, cols);
  },
} satisfies ThisType<SpritePage>;

function restoreBoundaryEditor(page: SpritePage, snapshot: SpriteWorkspaceSnapshot): void {
  const index = snapshot.selectedBoundsIndex;
  if (index === null) {
    if (snapshot.boundaryEditOriginal || snapshot.boundaryEditorOpen) {
      throw new Error("边界编辑器快照缺少选中边界");
    }
    return;
  }
  if (!page.autoBounds || index < 0 || index >= page.autoBounds.frameBounds.length) {
    throw new Error("边界编辑器快照索引无效");
  }
  page.selectedBoundsIndex = index;
  if (snapshot.boundaryEditOriginal && snapshot.boundaryEditOriginal.index !== index) {
    throw new Error("边界编辑原始状态与选中边界不一致");
  }
  page.boundaryEditOriginal = snapshot.boundaryEditOriginal
    ? structuredClone(snapshot.boundaryEditOriginal)
    : null;
  if (snapshot.boundaryEditorOpen) {
    page.openBoundaryEditor(index);
  } else {
    page.syncBoundaryControls();
  }
}

function validateGridLines(
  lines: SpriteWorkspaceSnapshot["gridLines"],
  rows: number,
  cols: number,
  region: NonNullable<SpriteWorkspaceSnapshot["splitRegion"]>
): void {
  if (!lines) return;
  const normalized = normalizeGridLines(lines, rows, cols, region);
  if (
    normalized.x.length !== lines.x.length ||
    normalized.y.length !== lines.y.length ||
    normalized.x.some((value, index) => value !== lines.x[index]) ||
    normalized.y.some((value, index) => value !== lines.y[index])
  ) {
    throw new Error("序列帧手动网格线快照无效");
  }
}

function requireGridSize(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > 20) {
    throw new Error(`${label}快照无效：${value}`);
  }
  return parsed;
}

function setSelectValue(select: HTMLSelectElement, value: string, label: string): void {
  if (!Array.from(select.options).some((option) => option.value === value)) {
    throw new Error(`${label}快照值无效：${value}`);
  }
  select.value = value;
}

export type SpriteWorkspaceMethods = typeof spriteWorkspaceMethods;
