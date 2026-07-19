import {
  applyCanvasBackgroundTransparent,
  applyCanvasConnectedErase,
  readImageAsBase64,
  saveMattedImageDataUrl,
  upsertWorkbenchRecords
} from "../../api/commands";
import { loadImageFromDataUrl } from "../../utils/image";
import { parseClampedInt } from "../../utils/number";
import { getFileName } from "../../utils/path";
import {
  cloneImageData,
  getCanvasPixelPoint,
} from "./matting";
import {
  deriveGeneratorBaseState,
  type GeneratorWorkflowState
} from "./workflow";

import type { GeneratorPage } from "./image-page";
import type { GeneratedImageRecord } from "./types";
import { getEraseFailureText, recordToWorkbenchDto, requiredFileNameStem, requiredGeneratedRecordModel } from "./helpers";

export const generatorMattingMethods = {
  async enterMattingMode(): Promise<void> {
    if (!this.canRunGeneratorAction("enterMatting")) return;
    const record = this.getSelectedRecord();
    if (!record) return;

    this.setWorkflowState("matting");
    this.els.toolbarStatus.textContent = "抠图模式";
    const loaded = await this.loadMattingCanvas(record.path);
    if (!loaded) {
      this.setWorkflowState(deriveGeneratorBaseState(this.getWorkflowContext()));
      this.updateSelectedPreview();
    }
  },

  exitMattingMode(): void {
    if (!this.canRunGeneratorAction("exitMatting")) return;
    this.invalidateMattingCanvasLoad();
    this.mattingDirty = false;
    this.mattingCanvasPath = null;
    this.mattingWorkspaceImagePath = "";
    this.clearMattingCanvas();
    this.clearMattingUndoStack();
    this.updateSelectedPreview();
    this.setWorkflowState(deriveGeneratorBaseState(this.getWorkflowContext()));
    this.els.toolbarStatus.textContent = "就绪";
  },

  syncMattingLabels(): void {
    this.els.mattingToleranceLabel.textContent = this.els.mattingTolerance.value;
    this.els.mattingFeatherLabel.textContent = `${this.els.mattingFeather.value}px`;
    this.els.mattingClickToleranceLabel.textContent = this.els.mattingClickTolerance.value;
    this.els.mattingClickRadiusLabel.textContent = `${this.els.mattingClickRadius.value}px`;
  },

  async loadMattingCanvas(path: string): Promise<boolean> {
    const loadToken = this.startMattingCanvasLoad(path);
    try {
      const base64 = await readImageAsBase64(path);
      if (!this.isActiveMattingCanvasLoad(loadToken, path)) {
        return true;
      }
      const image = await loadImageFromDataUrl(`data:image/png;base64,${base64}`);
      if (!this.isActiveMattingCanvasLoad(loadToken, path)) {
        return true;
      }
      const canvas = this.els.mattingCanvas;
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
      canvas.style.aspectRatio = `${image.naturalWidth} / ${image.naturalHeight}`;
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      if (!ctx) {
        throw new Error("无法创建抠图画布");
      }
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(image, 0, 0);
      this.mattingCanvasPath = path;
      this.mattingDirty = false;
      this.mattingRevision += 1;
      this.mattingWorkspaceImagePath = "";
      this.clearMattingUndoStack();
      this.syncWorkflowControls();
      this.els.toolbarStatus.textContent = "抠图模式";
      return true;
    } catch (err) {
      if (!this.isActiveMattingCanvasLoad(loadToken, path)) {
        return true;
      }
      console.error("[generator] 抠图画布载入失败:", err);
      this.mattingCanvasPath = null;
      this.mattingDirty = false;
      this.clearMattingUndoStack();
      this.clearMattingCanvas();
      this.syncWorkflowControls();
      this.setToolbarError("抠图画布载入失败", err);
      return false;
    }
  },

  startMattingCanvasLoad(path: string): number {
    this.mattingCanvasLoadToken += 1;
    this.mattingCanvasPath = null;
    this.mattingDirty = false;
    this.clearMattingUndoStack();
    this.clearMattingCanvas();
    this.syncWorkflowControls();
    this.els.toolbarStatus.textContent = `正在载入抠图画布: ${getFileName(path)}`;
    return this.mattingCanvasLoadToken;
  },

  invalidateMattingCanvasLoad(): void {
    this.mattingCanvasLoadToken += 1;
  },

  isActiveMattingCanvasLoad(token: number, path: string): boolean {
    return (
      token === this.mattingCanvasLoadToken &&
      this.isMattingMode &&
      this.selectedGeneratedPath === path
    );
  },

  clearMattingCanvas(): void {
    this.els.mattingCanvas.width = 0;
    this.els.mattingCanvas.height = 0;
    this.els.mattingCanvas.style.aspectRatio = "";
  },

  async drawMattingBase64ToCanvas(base64: string): Promise<void> {
    const image = await loadImageFromDataUrl(`data:image/png;base64,${base64}`);
    const canvas = this.els.mattingCanvas;
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    canvas.style.aspectRatio = `${image.naturalWidth} / ${image.naturalHeight}`;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) {
      throw new Error("无法创建抠图画布");
    }
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(image, 0, 0);
  },

  async handleMakeTransparentBackground(): Promise<void> {
    if (!this.canRunGeneratorAction("runAutoMatting")) return;
    const canvas = this.els.mattingCanvas;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx || canvas.width <= 0 || canvas.height <= 0) return;

    const beforeMatting = cloneImageData(ctx.getImageData(0, 0, canvas.width, canvas.height));
    this.setWorkflowState("mattingProcessing");
    this.els.toolbarStatus.textContent = "正在抠图...";

    try {
      const result = await applyCanvasBackgroundTransparent(
        canvas.toDataURL("image/png"),
        parseClampedInt(this.els.mattingTolerance.value, 36, 1, 120),
        parseClampedInt(this.els.mattingFeather.value, 1, 0, 3),
        this.els.mattingColorKey.value
      );
      await this.drawMattingBase64ToCanvas(result.base64_data);
      this.pushMattingUndoSnapshot(beforeMatting);
      this.mattingDirty = true;
      this.mattingRevision += 1;
      this.els.toolbarStatus.textContent = `已自动抠图 · 背景 ${result.background_color} · ${result.transparent_pixels} 像素`;
    } catch (err) {
      console.error("[generator] 一键抠图失败:", err);
      this.setToolbarError("抠图失败", err);
    } finally {
      this.setWorkflowState(this.selectedGeneratedPath ? "matting" : "empty");
    }
  },

  async handleSaveMattingEdits(): Promise<void> {
    if (!this.canRunGeneratorAction("saveMatting")) return;
    const sourcePath = this.mattingCanvasPath || this.selectedGeneratedPath;
    const sourceRecord = this.getSelectedRecord();
    if (!sourcePath || !sourceRecord) return;

    let nextState: GeneratorWorkflowState = "matting";
    this.setWorkflowState("mattingProcessing");
    this.els.toolbarStatus.textContent = "正在保存抠图...";

    try {
      const dataUrl = this.els.mattingCanvas.toDataURL("image/png");
      const result = await saveMattedImageDataUrl(sourcePath, dataUrl);
      const now = new Date();
      const outputRecord: GeneratedImageRecord = {
        id: `matting-${Date.now()}`,
        path: result.file_path,
        label: requiredFileNameStem(result.file_name, result.file_path, "抠图保存结果"),
        prompt: sourceRecord.prompt,
        model: `${requiredGeneratedRecordModel(sourceRecord, "抠图源图片")} · 手动抠图`,
        createdAt: now,
        updatedAt: now,
      };
      const dtos = await upsertWorkbenchRecords([recordToWorkbenchDto(outputRecord)]);
      this.applyWorkbenchDtos(dtos, outputRecord.path);
      this.renderGallery();
      const loaded = await this.loadMattingCanvas(outputRecord.path);
      if (!loaded) {
        nextState = deriveGeneratorBaseState(this.getWorkflowContext());
        this.updateSelectedPreview();
      }
      this.els.toolbarStatus.textContent = `抠图已保存 · 透明像素 ${result.transparent_pixels}`;
    } catch (err) {
      console.error("[generator] 保存抠图失败:", err);
      this.setToolbarError("保存抠图失败", err);
    } finally {
      this.setWorkflowState(this.selectedGeneratedPath ? nextState : "empty");
    }
  },

  async handleMattingCanvasClick(event: MouseEvent): Promise<void> {
    if (!this.canRunGeneratorAction("eraseMatting")) return;
    const canvas = this.els.mattingCanvas;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx || canvas.width <= 0 || canvas.height <= 0) return;

    const point = getCanvasPixelPoint(event, canvas);
    if (!point) {
      this.els.toolbarStatus.textContent = "点击位置不在图片内容区域";
      return;
    }

    const beforeErase = cloneImageData(ctx.getImageData(0, 0, canvas.width, canvas.height));
    this.setWorkflowState("mattingProcessing");
    try {
      const result = await applyCanvasConnectedErase(canvas.toDataURL("image/png"), {
        x: point.x,
        y: point.y,
        tolerance: parseClampedInt(this.els.mattingClickTolerance.value, 28, 1, 120),
        radius: parseClampedInt(this.els.mattingClickRadius.value, 1, 0, 8),
      });
      const operation = result.operations[0];
      if (!operation || result.erased_pixels === 0) {
        this.els.toolbarStatus.textContent = getEraseFailureText(operation?.reason ?? "no_seed");
        return;
      }
      await this.drawMattingBase64ToCanvas(result.base64_data);
      this.pushMattingUndoSnapshot(beforeErase);
      this.mattingDirty = true;
      this.mattingRevision += 1;
      this.syncWorkflowControls();
      this.els.toolbarStatus.textContent = `已擦除 ${result.erased_pixels} 像素`;
    } catch (err) {
      this.setToolbarError("擦除失败", err);
    } finally {
      this.setWorkflowState(this.selectedGeneratedPath ? "matting" : "empty");
    }
  },

  undoMattingErase(): void {
    if (!this.canRunGeneratorAction("undoMatting")) return;
    const snapshot = this.mattingUndoStack.pop();
    if (!snapshot) {
      this.syncWorkflowControls();
      return;
    }

    const ctx = this.els.mattingCanvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) return;
    this.pushMattingRedoSnapshot(ctx.getImageData(0, 0, this.els.mattingCanvas.width, this.els.mattingCanvas.height));
    ctx.putImageData(snapshot, 0, 0);
    this.mattingDirty = this.mattingUndoStack.length > 0;
    this.mattingRevision += 1;
    this.syncWorkflowControls();
    this.els.toolbarStatus.textContent = this.mattingDirty ? "已撤销一步" : "已回到未修改状态";
  },

  redoMattingErase(): void {
    if (!this.canRunGeneratorAction("redoMatting")) return;
    const snapshot = this.mattingRedoStack.pop();
    if (!snapshot) {
      this.syncWorkflowControls();
      return;
    }

    const ctx = this.els.mattingCanvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) return;
    this.pushMattingUndoSnapshot(
      ctx.getImageData(0, 0, this.els.mattingCanvas.width, this.els.mattingCanvas.height),
      false
    );
    ctx.putImageData(snapshot, 0, 0);
    this.mattingDirty = true;
    this.mattingRevision += 1;
    this.syncWorkflowControls();
    this.els.toolbarStatus.textContent = "已恢复一步";
  },

  handleMattingKeyboardShortcuts(event: KeyboardEvent): void {
    if (!this.isMattingMode || event.altKey || (!event.ctrlKey && !event.metaKey)) {
      return;
    }

    const key = event.key.toLowerCase();
    const shouldRedo = key === "y" || (key === "z" && event.shiftKey);
    const shouldUndo = key === "z" && !event.shiftKey;
    if (!shouldUndo && !shouldRedo) {
      return;
    }

    event.preventDefault();
    if (shouldUndo) {
      this.undoMattingErase();
    } else {
      this.redoMattingErase();
    }
  },

  pushMattingUndoSnapshot(snapshot: ImageData, clearRedo: boolean = true): void {
    this.mattingUndoStack.push(snapshot);
    if (this.mattingUndoStack.length > 20) {
      this.mattingUndoStack.shift();
    }
    if (clearRedo) {
      this.mattingRedoStack = [];
    }
    this.syncWorkflowControls();
  },

  pushMattingRedoSnapshot(snapshot: ImageData): void {
    this.mattingRedoStack.push(snapshot);
    if (this.mattingRedoStack.length > 20) {
      this.mattingRedoStack.shift();
    }
    this.syncWorkflowControls();
  },

  clearMattingUndoStack(): void {
    this.mattingUndoStack = [];
    this.mattingRedoStack = [];
    this.syncWorkflowControls();
  },


} satisfies ThisType<GeneratorPage>;

export type GeneratorMattingMethods = typeof generatorMattingMethods;
