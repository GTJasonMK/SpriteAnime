import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const manifest = resolve(root, "src-tauri/Cargo.toml");
const options = parseArgs(process.argv.slice(2));
const target = options.target || rustHostTarget();
const extension = target.includes("windows") ? ".exe" : "";
const cargoArgs = [
  "build",
  "--manifest-path",
  manifest,
  "--bin",
  "sprite-anime-cli",
];
if (options.release) cargoArgs.push("--release");
if (options.target) cargoArgs.push("--target", options.target);

execFileSync("cargo", cargoArgs, { cwd: root, stdio: "inherit" });

const profile = options.release ? "release" : "debug";
const source = resolve(
  root,
  "src-tauri/target",
  ...(options.target ? [options.target] : []),
  profile,
  `sprite-anime-cli${extension}`
);
const destination = resolve(
  root,
  "src-tauri/binaries",
  `sprite-anime-cli-${target}${extension}`
);
mkdirSync(dirname(destination), { recursive: true });
copyFileSync(source, destination);
process.stdout.write(`${destination}\n`);

function rustHostTarget() {
  return execFileSync("rustc", ["--print", "host-tuple"], {
    cwd: root,
    encoding: "utf8",
  }).trim();
}

function parseArgs(args) {
  const options = { release: false, target: "" };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--release") {
      options.release = true;
      continue;
    }
    if (arg === "--target") {
      const target = args[index + 1];
      if (!target) throw new Error("--target 缺少目标三元组");
      options.target = target;
      index += 1;
      continue;
    }
    throw new Error(`未知参数: ${arg}`);
  }
  return options;
}
