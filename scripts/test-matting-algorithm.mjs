import assert from "node:assert/strict";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import ts from "typescript";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outDir = path.join(tmpdir(), "spriteanime-matting-tests");
const sourcePath = path.join(root, "src", "pages", "generator-matting.ts");
const compiledPath = path.join(outDir, "generator-matting.js");

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
const transpiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: sourcePath,
});
writeFileSync(compiledPath, transpiled.outputText);
const matting = await import(pathToFileURL(compiledPath).href);

function makePixels(width, height, rgba = [0, 0, 0, 0]) {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let index = 0; index < width * height; index += 1) {
    data.set(rgba, index * 4);
  }
  return data;
}

function setPixel(data, width, x, y, rgba) {
  data.set(rgba, (y * width + x) * 4);
}

function alphaAt(data, width, x, y) {
  return data[(y * width + x) * 4 + 3];
}

function testContainMappingHorizontalLetterbox() {
  const bounds = { left: 0, top: 0, width: 300, height: 200 };
  const content = matting.getContainedCanvasContentRect(bounds, 100, 100);
  assert.deepEqual(content, { left: 50, top: 0, width: 200, height: 200 });
  assert.deepEqual(matting.mapClientPointToCanvasPixel(50, 0, bounds, 100, 100), {
    x: 0,
    y: 0,
  });
  assert.deepEqual(matting.mapClientPointToCanvasPixel(249.9, 199.9, bounds, 100, 100), {
    x: 99,
    y: 99,
  });
  assert.equal(matting.mapClientPointToCanvasPixel(25, 100, bounds, 100, 100), null);
}

function testContainMappingVerticalLetterbox() {
  const bounds = { left: 10, top: 20, width: 200, height: 300 };
  const content = matting.getContainedCanvasContentRect(bounds, 100, 50);
  assert.deepEqual(content, { left: 10, top: 120, width: 200, height: 100 });
  assert.deepEqual(matting.mapClientPointToCanvasPixel(110, 170, bounds, 100, 50), {
    x: 50,
    y: 25,
  });
  assert.equal(matting.mapClientPointToCanvasPixel(110, 80, bounds, 100, 50), null);
}

function testEraseConnectedWhiteResidue() {
  const width = 8;
  const height = 6;
  const data = makePixels(width, height);
  setPixel(data, width, 3, 2, [255, 255, 255, 255]);
  setPixel(data, width, 4, 2, [248, 248, 248, 255]);
  setPixel(data, width, 3, 3, [250, 250, 250, 255]);
  setPixel(data, width, 4, 3, [244, 244, 244, 255]);
  setPixel(data, width, 5, 2, [30, 70, 220, 255]);

  const result = matting.eraseConnectedRegion({
    data,
    width,
    height,
    startX: 3,
    startY: 2,
    tolerance: 28,
    radius: 0,
  });

  assert.equal(result.reason, "erased");
  assert.equal(result.erasedPixels, 4);
  assert.equal(alphaAt(data, width, 3, 2), 0);
  assert.equal(alphaAt(data, width, 4, 3), 0);
  assert.equal(alphaAt(data, width, 5, 2), 255);
}

function testTransparentClickFindsNearbyResidueSeed() {
  const width = 8;
  const height = 6;
  const data = makePixels(width, height);
  setPixel(data, width, 4, 2, [255, 255, 255, 255]);
  setPixel(data, width, 5, 2, [255, 255, 255, 255]);

  const result = matting.eraseConnectedRegion({
    data,
    width,
    height,
    startX: 3,
    startY: 2,
    tolerance: 20,
    radius: 1,
  });

  assert.equal(result.reason, "erased");
  assert.deepEqual(result.seed, { x: 4, y: 2 });
  assert.equal(result.erasedPixels, 2);
  assert.equal(alphaAt(data, width, 4, 2), 0);
  assert.equal(alphaAt(data, width, 5, 2), 0);
}

function testDiagonalResidueIsOneRegion() {
  const width = 5;
  const height = 5;
  const data = makePixels(width, height);
  setPixel(data, width, 1, 1, [255, 255, 255, 255]);
  setPixel(data, width, 2, 2, [252, 252, 252, 255]);
  setPixel(data, width, 3, 3, [249, 249, 249, 255]);

  const result = matting.eraseConnectedRegion({
    data,
    width,
    height,
    startX: 1,
    startY: 1,
    tolerance: 10,
    radius: 0,
  });

  assert.equal(result.erasedPixels, 3);
  assert.equal(alphaAt(data, width, 3, 3), 0);
}

function testNoSeedDoesNotMutateTransparentImage() {
  const width = 4;
  const height = 4;
  const data = makePixels(width, height);
  const before = Array.from(data);

  const result = matting.eraseConnectedRegion({
    data,
    width,
    height,
    startX: 1,
    startY: 1,
    tolerance: 28,
    radius: 1,
  });

  assert.equal(result.reason, "no_seed");
  assert.equal(result.erasedPixels, 0);
  assert.deepEqual(Array.from(data), before);
}

testContainMappingHorizontalLetterbox();
testContainMappingVerticalLetterbox();
testEraseConnectedWhiteResidue();
testTransparentClickFindsNearbyResidueSeed();
testDiagonalResidueIsOneRegion();
testNoSeedDoesNotMutateTransparentImage();

rmSync(outDir, { recursive: true, force: true });
console.log("Matting algorithm tests passed.");
