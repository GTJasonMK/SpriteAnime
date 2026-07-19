export interface PreviewCanvasTarget {
  canvas: HTMLCanvasElement;
  placeholder: HTMLElement;
  sizeLabel: HTMLElement;
}

export function fitPreviewCanvasSize(
  width: number,
  height: number,
  maxEdge: number
): { width: number; height: number } {
  const edge = Math.max(width, height);
  if (edge <= maxEdge) {
    return {
      width: Math.max(1, Math.round(width)),
      height: Math.max(1, Math.round(height)),
    };
  }
  const scale = maxEdge / edge;
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
}

export function preparePreviewCanvas(options: {
  target: PreviewCanvasTarget;
  canvasWidth: number;
  canvasHeight: number;
  aspectWidth?: number;
  aspectHeight?: number;
  sizeText: string;
}): CanvasRenderingContext2D | null {
  const { canvas, placeholder, sizeLabel } = options.target;
  canvas.width = Math.max(1, Math.round(options.canvasWidth));
  canvas.height = Math.max(1, Math.round(options.canvasHeight));
  canvas.style.aspectRatio =
    `${Math.max(1, options.aspectWidth ?? canvas.width)} / ${Math.max(1, options.aspectHeight ?? canvas.height)}`;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  placeholder.style.display = "none";
  sizeLabel.textContent = options.sizeText;
  return ctx;
}

export function clearPreviewCanvas(target: PreviewCanvasTarget, message: string): void {
  target.canvas.width = 1;
  target.canvas.height = 1;
  target.canvas.style.aspectRatio = "";
  target.canvas.closest(".preview-area")?.classList.remove("has-image");
  target.placeholder.textContent = message;
  target.placeholder.style.display = "";
  target.sizeLabel.textContent = "尺寸: -";
}
