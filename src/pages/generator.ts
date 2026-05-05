import { Channel, convertFileSrc } from "@tauri-apps/api/core";
import {
  getPresets,
  loadConfig,
  saveConfig,
  checkGenerationApi,
  checkPromptOptimizerApi,
  generateImage,
  getPromptHistory,
  addPromptHistory,
  readWorkbenchRecords,
  upsertWorkbenchRecords,
  deleteWorkbenchRecord,
  clearWorkbenchRecords as clearWorkbenchRecordsApi,
  applyCanvasBackgroundTransparent,
  saveMattedImageDataUrl,
  readImageAsBase64,
  optimizePrompt,
  selectDirectory,
  openImageFile,
  revealInExplorer,
  openImageFilePath,
  type PresetsPayload,
  type UserConfig,
  type ApiCheckResult,
  type GenerateEvent,
  type GenerationResult,
  type WorkbenchRecord,
} from "../api/commands";
import {
  cloneImageData,
  eraseConnectedRegion,
  getCanvasPixelPoint,
} from "./generator-matting";
import {
  deriveGeneratorBaseState,
  getGeneratorWorkflowPermissions,
  type GeneratorAction,
  type GeneratorWorkflowState,
} from "./generator-workflow";

interface SpriteGridPreset {
  rows: number;
  cols: number;
}

interface GeneratedImageRecord {
  id: string;
  path: string;
  label: string;
  prompt: string;
  model: string;
  durationSeconds?: number;
  createdAt: Date;
  updatedAt: Date;
}

const DEFAULT_PROMPT_OPTIMIZER_API_BASE = "https://api.deepseek.com";
const DEFAULT_PROMPT_OPTIMIZER_MODEL = "deepseek-v4-flash";

/// 图片生成页面控制器
export class GeneratorPage {
  // 预设数据
  private presets: PresetsPayload | null = null;
  private config: UserConfig | null = null;

  // 提示词历史
  private promptHistory: string[] = [];
  private historyIndex: number = -1;

  // 生成状态
  private workflowState: GeneratorWorkflowState = "empty";
  private generatedRecords: GeneratedImageRecord[] = [];
  private selectedGeneratedPath: string | null = null;
  private expectedImageCount: number = 0;
  private preferredSpriteGrid: SpriteGridPreset = { rows: 3, cols: 4 };
  private generationStartedAt: number | null = null;
  private generationTimer: number | null = null;
  private lastGenerationElapsedText: string = "00:00";
  private progressBaseText: string = "准备中...";
  private progressIsError: boolean = false;
  private mattingDirty: boolean = false;
  private mattingCanvasPath: string | null = null;
  private mattingCanvasLoadToken: number = 0;
  private mattingUndoStack: ImageData[] = [];
  private mattingRedoStack: ImageData[] = [];
  private referenceImagePath: string = "";
  private referenceImageName: string = "";

  // DOM元素缓存
  private els!: {
    apiKey: HTMLInputElement;
    apiBase: HTMLInputElement;
    proxyUrl: HTMLInputElement;
    toggleKey: HTMLButtonElement;
    model: HTMLInputElement;
    modelList: HTMLDataListElement;
    checkGenerationApi: HTMLButtonElement;
    generationApiCheckStatus: HTMLElement;
    optimizePrompt: HTMLButtonElement;
    promptOptimizerApiKey: HTMLInputElement;
    promptOptimizerApiBase: HTMLInputElement;
    promptOptimizerModel: HTMLInputElement;
    promptOptimizerVision: HTMLInputElement;
    checkPromptOptimizerApi: HTMLButtonElement;
    promptOptimizerApiCheckStatus: HTMLElement;
    style: HTMLSelectElement;
    ratio: HTMLSelectElement;
    resolution: HTMLSelectElement;
    count: HTMLSelectElement;
    prompt: HTMLTextAreaElement;
    negPrompt: HTMLInputElement;
    referenceImageName: HTMLInputElement;
    referenceImagePreview: HTMLImageElement;
    referenceImageEmpty: HTMLElement;
    pickReferenceImage: HTMLButtonElement;
    clearReferenceImage: HTMLButtonElement;
    saveDir: HTMLInputElement;
    ffmpegPath: HTMLInputElement;
    ffprobePath: HTMLInputElement;
    browseDir: HTMLButtonElement;
    generate: HTMLButtonElement;
    viewImage: HTMLButtonElement;
    openDir: HTMLButtonElement;
    transparentBackground: HTMLButtonElement;
    toSprite: HTMLButtonElement;
    saveConfig: HTMLButtonElement;
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
    btnSettings: HTMLButtonElement;
    modalOverlay: HTMLElement;
    btnCloseModal: HTMLButtonElement;
  };

  constructor() {
    this.cacheElements();
  }

  private get isGenerating(): boolean {
    return this.workflowState === "generating";
  }

  private get isMattingMode(): boolean {
    return this.workflowState === "matting" || this.workflowState === "mattingProcessing";
  }

  private getWorkflowContext() {
    return {
      hasRecords: this.generatedRecords.length > 0,
      hasSelection: Boolean(this.selectedGeneratedPath),
      hasMattingCanvas: Boolean(
        this.selectedGeneratedPath &&
        this.mattingCanvasPath === this.selectedGeneratedPath &&
        this.els?.mattingCanvas.width > 0 &&
        this.els?.mattingCanvas.height > 0
      ),
      mattingDirty: this.mattingDirty,
      hasMattingUndo: this.mattingUndoStack.length > 0,
      hasMattingRedo: this.mattingRedoStack.length > 0,
    };
  }

  private canRunGeneratorAction(action: GeneratorAction): boolean {
    return getGeneratorWorkflowPermissions(this.workflowState, this.getWorkflowContext())[action];
  }

  private setWorkflowState(nextState: GeneratorWorkflowState): void {
    this.workflowState = nextState;
    this.syncWorkflowControls();
  }

  private settleWorkflowState(): void {
    if (this.workflowState === "matting" || this.workflowState === "mattingProcessing") {
      this.setWorkflowState(this.selectedGeneratedPath ? "matting" : "empty");
      return;
    }
    this.setWorkflowState(deriveGeneratorBaseState(this.getWorkflowContext()));
  }

  private cacheElements(): void {
    const g = (id: string) => document.getElementById(id) as HTMLElement;
    this.els = {
      apiKey: g("api-key") as HTMLInputElement,
      apiBase: g("api-base") as HTMLInputElement,
      proxyUrl: g("proxy-url") as HTMLInputElement,
      toggleKey: g("btn-toggle-key") as HTMLButtonElement,
      model: g("model-input") as HTMLInputElement,
      modelList: g("model-list") as HTMLDataListElement,
      checkGenerationApi: g("btn-check-generation-api") as HTMLButtonElement,
      generationApiCheckStatus: g("generation-api-check-status"),
      optimizePrompt: g("btn-optimize-prompt") as HTMLButtonElement,
      promptOptimizerApiKey: g("prompt-optimizer-api-key") as HTMLInputElement,
      promptOptimizerApiBase: g("prompt-optimizer-api-base") as HTMLInputElement,
      promptOptimizerModel: g("prompt-optimizer-model-input") as HTMLInputElement,
      promptOptimizerVision: g("prompt-optimizer-vision") as HTMLInputElement,
      checkPromptOptimizerApi: g("btn-check-prompt-optimizer-api") as HTMLButtonElement,
      promptOptimizerApiCheckStatus: g("prompt-optimizer-api-check-status"),
      style: g("style-select") as HTMLSelectElement,
      ratio: g("ratio-select") as HTMLSelectElement,
      resolution: g("resolution-select") as HTMLSelectElement,
      count: g("count-select") as HTMLSelectElement,
      prompt: g("prompt-input") as HTMLTextAreaElement,
      negPrompt: g("neg-prompt-input") as HTMLInputElement,
      referenceImageName: g("reference-image-name") as HTMLInputElement,
      referenceImagePreview: g("reference-image-preview") as HTMLImageElement,
      referenceImageEmpty: g("reference-image-empty"),
      pickReferenceImage: g("btn-pick-reference-image") as HTMLButtonElement,
      clearReferenceImage: g("btn-clear-reference-image") as HTMLButtonElement,
      saveDir: g("save-dir-input") as HTMLInputElement,
      ffmpegPath: g("ffmpeg-path") as HTMLInputElement,
      ffprobePath: g("ffprobe-path") as HTMLInputElement,
      browseDir: g("btn-browse-dir") as HTMLButtonElement,
      generate: g("btn-generate") as HTMLButtonElement,
      viewImage: g("btn-view-image") as HTMLButtonElement,
      openDir: g("btn-open-dir") as HTMLButtonElement,
      transparentBackground: g("btn-transparent-background") as HTMLButtonElement,
      toSprite: g("btn-to-sprite") as HTMLButtonElement,
      saveConfig: g("btn-save-config") as HTMLButtonElement,
      progressContainer: g("progress-container"),
      progressFill: g("progress-fill"),
      progressText: g("progress-text"),
      toolbarStatus: g("toolbar-status"),
      generationParams: g("generation-params"),
      mattingParams: g("matting-params"),
      workspaceEmpty: g("workspace-empty"),
      resultCard: g("result-card"),
      resultGrid: g("result-grid"),
      resultActions: g("result-actions"),
      selectedPreview: g("selected-preview"),
      selectedImage: g("selected-image") as HTMLImageElement,
      mattingCanvas: g("matting-canvas") as HTMLCanvasElement,
      selectedMeta: g("selected-meta"),
      galleryCount: g("gallery-count"),
      addRecord: g("btn-add-record") as HTMLButtonElement,
      addRecordEmpty: g("btn-add-record-empty") as HTMLButtonElement,
      deleteRecord: g("btn-delete-record") as HTMLButtonElement,
      clearRecords: g("btn-clear-records") as HTMLButtonElement,
      exitMatting: g("btn-exit-matting") as HTMLButtonElement,
      runMatting: g("btn-run-matting") as HTMLButtonElement,
      undoMatting: g("btn-undo-matting") as HTMLButtonElement,
      redoMatting: g("btn-redo-matting") as HTMLButtonElement,
      saveMatting: g("btn-save-matting") as HTMLButtonElement,
      mattingTolerance: g("matting-tolerance-input") as HTMLInputElement,
      mattingToleranceLabel: g("matting-tolerance-label"),
      mattingFeather: g("matting-feather-input") as HTMLInputElement,
      mattingFeatherLabel: g("matting-feather-label"),
      mattingColorKey: g("matting-color-key-select") as HTMLSelectElement,
      mattingClickTolerance: g("matting-click-tolerance-input") as HTMLInputElement,
      mattingClickToleranceLabel: g("matting-click-tolerance-label"),
      mattingClickRadius: g("matting-click-radius-input") as HTMLInputElement,
      mattingClickRadiusLabel: g("matting-click-radius-label"),
      btnSettings: g("btn-settings") as HTMLButtonElement,
      modalOverlay: g("settings-modal"),
      btnCloseModal: g("btn-close-modal") as HTMLButtonElement,
    };
  }

  /// 初始化
  async init(): Promise<void> {
    console.log("[generator] 开始初始化...");
    try {
      // 加载预设
      this.presets = await getPresets();
      console.log("[generator] 预设加载完成", { models: this.presets.models.length, styles: this.presets.styles.length });

      // 加载用户配置
      this.config = await loadConfig();
      console.log("[generator] 配置加载完成", { model: this.config.last_model, hasApiKey: !!this.config.api_key });

      // 加载提示词历史
      this.promptHistory = await getPromptHistory(100);
      this.historyIndex = this.promptHistory.length;
      console.log("[generator] 提示词历史加载完成", { count: this.promptHistory.length });

      // 填充UI
      this.populateDropdowns();
      this.applyConfig();
      await this.loadWorkbenchRecords();
      this.bindEvents();
      this.syncWorkflowControls();
      console.log("[generator] 初始化完成");
    } catch (err) {
      console.error("[generator] 初始化失败:", err);
      throw err;
    }
  }

  setExternalReferenceImage(path: string, name?: string): void {
    this.setReferenceImage(path, name || getFileName(path) || "参考图");
    this.els.toolbarStatus.textContent = "已设置参考图";
  }

  private populateDropdowns(): void {
    if (!this.presets) return;

    const fill = (sel: HTMLSelectElement, items: { key?: string; label?: string; name?: string }[] | string[], valueFn?: (v: any) => string, labelFn?: (v: any) => string) => {
      sel.innerHTML = "";
      items.forEach((item) => {
        const opt = document.createElement("option");
        if (typeof item === "string") {
          opt.value = item;
          opt.textContent = item;
        } else if (valueFn && labelFn) {
          opt.value = valueFn(item);
          opt.textContent = labelFn(item);
        }
        sel.appendChild(opt);
      });
    };

    // 填充模型datalist
    this.els.modelList.innerHTML = "";
    this.presets.models.forEach((m) => {
      const opt = document.createElement("option");
      opt.value = m;
      this.els.modelList.appendChild(opt);
    });

    fill(this.els.style, this.presets.styles, (s) => s.key, (s) => s.label);
    fill(this.els.ratio, this.presets.ratios, (r) => r.key, (r) => r.key);
    fill(this.els.resolution, this.presets.resolutions);
  }

  private applyConfig(): void {
    if (!this.config) return;
    const c = this.config;
    this.els.apiKey.value = c.api_key;
    if (c.api_base) {
      this.els.apiBase.value = c.api_base;
    }
    if (c.proxy_url) {
      this.els.proxyUrl.value = c.proxy_url;
    }
    this.els.model.value = c.last_model;
    this.els.promptOptimizerApiKey.value = c.prompt_optimizer_api_key || "";
    this.els.promptOptimizerApiBase.value =
      c.prompt_optimizer_api_base || DEFAULT_PROMPT_OPTIMIZER_API_BASE;
    this.els.promptOptimizerModel.value =
      c.prompt_optimizer_model || DEFAULT_PROMPT_OPTIMIZER_MODEL;
    this.els.promptOptimizerVision.checked = Boolean(c.prompt_optimizer_vision);
    setSelectValue(this.els.style, c.last_style);
    setSelectValue(this.els.ratio, c.last_ratio);
    setSelectValue(this.els.resolution, c.last_resolution);
    setSelectValue(this.els.count, String(c.last_count));
    if (c.save_dir) {
      this.els.saveDir.value = c.save_dir;
    }
    this.els.ffmpegPath.value = c.ffmpeg_path || "";
    this.els.ffprobePath.value = c.ffprobe_path || "";
  }

  private bindEvents(): void {
    this.els.optimizePrompt.addEventListener("click", () => {
      this.handleOptimizePrompt();
    });

    // API Key 显示/隐藏
    this.els.toggleKey.addEventListener("click", () => {
      const isPwd = this.els.apiKey.type === "password";
      this.els.apiKey.type = isPwd ? "text" : "password";
      this.els.toggleKey.textContent = isPwd ? "🙈" : "👁";
    });

    this.els.checkGenerationApi.addEventListener("click", () => {
      this.handleCheckGenerationApi();
    });

    this.els.checkPromptOptimizerApi.addEventListener("click", () => {
      this.handleCheckPromptOptimizerApi();
    });

    // 浏览保存目录
    this.els.browseDir.addEventListener("click", async () => {
      try {
        const dir = await selectDirectory();
        if (dir) {
          this.els.saveDir.value = dir;
        }
      } catch (_) {
        // 用户取消
      }
    });

    this.els.pickReferenceImage.addEventListener("click", () => {
      this.handlePickReferenceImage();
    });

    this.els.clearReferenceImage.addEventListener("click", () => {
      this.clearReferenceImage();
    });

    // 生成图片
    this.els.generate.addEventListener("click", () => {
      console.log("[generator] 点击生成按钮");
      this.handleGenerate();
    });

    // Ctrl/Command + 上下键浏览历史，避免劫持 textarea 的多行光标移动。
    this.els.prompt.addEventListener("keydown", (e) => {
      if (this.promptHistory.length === 0) return;
      if (!e.ctrlKey && !e.metaKey) return;
      if (e.key === "ArrowUp") {
        e.preventDefault();
        if (this.historyIndex > 0) {
          this.historyIndex--;
          this.els.prompt.value = this.promptHistory[this.historyIndex];
        }
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        if (this.historyIndex < this.promptHistory.length - 1) {
          this.historyIndex++;
          this.els.prompt.value = this.promptHistory[this.historyIndex];
        } else if (this.historyIndex === this.promptHistory.length - 1) {
          this.historyIndex++;
          this.els.prompt.value = "";
        }
      }
    });

    // 保存配置按钮
    this.els.saveConfig.addEventListener("click", async () => {
      try {
        await this.saveCurrentConfig();
        console.log("[generator] 配置已保存");
        this.els.toolbarStatus.textContent = "配置已保存";
        setTimeout(() => {
          if (this.els.toolbarStatus.textContent === "配置已保存") {
            this.els.toolbarStatus.textContent = "就绪";
          }
        }, 2000);
      } catch (err) {
        console.error("[generator] 保存配置失败:", err);
        this.els.toolbarStatus.textContent = "保存失败";
      }
    });

    // 结果操作按钮
    this.els.viewImage.addEventListener("click", () => {
      if (!this.canRunGeneratorAction("openSelected")) return;
      const path = this.selectedGeneratedPath;
      if (path) {
        openImageFilePath(path).catch((err) => {
          console.error("[generator] 打开图片失败:", err);
          this.els.toolbarStatus.textContent = "打开图片失败";
        });
      }
    });

    this.els.openDir.addEventListener("click", () => {
      if (!this.canRunGeneratorAction("revealSelected")) return;
      const latestPath = this.generatedRecords[this.generatedRecords.length - 1]?.path || "";
      const targetDir =
        getDirectoryName(this.selectedGeneratedPath || "") ||
        getDirectoryName(latestPath) ||
        this.els.saveDir.value.trim();
      if (targetDir) {
        revealInExplorer(targetDir).catch((err) => {
          console.error("[generator] 打开目录失败:", err);
          this.els.toolbarStatus.textContent = "打开目录失败";
        });
      }
    });

    this.els.transparentBackground.addEventListener("click", () => {
      if (this.isMattingMode) {
        this.exitMattingMode();
      } else {
        this.enterMattingMode();
      }
    });

    this.els.exitMatting.addEventListener("click", () => this.exitMattingMode());
    this.els.runMatting.addEventListener("click", () => this.handleMakeTransparentBackground());
    this.els.undoMatting.addEventListener("click", () => this.undoMattingErase());
    this.els.redoMatting.addEventListener("click", () => this.redoMattingErase());
    this.els.saveMatting.addEventListener("click", () => this.handleSaveMattingEdits());
    document.addEventListener("keydown", (event) => this.handleMattingKeyboardShortcuts(event));
    this.els.mattingCanvas.addEventListener("click", (event) => this.handleMattingCanvasClick(event));
    [
      this.els.mattingTolerance,
      this.els.mattingFeather,
      this.els.mattingClickTolerance,
      this.els.mattingClickRadius,
    ].forEach((input) => {
      input.addEventListener("input", () => this.syncMattingLabels());
    });
    this.syncMattingLabels();

    this.els.toSprite.addEventListener("click", () => {
      if (!this.canRunGeneratorAction("sendToSprite")) return;
      // 切换到序列帧标签页
      const spriteTab = document.querySelector<HTMLButtonElement>(
        '.tab-button[data-tab="sprite"]'
      );
      spriteTab?.click();
      document.dispatchEvent(new CustomEvent("spriteanimte:prepare-sprite-from-generator"));
    });

    this.els.addRecord.addEventListener("click", () => {
      this.handleAddRecord();
    });

    this.els.addRecordEmpty.addEventListener("click", () => {
      this.handleAddRecord();
    });

    this.els.deleteRecord.addEventListener("click", () => {
      this.deleteSelectedRecord();
    });

    this.els.clearRecords.addEventListener("click", () => {
      this.clearWorkbenchRecords();
    });

    // 设置弹窗
    this.els.btnSettings.addEventListener("click", () => this.openSettings());
    this.bindModalEvents();
  }

  /// 打开设置弹窗
  private openSettings(): void {
    if (!this.canRunGeneratorAction("openSettings")) return;
    this.els.modalOverlay.style.display = "flex";
  }

  /// 关闭设置弹窗（自动保存配置）
  private async closeSettings(): Promise<void> {
    this.els.modalOverlay.style.display = "none";
    try {
      await this.saveCurrentConfig();
      console.log("[generator] 设置已自动保存");
    } catch (err) {
      console.error("[generator] 自动保存设置失败:", err);
    }
  }

  /// 绑定弹窗关闭事件（遮罩点击、关闭按钮、ESC键）
  private bindModalEvents(): void {
    this.els.btnCloseModal.addEventListener("click", () => this.closeSettings());

    this.els.modalOverlay.addEventListener("click", (e) => {
      if (e.target === this.els.modalOverlay) {
        this.closeSettings();
      }
    });

    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && this.els.modalOverlay.style.display === "flex") {
        this.closeSettings();
      }
    });
  }

  private async loadWorkbenchRecords(): Promise<void> {
    try {
      const dtos = await readWorkbenchRecords(200);
      const records = dtos.map(workbenchDtoToRecord);
      if (records.length === 0) return;
      this.generatedRecords = records;
      this.selectedGeneratedPath = records[records.length - 1].path;
      this.els.workspaceEmpty.style.display = "none";
      this.els.resultCard.style.display = "flex";
      this.els.resultActions.style.display = "flex";
      this.renderGallery();
      this.syncWorkflowControls();
    } catch (err) {
      console.warn("[generator] 加载工作台记录失败:", err);
    }
  }

  private async handlePickReferenceImage(): Promise<void> {
    if (!this.canRunGeneratorAction("editGenerationParams")) return;
    try {
      const file = await openImageFile();
      this.setReferenceImage(file.file_path, file.file_name);
      this.els.toolbarStatus.textContent = "已选择参考图";
    } catch (err) {
      if (!String(err).includes("用户取消")) {
        console.error("[generator] 选择参考图失败:", err);
        this.els.toolbarStatus.textContent = "选择参考图失败";
      }
    }
  }

  private async handleCheckGenerationApi(): Promise<void> {
    await this.runApiCheck(
      this.els.checkGenerationApi,
      this.els.generationApiCheckStatus,
      () => checkGenerationApi(
        this.els.apiKey.value.trim(),
        this.els.apiBase.value.trim(),
        this.els.model.value.trim(),
        this.els.proxyUrl.value.trim()
      )
    );
  }

  private async handleCheckPromptOptimizerApi(): Promise<void> {
    const apiKey = this.els.promptOptimizerApiKey.value.trim() || this.els.apiKey.value.trim();
    const apiBase = this.els.promptOptimizerApiBase.value.trim() || DEFAULT_PROMPT_OPTIMIZER_API_BASE;
    const model = this.els.promptOptimizerModel.value.trim() || DEFAULT_PROMPT_OPTIMIZER_MODEL;
    await this.runApiCheck(
      this.els.checkPromptOptimizerApi,
      this.els.promptOptimizerApiCheckStatus,
      () => checkPromptOptimizerApi(
        apiKey,
        apiBase,
        model,
        this.els.proxyUrl.value.trim()
      )
    );
  }

  private async runApiCheck(
    button: HTMLButtonElement,
    statusEl: HTMLElement,
    request: () => Promise<ApiCheckResult>
  ): Promise<void> {
    if (button.disabled) return;
    const originalText = button.textContent || "检测";
    button.disabled = true;
    button.textContent = "检测中";
    this.setApiCheckStatus(statusEl, "checking", "正在连接 API...");
    this.els.toolbarStatus.textContent = "正在检测 API...";

    try {
      const result = await request();
      const statusClass = result.status === "warning" ? "warning" : "ok";
      this.setApiCheckStatus(statusEl, statusClass, result.message, result);
      this.els.toolbarStatus.textContent =
        result.status === "warning" ? "API 检测完成，有提示" : "API 检测成功";
    } catch (err) {
      const message = `检测失败：${String(err)}`;
      this.setApiCheckStatus(statusEl, "error", message);
      this.els.toolbarStatus.textContent = "API 检测失败";
      console.error("[generator] API 检测失败:", err);
    } finally {
      button.textContent = originalText;
      this.syncWorkflowControls();
    }
  }

  private setApiCheckStatus(
    statusEl: HTMLElement,
    status: "ok" | "warning" | "error" | "checking",
    message: string,
    result?: ApiCheckResult
  ): void {
    statusEl.className = `config-check-status ${status}`;
    statusEl.textContent = message;
    statusEl.title = result
      ? `Endpoint: ${result.endpoint}${result.model ? `\nModel: ${result.model}` : ""}`
      : "";
  }

  private setReferenceImage(path: string, name: string): void {
    this.referenceImagePath = path;
    this.referenceImageName = name || getFileName(path) || "参考图";
    this.els.referenceImageName.value = this.referenceImageName;
    this.els.referenceImagePreview.src = convertFileSrc(path);
    this.els.referenceImagePreview.style.display = "block";
    this.els.referenceImageEmpty.style.display = "none";
    this.els.clearReferenceImage.disabled = false;
  }

  private clearReferenceImage(): void {
    this.referenceImagePath = "";
    this.referenceImageName = "";
    this.els.referenceImagePreview.removeAttribute("src");
    this.els.referenceImagePreview.style.display = "none";
    this.els.referenceImageEmpty.style.display = "inline";
    this.els.referenceImageName.value = "无参考图";
    this.els.clearReferenceImage.disabled = true;
    this.els.toolbarStatus.textContent = "已移除参考图";
  }

  private async handleAddRecord(): Promise<void> {
    if (!this.canRunGeneratorAction("addRecord")) return;
    try {
      const file = await openImageFile();
      const now = new Date();
      const record: GeneratedImageRecord = {
        id: `manual-${Date.now()}`,
        path: file.file_path,
        label: stripFileExtension(file.file_name) || "本地图片",
        prompt: "",
        model: "手动添加",
        createdAt: now,
        updatedAt: now,
      };
      const dtos = await upsertWorkbenchRecords([recordToWorkbenchDto(record)]);
      this.applyWorkbenchDtos(dtos, record.path);
      this.renderGallery();
      this.els.toolbarStatus.textContent = "已添加记录";
    } catch (err) {
      if (!String(err).includes("用户取消")) {
        console.error("[generator] 添加记录失败:", err);
        this.els.toolbarStatus.textContent = "添加记录失败";
      }
    }
  }

  private async deleteSelectedRecord(): Promise<void> {
    if (!this.canRunGeneratorAction("deleteRecord")) return;
    const record = this.getSelectedRecord();
    if (!record) return;
    const ok = window.confirm("仅从工作台移除此记录，不会删除图片文件。继续？");
    if (!ok) return;

    try {
      const currentIndex = this.generatedRecords.findIndex((item) => item.id === record.id);
      const dtos = await deleteWorkbenchRecord(record.id);
      const nextIndex = Math.min(Math.max(currentIndex, 0), dtos.length - 1);
      const nextPath = dtos[nextIndex]?.path || null;
      this.applyWorkbenchDtos(dtos, nextPath);
      this.renderGallery();
      this.els.toolbarStatus.textContent = "已移除记录";
    } catch (err) {
      console.error("[generator] 删除记录失败:", err);
      this.els.toolbarStatus.textContent = "删除记录失败";
    }
  }

  private async clearWorkbenchRecords(): Promise<void> {
    if (!this.canRunGeneratorAction("clearRecords")) return;
    const ok = window.confirm("清空工作台记录？图片文件会保留在磁盘上。");
    if (!ok) return;

    try {
      await clearWorkbenchRecordsApi();
      this.generatedRecords = [];
      this.selectedGeneratedPath = null;
      this.renderGallery();
      this.els.toolbarStatus.textContent = "记录已清空";
    } catch (err) {
      console.error("[generator] 清空记录失败:", err);
      this.els.toolbarStatus.textContent = "清空记录失败";
    }
  }

  private async enterMattingMode(): Promise<void> {
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
  }

  private exitMattingMode(): void {
    if (!this.canRunGeneratorAction("exitMatting")) return;
    this.invalidateMattingCanvasLoad();
    this.mattingDirty = false;
    this.mattingCanvasPath = null;
    this.clearMattingCanvas();
    this.clearMattingUndoStack();
    this.updateSelectedPreview();
    this.setWorkflowState(deriveGeneratorBaseState(this.getWorkflowContext()));
    this.els.toolbarStatus.textContent = "就绪";
  }

  private syncMattingLabels(): void {
    this.els.mattingToleranceLabel.textContent = this.els.mattingTolerance.value;
    this.els.mattingFeatherLabel.textContent = `${this.els.mattingFeather.value}px`;
    this.els.mattingClickToleranceLabel.textContent = this.els.mattingClickTolerance.value;
    this.els.mattingClickRadiusLabel.textContent = `${this.els.mattingClickRadius.value}px`;
  }

  private syncMattingModeButton(): void {
    this.els.transparentBackground.classList.toggle("mode-active", this.isMattingMode);
  }

  private async loadMattingCanvas(path: string): Promise<boolean> {
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
      this.els.toolbarStatus.textContent = "抠图画布载入失败";
      return false;
    }
  }

  private startMattingCanvasLoad(path: string): number {
    this.mattingCanvasLoadToken += 1;
    this.mattingCanvasPath = null;
    this.mattingDirty = false;
    this.clearMattingUndoStack();
    this.clearMattingCanvas();
    this.syncWorkflowControls();
    this.els.toolbarStatus.textContent = `正在载入抠图画布: ${getFileName(path)}`;
    return this.mattingCanvasLoadToken;
  }

  private invalidateMattingCanvasLoad(): void {
    this.mattingCanvasLoadToken += 1;
  }

  private isActiveMattingCanvasLoad(token: number, path: string): boolean {
    return (
      token === this.mattingCanvasLoadToken &&
      this.isMattingMode &&
      this.selectedGeneratedPath === path
    );
  }

  private clearMattingCanvas(): void {
    this.els.mattingCanvas.width = 0;
    this.els.mattingCanvas.height = 0;
    this.els.mattingCanvas.style.aspectRatio = "";
  }

  private async drawMattingBase64ToCanvas(base64: string): Promise<void> {
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
  }

  private async handleMakeTransparentBackground(): Promise<void> {
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
        normalizeSliderValue(this.els.mattingTolerance.value, 36, 1, 120),
        normalizeSliderValue(this.els.mattingFeather.value, 1, 0, 3),
        this.els.mattingColorKey.value
      );
      await this.drawMattingBase64ToCanvas(result.base64_data);
      this.pushMattingUndoSnapshot(beforeMatting);
      this.mattingDirty = true;
      this.els.toolbarStatus.textContent = `已自动抠图 · 背景 ${result.background_color} · ${result.transparent_pixels} 像素`;
    } catch (err) {
      console.error("[generator] 一键抠图失败:", err);
      this.els.toolbarStatus.textContent = `抠图失败: ${String(err)}`;
    } finally {
      this.setWorkflowState(this.selectedGeneratedPath ? "matting" : "empty");
    }
  }

  private async handleSaveMattingEdits(): Promise<void> {
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
        label: stripFileExtension(result.file_name) || `${sourceRecord.label}-抠图`,
        prompt: sourceRecord.prompt,
        model: sourceRecord.model ? `${sourceRecord.model} · 手动抠图` : "手动抠图",
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
      this.els.toolbarStatus.textContent = `保存抠图失败: ${String(err)}`;
    } finally {
      this.setWorkflowState(this.selectedGeneratedPath ? nextState : "empty");
    }
  }

  private handleMattingCanvasClick(event: MouseEvent): void {
    if (!this.canRunGeneratorAction("eraseMatting")) return;
    const canvas = this.els.mattingCanvas;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx || canvas.width <= 0 || canvas.height <= 0) return;

    const point = getCanvasPixelPoint(event, canvas);
    if (!point) {
      this.els.toolbarStatus.textContent = "点击位置不在图片内容区域";
      return;
    }

    const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const beforeErase = cloneImageData(imageData);
    const result = eraseConnectedRegion({
      data: imageData.data,
      width: imageData.width,
      height: imageData.height,
      startX: point.x,
      startY: point.y,
      tolerance: normalizeSliderValue(this.els.mattingClickTolerance.value, 28, 1, 120),
      radius: normalizeSliderValue(this.els.mattingClickRadius.value, 1, 0, 8),
    });
    if (result.erasedPixels === 0) {
      this.els.toolbarStatus.textContent = getEraseFailureText(result.reason);
      console.debug("[generator] 手动抠图未擦除像素", { point, result });
      return;
    }

    ctx.putImageData(imageData, 0, 0);
    this.pushMattingUndoSnapshot(beforeErase);
    this.mattingDirty = true;
    this.syncWorkflowControls();
    this.els.toolbarStatus.textContent = `已擦除 ${result.erasedPixels} 像素`;
    console.debug("[generator] 手动抠图擦除完成", { point, result });
  }

  private undoMattingErase(): void {
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
    this.syncWorkflowControls();
    this.els.toolbarStatus.textContent = this.mattingDirty ? "已撤销一步" : "已回到未修改状态";
  }

  private redoMattingErase(): void {
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
    this.syncWorkflowControls();
    this.els.toolbarStatus.textContent = "已恢复一步";
  }

  private handleMattingKeyboardShortcuts(event: KeyboardEvent): void {
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
  }

  private pushMattingUndoSnapshot(snapshot: ImageData, clearRedo: boolean = true): void {
    this.mattingUndoStack.push(snapshot);
    if (this.mattingUndoStack.length > 20) {
      this.mattingUndoStack.shift();
    }
    if (clearRedo) {
      this.mattingRedoStack = [];
    }
    this.syncWorkflowControls();
  }

  private pushMattingRedoSnapshot(snapshot: ImageData): void {
    this.mattingRedoStack.push(snapshot);
    if (this.mattingRedoStack.length > 20) {
      this.mattingRedoStack.shift();
    }
    this.syncWorkflowControls();
  }

  private clearMattingUndoStack(): void {
    this.mattingUndoStack = [];
    this.mattingRedoStack = [];
    this.syncWorkflowControls();
  }

  private async handleOptimizePrompt(): Promise<void> {
    if (!this.canRunGeneratorAction("optimizePrompt")) return;

    const prompt = this.els.prompt.value.trim();
    if (!prompt) {
      alert("请先输入需要优化的提示词");
      return;
    }

    const apiKey = this.els.promptOptimizerApiKey.value.trim() || this.els.apiKey.value.trim();
    if (!apiKey) {
      alert("请先在设置中填写提示词优化 API Key，或填写生图 API Key 以复用");
      return;
    }

    const apiBase = this.els.promptOptimizerApiBase.value.trim() || DEFAULT_PROMPT_OPTIMIZER_API_BASE;
    const model = this.els.promptOptimizerModel.value.trim() || DEFAULT_PROMPT_OPTIMIZER_MODEL;
    if (!model) {
      alert("请填写提示词优化模型");
      return;
    }

    this.setWorkflowState("optimizing");
    this.els.optimizePrompt.classList.add("is-loading");
    this.els.optimizePrompt.setAttribute("aria-busy", "true");
    this.els.optimizePrompt.textContent = "优化中";
    this.els.toolbarStatus.textContent = "正在优化提示词...";
    this.els.toolbarStatus.title = "";

    try {
      const result = await optimizePrompt(
        apiKey,
        apiBase,
        prompt,
        this.els.negPrompt.value.trim(),
        model,
        this.els.style.value,
        this.els.ratio.value,
        this.els.resolution.value,
        this.preferredSpriteGrid.rows,
        this.preferredSpriteGrid.cols,
        this.referenceImagePath,
        this.els.promptOptimizerVision.checked
      );

      this.els.prompt.value = result.prompt.trim();
      this.els.negPrompt.value = result.negativePrompt.trim();
      this.preferredSpriteGrid = {
        rows: normalizeGridSize(result.gridRows, this.preferredSpriteGrid.rows),
        cols: normalizeGridSize(result.gridCols, this.preferredSpriteGrid.cols),
      };
      await this.saveCurrentConfig();
      if (result.warning) {
        console.warn("[generator] 提示词优化降级:", result.warning);
        this.els.toolbarStatus.textContent = `提示词已优化 · 已降级 · 建议 ${this.preferredSpriteGrid.rows}x${this.preferredSpriteGrid.cols}`;
        this.els.toolbarStatus.title = result.warning;
      } else {
        this.els.toolbarStatus.textContent = `提示词已优化 · 建议 ${this.preferredSpriteGrid.rows}x${this.preferredSpriteGrid.cols}`;
        this.els.toolbarStatus.title = "";
      }
      this.els.prompt.focus();
    } catch (err) {
      const message = getErrorMessage(err);
      console.error("[generator] 提示词优化失败:", err);
      this.els.toolbarStatus.textContent = `提示词优化失败: ${message}`;
      this.els.toolbarStatus.title = message;
      alert(`提示词优化失败:\n${message}`);
    } finally {
      this.els.optimizePrompt.classList.remove("is-loading");
      this.els.optimizePrompt.removeAttribute("aria-busy");
      this.els.optimizePrompt.textContent = "优化提示词";
      this.settleWorkflowState();
    }
  }

  /// 核心：处理图片生成
  private async handleGenerate(): Promise<void> {
    console.log("[generator] handleGenerate 开始, isGenerating:", this.isGenerating);
    if (!this.canRunGeneratorAction("generate")) return;

    const apiKey = this.els.apiKey.value.trim();
    const prompt = this.els.prompt.value.trim();
    console.log("[generator] apiKey长度:", apiKey.length, "prompt长度:", prompt.length);

    if (!apiKey) {
      alert("请输入API Key");
      return;
    }
    if (!prompt) {
      alert("请输入提示词");
      return;
    }

    this.setWorkflowState("generating");
    this.showProgress(true);
    this.startGenerationTimer();

    // 构建参数
    const apiBase = this.els.apiBase.value.trim();
    const negPrompt = this.els.negPrompt.value.trim();
    const model = this.els.model.value;
    const style = this.els.style.value;
    const ratio = this.els.ratio.value;
    const resolution = this.els.resolution.value;
    const count = normalizeCount(this.els.count.value);
    this.expectedImageCount = count;

    try {
      // 生成前先保存提示词历史；历史失败不应阻塞图片生成。
      try {
        console.log("[generator] 即将调用 addPromptHistory");
        await addPromptHistory(prompt);
        console.log("[generator] addPromptHistory 完成");
        this.promptHistory = await getPromptHistory(100);
        this.historyIndex = this.promptHistory.length;
      } catch (historyErr) {
        console.warn("[generator] 保存提示词历史失败:", historyErr);
      }

      // 创建进度通道
      const channel = new Channel<GenerateEvent>();
      channel.onmessage = (event: GenerateEvent) => {
        this.handleProgress(event);
      };

      // 调用后端生成
      console.log("[generator] 即将调用 generateImage, 参数:", {
        model,
        style,
        ratio,
        resolution,
        count,
        hasReferenceImage: Boolean(this.referenceImagePath),
      });
      const result: GenerationResult = await generateImage(
        channel,
        apiKey,
        apiBase,
        prompt,
        negPrompt,
        model,
        style,
        ratio,
        resolution,
        count,
        this.referenceImagePath
      );

      // 显示结果
      await this.addResultsToWorkbench(result, { prompt, model });

      try {
        await this.saveCurrentConfig();
      } catch (saveErr) {
        console.error("[generator] 生成完成但保存配置失败:", saveErr);
        this.els.toolbarStatus.textContent = "生成完成，配置保存失败";
      }
    } catch (err) {
      console.error("[generator] 生成失败:", err);
      const elapsed = this.getElapsedText();
      this.updateProgressText(`错误: ${String(err)} · 用时 ${elapsed}`, true);
      this.els.toolbarStatus.textContent = `失败 ${elapsed}`;
    } finally {
      this.stopGenerationTimer();
      this.settleWorkflowState();
    }
  }

  /// 处理Channel进度事件
  private handleProgress(event: GenerateEvent): void {
    switch (event.event) {
      case "Started":
        this.updateProgressBar(0);
        this.updateProgressText("准备中...");
        break;
      case "SendingRequest":
        this.updateProgressBar(10);
        this.updateProgressText("正在发送请求...");
        break;
      case "ReceivingResponse":
        this.updateProgressBar(25);
        this.updateProgressText("正在接收模型响应...");
        break;
      case "ExtractingUrls":
        this.updateProgressBar(40);
        this.updateProgressText(`从响应中提取到 ${event.data.found} 张图片URL`);
        break;
      case "ProcessingImage":
        const total = Math.max(this.expectedImageCount, event.data.index, 1);
        const pct2 = 75 + ((event.data.index / total) * 20);
        this.updateProgressBar(pct2);
        this.updateProgressText(`正在处理第 ${event.data.index} 张图片 (${event.data.step})...`);
        break;
      case "Completed":
        const elapsed = this.getElapsedText();
        this.stopGenerationTimer();
        this.updateProgressBar(100);
        this.updateProgressText(
          `生成完成，共 ${event.data.total_images} 张图片，用时 ${elapsed}`
        );
        this.els.toolbarStatus.textContent = `完成 ${event.data.total_images} 张 · ${elapsed}`;
        break;
      case "Error":
        const errorElapsed = this.getElapsedText();
        this.stopGenerationTimer();
        this.updateProgressText(`错误: ${event.data.message} · 用时 ${errorElapsed}`, true);
        this.els.toolbarStatus.textContent = `失败 ${errorElapsed}`;
        break;
    }
  }

  private showProgress(show: boolean): void {
    this.els.progressContainer.style.display = show ? "flex" : "none";
    this.els.resultActions.style.display = "none";
    if (this.generatedRecords.length === 0) {
      this.els.resultCard.style.display = "none";
    } else {
      this.els.workspaceEmpty.style.display = "none";
      this.els.resultCard.style.display = "flex";
    }
    if (show) {
      this.updateProgressBar(0);
      this.updateProgressText("准备中...");
      this.els.toolbarStatus.textContent = "生成中...";
    }
  }

  private startGenerationTimer(): void {
    this.stopGenerationTimer();
    this.generationStartedAt = Date.now();
    this.lastGenerationElapsedText = "00:00";
    this.generationTimer = window.setInterval(() => {
      this.renderProgressText();
      this.els.toolbarStatus.textContent = `生成中 ${this.getElapsedText()}`;
    }, 1000);
    this.els.toolbarStatus.textContent = `生成中 ${this.getElapsedText()}`;
    this.renderProgressText();
  }

  private stopGenerationTimer(): void {
    if (this.generationStartedAt !== null) {
      this.lastGenerationElapsedText = this.formatElapsedText(
        Date.now() - this.generationStartedAt
      );
    }
    if (this.generationTimer !== null) {
      window.clearInterval(this.generationTimer);
      this.generationTimer = null;
    }
    this.generationStartedAt = null;
  }

  private updateProgressBar(percent: number): void {
    this.els.progressFill.style.width = `${Math.min(100, Math.max(0, percent))}%`;
  }

  private updateProgressText(text: string, isError: boolean = false): void {
    this.progressBaseText = text;
    this.progressIsError = isError;
    this.renderProgressText();
    this.els.progressText.style.color = isError ? "#c6613f" : "";
  }

  private renderProgressText(): void {
    if (this.generationStartedAt === null || this.progressIsError) {
      this.els.progressText.textContent = this.progressBaseText;
      return;
    }
    const elapsed = this.getElapsedText();
    this.els.progressText.textContent = `${this.progressBaseText} · 已等待 ${elapsed}`;
  }

  private getElapsedText(): string {
    if (this.generationStartedAt === null) {
      return this.lastGenerationElapsedText;
    }
    this.lastGenerationElapsedText = this.formatElapsedText(
      Date.now() - this.generationStartedAt
    );
    return this.lastGenerationElapsedText;
  }

  private formatElapsedText(elapsedMs: number): string {
    const elapsedSeconds = Math.max(0, Math.floor(elapsedMs / 1000));
    const minutes = Math.floor(elapsedSeconds / 60);
    const seconds = elapsedSeconds % 60;
    return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }

  private async addResultsToWorkbench(
    result: GenerationResult,
    context: { prompt: string; model: string }
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
      const record: GeneratedImageRecord = {
        id: `${Date.now()}-${i}-${this.generatedRecords.length}`,
        path,
        label: stripFileExtension(getFileName(path)) || `图片 ${this.generatedRecords.length + 1}`,
        prompt: context.prompt,
        model: context.model,
        durationSeconds: result.duration_seconds,
        createdAt: new Date(),
        updatedAt: new Date(),
      };
      if (existingIndex >= 0) {
        this.generatedRecords[existingIndex] = {
          ...this.generatedRecords[existingIndex],
          label: this.generatedRecords[existingIndex].label || record.label,
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
      console.warn("[generator] 同步工作台记录失败:", err);
      this.els.toolbarStatus.textContent = "生成完成，记录同步失败";
    }

    this.renderGallery();
  }

  private applyWorkbenchDtos(
    dtos: WorkbenchRecord[],
    selectedPath?: string | null
  ): void {
    this.generatedRecords = dtos.map(workbenchDtoToRecord);
    if (selectedPath && this.generatedRecords.some((item) => item.path === selectedPath)) {
      this.selectedGeneratedPath = selectedPath;
    } else if (!this.selectedGeneratedPath || !this.generatedRecords.some((item) => item.path === this.selectedGeneratedPath)) {
      this.selectedGeneratedPath = this.generatedRecords[this.generatedRecords.length - 1]?.path || null;
    }
  }

  private getSelectedRecord(): GeneratedImageRecord | null {
    if (!this.selectedGeneratedPath) return null;
    return this.generatedRecords.find((item) => item.path === this.selectedGeneratedPath) || null;
  }

  private renderGallery(): void {
    this.els.resultGrid.innerHTML = "";
    this.els.galleryCount.textContent = `${this.generatedRecords.length} 张`;

    if (this.generatedRecords.length === 0) {
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

    this.generatedRecords.forEach((record, i) => {
      const item = document.createElement("div");
      item.className = "image-item";
      item.dataset.path = record.path;
      if (record.path === this.selectedGeneratedPath) {
        item.classList.add("selected");
      }
      item.tabIndex = 0;

      const img = document.createElement("img");
      const assetSrc = convertFileSrc(record.path);
      let triedAssetFallback = false;
      img.src = assetSrc;
      img.alt = `生成图片 ${i + 1}`;
      img.loading = "lazy";
      img.onerror = () => {
        if (!triedAssetFallback) {
          triedAssetFallback = true;
          img.src = assetSrc;
        }
      };

      const meta = document.createElement("div");
      meta.className = "image-meta";
      const timeRow = document.createElement("span");
      timeRow.textContent = formatTime(record.createdAt);
      const durationRow = document.createElement("span");
      durationRow.textContent = `耗时 ${formatDuration(record.durationSeconds)}`;
      meta.appendChild(timeRow);
      meta.appendChild(durationRow);

      item.appendChild(img);
      item.appendChild(meta);
      item.addEventListener("click", () => this.selectGeneratedImage(record.path));
      item.addEventListener("dblclick", () => {
        if (!this.canRunGeneratorAction("openSelected")) return;
        openImageFilePath(record.path).catch((err) => {
          console.error("[generator] 打开图片失败:", err);
          this.els.toolbarStatus.textContent = "打开图片失败";
        });
      });
      item.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          this.selectGeneratedImage(record.path);
        }
      });
      this.els.resultGrid.appendChild(item);
    });

    if (!this.selectedGeneratedPath || !this.generatedRecords.some((item) => item.path === this.selectedGeneratedPath)) {
      this.selectedGeneratedPath = this.generatedRecords[this.generatedRecords.length - 1].path;
    }
    this.updateSelectedPreview();
    if (this.workflowState === "empty" || this.workflowState === "ready") {
      this.setWorkflowState(deriveGeneratorBaseState(this.getWorkflowContext()));
    } else {
      this.syncWorkflowControls();
    }
  }

  private selectGeneratedImage(path: string): void {
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
  }

  private updateGallerySelection(): void {
    this.els.resultGrid.querySelectorAll<HTMLElement>(".image-item").forEach((item) => {
      item.classList.toggle("selected", item.dataset.path === this.selectedGeneratedPath);
    });
  }

  private updateSelectedPreview(): void {
    const record = this.generatedRecords.find((item) => item.path === this.selectedGeneratedPath);
    if (!record) {
      this.els.selectedMeta.textContent = "未选择图片";
      this.els.selectedImage.removeAttribute("src");
      return;
    }

    const assetSrc = convertFileSrc(record.path);
    let triedAssetFallback = false;
    this.els.selectedImage.src = assetSrc;
    this.els.selectedImage.onerror = () => {
      if (!triedAssetFallback) {
        triedAssetFallback = true;
        this.els.selectedImage.src = assetSrc;
      }
    };
    this.els.selectedMeta.textContent =
      `${record.label} · ${record.model || "未知模型"} · ${formatTime(record.createdAt)} · 耗时 ${formatDuration(record.durationSeconds)}`;
  }

  private async saveCurrentConfig(): Promise<void> {
    if (!this.config) return;
    console.log("[generator] 保存配置...");
    this.config.api_key = this.els.apiKey.value.trim();
    this.config.api_base = this.els.apiBase.value.trim();
    this.config.proxy_url = this.els.proxyUrl.value.trim();
    this.config.last_model = this.els.model.value.trim();
    this.config.prompt_optimizer_api_key = this.els.promptOptimizerApiKey.value.trim();
    this.config.prompt_optimizer_api_base = this.els.promptOptimizerApiBase.value.trim();
    this.config.prompt_optimizer_model = this.els.promptOptimizerModel.value.trim();
    this.config.prompt_optimizer_vision = this.els.promptOptimizerVision.checked;
    this.config.last_style = this.els.style.value;
    this.config.last_ratio = this.els.ratio.value;
    this.config.last_resolution = this.els.resolution.value;
    this.config.last_count = normalizeCount(this.els.count.value);
    this.config.save_dir = this.els.saveDir.value;
    this.config.ffmpeg_path = this.els.ffmpegPath.value.trim();
    this.config.ffprobe_path = this.els.ffprobePath.value.trim();
    await saveConfig(this.config);
  }

  private syncWorkflowControls(): void {
    if (!this.els) return;

    const context = this.getWorkflowContext();
    const permissions = getGeneratorWorkflowPermissions(this.workflowState, context);
    const mattingVisible = this.isMattingMode;
    this.els.generationParams.hidden = mattingVisible;
    this.els.mattingParams.hidden = !mattingVisible;
    this.els.selectedPreview.classList.toggle("matting-active", mattingVisible);
    this.syncMattingModeButton();

    const resultActionsVisible = context.hasSelection && this.workflowState !== "generating";
    this.els.resultActions.style.display = resultActionsVisible ? "flex" : "none";

    this.els.generate.disabled = !permissions.generate;
    this.els.generate.setAttribute("aria-disabled", String(!permissions.generate));
    this.els.generate.classList.toggle("disabled", !permissions.generate);
    this.els.optimizePrompt.disabled = !permissions.optimizePrompt;
    this.els.addRecord.disabled = !permissions.addRecord;
    this.els.addRecordEmpty.disabled = !permissions.addRecord;
    this.els.viewImage.disabled = !permissions.openSelected;
    this.els.openDir.disabled = !permissions.revealSelected;
    this.els.transparentBackground.disabled = !(permissions.enterMatting || permissions.exitMatting);
    this.els.transparentBackground.textContent = mattingVisible ? "退出抠图" : "抠图模式";
    this.els.transparentBackground.title = mattingVisible ? "退出抠图模式" : "进入抠图模式";
    this.els.toSprite.disabled = !permissions.sendToSprite;
    this.els.deleteRecord.disabled = !permissions.deleteRecord;
    this.els.clearRecords.disabled = !permissions.clearRecords;
    this.els.btnSettings.disabled = !permissions.openSettings;
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

    [
      this.els.apiKey,
      this.els.apiBase,
      this.els.proxyUrl,
      this.els.model,
      this.els.checkGenerationApi,
      this.els.promptOptimizerApiKey,
      this.els.promptOptimizerApiBase,
      this.els.promptOptimizerModel,
      this.els.promptOptimizerVision,
      this.els.checkPromptOptimizerApi,
      this.els.saveDir,
      this.els.ffmpegPath,
      this.els.ffprobePath,
      this.els.browseDir,
      this.els.toggleKey,
      this.els.saveConfig,
    ].forEach((control) => {
      control.disabled = !permissions.openSettings;
    });
  }

  /// 获取最近一次生成的图片路径
  getLastGeneratedImages(): string[] {
    return this.generatedRecords.map((item) => item.path);
  }

  getSelectedGeneratedImagePath(): string | null {
    return this.selectedGeneratedPath;
  }

  /// 获取提示词优化器建议的序列帧网格
  getPreferredSpriteGrid(): SpriteGridPreset {
    return { ...this.preferredSpriteGrid };
  }
}

/// 辅助：设置select的值
function setSelectValue(sel: HTMLSelectElement, value: string): void {
  for (let i = 0; i < sel.options.length; i++) {
    if (sel.options[i].value === value) {
      sel.selectedIndex = i;
      return;
    }
  }
  // 如果值不存在，选择第一个
  if (sel.options.length > 0) {
    sel.selectedIndex = 0;
  }
}

function getDirectoryName(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const index = normalized.lastIndexOf("/");
  return index > 0 ? path.slice(0, index) : "";
}

function getFileName(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const index = normalized.lastIndexOf("/");
  return index >= 0 ? normalized.slice(index + 1) : normalized;
}

function stripFileExtension(fileName: string): string {
  const index = fileName.lastIndexOf(".");
  return index > 0 ? fileName.slice(0, index) : fileName;
}

function formatTime(date: Date): string {
  return date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

function parseLogTime(value: string): Date {
  if (!value) return new Date();
  const normalized = value.replace(" ", "T");
  const parsed = new Date(normalized);
  return Number.isNaN(parsed.getTime()) ? new Date() : parsed;
}

function workbenchDtoToRecord(dto: WorkbenchRecord): GeneratedImageRecord {
  return {
    id: dto.id,
    path: dto.path,
    label: dto.label || stripFileExtension(getFileName(dto.path)) || "未命名图片",
    prompt: dto.prompt || "",
    model: dto.model || "",
    durationSeconds: normalizeDurationSeconds(dto.durationSeconds),
    createdAt: parseLogTime(dto.createdAt),
    updatedAt: parseLogTime(dto.updatedAt || dto.createdAt),
  };
}

function recordToWorkbenchDto(record: GeneratedImageRecord): WorkbenchRecord {
  return {
    id: record.id,
    path: record.path,
    label: record.label,
    prompt: record.prompt,
    model: record.model,
    durationSeconds: record.durationSeconds,
    createdAt: formatRecordTime(record.createdAt),
    updatedAt: formatRecordTime(record.updatedAt),
  };
}

function normalizeDurationSeconds(value: number | undefined): number | undefined {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return undefined;
  }
  return Math.max(0, Math.round(Number(value) * 100) / 100);
}

function formatDuration(value: number | undefined): string {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return "-";
  }
  const seconds = Math.max(0, Number(value));
  if (seconds < 60) {
    return `${seconds.toFixed(seconds < 10 ? 1 : 0)}s`;
  }
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds % 60);
  return `${minutes}m ${String(rest).padStart(2, "0")}s`;
}

function formatRecordTime(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hour = String(date.getHours()).padStart(2, "0");
  const minute = String(date.getMinutes()).padStart(2, "0");
  const second = String(date.getSeconds()).padStart(2, "0");
  return `${year}-${month}-${day} ${hour}:${minute}:${second}`;
}

function normalizeCount(value: string): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return 1;
  }
  return Math.min(4, Math.max(1, parsed));
}

function normalizeGridSize(value: number, fallback: number): number {
  if (!Number.isFinite(value)) {
    return fallback;
  }
  return Math.min(20, Math.max(1, Math.round(value)));
}

function normalizeSliderValue(value: string, fallback: number, min: number, max: number): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.min(max, Math.max(min, parsed));
}

function loadImageFromDataUrl(dataUrl: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("图片载入失败"));
    image.src = dataUrl;
  });
}

function getErrorMessage(err: unknown): string {
  if (err instanceof Error && err.message) {
    return err.message;
  }
  if (typeof err === "string") {
    return err;
  }
  if (err && typeof err === "object") {
    const record = err as Record<string, unknown>;
    for (const key of ["message", "error", "reason"]) {
      const value = record[key];
      if (typeof value === "string" && value.trim()) {
        return value;
      }
    }
    try {
      return JSON.stringify(err);
    } catch (_) {
      return String(err);
    }
  }
  return String(err);
}

function getEraseFailureText(reason: "outside" | "no_seed" | "no_match" | "erased"): string {
  switch (reason) {
    case "outside":
      return "点击位置超出图片范围";
    case "no_seed":
      return "附近没有可擦除像素";
    case "no_match":
      return "点击区域没有可擦除像素";
    case "erased":
    default:
      return "点击位置已透明或没有可擦除区域";
  }
}
