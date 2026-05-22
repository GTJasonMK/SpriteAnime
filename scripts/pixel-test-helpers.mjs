export function makePixels(width, height, rgba = [0, 0, 0, 0]) {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let index = 0; index < width * height; index += 1) {
    data.set(rgba, index * 4);
  }
  return data;
}

export function makeImage(width, height, rgba = [255, 255, 255, 255]) {
  return makePixels(width, height, rgba);
}

export function setPixel(data, width, x, y, rgba) {
  data.set(rgba, (y * width + x) * 4);
}

export function fillRect(data, imageWidth, x, y, width, height, rgba = [0, 0, 0, 255]) {
  for (let py = y; py < y + height; py += 1) {
    for (let px = x; px < x + width; px += 1) {
      setPixel(data, imageWidth, px, py, rgba);
    }
  }
}

export function alphaAt(data, width, x, y) {
  return data[(y * width + x) * 4 + 3];
}
