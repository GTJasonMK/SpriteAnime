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

export interface EraseConnectedRegionResult {
  erasedPixels: number;
  seed: CanvasPixelPoint | null;
  targetColor: { r: number; g: number; b: number; a: number } | null;
  reason: "erased" | "outside" | "no_seed" | "no_match";
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

export function eraseConnectedRegion(options: {
  data: Uint8ClampedArray;
  width: number;
  height: number;
  startX: number;
  startY: number;
  tolerance: number;
  radius: number;
}): EraseConnectedRegionResult {
  const { data, width, height, startX, startY } = options;
  const tolerance = Math.max(0, options.tolerance);
  const radius = Math.max(0, Math.round(options.radius));

  if (
    width <= 0 ||
    height <= 0 ||
    startX < 0 ||
    startY < 0 ||
    startX >= width ||
    startY >= height
  ) {
    return emptyEraseResult("outside", null, null);
  }

  const seedSearchRadius = Math.max(2, radius + 1);
  const seed = findEraseSeed(data, width, height, startX, startY, seedSearchRadius);
  if (!seed) {
    return emptyEraseResult("no_seed", null, null);
  }

  const startIndex = (seed.y * width + seed.x) * 4;
  const targetColor = {
    r: data[startIndex],
    g: data[startIndex + 1],
    b: data[startIndex + 2],
    a: data[startIndex + 3],
  };
  if (targetColor.a === 0) {
    return emptyEraseResult("no_seed", seed, targetColor);
  }

  const visited = new Uint8Array(width * height);
  const queue = new Int32Array(width * height);
  const pixels: number[] = [];
  let head = 0;
  let tail = 0;
  const seedIndex = seed.y * width + seed.x;
  queue[tail] = seedIndex;
  tail += 1;
  visited[seedIndex] = 1;

  while (head < tail) {
    const pixelIndex = queue[head];
    head += 1;
    const x = pixelIndex % width;
    const y = Math.floor(pixelIndex / width);
    const dataIndex = pixelIndex * 4;
    if (!pixelMatchesTarget(data, dataIndex, targetColor, tolerance)) {
      continue;
    }

    pixels.push(pixelIndex);
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        if (dx === 0 && dy === 0) continue;
        const nx = x + dx;
        const ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= width || ny >= height) continue;
        const nextIndex = ny * width + nx;
        if (visited[nextIndex]) continue;
        visited[nextIndex] = 1;
        queue[tail] = nextIndex;
        tail += 1;
      }
    }
  }

  if (pixels.length === 0) {
    return emptyEraseResult("no_match", seed, targetColor);
  }

  let erasedPixels = 0;
  pixels.forEach((pixelIndex) => {
    if (eraseAlphaAtIndex(data, pixelIndex)) {
      erasedPixels += 1;
    }
  });
  if (radius > 0) {
    erasedPixels += eraseBrushDisk(data, width, height, seed.x, seed.y, radius);
  }

  return {
    erasedPixels,
    seed,
    targetColor,
    reason: erasedPixels > 0 ? "erased" : "no_match",
  };
}

function emptyEraseResult(
  reason: EraseConnectedRegionResult["reason"],
  seed: CanvasPixelPoint | null,
  targetColor: EraseConnectedRegionResult["targetColor"]
): EraseConnectedRegionResult {
  return {
    erasedPixels: 0,
    seed,
    targetColor,
    reason,
  };
}

function findEraseSeed(
  data: Uint8ClampedArray,
  width: number,
  height: number,
  startX: number,
  startY: number,
  radius: number
): CanvasPixelPoint | null {
  const startAlpha = data[(startY * width + startX) * 4 + 3];
  if (startAlpha > 0) {
    return { x: startX, y: startY };
  }

  const safeRadius = Math.max(1, Math.round(radius));
  let best: { x: number; y: number; distance: number } | null = null;
  for (let dy = -safeRadius; dy <= safeRadius; dy++) {
    for (let dx = -safeRadius; dx <= safeRadius; dx++) {
      if (dx * dx + dy * dy > safeRadius * safeRadius) continue;
      const x = startX + dx;
      const y = startY + dy;
      if (x < 0 || y < 0 || x >= width || y >= height) continue;
      const alpha = data[(y * width + x) * 4 + 3];
      if (alpha === 0) continue;
      const distance = dx * dx + dy * dy;
      if (!best || distance < best.distance) {
        best = { x, y, distance };
      }
    }
  }

  return best ? { x: best.x, y: best.y } : null;
}

function pixelMatchesTarget(
  data: Uint8ClampedArray,
  index: number,
  target: { r: number; g: number; b: number; a: number },
  tolerance: number
): boolean {
  if (data[index + 3] === 0) {
    return false;
  }
  const dr = data[index] - target.r;
  const dg = data[index + 1] - target.g;
  const db = data[index + 2] - target.b;
  return dr * dr + dg * dg + db * db <= tolerance * tolerance * 3;
}

function eraseBrushDisk(
  data: Uint8ClampedArray,
  width: number,
  height: number,
  x: number,
  y: number,
  radius: number
): number {
  const safeRadius = Math.max(0, Math.round(radius));
  let erasedCount = 0;
  for (let dy = -safeRadius; dy <= safeRadius; dy++) {
    for (let dx = -safeRadius; dx <= safeRadius; dx++) {
      if (dx * dx + dy * dy > safeRadius * safeRadius) continue;
      const nx = x + dx;
      const ny = y + dy;
      if (nx < 0 || ny < 0 || nx >= width || ny >= height) continue;
      if (eraseAlphaAtIndex(data, ny * width + nx)) {
        erasedCount += 1;
      }
    }
  }
  return erasedCount;
}

function eraseAlphaAtIndex(data: Uint8ClampedArray, pixelIndex: number): boolean {
  const alphaIndex = pixelIndex * 4 + 3;
  if (data[alphaIndex] === 0) {
    return false;
  }
  data[alphaIndex] = 0;
  return true;
}

function clampInt(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
