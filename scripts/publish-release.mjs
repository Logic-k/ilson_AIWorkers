import fs from "node:fs";
import path from "node:path";
import { execFileSync, execSync } from "node:child_process";
import os from "node:os";

const repo = "Logic-k/ilson_AIWorkers";
const conf = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
const version = conf.version;
const tag = `v${version}`;

console.log(`=== Publishing ilson ${tag} auto-update release ===`);

const releaseDir = path.join("src-tauri", "target", "release");
const bundleDir = path.join(releaseDir, "bundle", "nsis");
const setupFile = path.join(bundleDir, `일손_${version}_x64-setup.exe`);
const sigFile = `${setupFile}.sig`;
const asciiSetup = path.join(bundleDir, `ilson_${version}_x64-setup.exe`);
const latestJsonPath = path.join(bundleDir, "latest.json");

if (!fs.existsSync(setupFile) || !fs.existsSync(sigFile)) {
  const exePath = path.join(releaseDir, "ilson.exe");
  if (fs.existsSync(exePath)) {
    try {
      fs.renameSync(exePath, path.join(releaseDir, `ilson_running_${Date.now()}.exe`));
    } catch {
      // Ignore if not locked or already moved
    }
  }
  const keyPath = path.join(os.homedir(), ".tauri", "ilson.key");
  execSync("npx tauri build", {
    stdio: "inherit",
    env: {
      ...process.env,
      TAURI_SIGNING_PRIVATE_KEY: keyPath,
      TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "",
    },
  });
}

fs.copyFileSync(setupFile, asciiSetup);
const signature = fs.readFileSync(sigFile, "utf8").trim();
const latest = {
  version,
  notes: `일손 ${tag} 무설치 자동 업데이트`,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature,
      url: `https://github.com/${repo}/releases/download/${tag}/ilson_${version}_x64-setup.exe`,
    },
  },
};

fs.writeFileSync(latestJsonPath, JSON.stringify(latest, null, 2), "utf8");
console.log(`Generated ${latestJsonPath}`);

let releaseExists = false;
try {
  execFileSync("gh", ["release", "view", tag, "--repo", repo], { stdio: "ignore" });
  releaseExists = true;
} catch {
  releaseExists = false;
}

if (releaseExists) {
  execFileSync(
    "gh",
    ["release", "upload", tag, asciiSetup, setupFile, latestJsonPath, "--clobber", "--repo", repo],
    { stdio: "inherit" }
  );
} else {
  execFileSync(
    "gh",
    [
      "release",
      "create",
      tag,
      asciiSetup,
      setupFile,
      latestJsonPath,
      "--title",
      `일손 ${tag}`,
      "--notes",
      `일손 ${tag} 무설치 자동 업데이트 배포판`,
      "--repo",
      repo,
    ],
    { stdio: "inherit" }
  );
}

console.log(`=== Published ${tag} to https://github.com/${repo}/releases/tag/${tag} ===`);
