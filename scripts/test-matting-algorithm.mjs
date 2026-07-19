import assert from "node:assert/strict";
import {
  cleanupTempDir,
  compileEsModule,
  getRepoRoot,
  pathToFileURL,
  resetTempDir,
  runTests,
} from "./ts-test-helpers.mjs";

const root = getRepoRoot(import.meta.url);
const outDir = resetTempDir("spriteanime-matting-tests");
const compiledPath = compileEsModule(
  root,
  outDir,
  "src/features/image/matting.ts",
  "generator-matting.js"
);
const matting = await import(pathToFileURL(compiledPath).href);

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

runTests([
  testContainMappingHorizontalLetterbox,
  testContainMappingVerticalLetterbox,
]);

cleanupTempDir(outDir);
console.log("Matting coordinate tests passed.");
