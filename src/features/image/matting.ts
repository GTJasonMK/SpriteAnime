export interface RectLike {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface CanvasPixelPoint {
  x: number;
  y: number;
}

export function cloneImageData(imageData: ImageData): ImageData {
  return new ImageData(
    new Uint8ClampedArray(imageData.data),
    imageData.width,
    imageData.height
  );
}

export function getCanvasPixelPoint(
  event: MouseEvent,
  canvas: HTMLCanvasElement
): CanvasPixelPoint | null {
  const rect = canvas.getBoundingClientRect();
  return mapClientPointToCanvasPixel(
    event.clientX,
    event.clientY,
    rect,
    canvas.width,
    canvas.height
  );
}

export function mapClientPointToCanvasPixel(
  clientX: number,
  clientY: number,
  bounds: RectLike,
  bitmapWidth: number,
  bitmapHeight: number
): CanvasPixelPoint | null {
  const contentRect = getContainedCanvasContentRect(bounds, bitmapWidth, bitmapHeight);
  if (!contentRect) return null;

  const right = contentRect.left + contentRect.width;
  const bottom = contentRect.top + contentRect.height;
  if (
    clientX < contentRect.left ||
    clientY < contentRect.top ||
    clientX > right ||
    clientY > bottom
  ) {
    return null;
  }

  const ratioX = (clientX - contentRect.left) / contentRect.width;
  const ratioY = (clientY - contentRect.top) / contentRect.height;
  return {
    x: clampInt(Math.floor(ratioX * bitmapWidth), 0, bitmapWidth - 1),
    y: clampInt(Math.floor(ratioY * bitmapHeight), 0, bitmapHeight - 1),
  };
}

export function getContainedCanvasContentRect(
  bounds: RectLike,
  bitmapWidth: number,
  bitmapHeight: number
): RectLike | null {
  if (
    bounds.width <= 0 ||
    bounds.height <= 0 ||
    bitmapWidth <= 0 ||
    bitmapHeight <= 0
  ) {
    return null;
  }

  const bitmapAspect = bitmapWidth / bitmapHeight;
  const boundsAspect = bounds.width / bounds.height;
  if (!Number.isFinite(bitmapAspect) || !Number.isFinite(boundsAspect)) {
    return null;
  }

  let width = bounds.width;
  let height = bounds.height;
  let left = bounds.left;
  let top = bounds.top;

  if (boundsAspect > bitmapAspect) {
    width = bounds.height * bitmapAspect;
    left += (bounds.width - width) / 2;
  } else if (boundsAspect < bitmapAspect) {
    height = bounds.width / bitmapAspect;
    top += (bounds.height - height) / 2;
  }

  return { left, top, width, height };
}

function clampInt(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
