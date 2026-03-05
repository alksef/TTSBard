#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

const baseVersion = process.argv[2];
const fullSha = process.argv[3];

if (!baseVersion || !fullSha) {
  console.error('Usage: node set-version.js <base-version> <commit-sha>');
  process.exit(1);
}

// Use short SHA (first 7 characters)
const shortSha = fullSha.slice(0, 7);
const version = `${baseVersion}-${shortSha}`;

console.log(`Setting version: ${version}`);

// Update package.json
const packageJsonPath = path.join(__dirname, '..', 'package.json');
const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
packageJson.version = version;
fs.writeFileSync(packageJsonPath, JSON.stringify(packageJson, null, 2) + '\n');
console.log(`Updated package.json: ${version}`);

// Update tauri.conf.json
const tauriConfPath = path.join(__dirname, '..', 'src-tauri', 'tauri.conf.json');
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
tauriConf.version = version;
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n');
console.log(`Updated tauri.conf.json: ${version}`);

// Create version.ts for frontend
const versionTsPath = path.join(__dirname, '..', 'src', 'version.ts');
const versionContent = `export const APP_VERSION = '${version}';
export const APP_VERSION_BASE = '${baseVersion}';
export const APP_COMMIT_SHA = '${shortSha}';
`;
fs.writeFileSync(versionTsPath, versionContent);
console.log(`Created src/version.ts`);
