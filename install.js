#!/usr/bin/env node
"use strict";

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const https = require("https");

const VERSION = require("./package.json").version;
const REPO = "ricardo-hdrn/mcp-await";
const BIN_DIR = path.join(__dirname, "bin");
const NATIVE_NAME = process.platform === "win32" ? "mcp-await-native.exe" : "mcp-await-native";
const BIN_PATH = path.join(BIN_DIR, NATIVE_NAME);

const PLATFORM_MAP = {
  darwin: "apple-darwin",
  linux: "unknown-linux-gnu",
  win32: "pc-windows-msvc",
};

const ARCH_MAP = {
  x64: "x86_64",
  arm64: "aarch64",
};

function getAssetName() {
  const platform = PLATFORM_MAP[process.platform];
  const arch = ARCH_MAP[process.arch];

  if (!platform || !arch) {
    throw new Error(
      `Unsupported platform: ${process.platform}-${process.arch}`
    );
  }

  const target = `${arch}-${platform}`;
  const ext = process.platform === "win32" ? "zip" : "tar.gz";
  return { name: `mcp-await-${target}.${ext}`, target };
}

function download(url) {
  return new Promise((resolve, reject) => {
    https
      .get(url, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          return download(res.headers.location).then(resolve, reject);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`HTTP ${res.statusCode}: ${url}`));
        }
        const chunks = [];
        res.on("data", (chunk) => chunks.push(chunk));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      })
      .on("error", reject);
  });
}

async function main() {
  const { name } = getAssetName();
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${name}`;

  console.log(`Downloading mcp-await v${VERSION} for ${process.platform}-${process.arch}...`);

  const data = await download(url);

  fs.mkdirSync(BIN_DIR, { recursive: true });

  if (name.endsWith(".zip")) {
    // Write zip and extract with system tool
    const tmpZip = path.join(BIN_DIR, "tmp.zip");
    fs.writeFileSync(tmpZip, data);
    execSync(`powershell -Command "Expand-Archive -Force '${tmpZip}' '${BIN_DIR}'"`, {
      stdio: "inherit",
    });
    fs.unlinkSync(tmpZip);
  } else {
    // Write tar.gz and extract
    const tmpTar = path.join(BIN_DIR, "tmp.tar.gz");
    fs.writeFileSync(tmpTar, data);
    execSync(`tar xzf "${tmpTar}" -C "${BIN_DIR}"`, { stdio: "inherit" });
    fs.unlinkSync(tmpTar);
  }

  // Rename extracted binary to avoid collision with the node wrapper
  const extractedName = process.platform === "win32" ? "mcp-await.exe" : "mcp-await";
  const extractedPath = path.join(BIN_DIR, extractedName);
  if (fs.existsSync(extractedPath) && extractedPath !== BIN_PATH) {
    fs.renameSync(extractedPath, BIN_PATH);
  }

  if (process.platform !== "win32") {
    fs.chmodSync(BIN_PATH, 0o755);
  }

  console.log(`Installed mcp-await to ${BIN_PATH}`);
}

main().catch((err) => {
  console.error(`Failed to install mcp-await: ${err.message}`);
  process.exit(1);
});
