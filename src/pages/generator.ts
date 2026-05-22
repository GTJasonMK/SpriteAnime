import { Channel, convertFileSrc } from "@tauri-apps/api/core";
import {
  getPresets,
  loadConfig,
  saveConfig,
  exportConfig,
  importConfig,
  checkGenerationApi,
  checkPromptOptimizerApi,
  checkFfmpegTools,
  downloadFfmpegTools,
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
  openImageFile,
  importImageToLibrary,
  revealInExplorer,
  openImageFilePath,
  type PresetsPayload,
  type UserConfig,
  type ApiProfile,
  type ApiCheckResult,
  type FfmpegToolStatus,
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
import { getById, queryAll } from "../utils/dom";
import { loadImageFromDataUrl } from "../utils/image";
import { parseClampedInt } from "../utils/number";
import { getDirectoryName, getFileName, stripFileExtension } from "../utils/path";
import { clickTab, dispatchPrepareSpriteFromGenerator } from "./navigation";
import {
  renderGeneratedGallery,
  setSelectedGeneratedPreview,
  updateGeneratedGallerySelection,
} from "./generator-gallery";

interface SpriteGridPreset {
  rows: number;
  cols: number;
}

interface PromptOptimizerSettings {
  apiKey: string;
  apiBase: string;
  model: string;
}

interface ApiProfileFormValues {
  apiKey: string;
  apiBase: string;
  proxyUrl: string;
  apiMode: string;
  model: string;
  videoApiKey: string;
  videoApiBase: string;
  videoProxyUrl: string;
  videoModel: string;
  videoApiMode: string;
  promptOptimizerApiKey: string;
  promptOptimizerApiBase: string;
  promptOptimizerModel: string;
  promptOptimizerVision: boolean;
}

export interface ActiveApiSettings {
  apiKey: string;
  apiBase: string;
  proxyUrl: string;
  apiMode: string;
  model: string;
  videoApiKey: string;
  videoApiBase: string;
  videoProxyUrl: string;
  videoModel: string;
  videoApiMode: string;
  profileName: string;
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
const DEFAULT_GENERATION_MODEL = "gpt-5.3-codex";
const DEFAULT_GENERATION_API_MODE = "responses";
const DEFAULT_VIDEO_MODEL = "sora-2";
const DEFAULT_VIDEO_API_MODE = "chat_completions";

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
  private isInstallingFfmpeg: boolean = false;

  // DOM元素缓存
  private els!: {
    apiKey: HTMLInputElement;
    apiBase: HTMLInputElement;
    proxyUrl: HTMLInputElement;
    generationApiMode: HTMLSelectElement;
    activeApiProfile: HTMLSelectElement;
    profileName: HTMLInputElement;
    profileList: HTMLElement;
    addApiProfile: HTMLButtonElement;
    duplicateApiProfile: HTMLButtonElement;
    deleteApiProfile: HTMLButtonElement;
    importConfig: HTMLButtonElement;
    exportConfig: HTMLButtonElement;
    toggleKey: HTMLButtonElement;
    model: HTMLInputElement;
    modelList: HTMLDataListElement;
    checkGenerationApi: HTMLButtonElement;
    generationApiCheckStatus: HTMLElement;
    videoApiKey: HTMLInputElement;
    videoApiBase: HTMLInputElement;
    videoProxyUrl: HTMLInputElement;
    videoModel: HTMLInputElement;
    videoApiMode: HTMLSelectElement;
    checkVideoApi: HTMLButtonElement;
    videoApiCheckStatus: HTMLElement;
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
    checkFfmpegTools: HTMLButtonElement;
    downloadFfmpegTools: HTMLButtonElement;
    ffmpegToolStatus: HTMLElement;
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
    settingsTabs: HTMLElement[];
    settingsPanels: HTMLElement[];
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
    this.els = {
      apiKey: getById<HTMLInputElement>("api-key"),
      apiBase: getById<HTMLInputElement>("api-base"),
      proxyUrl: getById<HTMLInputElement>("proxy-url"),
      generationApiMode: getById<HTMLSelectElement>("generation-api-mode"),
      activeApiProfile: getById<HTMLSelectElement>("active-api-profile"),
      profileName: getById<HTMLInputElement>("api-profile-name"),
      profileList: getById("api-profile-list"),
      addApiProfile: getById<HTMLButtonElement>("btn-add-api-profile"),
      duplicateApiProfile: getById<HTMLButtonElement>("btn-duplicate-api-profile"),
      deleteApiProfile: getById<HTMLButtonElement>("btn-delete-api-profile"),
      importConfig: getById<HTMLButtonElement>("btn-import-config"),
      exportConfig: getById<HTMLButtonElement>("btn-export-config"),
      toggleKey: getById<HTMLButtonElement>("btn-toggle-key"),
      model: getById<HTMLInputElement>("model-input"),
      modelList: getById<HTMLDataListElement>("model-list"),
      checkGenerationApi: getById<HTMLButtonElement>("btn-check-generation-api"),
      generationApiCheckStatus: getById("generation-api-check-status"),
      videoApiKey: getById<HTMLInputElement>("video-api-key"),
      videoApiBase: getById<HTMLInputElement>("video-api-base"),
      videoProxyUrl: getById<HTMLInputElement>("video-proxy-url"),
      videoModel: getById<HTMLInputElement>("video-model-input"),
      videoApiMode: getById<HTMLSelectElement>("video-api-mode"),
      checkVideoApi: getById<HTMLButtonElement>("btn-check-video-api"),
      videoApiCheckStatus: getById("video-api-check-status"),
      optimizePrompt: getById<HTMLButtonElement>("btn-optimize-prompt"),
      promptOptimizerApiKey: getById<HTMLInputElement>("prompt-optimizer-api-key"),
      promptOptimizerApiBase: getById<HTMLInputElement>("prompt-optimizer-api-base"),
      promptOptimizerModel: getById<HTMLInputElement>("prompt-optimizer-model-input"),
      promptOptimizerVision: getById<HTMLInputElement>("prompt-optimizer-vision"),
      checkPromptOptimizerApi: getById<HTMLButtonElement>("btn-check-prompt-optimizer-api"),
      promptOptimizerApiCheckStatus: getById("prompt-optimizer-api-check-status"),
      style: getById<HTMLSelectElement>("style-select"),
      ratio: getById<HTMLSelectElement>("ratio-select"),
      resolution: getById<HTMLSelectElement>("resolution-select"),
      count: getById<HTMLSelectElement>("count-select"),
      prompt: getById<HTMLTextAreaElement>("prompt-input"),
      negPrompt: getById<HTMLInputElement>("neg-prompt-input"),
      referenceImageName: getById<HTMLInputElement>("reference-image-name"),
      referenceImagePreview: getById<HTMLImageElement>("reference-image-preview"),
      referenceImageEmpty: getById("reference-image-empty"),
      pickReferenceImage: getById<HTMLButtonElement>("btn-pick-reference-image"),
      clearReferenceImage: getById<HTMLButtonElement>("btn-clear-reference-image"),
      saveDir: getById<HTMLInputElement>("save-dir-input"),
      ffmpegPath: getById<HTMLInputElement>("ffmpeg-path"),
      ffprobePath: getById<HTMLInputElement>("ffprobe-path"),
      checkFfmpegTools: getById<HTMLButtonElement>("btn-check-ffmpeg-tools"),
      downloadFfmpegTools: getById<HTMLButtonElement>("btn-download-ffmpeg-tools"),
      ffmpegToolStatus: getById("ffmpeg-tool-status"),
      generate: getById<HTMLButtonElement>("btn-generate"),
      viewImage: getById<HTMLButtonElement>("btn-view-image"),
      openDir: getById<HTMLButtonElement>("btn-open-dir"),
      transparentBackground: getById<HTMLButtonElement>("btn-transparent-background"),
      toSprite: getById<HTMLButtonElement>("btn-to-sprite"),
      saveConfig: getById<HTMLButtonElement>("btn-save-config"),
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
      btnSettings: getById<HTMLButtonElement>("btn-settings"),
      modalOverlay: getById("settings-modal"),
      btnCloseModal: getById<HTMLButtonElement>("btn-close-modal"),
      settingsTabs: queryAll<HTMLElement>(".settings-tab"),
      settingsPanels: queryAll<HTMLElement>(".settings-panel"),
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
      this.normalizeApiProfiles();
      console.log("[generator] 配置加载完成", {
        model: this.getActiveApiProfile().last_model,
        profiles: this.config.api_profiles.length,
        hasApiKey: !!this.getActiveApiProfile().api_key,
      });

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

  getActiveApiSettings(): ActiveApiSettings {
    if (!this.config) {
      return {
        apiKey: "",
        apiBase: "",
        proxyUrl: "",
        apiMode: DEFAULT_GENERATION_API_MODE,
        model: DEFAULT_GENERATION_MODEL,
        videoApiKey: "",
        videoApiBase: "",
        videoProxyUrl: "",
        videoModel: DEFAULT_VIDEO_MODEL,
        videoApiMode: DEFAULT_VIDEO_API_MODE,
        profileName: "默认 API",
      };
    }
    this.writeFormToActiveProfile();
    const profile = this.getActiveApiProfile();
    return {
      apiKey: profile.api_key || "",
      apiBase: profile.api_base || "",
      proxyUrl: profile.proxy_url || "",
      apiMode: normalizeGenerationApiMode(profile.generation_api_mode),
      model: profile.last_model || DEFAULT_GENERATION_MODEL,
      videoApiKey: profile.video_api_key || profile.api_key || "",
      videoApiBase: profile.video_api_base || profile.api_base || "",
      videoProxyUrl: profile.video_proxy_url || profile.proxy_url || "",
      videoModel: profile.video_model || DEFAULT_VIDEO_MODEL,
      videoApiMode: normalizeVideoApiMode(profile.video_api_mode),
      profileName: profile.name || "API 配置",
    };
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
    this.normalizeApiProfiles();
    this.renderApiProfiles();
    this.applyProfileToForm(this.getActiveApiProfile());
    setSelectValue(this.els.style, c.last_style);
    setSelectValue(this.els.ratio, c.last_ratio);
    setSelectValue(this.els.resolution, c.last_resolution);
    setSelectValue(this.els.count, String(c.last_count));
    this.els.saveDir.value = c.save_dir || "";
    this.els.ffmpegPath.value = c.ffmpeg_path || "";
    this.els.ffprobePath.value = c.ffprobe_path || "";
  }

  private normalizeApiProfiles(): void {
    if (!this.config) return;
    const profiles = Array.isArray(this.config.api_profiles)
      ? this.config.api_profiles
      : [];
    if (profiles.length === 0) {
      profiles.push(this.createApiProfile("默认 API", {
        apiKey: this.config.api_key || "",
        apiBase: this.config.api_base || "",
        proxyUrl: this.config.proxy_url || "",
        apiMode: normalizeGenerationApiMode(this.config.generation_api_mode),
        model: this.config.last_model || DEFAULT_GENERATION_MODEL,
        videoApiKey: this.config.video_api_key || "",
        videoApiBase: this.config.video_api_base || "",
        videoProxyUrl: this.config.video_proxy_url || "",
        videoModel: this.config.video_model || DEFAULT_VIDEO_MODEL,
        videoApiMode: normalizeVideoApiMode(this.config.video_api_mode),
        promptOptimizerApiKey: this.config.prompt_optimizer_api_key || "",
        promptOptimizerApiBase:
          this.config.prompt_optimizer_api_base || DEFAULT_PROMPT_OPTIMIZER_API_BASE,
        promptOptimizerModel:
          this.config.prompt_optimizer_model || DEFAULT_PROMPT_OPTIMIZER_MODEL,
        promptOptimizerVision: Boolean(this.config.prompt_optimizer_vision),
      }));
    }

    const usedIds = new Set<string>();
    this.config.api_profiles = profiles.map((profile, index) => {
      const fallbackId = index === 0 ? "default" : `api-profile-${index + 1}`;
      const id = this.uniqueApiProfileId(profile.id || fallbackId, usedIds);
      usedIds.add(id);
      return {
        id,
        name: (profile.name || "").trim() || `API 配置 ${index + 1}`,
        api_key: (profile.api_key || "").trim(),
        api_base: (profile.api_base || "").trim(),
        proxy_url: (profile.proxy_url || "").trim(),
        generation_api_mode: normalizeGenerationApiMode(profile.generation_api_mode),
        last_model: (profile.last_model || "").trim() || DEFAULT_GENERATION_MODEL,
        video_api_key: (profile.video_api_key || "").trim(),
        video_api_base: (profile.video_api_base || "").trim(),
        video_proxy_url: (profile.video_proxy_url || "").trim(),
        video_model: (profile.video_model || "").trim() || DEFAULT_VIDEO_MODEL,
        video_api_mode: normalizeVideoApiMode(profile.video_api_mode),
        prompt_optimizer_api_key: (profile.prompt_optimizer_api_key || "").trim(),
        prompt_optimizer_api_base:
          (profile.prompt_optimizer_api_base || "").trim() ||
          DEFAULT_PROMPT_OPTIMIZER_API_BASE,
        prompt_optimizer_model:
          (profile.prompt_optimizer_model || "").trim() ||
          DEFAULT_PROMPT_OPTIMIZER_MODEL,
        prompt_optimizer_vision: Boolean(profile.prompt_optimizer_vision),
      };
    });

    if (
      !this.config.api_profiles.some(
        (profile) => profile.id === this.config!.active_api_profile_id
      )
    ) {
      this.config.active_api_profile_id = this.config.api_profiles[0].id;
    }
    this.syncActiveProfileToLegacyConfig();
  }

  private getActiveApiProfile(): ApiProfile {
    if (!this.config) {
      return this.createApiProfile("默认 API");
    }
    return (
      this.config.api_profiles.find(
        (profile) => profile.id === this.config!.active_api_profile_id
      ) || this.config.api_profiles[0]
    );
  }

  private getCurrentProfileFormValues(): ApiProfileFormValues {
    return {
      apiKey: this.els.apiKey.value.trim(),
      apiBase: this.els.apiBase.value.trim(),
      proxyUrl: this.els.proxyUrl.value.trim(),
      apiMode: normalizeGenerationApiMode(this.els.generationApiMode.value),
      model: this.els.model.value.trim(),
      videoApiKey: this.els.videoApiKey.value.trim(),
      videoApiBase: this.els.videoApiBase.value.trim(),
      videoProxyUrl: this.els.videoProxyUrl.value.trim(),
      videoModel: this.els.videoModel.value.trim(),
      videoApiMode: normalizeVideoApiMode(this.els.videoApiMode.value),
      promptOptimizerApiKey: this.els.promptOptimizerApiKey.value.trim(),
      promptOptimizerApiBase: this.els.promptOptimizerApiBase.value.trim(),
      promptOptimizerModel: this.els.promptOptimizerModel.value.trim(),
      promptOptimizerVision: this.els.promptOptimizerVision.checked,
    };
  }

  private applyProfileToForm(profile: ApiProfile): void {
    this.els.apiKey.value = profile.api_key || "";
    this.els.apiBase.value = profile.api_base || "";
    this.els.proxyUrl.value = profile.proxy_url || "";
    setSelectValue(this.els.generationApiMode, normalizeGenerationApiMode(profile.generation_api_mode));
    this.els.model.value = profile.last_model || DEFAULT_GENERATION_MODEL;
    this.els.videoApiKey.value = profile.video_api_key || "";
    this.els.videoApiBase.value = profile.video_api_base || "";
    this.els.videoProxyUrl.value = profile.video_proxy_url || "";
    this.els.videoModel.value = profile.video_model || DEFAULT_VIDEO_MODEL;
    setSelectValue(this.els.videoApiMode, normalizeVideoApiMode(profile.video_api_mode));
    this.els.promptOptimizerApiKey.value = profile.prompt_optimizer_api_key || "";
    this.els.promptOptimizerApiBase.value =
      profile.prompt_optimizer_api_base || DEFAULT_PROMPT_OPTIMIZER_API_BASE;
    this.els.promptOptimizerModel.value =
      profile.prompt_optimizer_model || DEFAULT_PROMPT_OPTIMIZER_MODEL;
    this.els.promptOptimizerVision.checked = Boolean(profile.prompt_optimizer_vision);
    this.els.profileName.value = profile.name || "";
    this.clearApiCheckStatuses();
  }

  private writeFormToActiveProfile(): void {
    if (!this.config) return;
    const profile = this.getActiveApiProfile();
    const values = this.getCurrentProfileFormValues();
    profile.name = this.els.profileName.value.trim() || profile.name || "API 配置";
    profile.api_key = values.apiKey;
    profile.api_base = values.apiBase;
    profile.proxy_url = values.proxyUrl;
    profile.generation_api_mode = values.apiMode;
    profile.last_model = values.model || DEFAULT_GENERATION_MODEL;
    profile.video_api_key = values.videoApiKey;
    profile.video_api_base = values.videoApiBase;
    profile.video_proxy_url = values.videoProxyUrl;
    profile.video_model = values.videoModel || DEFAULT_VIDEO_MODEL;
    profile.video_api_mode = values.videoApiMode;
    profile.prompt_optimizer_api_key = values.promptOptimizerApiKey;
    profile.prompt_optimizer_api_base =
      values.promptOptimizerApiBase || DEFAULT_PROMPT_OPTIMIZER_API_BASE;
    profile.prompt_optimizer_model =
      values.promptOptimizerModel || DEFAULT_PROMPT_OPTIMIZER_MODEL;
    profile.prompt_optimizer_vision = values.promptOptimizerVision;
    this.syncActiveProfileToLegacyConfig();
  }

  private syncActiveProfileToLegacyConfig(): void {
    if (!this.config) return;
    const profile = this.getActiveApiProfile();
    this.config.api_key = profile.api_key;
    this.config.api_base = profile.api_base;
    this.config.proxy_url = profile.proxy_url;
    this.config.generation_api_mode = normalizeGenerationApiMode(profile.generation_api_mode);
    this.config.last_model = profile.last_model;
    this.config.video_api_key = profile.video_api_key;
    this.config.video_api_base = profile.video_api_base;
    this.config.video_proxy_url = profile.video_proxy_url;
    this.config.video_model = profile.video_model || DEFAULT_VIDEO_MODEL;
    this.config.video_api_mode = normalizeVideoApiMode(profile.video_api_mode);
    this.config.prompt_optimizer_api_key = profile.prompt_optimizer_api_key;
    this.config.prompt_optimizer_api_base = profile.prompt_optimizer_api_base;
    this.config.prompt_optimizer_model = profile.prompt_optimizer_model;
    this.config.prompt_optimizer_vision = profile.prompt_optimizer_vision;
  }

  private syncActiveApiProfileFromForm(): void {
    if (!this.config) return;
    this.writeFormToActiveProfile();
    this.renderApiProfiles();
  }

  private renderApiProfiles(): void {
    if (!this.config) return;
    const activeId = this.config.active_api_profile_id;

    this.els.activeApiProfile.innerHTML = "";
    this.config.api_profiles.forEach((profile) => {
      const option = document.createElement("option");
      option.value = profile.id;
      option.textContent = profile.name || "API 配置";
      option.selected = profile.id === activeId;
      this.els.activeApiProfile.appendChild(option);
    });

    this.els.profileList.innerHTML = "";
    this.config.api_profiles.forEach((profile) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "api-profile-item";
      button.classList.toggle("active", profile.id === activeId);
      button.dataset.profileId = profile.id;

      const name = document.createElement("span");
      name.className = "api-profile-name";
      name.textContent = profile.name || "API 配置";
      button.appendChild(name);

      const meta = document.createElement("span");
      meta.className = "api-profile-meta";
      meta.textContent = `图: ${profile.last_model || DEFAULT_GENERATION_MODEL} · 视频: ${profile.video_model || DEFAULT_VIDEO_MODEL}`;
      button.appendChild(meta);

      button.addEventListener("click", () => this.switchApiProfile(profile.id));
      this.els.profileList.appendChild(button);
    });

    const activeProfile = this.getActiveApiProfile();
    this.els.profileName.value = activeProfile.name;
    this.els.deleteApiProfile.disabled = this.config.api_profiles.length <= 1;
  }

  private createApiProfile(
    name: string,
    values: Partial<ApiProfileFormValues> = {}
  ): ApiProfile {
    return {
      id: this.createApiProfileId(),
      name,
      api_key: values.apiKey || "",
      api_base: values.apiBase || "",
      proxy_url: values.proxyUrl || "",
      generation_api_mode: normalizeGenerationApiMode(values.apiMode),
      last_model: values.model || DEFAULT_GENERATION_MODEL,
      video_api_key: values.videoApiKey || "",
      video_api_base: values.videoApiBase || "",
      video_proxy_url: values.videoProxyUrl || "",
      video_model: values.videoModel || DEFAULT_VIDEO_MODEL,
      video_api_mode: normalizeVideoApiMode(values.videoApiMode),
      prompt_optimizer_api_key: values.promptOptimizerApiKey || "",
      prompt_optimizer_api_base:
        values.promptOptimizerApiBase || DEFAULT_PROMPT_OPTIMIZER_API_BASE,
      prompt_optimizer_model:
        values.promptOptimizerModel || DEFAULT_PROMPT_OPTIMIZER_MODEL,
      prompt_optimizer_vision: Boolean(values.promptOptimizerVision),
    };
  }

  private createApiProfileId(): string {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
      return `api-${crypto.randomUUID()}`;
    }
    return `api-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  }

  private uniqueApiProfileId(baseId: string, usedIds: Set<string>): string {
    const cleanBase = baseId.trim() || "api-profile";
    if (!usedIds.has(cleanBase)) {
      return cleanBase;
    }
    let index = 2;
    while (usedIds.has(`${cleanBase}-${index}`)) {
      index += 1;
    }
    return `${cleanBase}-${index}`;
  }

  private clearApiCheckStatuses(): void {
    this.els.generationApiCheckStatus.className = "config-check-status";
    this.els.generationApiCheckStatus.textContent = "";
    this.els.generationApiCheckStatus.title = "";
    this.els.videoApiCheckStatus.className = "config-check-status";
    this.els.videoApiCheckStatus.textContent = "";
    this.els.videoApiCheckStatus.title = "";
    this.els.promptOptimizerApiCheckStatus.className = "config-check-status";
    this.els.promptOptimizerApiCheckStatus.textContent = "";
    this.els.promptOptimizerApiCheckStatus.title = "";
  }

  private switchApiProfile(profileId: string): void {
    if (!this.config) return;
    if (!this.config.api_profiles.some((profile) => profile.id === profileId)) {
      this.renderApiProfiles();
      return;
    }
    if (this.config.active_api_profile_id === profileId) {
      this.renderApiProfiles();
      return;
    }

    this.writeFormToActiveProfile();
    this.config.active_api_profile_id = profileId;
    this.syncActiveProfileToLegacyConfig();
    this.renderApiProfiles();
    this.applyProfileToForm(this.getActiveApiProfile());
    this.els.toolbarStatus.textContent = `已切换 API：${this.getActiveApiProfile().name}`;
  }

  private renameActiveApiProfile(name: string): void {
    if (!this.config) return;
    const profile = this.getActiveApiProfile();
    profile.name = name;
    const selectionStart = this.els.profileName.selectionStart;
    const selectionEnd = this.els.profileName.selectionEnd;
    this.renderApiProfiles();
    this.els.profileName.focus();
    this.els.profileName.setSelectionRange(selectionStart, selectionEnd);
  }

  private addApiProfile(): void {
    if (!this.config) return;
    this.writeFormToActiveProfile();
    const profileNumber = this.config.api_profiles.length + 1;
    const profile = this.createApiProfile(`API 配置 ${profileNumber}`);
    this.config.api_profiles.push(profile);
    this.config.active_api_profile_id = profile.id;
    this.syncActiveProfileToLegacyConfig();
    this.renderApiProfiles();
    this.applyProfileToForm(profile);
    this.els.profileName.focus();
    this.els.profileName.select();
    this.els.toolbarStatus.textContent = "已新增 API 配置";
  }

  private duplicateApiProfile(): void {
    if (!this.config) return;
    this.writeFormToActiveProfile();
    const source = this.getActiveApiProfile();
    const profile = {
      ...source,
      id: this.createApiProfileId(),
      name: `${source.name || "API 配置"} 副本`,
    };
    this.config.api_profiles.push(profile);
    this.config.active_api_profile_id = profile.id;
    this.syncActiveProfileToLegacyConfig();
    this.renderApiProfiles();
    this.applyProfileToForm(profile);
    this.els.profileName.focus();
    this.els.profileName.select();
    this.els.toolbarStatus.textContent = "已复制 API 配置";
  }

  private deleteApiProfile(): void {
    if (!this.config || this.config.api_profiles.length <= 1) return;
    const activeProfile = this.getActiveApiProfile();
    const ok = window.confirm(`删除 API 配置「${activeProfile.name}」？`);
    if (!ok) {
      this.renderApiProfiles();
      return;
    }

    const oldIndex = this.config.api_profiles.findIndex(
      (profile) => profile.id === activeProfile.id
    );
    this.config.api_profiles = this.config.api_profiles.filter(
      (profile) => profile.id !== activeProfile.id
    );
    const nextIndex = Math.min(Math.max(oldIndex, 0), this.config.api_profiles.length - 1);
    const nextProfile = this.config.api_profiles[nextIndex];
    this.config.active_api_profile_id = nextProfile.id;
    this.syncActiveProfileToLegacyConfig();
    this.renderApiProfiles();
    this.applyProfileToForm(nextProfile);
    this.els.toolbarStatus.textContent = "已删除 API 配置";
  }

  private syncConfigFromForm(): UserConfig | null {
    if (!this.config) return null;
    this.writeFormToActiveProfile();
    this.config.last_style = this.els.style.value;
    this.config.last_ratio = this.els.ratio.value;
    this.config.last_resolution = this.els.resolution.value;
    this.config.last_count = parseClampedInt(this.els.count.value, 1, 1, 4);
    this.config.save_dir = this.els.saveDir.value;
    this.config.ffmpeg_path = this.els.ffmpegPath.value.trim();
    this.config.ffprobe_path = this.els.ffprobePath.value.trim();
    this.config.prompt_history = [...this.promptHistory];
    return this.config;
  }

  private bindEvents(): void {
    this.els.optimizePrompt.addEventListener("click", () => {
      this.handleOptimizePrompt();
    });

    this.els.activeApiProfile.addEventListener("change", () => {
      this.switchApiProfile(this.els.activeApiProfile.value);
    });

    this.els.profileName.addEventListener("input", () => {
      this.renameActiveApiProfile(this.els.profileName.value);
    });

    [
      this.els.apiKey,
      this.els.apiBase,
      this.els.proxyUrl,
      this.els.model,
      this.els.videoApiKey,
      this.els.videoApiBase,
      this.els.videoProxyUrl,
      this.els.videoModel,
      this.els.videoApiMode,
      this.els.promptOptimizerApiKey,
      this.els.promptOptimizerApiBase,
      this.els.promptOptimizerModel,
    ].forEach((input) => {
      input.addEventListener("input", () => this.syncActiveApiProfileFromForm());
    });
    this.els.promptOptimizerVision.addEventListener("change", () => {
      this.syncActiveApiProfileFromForm();
    });
    this.els.generationApiMode.addEventListener("change", () => {
      this.syncActiveApiProfileFromForm();
    });
    this.els.videoApiMode.addEventListener("change", () => {
      this.syncActiveApiProfileFromForm();
    });

    this.els.addApiProfile.addEventListener("click", () => {
      this.addApiProfile();
    });

    this.els.duplicateApiProfile.addEventListener("click", () => {
      this.duplicateApiProfile();
    });

    this.els.deleteApiProfile.addEventListener("click", () => {
      this.deleteApiProfile();
    });

    this.els.importConfig.addEventListener("click", () => {
      this.handleImportConfig();
    });

    this.els.exportConfig.addEventListener("click", () => {
      this.handleExportConfig();
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

    this.els.checkVideoApi.addEventListener("click", () => {
      this.handleCheckVideoApi();
    });

    this.els.checkPromptOptimizerApi.addEventListener("click", () => {
      this.handleCheckPromptOptimizerApi();
    });

    this.els.checkFfmpegTools.addEventListener("click", () => {
      this.handleCheckFfmpegTools();
    });

    this.els.downloadFfmpegTools.addEventListener("click", () => {
      this.handleDownloadFfmpegTools();
    });

    this.els.settingsTabs.forEach((tab) => {
      tab.addEventListener("click", () => {
        this.showSettingsTab(tab.dataset.settingsTab || "profiles");
      });
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
      clickTab("sprite");
      dispatchPrepareSpriteFromGenerator();
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
    const activeTab = this.els.settingsTabs.find((tab) =>
      tab.classList.contains("active")
    );
    this.showSettingsTab(activeTab?.dataset.settingsTab || "profiles");
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

  private showSettingsTab(tabName: string): void {
    const hasTab = this.els.settingsTabs.some(
      (tab) => tab.dataset.settingsTab === tabName
    );
    const nextTab = hasTab ? tabName : "profiles";
    this.els.settingsTabs.forEach((tab) => {
      const active = tab.dataset.settingsTab === nextTab;
      tab.classList.toggle("active", active);
      tab.setAttribute("aria-selected", String(active));
    });
    this.els.settingsPanels.forEach((panel) => {
      const active = panel.dataset.settingsPanel === nextTab;
      panel.classList.toggle("active", active);
      panel.hidden = !active;
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
      const imported = await importImageToLibrary(file.file_path);
      this.setReferenceImage(imported.file_path, imported.file_name || file.file_name);
      this.els.toolbarStatus.textContent = "已选择参考图";
    } catch (err) {
      if (!String(err).includes("用户取消")) {
        console.error("[generator] 选择参考图失败:", err);
        this.els.toolbarStatus.textContent = "选择参考图失败";
      }
    }
  }

  private async handleImportConfig(): Promise<void> {
    if (this.els.importConfig.disabled) return;
    const ok = window.confirm("导入配置会替换当前所有设置。继续？");
    if (!ok) return;

    const originalText = this.els.importConfig.textContent || "导入配置";
    this.els.importConfig.disabled = true;
    this.els.importConfig.textContent = "导入中";
    this.els.toolbarStatus.textContent = "正在导入配置...";

    try {
      const result = await importConfig();
      this.config = result.config;
      this.normalizeApiProfiles();
      this.promptHistory = Array.isArray(this.config.prompt_history)
        ? [...this.config.prompt_history]
        : await getPromptHistory(100);
      this.historyIndex = this.promptHistory.length;
      this.applyConfig();
      this.syncWorkflowControls();
      this.els.toolbarStatus.textContent = `配置已导入：${getFileName(result.file_path) || result.file_path}`;
    } catch (err) {
      if (String(err).includes("用户取消")) {
        this.els.toolbarStatus.textContent = "已取消导入";
      } else {
        console.error("[generator] 导入配置失败:", err);
        this.els.toolbarStatus.textContent = "导入配置失败";
      }
    } finally {
      this.els.importConfig.textContent = originalText;
      this.syncWorkflowControls();
    }
  }

  private async handleExportConfig(): Promise<void> {
    if (this.els.exportConfig.disabled || !this.config) return;

    const originalText = this.els.exportConfig.textContent || "导出配置";
    this.els.exportConfig.disabled = true;
    this.els.exportConfig.textContent = "导出中";
    this.els.toolbarStatus.textContent = "正在导出配置...";

    try {
      this.promptHistory = await getPromptHistory(100);
      const config = this.syncConfigFromForm();
      if (!config) return;
      const result = await exportConfig(config);
      this.els.toolbarStatus.textContent = `配置已导出：${getFileName(result.file_path) || result.file_path}`;
    } catch (err) {
      if (String(err).includes("用户取消")) {
        this.els.toolbarStatus.textContent = "已取消导出";
      } else {
        console.error("[generator] 导出配置失败:", err);
        this.els.toolbarStatus.textContent = "导出配置失败";
      }
    } finally {
      this.els.exportConfig.textContent = originalText;
      this.syncWorkflowControls();
    }
  }

  private async handleCheckGenerationApi(): Promise<void> {
    await this.runApiCheck(
      this.els.checkGenerationApi,
      this.els.generationApiCheckStatus,
      async () => {
        await this.saveCurrentConfig();
        return checkGenerationApi(
          this.els.apiKey.value.trim(),
          this.els.apiBase.value.trim(),
          this.els.model.value.trim(),
          this.els.proxyUrl.value.trim()
        );
      }
    );
  }

  private async handleCheckVideoApi(): Promise<void> {
    await this.runApiCheck(
      this.els.checkVideoApi,
      this.els.videoApiCheckStatus,
      async () => {
        await this.saveCurrentConfig();
        return checkGenerationApi(
          this.els.videoApiKey.value.trim() || this.els.apiKey.value.trim(),
          this.els.videoApiBase.value.trim() || this.els.apiBase.value.trim(),
          this.els.videoModel.value.trim() || DEFAULT_VIDEO_MODEL,
          this.els.videoProxyUrl.value.trim() || this.els.proxyUrl.value.trim()
        );
      }
    );
  }

  private async handleCheckPromptOptimizerApi(): Promise<void> {
    await this.runApiCheck(
      this.els.checkPromptOptimizerApi,
      this.els.promptOptimizerApiCheckStatus,
      async () => {
        await this.saveCurrentConfig();
        const { apiKey, apiBase, model } = this.getPromptOptimizerSettings();
        return checkPromptOptimizerApi(
          apiKey,
          apiBase,
          model,
          this.els.proxyUrl.value.trim()
        );
      }
    );
  }

  private async handleCheckFfmpegTools(): Promise<void> {
    if (this.els.checkFfmpegTools.disabled || this.isInstallingFfmpeg) return;
    const originalText = this.els.checkFfmpegTools.textContent || "检测工具";
    this.els.checkFfmpegTools.disabled = true;
    this.els.checkFfmpegTools.textContent = "检测中";
    this.setFfmpegToolStatus("checking", "正在检测 FFmpeg/FFprobe...");
    this.els.toolbarStatus.textContent = "正在检测 FFmpeg...";

    try {
      await this.saveCurrentConfig();
      const status = await checkFfmpegTools();
      this.setFfmpegToolStatus(status.available ? "ok" : "warning", status.message, status);
      this.els.toolbarStatus.textContent = status.available ? "FFmpeg 可用" : "FFmpeg 未配置";
    } catch (err) {
      this.setFfmpegToolStatus("error", `检测失败：${String(err)}`);
      this.els.toolbarStatus.textContent = "FFmpeg 检测失败";
      console.error("[generator] FFmpeg 检测失败:", err);
    } finally {
      this.els.checkFfmpegTools.textContent = originalText;
      this.syncWorkflowControls();
    }
  }

  private async handleDownloadFfmpegTools(): Promise<void> {
    if (this.els.downloadFfmpegTools.disabled || this.isInstallingFfmpeg) return;
    const ok = window.confirm("将下载 FFmpeg/FFprobe 到应用旁数据目录，并自动写入当前配置。继续？");
    if (!ok) return;

    const originalText = this.els.downloadFfmpegTools.textContent || "下载并配置";
    this.isInstallingFfmpeg = true;
    this.els.downloadFfmpegTools.textContent = "下载中";
    this.setFfmpegToolStatus("checking", "正在下载并安装 FFmpeg，这可能需要几分钟...");
    this.els.toolbarStatus.textContent = "正在下载 FFmpeg...";
    this.syncWorkflowControls();

    try {
      await this.saveCurrentConfig();
      const result = await downloadFfmpegTools();
      this.els.ffmpegPath.value = result.ffmpeg_path;
      this.els.ffprobePath.value = result.ffprobe_path;
      if (this.config) {
        this.config.ffmpeg_path = result.ffmpeg_path;
        this.config.ffprobe_path = result.ffprobe_path;
      }
      await this.saveCurrentConfig();
      this.setFfmpegToolStatus(
        "ok",
        `FFmpeg 已安装并配置。来源：${result.source}；目录：${result.install_dir}`,
        {
          available: true,
          download_supported: true,
          ffmpeg_path: result.ffmpeg_path,
          ffprobe_path: result.ffprobe_path,
          install_dir: result.install_dir,
          message: "FFmpeg 已安装并配置。",
          ffmpeg_version: result.ffmpeg_version,
          ffprobe_version: result.ffprobe_version,
        }
      );
      this.els.toolbarStatus.textContent = "FFmpeg 已安装";
    } catch (err) {
      this.setFfmpegToolStatus("error", `下载或安装失败：${String(err)}`);
      this.els.toolbarStatus.textContent = "FFmpeg 安装失败";
      console.error("[generator] FFmpeg 下载或安装失败:", err);
    } finally {
      this.isInstallingFfmpeg = false;
      this.els.downloadFfmpegTools.textContent = originalText;
      this.syncWorkflowControls();
    }
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

  private setFfmpegToolStatus(
    status: "ok" | "warning" | "error" | "checking",
    message: string,
    result?: FfmpegToolStatus
  ): void {
    this.els.ffmpegToolStatus.className = `config-check-status ${status}`;
    this.els.ffmpegToolStatus.textContent = message;
    this.els.ffmpegToolStatus.title = result
      ? [
          `FFmpeg: ${result.ffmpeg_path}`,
          `FFprobe: ${result.ffprobe_path}`,
          `Install dir: ${result.install_dir}`,
          result.ffmpeg_version ? `FFmpeg version: ${result.ffmpeg_version}` : "",
          result.ffprobe_version ? `FFprobe version: ${result.ffprobe_version}` : "",
        ]
          .filter(Boolean)
          .join("\n")
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
      const imported = await importImageToLibrary(file.file_path);
      const now = new Date();
      const record: GeneratedImageRecord = {
        id: `manual-${Date.now()}`,
        path: imported.file_path,
        label: stripFileExtension(imported.file_name || file.file_name) || "本地图片",
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
        parseClampedInt(this.els.mattingTolerance.value, 36, 1, 120),
        parseClampedInt(this.els.mattingFeather.value, 1, 0, 3),
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
      tolerance: parseClampedInt(this.els.mattingClickTolerance.value, 28, 1, 120),
      radius: parseClampedInt(this.els.mattingClickRadius.value, 1, 0, 8),
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

    const { apiKey, apiBase, model } = this.getPromptOptimizerSettings();
    if (!apiKey) {
      alert("请先在设置中填写提示词优化 API Key，或填写生图 API Key 以复用");
      return;
    }

    if (!model) {
      alert("请填写提示词优化模型");
      return;
    }

    try {
      await this.saveCurrentConfig();
    } catch (err) {
      console.error("[generator] 优化前保存配置失败:", err);
      alert(`保存当前 API 配置失败:\n${getErrorMessage(err)}`);
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

    const apiSettings = this.getActiveApiSettings();
    const apiKey = apiSettings.apiKey.trim();
    const prompt = this.els.prompt.value.trim();
    console.log("[generator] apiKey长度:", apiKey.length, "prompt长度:", prompt.length);

    if (!apiKey) {
      alert("请输入API Key");
      return;
    }
    if (!apiSettings.model.trim()) {
      alert("请输入生图模型名称");
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
    const apiBase = apiSettings.apiBase.trim();
    const negPrompt = this.els.negPrompt.value.trim();
    const model = apiSettings.model.trim();
    const apiMode = normalizeGenerationApiMode(apiSettings.apiMode);
    const style = this.els.style.value;
    const ratio = this.els.ratio.value;
    const resolution = this.els.resolution.value;
    const count = parseClampedInt(this.els.count.value, 1, 1, 4);
    this.expectedImageCount = count;

    try {
      await this.saveCurrentConfig();

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
        apiMode,
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
        apiMode,
        this.referenceImagePath
      );

      // 显示结果
      await this.addResultsToWorkbench(result, { prompt, model });

      this.els.toolbarStatus.textContent = this.els.toolbarStatus.textContent || "生成完成";
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
          this.els.toolbarStatus.textContent = "打开图片失败";
        });
      },
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
    updateGeneratedGallerySelection(this.els.resultGrid, this.selectedGeneratedPath);
  }

  private updateSelectedPreview(): void {
    const record = this.generatedRecords.find((item) => item.path === this.selectedGeneratedPath);
    setSelectedGeneratedPreview({
      image: this.els.selectedImage,
      meta: this.els.selectedMeta,
      record: record || null,
      formatTime,
      formatDuration,
    });
  }

  private async saveCurrentConfig(): Promise<void> {
    if (!this.config) return;
    console.log("[generator] 保存配置...");
    this.promptHistory = await getPromptHistory(100);
    this.historyIndex = this.promptHistory.length;
    const config = this.syncConfigFromForm();
    if (config) {
      await saveConfig(config);
    }
  }

  private getPromptOptimizerSettings(): PromptOptimizerSettings {
    return {
      apiKey: this.els.promptOptimizerApiKey.value.trim() || this.els.apiKey.value.trim(),
      apiBase: this.els.promptOptimizerApiBase.value.trim() || DEFAULT_PROMPT_OPTIMIZER_API_BASE,
      model: this.els.promptOptimizerModel.value.trim() || DEFAULT_PROMPT_OPTIMIZER_MODEL,
    };
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
      this.els.generationApiMode,
      this.els.activeApiProfile,
      this.els.profileName,
      this.els.addApiProfile,
      this.els.duplicateApiProfile,
      this.els.model,
      this.els.checkGenerationApi,
      this.els.videoApiKey,
      this.els.videoApiBase,
      this.els.videoProxyUrl,
      this.els.videoModel,
      this.els.videoApiMode,
      this.els.checkVideoApi,
      this.els.promptOptimizerApiKey,
      this.els.promptOptimizerApiBase,
      this.els.promptOptimizerModel,
      this.els.promptOptimizerVision,
      this.els.checkPromptOptimizerApi,
      this.els.saveDir,
      this.els.ffmpegPath,
      this.els.ffprobePath,
      this.els.checkFfmpegTools,
      this.els.downloadFfmpegTools,
      this.els.toggleKey,
      this.els.saveConfig,
      this.els.importConfig,
      this.els.exportConfig,
    ].forEach((control) => {
      control.disabled = !permissions.openSettings;
    });
    this.els.checkFfmpegTools.disabled = !permissions.openSettings || this.isInstallingFfmpeg;
    this.els.downloadFfmpegTools.disabled = !permissions.openSettings || this.isInstallingFfmpeg;
    this.els.deleteApiProfile.disabled =
      !permissions.openSettings || (this.config?.api_profiles.length || 0) <= 1;
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

function normalizeGenerationApiMode(value: string | undefined): string {
  const normalized = (value || "").trim().toLowerCase();
  if (
    normalized === "chat_completions" ||
    normalized === "chat-completions" ||
    normalized === "chat/completions"
  ) {
    return "chat_completions";
  }
  return DEFAULT_GENERATION_API_MODE;
}

function normalizeVideoApiMode(value: string | undefined): string {
  const normalized = (value || "").trim().toLowerCase();
  if (
    normalized === "videos" ||
    normalized === "video" ||
    normalized === "/videos" ||
    normalized === "v1/videos" ||
    normalized === "/v1/videos"
  ) {
    return "videos";
  }
  if (
    normalized === "chat_completions" ||
    normalized === "chat-completions" ||
    normalized === "chat/completions" ||
    normalized === "/chat/completions" ||
    normalized === "v1/chat/completions" ||
    normalized === "/v1/chat/completions"
  ) {
    return "chat_completions";
  }
  return DEFAULT_VIDEO_API_MODE;
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

function normalizeGridSize(value: number, fallback: number): number {
  if (!Number.isFinite(value)) {
    return fallback;
  }
  return Math.min(20, Math.max(1, Math.round(value)));
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
