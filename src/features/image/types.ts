export interface SpriteGridPreset {
  rows: number;
  cols: number;
}

export interface GeneratedImageRecord {
  id: string;
  path: string;
  label: string;
  prompt: string;
  model: string;
  durationSeconds?: number;
  createdAt: Date;
  updatedAt: Date;
}
