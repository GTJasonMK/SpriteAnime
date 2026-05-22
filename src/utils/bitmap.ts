export async function loadBitmapFromBase64(base64: string): Promise<ImageBitmap> {
  const response = await fetch(`data:image/png;base64,${base64}`);
  const blob = await response.blob();
  return createImageBitmap(blob);
}
