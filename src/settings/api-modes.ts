export const DEFAULT_GENERATION_MODEL = "gpt-5.3-codex";
export const DEFAULT_GENERATION_API_MODE = "responses";
export const DEFAULT_VIDEO_MODEL = "sora-2";
export const DEFAULT_VIDEO_API_MODE = "chat_completions";
export const DEFAULT_PROMPT_OPTIMIZER_MODEL = "deepseek-v4-flash";
export const DEFAULT_PROMPT_OPTIMIZER_API_MODE = "chat_completions";

export function requiredGenerationApiMode(value: string, context: string): string {
  const mode = value.trim();
  if (["responses", "chat_completions", "images_generations", "images_edits_json", "images_edits_multipart"].includes(mode)) {
    return mode;
  }
  if (!mode) throw new Error(`${context}为空。请在设置 > 图片生成中选择调用方式后重试。`);
  throw new Error(`${context}无效：${mode}。generation_api_mode 必须使用设置界面列出的值。`);
}

export function requiredVideoApiMode(value: string, context: string): string {
  const mode = value.trim();
  if (["chat_completions", "videos", "videos_generations", "videos_edits", "videos_extensions"].includes(mode)) {
    return mode;
  }
  if (!mode) throw new Error(`${context}为空。请在设置 > 视频生成中选择调用方式后重试。`);
  throw new Error(`${context}无效：${mode}。video_api_mode 必须使用设置界面列出的值。`);
}

export function requiredPromptOptimizerApiMode(value: string, context: string): string {
  const mode = value.trim();
  if (mode === "responses" || mode === "chat_completions") return mode;
  if (!mode) throw new Error(`${context}为空。请在设置 > 提示词优化中选择调用方式后重试。`);
  throw new Error(`${context}无效：${mode}。prompt_optimizer_api_mode 只能是 responses 或 chat_completions。`);
}

export function setRequiredSelectValue(select: HTMLSelectElement, value: string, context: string): void {
  const option = Array.from(select.options).find((item) => item.value === value);
  if (!option) throw new Error(`${context}选项不存在：${value || "(空)"}`);
  select.value = value;
}
