export const MAX_REDRAW_GROUP_AXIS = 4;
export const MAX_REDRAW_GROUP_FRAMES = 9;
export const REDRAW_GROUP_WARNING_FRAMES = 6;

export interface RedrawBatchPlanItem {
  index: number;
  globalStart: number;
  validCount: number;
  paddingCount: number;
  sourceIndices: number[];
}

export interface RedrawPlan {
  totalFrames: number;
  finalCols: number;
  finalRows: number;
  groupRows: number;
  groupCols: number;
  batches: RedrawBatchPlanItem[];
}

export interface RedrawFrameSource {
  blob: Blob;
  width: number;
  height: number;
}

export interface ProjectedRedrawOutput {
  width: number;
  height: number;
  pixels: number;
}

const MAX_FINAL_DIMENSION = 16_384;
const MAX_FINAL_PIXELS = 64 * 1024 * 1024;

export function buildRedrawPlan(
  totalFrames: number,
  finalCols: number,
  groupRows: number,
  groupCols: number
): RedrawPlan {
  requireIntegerInRange(totalFrames, "总帧数", 2, 64);
  requireIntegerInRange(finalCols, "最终列数", 1, 20);
  requireIntegerInRange(groupRows, "分组行数", 1, MAX_REDRAW_GROUP_AXIS);
  requireIntegerInRange(groupCols, "分组列数", 1, MAX_REDRAW_GROUP_AXIS);

  const groupCapacity = groupRows * groupCols;
  if (groupCapacity > MAX_REDRAW_GROUP_FRAMES) {
    throw new Error(`每组最多 ${MAX_REDRAW_GROUP_FRAMES} 帧，当前为 ${groupCapacity} 帧`);
  }
  if (groupCapacity > totalFrames) {
    throw new Error(`每组容量 ${groupCapacity} 不能大于总帧数 ${totalFrames}`);
  }

  const finalRows = Math.ceil(totalFrames / finalCols);
  const batchCount = Math.ceil(totalFrames / groupCapacity);
  const batches = Array.from({ length: batchCount }, (_, index) => {
    const globalStart = index * groupCapacity;
    const validCount = Math.min(groupCapacity, totalFrames - globalStart);
    const lastValidIndex = globalStart + validCount - 1;
    const sourceIndices = Array.from({ length: groupCapacity }, (__, localIndex) =>
      Math.min(globalStart + localIndex, lastValidIndex)
    );
    return {
      index,
      globalStart,
      validCount,
      paddingCount: groupCapacity - validCount,
      sourceIndices,
    };
  });

  return {
    totalFrames,
    finalCols,
    finalRows,
    groupRows,
    groupCols,
    batches,
  };
}

export function getProjectedRedrawOutput(
  plan: RedrawPlan,
  resolution: string
): ProjectedRedrawOutput {
  const base = resolution === "2K" ? 2048 : 1024;
  const generatedWidth = plan.groupCols >= plan.groupRows
    ? base
    : Math.max(1, Math.round(base * plan.groupCols / plan.groupRows));
  const generatedHeight = plan.groupCols >= plan.groupRows
    ? Math.max(1, Math.round(base * plan.groupRows / plan.groupCols))
    : base;
  const cellWidth = Math.max(1, Math.floor(generatedWidth / plan.groupCols));
  const cellHeight = Math.max(1, Math.floor(generatedHeight / plan.groupRows));
  const width = plan.finalCols * cellWidth;
  const height = plan.finalRows * cellHeight;
  return {
    width,
    height,
    pixels: width * height,
  };
}

export function validateProjectedRedrawOutput(
  plan: RedrawPlan,
  resolution: string
): ProjectedRedrawOutput {
  const output = getProjectedRedrawOutput(plan, resolution);
  if (output.width > MAX_FINAL_DIMENSION || output.height > MAX_FINAL_DIMENSION) {
    throw new Error(
      `预计最终尺寸 ${output.width}×${output.height} 超过最长边 ${MAX_FINAL_DIMENSION}，请调整最终列数或降低分辨率`
    );
  }
  if (output.pixels > MAX_FINAL_PIXELS) {
    throw new Error(
      `预计最终像素数 ${output.pixels} 超过上限 ${MAX_FINAL_PIXELS}，请调整最终列数或降低分辨率`
    );
  }
  return output;
}

export async function composeRedrawBatchInput(
  frames: RedrawFrameSource[],
  batch: RedrawBatchPlanItem,
  groupRows: number,
  groupCols: number,
  transparent: boolean
): Promise<Blob> {
  if (frames.length === 0) {
    throw new Error("没有可用于分组重绘的序列帧");
  }
  const cellWidth = Math.max(...frames.map((frame) => frame.width), 1);
  const cellHeight = Math.max(...frames.map((frame) => frame.height), 1);
  const canvas = new OffscreenCanvas(groupCols * cellWidth, groupRows * cellHeight);
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("无法创建分组参考图画布");
  }
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (!transparent) {
    ctx.fillStyle = "#f4efe8";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
  }

  for (let localIndex = 0; localIndex < batch.sourceIndices.length; localIndex += 1) {
    const sourceIndex = batch.sourceIndices[localIndex];
    const frame = frames[sourceIndex];
    if (!frame) {
      throw new Error(`分组引用的第 ${sourceIndex + 1} 帧不存在`);
    }
    const bitmap = await createImageBitmap(frame.blob);
    try {
      const col = localIndex % groupCols;
      const row = Math.floor(localIndex / groupCols);
      const x = col * cellWidth + Math.round((cellWidth - frame.width) / 2);
      const y = row * cellHeight + (cellHeight - frame.height);
      ctx.drawImage(bitmap, x, y, frame.width, frame.height);
    } finally {
      bitmap.close();
    }
  }

  return canvas.convertToBlob({ type: "image/png" });
}

function requireIntegerInRange(value: number, label: string, min: number, max: number): void {
  if (!Number.isInteger(value) || value < min || value > max) {
    throw new Error(`${label}必须是 ${min} 到 ${max} 之间的整数`);
  }
}
