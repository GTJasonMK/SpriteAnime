import {
  openImageFilePath,
  upsertWorkbenchRecords,
  type GenerationResult,
  type WorkbenchRecord
} from "../../api/commands";
import { getErrorMessage } from "../../utils/errors";
import {
  renderGeneratedGallery,
  setSelectedGeneratedPreview,
  updateGeneratedGallerySelection,
} from "./gallery";
import {
  deriveGeneratorBaseState,
  getGeneratorWorkflowPermissions
} from "./workflow";

import type { GeneratorPage } from "./image-page";
import type {
  GeneratedImageRecord,
  SpriteGridPreset,
} from "./types";
import { formatDuration, formatTime, recordToWorkbenchDto, requiredPathFileStem, workbenchDtoToRecord } from "./helpers";

export const generatorGalleryMethods = {
  async addResultsToWorkbench(
    result: GenerationResult,
    context: { prompt: string; model: string; }
  ): Promise<void> {
    this.els.workspaceEmpty.style.display = "none";
    this.els.resultCard.style.display = "flex";
    this.els.resultActions.style.display = "flex";

    if (result.image_urls.length === 0) {
      this.renderGallery();
      return;
    }

    result.image_urls.forEach((path, i) => {
      const existingIndex = this.generatedRecords.findIndex((item) => item.path === path);
      const label = requiredPathFileStem(path, "生成图片结果");
      const record: GeneratedImageRecord = {
        id: `${Date.now()}-${i}-${this.generatedRecords.length}`,
        path,
        label,
        prompt: context.prompt,
        model: context.model,
        durationSeconds: result.duration_seconds,
        createdAt: new Date(),
        updatedAt: new Date(),
      };
      if (existingIndex >= 0) {
        this.generatedRecords[existingIndex] = {
          ...this.generatedRecords[existingIndex],
          label: record.label,
          prompt: context.prompt,
          model: context.model,
          durationSeconds: result.duration_seconds,
          updatedAt: new Date(),
        };
      } else {
        this.generatedRecords.push(record);
      }
      this.selectedGeneratedPath = path;
    });

    try {
      const dtos = await upsertWorkbenchRecords(
        result.image_urls
          .map((path) => this.generatedRecords.find((item) => item.path === path))
          .filter((record): record is GeneratedImageRecord => Boolean(record))
          .map(recordToWorkbenchDto)
      );
      const selectedPath = result.image_urls[result.image_urls.length - 1];
      this.applyWorkbenchDtos(dtos, selectedPath);
    } catch (err) {
      console.error("[generator] 同步工作台记录失败:", err);
      this.setToolbarError("生成完成，但工作台记录同步失败", err);
    }

    this.renderGallery();
  },

  setToolbarError(prefix: string, err: unknown): void {
    const message = getErrorMessage(err);
    this.els.toolbarStatus.textContent = `${prefix}: ${message}`;
    this.els.toolbarStatus.title = message;
  },

  applyWorkbenchDtos(
    dtos: WorkbenchRecord[],
    selectedPath: string | null
  ): void {
    this.generatedRecords = dtos.map(workbenchDtoToRecord);
    if (this.generatedRecords.length === 0) {
      if (selectedPath !== null) {
        throw new Error("空工作台不能包含选中路径");
      }
      this.selectedGeneratedPath = null;
      return;
    }
    if (!selectedPath || !this.generatedRecords.some((item) => item.path === selectedPath)) {
      throw new Error(`工作台选中路径不存在：${selectedPath || "(空路径)"}`);
    }
    this.selectedGeneratedPath = selectedPath;
  },

  getSelectedRecord(): GeneratedImageRecord | null {
    if (!this.selectedGeneratedPath) return null;
    const record = this.generatedRecords.find((item) => item.path === this.selectedGeneratedPath);
    if (!record) {
      throw new Error(`工作台选中记录不存在：${this.selectedGeneratedPath}`);
    }
    return record;
  },

  renderGallery(): void {
    this.els.galleryCount.textContent = `${this.generatedRecords.length} 张`;

    if (this.generatedRecords.length === 0) {
      this.els.resultGrid.innerHTML = "";
      this.selectedGeneratedPath = null;
      this.mattingDirty = false;
      this.mattingCanvasPath = null;
      this.clearMattingUndoStack();
      this.els.selectedMeta.textContent = "未选择图片";
      this.els.selectedImage.removeAttribute("src");
      this.els.workspaceEmpty.style.display = "flex";
      this.els.resultCard.style.display = "none";
      this.setWorkflowState("empty");
      return;
    }

    this.els.workspaceEmpty.style.display = "none";
    this.els.resultCard.style.display = "flex";

    renderGeneratedGallery({
      container: this.els.resultGrid,
      records: this.generatedRecords,
      selectedPath: this.selectedGeneratedPath,
      formatTime,
      formatDuration,
      onSelect: (path) => this.selectGeneratedImage(path),
      onOpen: (path) => {
        if (!this.canRunGeneratorAction("openSelected")) return;
        openImageFilePath(path).catch((err) => {
          console.error("[generator] 打开图片失败:", err);
          this.setToolbarError("打开图片失败", err);
        });
      },
      onImageLoadError: (path, role) => this.handleGeneratedImageLoadError(path, role),
    });

    if (!this.selectedGeneratedPath) {
      throw new Error("非空工作台缺少选中记录");
    }
    this.updateSelectedPreview();
    if (this.workflowState === "empty" || this.workflowState === "ready") {
      this.setWorkflowState(deriveGeneratorBaseState(this.getWorkflowContext()));
    } else {
      this.syncWorkflowControls();
    }
  },

  selectGeneratedImage(path: string): void {
    if (!this.canRunGeneratorAction("selectRecord")) return;
    if (this.selectedGeneratedPath === path) {
      return;
    }
    if (this.isMattingMode && this.mattingDirty) {
      const ok = window.confirm("当前抠图修改尚未保存，切换图片会丢弃这些修改。继续？");
      if (!ok) return;
    }
    this.selectedGeneratedPath = path;
    this.updateGallerySelection();
    this.updateSelectedPreview();
    this.syncWorkflowControls();
    this.els.toolbarStatus.textContent = "已选择图片";
    if (this.isMattingMode) {
      void this.loadMattingCanvas(path).then((loaded) => {
        if (!loaded) {
          this.setWorkflowState(deriveGeneratorBaseState(this.getWorkflowContext()));
          this.updateSelectedPreview();
        }
      });
    }
  },

  updateGallerySelection(): void {
    updateGeneratedGallerySelection(this.els.resultGrid, this.selectedGeneratedPath);
  },

  updateSelectedPreview(): void {
    const record = this.getSelectedRecord();
    setSelectedGeneratedPreview({
      image: this.els.selectedImage,
      meta: this.els.selectedMeta,
      record,
      formatTime,
      formatDuration,
      onImageLoadError: (path, role) => this.handleGeneratedImageLoadError(path, role),
    });
  },

  handleGeneratedImageLoadError(path: string, role: "thumbnail" | "preview"): void {
    const target = role === "thumbnail" ? "缩略图" : "预览图";
    const message = `${target}载入失败：${path}。解决方法：请确认文件仍存在且应用有权限读取；如果文件已移动或删除，请从工作台移除该记录后重新导入。`;
    console.error("[generator] 图片载入失败:", { path, role });
    this.els.toolbarStatus.textContent = message;
    this.els.toolbarStatus.title = message;
  },

  syncWorkflowControls(): void {
    if (!this.els) return;

    const context = this.getWorkflowContext();
    const permissions = getGeneratorWorkflowPermissions(this.workflowState, context);
    const mattingVisible = this.isMattingMode;
    this.els.generationParams.hidden = mattingVisible;
    this.els.mattingParams.hidden = !mattingVisible;
    this.els.selectedPreview.classList.toggle("matting-active", mattingVisible);

    const resultActionsVisible = context.hasSelection && this.workflowState !== "generating";
    this.els.resultActions.style.display = resultActionsVisible ? "flex" : "none";

    this.els.optimizePrompt.disabled = !permissions.optimizePrompt;
    this.els.addRecord.disabled = !permissions.addRecord;
    this.els.addRecordEmpty.disabled = !permissions.addRecord;
    this.els.viewImage.disabled = !permissions.openSelected;
    this.els.openDir.disabled = !permissions.revealSelected;
    this.els.deleteRecord.disabled = !permissions.deleteRecord;
    this.els.clearRecords.disabled = !permissions.clearRecords;
    this.els.exitMatting.disabled = !permissions.exitMatting;
    this.els.runMatting.disabled = !permissions.runAutoMatting;
    this.els.undoMatting.disabled = !permissions.undoMatting;
    this.els.redoMatting.disabled = !permissions.redoMatting;
    this.els.saveMatting.disabled = !permissions.saveMatting;

    [
      this.els.prompt,
      this.els.negPrompt,
      this.els.referenceImageName,
      this.els.style,
      this.els.ratio,
      this.els.resolution,
      this.els.count,
    ].forEach((control) => {
      control.disabled = !permissions.editGenerationParams;
    });
    this.els.pickReferenceImage.disabled = !permissions.editGenerationParams;
    this.els.clearReferenceImage.disabled =
      !permissions.editGenerationParams || !this.referenceImagePath;
    this.syncImageConstraintControls();

  },

  /// 获取提示词优化器建议的序列帧网格
  getPreferredSpriteGrid(): SpriteGridPreset {
    return { ...this.preferredSpriteGrid };
  },
} satisfies ThisType<GeneratorPage>;

export type GeneratorGalleryMethods = typeof generatorGalleryMethods;
