import {
  getPresets,
  loadConfig,
  saveConfig,
  type ActiveApiSettings,
  type PresetsPayload,
  type UserConfig,
} from "../api/commands";
import { getById, queryAll } from "../utils/dom";
import {
  requiredGenerationApiMode,
  requiredPromptOptimizerApiMode,
  requiredVideoApiMode,
} from "./api-modes";
import { settingsActionMethods, type SettingsActionMethods } from "./settings-actions";
import { settingsProfileMethods, type SettingsProfileMethods } from "./settings-profiles";
import type {
  GenerationPreferences,
  PromptOptimizerSettings,
} from "./types";

export class SettingsController {
  presets!: PresetsPayload;
  config!: UserConfig;
  isInstallingFfmpeg = false;

  readonly els = {
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
    toggleVideoKey: getById<HTMLButtonElement>("btn-toggle-video-key"),
    togglePromptOptimizerKey: getById<HTMLButtonElement>("btn-toggle-prompt-optimizer-key"),
    copyImageApiToVideo: getById<HTMLButtonElement>("btn-copy-image-api-to-video"),
    copyImageApiToOptimizer: getById<HTMLButtonElement>("btn-copy-image-api-to-optimizer"),
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
    promptOptimizerApiKey: getById<HTMLInputElement>("prompt-optimizer-api-key"),
    promptOptimizerApiBase: getById<HTMLInputElement>("prompt-optimizer-api-base"),
    promptOptimizerApiMode: getById<HTMLSelectElement>("prompt-optimizer-api-mode"),
    promptOptimizerModel: getById<HTMLInputElement>("prompt-optimizer-model-input"),
    promptOptimizerVision: getById<HTMLInputElement>("prompt-optimizer-vision"),
    checkPromptOptimizerApi: getById<HTMLButtonElement>("btn-check-prompt-optimizer-api"),
    promptOptimizerApiCheckStatus: getById("prompt-optimizer-api-check-status"),
    ffmpegPath: getById<HTMLInputElement>("ffmpeg-path"),
    ffprobePath: getById<HTMLInputElement>("ffprobe-path"),
    checkFfmpegTools: getById<HTMLButtonElement>("btn-check-ffmpeg-tools"),
    downloadFfmpegTools: getById<HTMLButtonElement>("btn-download-ffmpeg-tools"),
    ffmpegToolStatus: getById("ffmpeg-tool-status"),
    save: getById<HTMLButtonElement>("btn-save-config"),
    open: getById<HTMLButtonElement>("btn-settings"),
    modal: getById("settings-modal"),
    close: getById<HTMLButtonElement>("btn-close-modal"),
    tabs: queryAll<HTMLElement>(".settings-tab"),
    panels: queryAll<HTMLElement>(".settings-panel"),
    status: getById("settings-status"),
  };

  async init(): Promise<void> {
    this.presets = await getPresets();
    this.config = await loadConfig();
    this.populateModelList();
    this.applyConfig();
    this.bindEvents();
  }

  getActiveApiSettings(): ActiveApiSettings {
    this.writeFormToActiveProfile();
    const profile = this.getActiveApiProfile();
    return {
      profileId: profile.id,
      apiKey: profile.api_key,
      apiBase: profile.api_base,
      proxyUrl: profile.proxy_url,
      apiMode: requiredGenerationApiMode(profile.generation_api_mode, `API 配置「${profile.name}」图片生成调用方式`),
      model: profile.last_model,
      videoApiKey: profile.video_api_key,
      videoApiBase: profile.video_api_base,
      videoProxyUrl: profile.video_proxy_url,
      videoModel: profile.video_model,
      videoApiMode: requiredVideoApiMode(profile.video_api_mode, `API 配置「${profile.name}」视频生成调用方式`),
      profileName: profile.name,
    };
  }

  getActiveProfileName(): string {
    return this.getActiveApiProfile().name;
  }

  getActiveVideoApiMode(): string {
    const profile = this.getActiveApiProfile();
    return requiredVideoApiMode(
      profile.video_api_mode,
      `API 配置「${profile.name}」视频生成调用方式`
    );
  }

  getPromptOptimizerSettings(): PromptOptimizerSettings {
    return {
      apiKey: this.els.promptOptimizerApiKey.value.trim(),
      apiBase: this.els.promptOptimizerApiBase.value.trim(),
      apiMode: requiredPromptOptimizerApiMode(this.els.promptOptimizerApiMode.value, "提示词优化调用方式"),
      proxyUrl: this.els.proxyUrl.value.trim(),
      model: this.els.promptOptimizerModel.value.trim(),
      vision: this.els.promptOptimizerVision.checked,
    };
  }

  getGenerationPreferences(): GenerationPreferences {
    return {
      style: this.config.last_style,
      ratio: this.config.last_ratio,
      resolution: this.config.last_resolution,
      count: this.config.last_count,
      promptHistory: [...this.config.prompt_history],
    };
  }

  updateGenerationPreferences(preferences: GenerationPreferences): void {
    this.config.last_style = preferences.style;
    this.config.last_ratio = preferences.ratio;
    this.config.last_resolution = preferences.resolution;
    this.config.last_count = preferences.count;
    this.config.prompt_history = [...preferences.promptHistory];
  }

  replacePromptHistory(history: string[]): void {
    this.config.prompt_history = [...history];
  }

  async saveCurrentConfig(): Promise<void> {
    this.writeFormToActiveProfile();
    this.config.ffmpeg_path = this.els.ffmpegPath.value.trim();
    this.config.ffprobe_path = this.els.ffprobePath.value.trim();
    await saveConfig(this.config);
  }

  notifyChanged(): void {
    document.dispatchEvent(new CustomEvent("spriteanime:settings-change"));
  }
}

export interface SettingsController extends SettingsProfileMethods, SettingsActionMethods {}

Object.assign(SettingsController.prototype, settingsProfileMethods, settingsActionMethods);
