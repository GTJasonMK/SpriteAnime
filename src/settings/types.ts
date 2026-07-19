import type { ActiveApiSettings } from "../api/commands";

export interface ApiSettingsProvider {
  getActiveApiSettings(): ActiveApiSettings;
  getActiveProfileName(): string;
}

export interface PromptOptimizerSettings {
  apiKey: string;
  apiBase: string;
  apiMode: string;
  proxyUrl: string;
  model: string;
  vision: boolean;
}

export interface GenerationPreferences {
  style: string;
  ratio: string;
  resolution: string;
  count: number;
  promptHistory: string[];
}

export interface ApiProfileFormValues {
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
  promptOptimizerApiMode: string;
  promptOptimizerModel: string;
  promptOptimizerVision: boolean;
}
