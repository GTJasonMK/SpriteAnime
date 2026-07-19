import { getById } from "../../utils/dom";
import type { GenerationBackgroundMode, GenerationFraming } from "../../generation/constraints";
import type { SettingsController } from "../../settings/controller";
import type { GenerationPreferences } from "../../settings/types";
import { notifyTaskStateChanged } from "../../workflows/events";
import {
  type GeneratedImageRecord,
  type SpriteGridPreset,
} from "./types";
import {
  deriveGeneratorBaseState,
  getGeneratorWorkflowPermissions,
  type GeneratorAction,
  type GeneratorWorkflowState,
} from "./workflow";

import { generatorGalleryMethods, type GeneratorGalleryMethods } from "./gallery-methods";
import { generatorGenerationMethods, type GeneratorGenerationMethods } from "./generation";
import { generatorMattingMethods, type GeneratorMattingMethods } from "./matting-methods";
import { generatorSettingsMethods, type GeneratorSettingsMethods } from "./settings";
import { generatorWorkbenchMethods, type GeneratorWorkbenchMethods } from "./workbench";
import { generatorWorkspaceMethods, type GeneratorWorkspaceMethods } from "./workspace";

/// 图片生成页面控制器
export class GeneratorPage {
  // 提示词历史
  promptHistory: string[] = [];
  historyIndex: number = -1;

  // 生成状态
  workflowState: GeneratorWorkflowState = "empty";
  generatedRecords: GeneratedImageRecord[] = [];
  selectedGeneratedPath: string | null = null;
  expectedImageCount: number = 0;
  preferredSpriteGrid: SpriteGridPreset = { rows: 3, cols: 4 };
  generationStartedAt: number | null = null;
  generationTimer: number | null = null;
  lastGenerationElapsedText: string = "00:00";
  progressBaseText: string = "准备中...";
  progressIsError: boolean = false;
  mattingDirty: boolean = false;
  mattingCanvasPath: string | null = null;
  mattingCanvasLoadToken: number = 0;
  mattingUndoStack: ImageData[] = [];
  mattingRedoStack: ImageData[] = [];
  referenceImagePath: string = "";
  referenceImageName: string = "";
  mattingRevision: number = 0;
  mattingWorkspaceRevision: number = -1;
  mattingWorkspaceImagePath: string = "";

  // DOM元素缓存
  els!: {
    optimizePrompt: HTMLButtonElement;
    style: HTMLSelectElement;
    ratio: HTMLSelectElement;
    resolution: HTMLSelectElement;
    count: HTMLSelectElement;
    prompt: HTMLTextAreaElement;
    negPrompt: HTMLInputElement;
    imageConstraintsEnabled: HTMLInputElement;
    imageConstraintsRows: HTMLInputElement;
    imageConstraintsCols: HTMLInputElement;
    imageConstraintsBackground: HTMLSelectElement;
    imageConstraintsBackgroundDescription: HTMLInputElement;
    imageConstraintsFraming: HTMLSelectElement;
    referenceImageName: HTMLInputElement;
    referenceImagePreview: HTMLImageElement;
    referenceImageEmpty: HTMLElement;
    pickReferenceImage: HTMLButtonElement;
    clearReferenceImage: HTMLButtonElement;
    viewImage: HTMLButtonElement;
    openDir: HTMLButtonElement;
    progressContainer: HTMLElement;
    progressFill: HTMLElement;
    progressText: HTMLElement;
    toolbarStatus: HTMLElement;
    generationParams: HTMLElement;
    mattingParams: HTMLElement;
    workspaceEmpty: HTMLElement;
    resultCard: HTMLElement;
    resultGrid: HTMLElement;
    resultActions: HTMLElement;
    selectedPreview: HTMLElement;
    selectedImage: HTMLImageElement;
    mattingCanvas: HTMLCanvasElement;
    selectedMeta: HTMLElement;
    galleryCount: HTMLElement;
    addRecord: HTMLButtonElement;
    addRecordEmpty: HTMLButtonElement;
    deleteRecord: HTMLButtonElement;
    clearRecords: HTMLButtonElement;
    exitMatting: HTMLButtonElement;
    runMatting: HTMLButtonElement;
    undoMatting: HTMLButtonElement;
    redoMatting: HTMLButtonElement;
    saveMatting: HTMLButtonElement;
    mattingTolerance: HTMLInputElement;
    mattingToleranceLabel: HTMLElement;
    mattingFeather: HTMLInputElement;
    mattingFeatherLabel: HTMLElement;
    mattingColorKey: HTMLSelectElement;
    mattingClickTolerance: HTMLInputElement;
    mattingClickToleranceLabel: HTMLElement;
    mattingClickRadius: HTMLInputElement;
    mattingClickRadiusLabel: HTMLElement;
  };

  constructor(readonly settings: SettingsController) {
    this.cacheElements();
  }

  get isMattingMode(): boolean {
    return this.workflowState === "matting" || this.workflowState === "mattingProcessing";
  }

  getWorkflowContext() {
    return {
      hasRecords: this.generatedRecords.length > 0,
      hasSelection: Boolean(this.selectedGeneratedPath),
      hasMattingCanvas: Boolean(
        this.selectedGeneratedPath &&
        this.mattingCanvasPath === this.selectedGeneratedPath &&
        this.els.mattingCanvas.width > 0 &&
        this.els.mattingCanvas.height > 0
      ),
      mattingDirty: this.mattingDirty,
      hasMattingUndo: this.mattingUndoStack.length > 0,
      hasMattingRedo: this.mattingRedoStack.length > 0,
    };
  }

  canRunGeneratorAction(action: GeneratorAction): boolean {
    return getGeneratorWorkflowPermissions(this.workflowState, this.getWorkflowContext())[action];
  }

  setWorkflowState(nextState: GeneratorWorkflowState): void {
    this.workflowState = nextState;
    this.syncWorkflowControls();
    notifyTaskStateChanged();
  }

  settleWorkflowState(): void {
    if (this.workflowState === "matting" || this.workflowState === "mattingProcessing") {
      this.setWorkflowState(this.selectedGeneratedPath ? "matting" : "empty");
      return;
    }
    this.setWorkflowState(deriveGeneratorBaseState(this.getWorkflowContext()));
  }

  cacheElements(): void {
    this.els = {
      optimizePrompt: getById<HTMLButtonElement>("btn-optimize-prompt"),
      style: getById<HTMLSelectElement>("style-select"),
      ratio: getById<HTMLSelectElement>("ratio-select"),
      resolution: getById<HTMLSelectElement>("resolution-select"),
      count: getById<HTMLSelectElement>("count-select"),
      prompt: getById<HTMLTextAreaElement>("prompt-input"),
      negPrompt: getById<HTMLInputElement>("neg-prompt-input"),
      imageConstraintsEnabled: getById<HTMLInputElement>("image-constraints-enabled"),
      imageConstraintsRows: getById<HTMLInputElement>("image-constraints-rows"),
      imageConstraintsCols: getById<HTMLInputElement>("image-constraints-cols"),
      imageConstraintsBackground: getById<HTMLSelectElement>("image-constraints-background"),
      imageConstraintsBackgroundDescription: getById<HTMLInputElement>("image-constraints-background-description"),
      imageConstraintsFraming: getById<HTMLSelectElement>("image-constraints-framing"),
      referenceImageName: getById<HTMLInputElement>("reference-image-name"),
      referenceImagePreview: getById<HTMLImageElement>("reference-image-preview"),
      referenceImageEmpty: getById("reference-image-empty"),
      pickReferenceImage: getById<HTMLButtonElement>("btn-pick-reference-image"),
      clearReferenceImage: getById<HTMLButtonElement>("btn-clear-reference-image"),
      viewImage: getById<HTMLButtonElement>("btn-view-image"),
      openDir: getById<HTMLButtonElement>("btn-open-dir"),
      progressContainer: getById("progress-container"),
      progressFill: getById("progress-fill"),
      progressText: getById("progress-text"),
      toolbarStatus: getById("toolbar-status"),
      generationParams: getById("generation-params"),
      mattingParams: getById("matting-params"),
      workspaceEmpty: getById("workspace-empty"),
      resultCard: getById("result-card"),
      resultGrid: getById("result-grid"),
      resultActions: getById("result-actions"),
      selectedPreview: getById("selected-preview"),
      selectedImage: getById<HTMLImageElement>("selected-image"),
      mattingCanvas: getById<HTMLCanvasElement>("matting-canvas"),
      selectedMeta: getById("selected-meta"),
      galleryCount: getById("gallery-count"),
      addRecord: getById<HTMLButtonElement>("btn-add-record"),
      addRecordEmpty: getById<HTMLButtonElement>("btn-add-record-empty"),
      deleteRecord: getById<HTMLButtonElement>("btn-delete-record"),
      clearRecords: getById<HTMLButtonElement>("btn-clear-records"),
      exitMatting: getById<HTMLButtonElement>("btn-exit-matting"),
      runMatting: getById<HTMLButtonElement>("btn-run-matting"),
      undoMatting: getById<HTMLButtonElement>("btn-undo-matting"),
      redoMatting: getById<HTMLButtonElement>("btn-redo-matting"),
      saveMatting: getById<HTMLButtonElement>("btn-save-matting"),
      mattingTolerance: getById<HTMLInputElement>("matting-tolerance-input"),
      mattingToleranceLabel: getById("matting-tolerance-label"),
      mattingFeather: getById<HTMLInputElement>("matting-feather-input"),
      mattingFeatherLabel: getById("matting-feather-label"),
      mattingColorKey: getById<HTMLSelectElement>("matting-color-key-select"),
      mattingClickTolerance: getById<HTMLInputElement>("matting-click-tolerance-input"),
      mattingClickToleranceLabel: getById("matting-click-tolerance-label"),
      mattingClickRadius: getById<HTMLInputElement>("matting-click-radius-input"),
      mattingClickRadiusLabel: getById("matting-click-radius-label"),
    };
  }

  /// 初始化
  async init(): Promise<void> {
    const preferences = this.settings.getGenerationPreferences();
    this.promptHistory = [...preferences.promptHistory];
    this.historyIndex = this.promptHistory.length;
    this.populateDropdowns();
    this.applyGenerationPreferences(preferences);
    await this.loadWorkbenchRecords();
    this.bindEvents();
    document.addEventListener("spriteanime:settings-imported", () => {
      const imported = this.settings.getGenerationPreferences();
      this.promptHistory = [...imported.promptHistory];
      this.historyIndex = this.promptHistory.length;
      this.applyGenerationPreferences(imported);
    });
    this.syncWorkflowControls();
  }

  populateDropdowns(): void {
    const fill = (
      select: HTMLSelectElement,
      items: string[] | Array<{ key: string; label?: string }>
    ): void => {
      select.innerHTML = "";
      items.forEach((item) => {
        const option = document.createElement("option");
        option.value = typeof item === "string" ? item : item.key;
        option.textContent = typeof item === "string" ? item : item.label ?? item.key;
        select.appendChild(option);
      });
    };
    fill(this.els.style, this.settings.presets.styles);
    fill(this.els.ratio, this.settings.presets.ratios);
    fill(this.els.resolution, this.settings.presets.resolutions);
  }

  applyGenerationPreferences(preferences: GenerationPreferences): void {
    setExistingSelectValue(this.els.style, preferences.style, "图片风格");
    setExistingSelectValue(this.els.ratio, preferences.ratio, "图片比例");
    setExistingSelectValue(this.els.resolution, preferences.resolution, "图片分辨率");
    setExistingSelectValue(this.els.count, String(preferences.count), "生成数量");
  }

  async saveGenerationPreferences(): Promise<void> {
    this.settings.updateGenerationPreferences({
      style: this.els.style.value,
      ratio: this.els.ratio.value,
      resolution: this.els.resolution.value,
      count: Number(this.els.count.value),
      promptHistory: this.promptHistory,
    });
    await this.settings.saveCurrentConfig();
  }

  setExternalReferenceImage(path: string, name: string): void {
    this.setReferenceImage(path, name);
    this.els.toolbarStatus.textContent = "已设置参考图";
  }

  readImageGenerationConstraints() {
    return {
      enabled: this.els.imageConstraintsEnabled.checked,
      backgroundMode: this.els.imageConstraintsBackground.value as GenerationBackgroundMode,
      backgroundDescription: this.els.imageConstraintsBackgroundDescription.value,
      framing: this.els.imageConstraintsFraming.value as GenerationFraming,
    };
  }


}

export interface GeneratorPage extends GeneratorSettingsMethods, GeneratorWorkbenchMethods, GeneratorMattingMethods, GeneratorGenerationMethods, GeneratorGalleryMethods, GeneratorWorkspaceMethods { }

Object.assign(
  GeneratorPage.prototype,
  generatorSettingsMethods,
  generatorWorkbenchMethods,
  generatorMattingMethods,
  generatorGenerationMethods,
  generatorGalleryMethods,
  generatorWorkspaceMethods
);

function setExistingSelectValue(select: HTMLSelectElement, value: string, label: string): void {
  if (!Array.from(select.options).some((option) => option.value === value)) {
    throw new Error(`${label}选项不存在：${value}`);
  }
  select.value = value;
}
