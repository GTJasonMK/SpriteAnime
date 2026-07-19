import {
  importImageToLibrary,
  openImageFile
} from "../../api/commands";
import { getErrorMessage, isUserCancelError } from "../../utils/errors";
import { CanvasPlayer } from "../../widgets/canvas-player";
import { getBoundaryEditorInputTarget, isEditableKeyboardTarget } from "./controller/helpers";
import { cacheSpriteElements, type SpriteElements } from "./dom";
import { SpriteFrameController } from "./frame-controller";
import { updateGridPresetState } from "./grid-controls";
import {
  addImageSource as addImageSourceOption
} from "./image-sources";
import type {
  AutoBoundsResult,
  BoundaryEditSnapshot,
  GridLineDragState,
  GridLineHit,
  GridLines,
  RegionDragState,
  SplitRegion,
} from "./types";
import { normalizeGridSize } from "./utils";
import { notifyTaskStateChanged } from "../../workflows/events";
import {
  deriveSpriteBaseState,
  getSpriteWorkflowPermissions,
  type SpriteAction,
  type SpriteWorkflowState,
} from "./workflow-state";

import { spritePageBoundaryMethods, type SpritePageBoundaryMethods } from "./controller/boundary";
import { spritePageGridMethods, type SpritePageGridMethods } from "./controller/grid";
import { spritePageLoadingMethods, type SpritePageLoadingMethods } from "./controller/loading";
import { spritePageOutputMethods, type SpritePageOutputMethods } from "./controller/output";
import { spritePageRegionMethods, type SpritePageRegionMethods } from "./controller/region";
import { spriteWorkspaceMethods, type SpriteWorkspaceMethods } from "./workspace";
/// 序列帧预览页面控制器
export class SpritePage {
  // 状态
  sheetImage: HTMLImageElement | null = null;
  sheetImagePath: string = "";
  splitRegion: SplitRegion | null = null;
  regionDrag: RegionDragState | null = null;
  gridLines: GridLines | null = null;
  gridLineDrag: GridLineDragState | null = null;
  hoveredGridLine: GridLineHit | null = null;
  autoBounds: AutoBoundsResult | null = null;
  selectedBoundsIndex: number | null = null;
  boundaryEditOriginal: BoundaryEditSnapshot | null = null;
  workflowState: SpriteWorkflowState = "empty";

  // 子组件
  canvasPlayer!: CanvasPlayer;
  frameController!: SpriteFrameController;

  // DOM元素缓存
  els!: SpriteElements;

  constructor() {
    this.els = cacheSpriteElements();
    this.canvasPlayer = new CanvasPlayer(this.els.canvas);
    this.frameController = new SpriteFrameController(this.els, this.canvasPlayer, {
      renderFallbackPreview: () => this.renderFallbackPreview(),
    });
  }

  getSpriteWorkflowContext() {
    return {
      hasImage: Boolean(this.sheetImage && this.splitRegion),
      hasFrames: this.frameController.hasFrames(),
    };
  }

  canRunSpriteAction(action: SpriteAction): boolean {
    return getSpriteWorkflowPermissions(this.workflowState, this.getSpriteWorkflowContext())[action];
  }

  setWorkflowState(nextState: SpriteWorkflowState): void {
    this.workflowState = nextState;
    this.syncBoundaryControls();
    notifyTaskStateChanged();
  }

  settleWorkflowState(): void {
    this.setWorkflowState(deriveSpriteBaseState(this.getSpriteWorkflowContext()));
  }

  init(): void {
    this.bindEvents();
    updateGridPresetState(
      this.els,
      normalizeGridSize(this.els.rows.value, 3),
      normalizeGridSize(this.els.cols.value, 4)
    );
    this.syncAutoBoundaryOptions();
    this.syncRegionInputs();
    this.syncBoundaryControls();
  }

  /// 添加图片到源选择列表
  addImageSource(path: string, select: boolean = true): boolean {
    return addImageSourceOption(this.els, path, select);
  }

  bindEvents(): void {
    this.els.imageSelect.addEventListener("change", () => {
      this.handlePreviewGrid();
    });

    this.els.rows.addEventListener("input", () => {
      this.gridLines = null;
      this.hoveredGridLine = null;
      this.autoBounds = null;
      this.clearSelectedBounds();
      this.renderGridPreviewFromCurrentImage();
    });
    this.els.cols.addEventListener("input", () => {
      this.gridLines = null;
      this.hoveredGridLine = null;
      this.autoBounds = null;
      this.clearSelectedBounds();
      this.renderGridPreviewFromCurrentImage();
    });
    this.els.gridPresets.forEach((button) => {
      button.addEventListener("click", () => {
        this.setGridSize(
          Number.parseInt(button.dataset.rows!, 10),
          Number.parseInt(button.dataset.cols!, 10)
        );
      });
    });
    [
      this.els.regionX,
      this.els.regionY,
      this.els.regionW,
      this.els.regionH,
    ].forEach((input) => {
      input.addEventListener("input", () => this.handleRegionInput());
    });
    this.els.regionFull.addEventListener("click", () => this.setFullRegion());
    this.els.resetGridLines.addEventListener("click", () => this.resetGridLines());
    this.els.autoTrim.addEventListener("change", () => {
      this.autoBounds = null;
      this.clearSelectedBounds();
      this.syncAutoBoundaryOptions();
      this.renderGridPreviewFromCurrentImage();
    });
    this.els.autoBgMode.addEventListener("change", () => this.invalidateAutoBounds());
    this.els.autoExpand.addEventListener("change", () => this.invalidateAutoBounds());
    this.els.autoTrimMode.addEventListener("change", () => {
      this.syncBoundaryControls();
      this.renderGridPreviewFromCurrentImage();
    });
    this.els.autoThreshold.addEventListener("input", () => this.invalidateAutoBounds());
    this.els.detectBounds.addEventListener("click", () => {
      this.els.autoTrim.checked = true;
      this.syncAutoBoundaryOptions();
      this.clearSelectedBounds();
      void this.detectAutoBounds();
    });
    this.els.boundaryAdd.addEventListener("click", () => this.handleAddBoundary());
    this.els.boundaryEdit.addEventListener("click", () => this.handleEditBoundary());
    this.els.boundaryDelete.addEventListener("click", () => this.handleDeleteBoundary());
    this.els.boundaryApply.addEventListener("click", () => this.handleApplyBoundary());
    [
      this.els.boundaryLeft,
      this.els.boundaryTop,
      this.els.boundaryRight,
      this.els.boundaryBottom,
      this.els.boundaryAnchorX,
    ].forEach((input) => {
      input.addEventListener("input", () => this.handleLiveBoundaryInput());
    });
    this.els.boundaryAnchorCenter.addEventListener("click", () => this.handleCenterBoundaryAnchor());
    this.els.boundaryCancel.addEventListener("click", () => this.closeBoundaryEditor(true));
    this.els.boundaryClose.addEventListener("click", () => this.clearSelectedBounds(true));
    this.els.boundaryEditorDelete.addEventListener("click", () => this.handleDeleteBoundary());
    document.addEventListener("keydown", (event) => this.handleBoundaryKeyboard(event));
    this.els.canvas.addEventListener("pointerdown", (event) => this.handleRegionPointerDown(event));
    this.els.canvas.addEventListener("pointermove", (event) => this.handleRegionPointerMove(event));
    this.els.canvas.addEventListener("pointerup", (event) => this.handleRegionPointerUp(event));
    this.els.canvas.addEventListener("pointercancel", (event) => this.handleRegionPointerUp(event));

    this.frameController.bindEvents();
  }

  handleBoundaryKeyboard(event: KeyboardEvent): void {
    if (!this.canRunSpriteAction("editBoundary")) {
      return;
    }
    if (this.selectedBoundsIndex === null) {
      return;
    }

    if (event.key === "Escape") {
      this.clearSelectedBounds(true);
      return;
    }

    if (
      event.key !== "ArrowLeft" &&
      event.key !== "ArrowRight"
    ) {
      return;
    }

    if (
      event.altKey ||
      event.ctrlKey ||
      event.metaKey
    ) {
      return;
    }

    const boundaryInput = getBoundaryEditorInputTarget(event.target);
    if (!boundaryInput && isEditableKeyboardTarget(event.target)) {
      return;
    }

    if (boundaryInput && !this.applyBoundaryEditorValues()) {
      return;
    }

    const delta = event.key === "ArrowLeft" ? -1 : 1;
    if (this.switchSelectedBoundary(delta)) {
      event.preventDefault();
      boundaryInput?.focus();
    }
  }

  /// 处理：加载图片并分割为帧
  async handlePickImage(): Promise<void> {
    if (!this.canRunSpriteAction("pickImage")) return;
    try {
      const result = await openImageFile();
      const imported = await importImageToLibrary(result.file_path);
      this.addImageSource(imported.file_path);
      this.els.imageSelect.value = imported.file_path;
      await this.handlePreviewGrid();
    } catch (err) {
      if (!isUserCancelError(err)) {
        console.error("[sprite] 选择图片失败:", err);
        alert(`选择图片失败: ${getErrorMessage(err)}`);
      }
    }
  }

  async handlePreviewGrid(): Promise<void> {
    if (!this.canRunSpriteAction("previewGrid")) {
      return;
    }

    let imagePath = this.els.imageSelect.value;
    if (!imagePath) {
      try {
        const result = await openImageFile();
        const imported = await importImageToLibrary(result.file_path);
        this.addImageSource(imported.file_path);
        imagePath = imported.file_path;
      } catch (err) {
        if (!isUserCancelError(err)) {
          console.error("[sprite] 选择待切分图片失败:", err);
          alert(`选择图片失败: ${getErrorMessage(err)}`);
        }
        return;
      }
    }

    await this.loadGridPreview(imagePath);
  }


}

export interface SpritePage extends SpritePageLoadingMethods, SpritePageBoundaryMethods, SpritePageGridMethods, SpritePageRegionMethods, SpritePageOutputMethods, SpriteWorkspaceMethods { }

Object.assign(
  SpritePage.prototype,
  spritePageLoadingMethods,
  spritePageBoundaryMethods,
  spritePageGridMethods,
  spritePageRegionMethods,
  spritePageOutputMethods,
  spriteWorkspaceMethods
);
