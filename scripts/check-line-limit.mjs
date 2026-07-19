import { readdir, readFile } from "node:fs/promises";
import { extname, join, relative } from "node:path";
import process from "node:process";

const root = process.cwd();
const limit = 500;
const sourceExtensions = new Set([
  ".css",
  ".html",
  ".js",
  ".mjs",
  ".rs",
  ".sh",
  ".ts",
  ".tsx",
]);
const excludedDirectories = new Set([
  ".git",
  "dist",
  "logs",
  "node_modules",
  "output",
  "SpriteAnimteData",
  "target",
]);

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const paths = [];
  for (const entry of entries) {
    if (entry.isDirectory() && excludedDirectories.has(entry.name)) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      paths.push(...(await collectFiles(path)));
    } else if (entry.isFile() && sourceExtensions.has(extname(entry.name))) {
      paths.push(path);
    }
  }
  return paths;
}

const violations = [];
for (const path of await collectFiles(root)) {
  const content = await readFile(path, "utf8");
  const lines = content === "" ? 0 : content.split(/\r?\n/).length;
  if (lines > limit) {
    violations.push({ path: relative(root, path), lines });
  }
}

violations.sort((left, right) => right.lines - left.lines);
if (violations.length > 0) {
  console.error(`发现 ${violations.length} 个文件超过 ${limit} 行：`);
  for (const violation of violations) {
    console.error(`${String(violation.lines).padStart(5)}  ${violation.path}`);
  }
  process.exitCode = 1;
} else {
  console.log(`行数检查通过：所有源码与界面文件均不超过 ${limit} 行。`);
}
