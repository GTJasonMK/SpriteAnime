import { readImageAsBase64, saveWorkspaceImageDataUrl } from "../../api/commands";
import type { GeneratorWorkspaceSnapshot } from "../../workspace/types";
import type { GeneratorPage } from "./image-page";

export const generatorWorkspaceMethods = {
  async createWorkspaceSnapshot(): Promise<GeneratorWorkspaceSnapshot> {
    let workspaceImagePath = "";
    if (this.isMattingMode && this.mattingDirty) {
      if (this.els.mattingCanvas.width <= 0 || this.els.mattingCanvas.height <= 0) {
        throw new Error("抠图状态为已修改，但画布内容为空");
      }
      if (this.mattingWorkspaceRevision !== this.mattingRevision) {
        this.mattingWorkspaceImagePath = await saveWorkspaceImageDataUrl(
          "generator-matting",
          this.els.mattingCanvas.toDataURL("image/png")
        );
        this.mattingWorkspaceRevision = this.mattingRevision;
      }
      workspaceImagePath = this.mattingWorkspaceImagePath;
    }

    return {
      style: this.els.style.value,
      ratio: this.els.ratio.value,
      resolution: this.els.resolution.value,
      count: this.els.count.value,
      prompt: this.els.prompt.value,
      negativePrompt: this.els.negPrompt.value,
      referenceImagePath: this.referenceImagePath,
      referenceImageName: this.referenceImageName,
      selectedGeneratedPath: this.selectedGeneratedPath,
      preferredSpriteGrid: { ...this.preferredSpriteGrid },
      generationConstraints: this.readImageGenerationConstraints(),
      matting: {
        active: this.isMattingMode,
        dirty: this.mattingDirty,
        workspaceImagePath,
        tolerance: this.els.mattingTolerance.value,
        feather: this.els.mattingFeather.value,
        colorKey: this.els.mattingColorKey.value,
        clickTolerance: this.els.mattingClickTolerance.value,
        clickRadius: this.els.mattingClickRadius.value,
      },
    };
  },

  async restoreWorkspaceSnapshot(snapshot: GeneratorWorkspaceSnapshot): Promise<void> {
    setSelectValue(this.els.style, snapshot.style, "图片风格");
    setSelectValue(this.els.ratio, snapshot.ratio, "图片比例");
    setSelectValue(this.els.resolution, snapshot.resolution, "图片分辨率");
    setSelectValue(this.els.count, snapshot.count, "生成数量");
    this.els.prompt.value = snapshot.prompt;
    this.els.negPrompt.value = snapshot.negativePrompt;
    this.preferredSpriteGrid = requireGrid(snapshot.preferredSpriteGrid);
    this.els.imageConstraintsRows.value = String(this.preferredSpriteGrid.rows);
    this.els.imageConstraintsCols.value = String(this.preferredSpriteGrid.cols);
    this.els.imageConstraintsEnabled.checked = snapshot.generationConstraints.enabled;
    setSelectValue(
      this.els.imageConstraintsBackground,
      snapshot.generationConstraints.backgroundMode,
      "图片约束背景模式"
    );
    this.els.imageConstraintsBackgroundDescription.value =
      snapshot.generationConstraints.backgroundDescription;
    setSelectValue(
      this.els.imageConstraintsFraming,
      snapshot.generationConstraints.framing,
      "图片约束角色构图"
    );
    this.syncImageConstraintControls();

    if (snapshot.referenceImagePath) {
      this.setReferenceImage(
        snapshot.referenceImagePath,
        requiredText(snapshot.referenceImageName, "参考图名称")
      );
    } else {
      this.clearReferenceImage();
    }

    if (this.generatedRecords.length === 0) {
      if (snapshot.selectedGeneratedPath !== null) {
        throw new Error("工作台为空，但工作区快照包含选中图片");
      }
    } else {
      const selected = snapshot.selectedGeneratedPath;
      if (!selected || !this.generatedRecords.some((record) => record.path === selected)) {
        throw new Error(`工作区选中图片不在工作台记录中：${selected || "(空路径)"}`);
      }
      this.selectedGeneratedPath = selected;
    }
    this.renderGallery();

    this.els.mattingTolerance.value = snapshot.matting.tolerance;
    this.els.mattingFeather.value = snapshot.matting.feather;
    setSelectValue(this.els.mattingColorKey, snapshot.matting.colorKey, "抠图颜色模式");
    this.els.mattingClickTolerance.value = snapshot.matting.clickTolerance;
    this.els.mattingClickRadius.value = snapshot.matting.clickRadius;
    this.syncMattingLabels();

    if (!snapshot.matting.active) {
      return;
    }
    if (!this.selectedGeneratedPath) {
      throw new Error("抠图工作区缺少选中图片");
    }
    this.setWorkflowState("matting");
    if (!snapshot.matting.dirty) {
      const loaded = await this.loadMattingCanvas(this.selectedGeneratedPath);
      if (!loaded) throw new Error("恢复抠图源画布失败");
      return;
    }
    const imagePath = requiredText(snapshot.matting.workspaceImagePath, "未保存抠图路径");
    await this.drawMattingBase64ToCanvas(await readImageAsBase64(imagePath));
    this.mattingCanvasPath = this.selectedGeneratedPath;
    this.mattingDirty = true;
    this.mattingRevision += 1;
    this.mattingWorkspaceRevision = this.mattingRevision;
    this.mattingWorkspaceImagePath = imagePath;
    this.clearMattingUndoStack();
    this.syncWorkflowControls();
    this.els.toolbarStatus.textContent = "已恢复未保存的抠图工作区";
  },
} satisfies ThisType<GeneratorPage>;

function setSelectValue(select: HTMLSelectElement, value: string, label: string): void {
  if (!Array.from(select.options).some((option) => option.value === value)) {
    throw new Error(`${label}快照值无效：${value}`);
  }
  select.value = value;
}

function requireGrid(grid: { rows: number; cols: number }): { rows: number; cols: number } {
  if (!Number.isInteger(grid.rows) || grid.rows < 1 || !Number.isInteger(grid.cols) || grid.cols < 1) {
    throw new Error("工作区首选序列帧网格无效");
  }
  return { ...grid };
}

function requiredText(value: string, label: string): string {
  if (!value.trim()) throw new Error(`${label}为空`);
  return value;
}

export type GeneratorWorkspaceMethods = typeof generatorWorkspaceMethods;
