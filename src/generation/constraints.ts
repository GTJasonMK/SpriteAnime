export type GenerationBackgroundMode = "preserve" | "solid" | "transparent" | "custom";
export type GenerationFraming = "preserve" | "full_body" | "upper_body";

export interface ImageGenerationConstraints {
  enabled: boolean;
  backgroundMode: GenerationBackgroundMode;
  backgroundDescription: string;
  framing: GenerationFraming;
}

export interface VideoGenerationConstraints {
  enabled: boolean;
  backgroundMode: Exclude<GenerationBackgroundMode, "transparent">;
  backgroundDescription: string;
  framing: GenerationFraming;
  fixedCamera: boolean;
  loopAction: boolean;
}

export function usesBackgroundDescription(mode: GenerationBackgroundMode): boolean {
  return mode === "solid" || mode === "custom";
}
