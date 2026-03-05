#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

export function readWorkspaceVersion(cargoTomlPath = "Cargo.toml") {
  const cargoToml = fs.readFileSync(cargoTomlPath, "utf8");
  const lines = cargoToml.split(/\r?\n/);
  const sectionHeader = "[workspace.package]";
  const startIndex = lines.findIndex((line) => line.trim() === sectionHeader);
  if (startIndex < 0) {
    throw new Error(`Could not find [workspace.package] in ${cargoTomlPath}`);
  }

  const sectionLines = [];
  for (let i = startIndex + 1; i < lines.length; i += 1) {
    const line = lines[i];
    if (/^\s*\[[^\]]+\]\s*$/.test(line)) {
      break;
    }
    sectionLines.push(line);
  }

  const sectionBody = sectionLines.join("\n");
  const versionMatch = sectionBody.match(/\bversion\s*=\s*"([^"]+)"/);
  if (!versionMatch) {
    throw new Error(
      `Could not find workspace.package.version in ${cargoTomlPath}`,
    );
  }
  return versionMatch[1];
}

const invokedScriptUrl = process.argv[1]
  ? pathToFileURL(path.resolve(process.argv[1])).href
  : null;

if (invokedScriptUrl && import.meta.url === invokedScriptUrl) {
  const cargoTomlPath = process.argv[2] ?? "Cargo.toml";
  process.stdout.write(readWorkspaceVersion(cargoTomlPath));
}
