import type { BackgroundMode, PixelBounds } from "./types";

export function parseBackgroundMode(value: string): BackgroundMode {
  if (value === "firstFrame" || value === "none") {
    return value;
  }
  return "edge";
}

export function clampPixelBounds(region: PixelBounds, width: number, height: number): PixelBounds {
  const imageWidth = Math.max(1, Math.round(width));
  const imageHeight = Math.max(1, Math.round(height));
  const regionWidth = Math.max(1, Math.min(imageWidth, Math.round(region.width)));
  const regionHeight = Math.max(1, Math.min(imageHeight, Math.round(region.height)));
  return {
    x: Math.max(0, Math.min(imageWidth - regionWidth, Math.round(region.x))),
    y: Math.max(0, Math.min(imageHeight - regionHeight, Math.round(region.y))),
    width: regionWidth,
    height: regionHeight,
  };
}

export function expandPixelBounds(
  bounds: PixelBounds,
  padding: number,
  imageWidth: number,
  imageHeight: number
): PixelBounds {
  const x = Math.max(0, bounds.x - padding);
  const y = Math.max(0, bounds.y - padding);
  const right = Math.min(imageWidth, bounds.x + bounds.width + padding);
  const bottom = Math.min(imageHeight, bounds.y + bounds.height + padding);
  return {
    x,
    y,
    width: Math.max(1, right - x),
    height: Math.max(1, bottom - y),
  };
}

export function formatPixelBounds(region: PixelBounds): string {
  return `${Math.round(region.x)},${Math.round(region.y)},${Math.round(region.width)}x${Math.round(region.height)}`;
}
