import type { ApiProfile } from "../api/commands";
import type { SettingsController } from "./controller";
import {
  DEFAULT_GENERATION_API_MODE,
  DEFAULT_GENERATION_MODEL,
  DEFAULT_PROMPT_OPTIMIZER_API_MODE,
  DEFAULT_PROMPT_OPTIMIZER_MODEL,
  DEFAULT_VIDEO_API_MODE,
  DEFAULT_VIDEO_MODEL,
  requiredGenerationApiMode,
  requiredPromptOptimizerApiMode,
  requiredVideoApiMode,
  setRequiredSelectValue,
} from "./api-modes";
import type { ApiProfileFormValues } from "./types";

export const settingsProfileMethods = {
  populateModelList(): void {
    this.els.modelList.innerHTML = "";
    this.presets.models.forEach((model) => {
      const option = document.createElement("option");
      option.value = model;
      this.els.modelList.appendChild(option);
    });
  },

  applyConfig(): void {
    this.renderApiProfiles();
    this.applyProfileToForm(this.getActiveApiProfile());
    this.els.ffmpegPath.value = this.config.ffmpeg_path;
    this.els.ffprobePath.value = this.config.ffprobe_path;
  },

  getActiveApiProfile(): ApiProfile {
    const profile = this.config.api_profiles.find((item) => item.id === this.config.active_api_profile_id);
    if (!profile) throw new Error(`活动 API 配置不存在：${this.config.active_api_profile_id}`);
    return profile;
  },

  getCurrentProfileFormValues(): ApiProfileFormValues {
    return {
      apiKey: this.els.apiKey.value.trim(),
      apiBase: this.els.apiBase.value.trim(),
      proxyUrl: this.els.proxyUrl.value.trim(),
      apiMode: requiredGenerationApiMode(this.els.generationApiMode.value, "图片生成调用方式"),
      model: this.els.model.value.trim(),
      videoApiKey: this.els.videoApiKey.value.trim(),
      videoApiBase: this.els.videoApiBase.value.trim(),
      videoProxyUrl: this.els.videoProxyUrl.value.trim(),
      videoModel: this.els.videoModel.value.trim(),
      videoApiMode: requiredVideoApiMode(this.els.videoApiMode.value, "视频生成调用方式"),
      promptOptimizerApiKey: this.els.promptOptimizerApiKey.value.trim(),
      promptOptimizerApiBase: this.els.promptOptimizerApiBase.value.trim(),
      promptOptimizerApiMode: requiredPromptOptimizerApiMode(this.els.promptOptimizerApiMode.value, "提示词优化调用方式"),
      promptOptimizerModel: this.els.promptOptimizerModel.value.trim(),
      promptOptimizerVision: this.els.promptOptimizerVision.checked,
    };
  },

  applyProfileToForm(profile: ApiProfile): void {
    this.els.apiKey.value = profile.api_key;
    this.els.apiBase.value = profile.api_base;
    this.els.proxyUrl.value = profile.proxy_url;
    setRequiredSelectValue(this.els.generationApiMode, requiredGenerationApiMode(profile.generation_api_mode, `API 配置「${profile.name}」图片生成调用方式`), "图片生成调用方式");
    this.els.model.value = profile.last_model;
    this.els.videoApiKey.value = profile.video_api_key;
    this.els.videoApiBase.value = profile.video_api_base;
    this.els.videoProxyUrl.value = profile.video_proxy_url;
    this.els.videoModel.value = profile.video_model;
    setRequiredSelectValue(this.els.videoApiMode, requiredVideoApiMode(profile.video_api_mode, `API 配置「${profile.name}」视频生成调用方式`), "视频生成调用方式");
    this.els.promptOptimizerApiKey.value = profile.prompt_optimizer_api_key;
    this.els.promptOptimizerApiBase.value = profile.prompt_optimizer_api_base;
    setRequiredSelectValue(this.els.promptOptimizerApiMode, requiredPromptOptimizerApiMode(profile.prompt_optimizer_api_mode, `API 配置「${profile.name}」提示词优化调用方式`), "提示词优化调用方式");
    this.els.promptOptimizerModel.value = profile.prompt_optimizer_model;
    this.els.promptOptimizerVision.checked = profile.prompt_optimizer_vision;
    this.els.profileName.value = profile.name;
    this.hideApiKeys();
    this.clearApiCheckStatuses();
  },

  writeFormToActiveProfile(): void {
    const profile = this.getActiveApiProfile();
    const values = this.getCurrentProfileFormValues();
    const name = this.els.profileName.value.trim();
    if (!name) throw new Error("API 配置名称为空");
    Object.assign(profile, {
      name,
      api_key: values.apiKey,
      api_base: values.apiBase,
      proxy_url: values.proxyUrl,
      generation_api_mode: values.apiMode,
      last_model: values.model,
      video_api_key: values.videoApiKey,
      video_api_base: values.videoApiBase,
      video_proxy_url: values.videoProxyUrl,
      video_model: values.videoModel,
      video_api_mode: values.videoApiMode,
      prompt_optimizer_api_key: values.promptOptimizerApiKey,
      prompt_optimizer_api_base: values.promptOptimizerApiBase,
      prompt_optimizer_api_mode: values.promptOptimizerApiMode,
      prompt_optimizer_model: values.promptOptimizerModel,
      prompt_optimizer_vision: values.promptOptimizerVision,
    });
  },

  renderApiProfiles(): void {
    const activeId = this.config.active_api_profile_id;
    this.els.activeApiProfile.innerHTML = "";
    this.els.profileList.innerHTML = "";
    this.config.api_profiles.forEach((profile) => {
      const option = document.createElement("option");
      option.value = profile.id;
      option.textContent = profile.name;
      option.selected = profile.id === activeId;
      this.els.activeApiProfile.appendChild(option);

      const button = document.createElement("button");
      button.type = "button";
      button.className = "api-profile-item";
      button.classList.toggle("active", profile.id === activeId);
      button.dataset.profileId = profile.id;
      button.innerHTML = `<span class="api-profile-name"></span><span class="api-profile-meta"></span>`;
      button.querySelector<HTMLElement>(".api-profile-name")!.textContent = profile.name;
      button.querySelector<HTMLElement>(".api-profile-meta")!.textContent = `图: ${profile.last_model || "未填写"} · 视频: ${profile.video_model || "未填写"}`;
      button.addEventListener("click", () => this.switchApiProfile(profile.id));
      this.els.profileList.appendChild(button);
    });
    this.els.profileName.value = this.getActiveApiProfile().name;
    this.els.deleteApiProfile.disabled = this.config.api_profiles.length <= 1;
  },

  createApiProfile(name: string): ApiProfile {
    return {
      id: `api-${crypto.randomUUID()}`,
      name,
      api_key: "",
      api_base: "",
      proxy_url: "",
      generation_api_mode: DEFAULT_GENERATION_API_MODE,
      last_model: DEFAULT_GENERATION_MODEL,
      video_api_key: "",
      video_api_base: "",
      video_proxy_url: "",
      video_model: DEFAULT_VIDEO_MODEL,
      video_api_mode: DEFAULT_VIDEO_API_MODE,
      prompt_optimizer_api_key: "",
      prompt_optimizer_api_base: "",
      prompt_optimizer_api_mode: DEFAULT_PROMPT_OPTIMIZER_API_MODE,
      prompt_optimizer_model: DEFAULT_PROMPT_OPTIMIZER_MODEL,
      prompt_optimizer_vision: false,
    };
  },
} satisfies ThisType<SettingsController>;

export type SettingsProfileMethods = typeof settingsProfileMethods;
