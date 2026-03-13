#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

const version = process.argv[2];
const fullSha = process.argv[3];

if (!version) {
  console.error('Usage: node set-version.js <version> [commit-sha]');
  process.exit(1);
}

const shortSha = fullSha ? fullSha.slice(0, 7) : 'unknown';

console.log(`Setting version: ${version}`);

// Update package.json
const packageJsonPath = path.join(__dirname, '..', 'package.json');
const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
packageJson.version = version;
fs.writeFileSync(packageJsonPath, JSON.stringify(packageJson, null, 2) + '\n');
console.log(`  package.json: ${version}`);

// Update tauri.conf.json
const tauriConfPath = path.join(__dirname, '..', 'src-tauri', 'tauri.conf.json');
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
tauriConf.version = version;
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n');
console.log(`  tauri.conf.json: ${version}`);

// Update Cargo.toml
const cargoPath = path.join(__dirname, '..', 'src-tauri', 'Cargo.toml');
let cargoContent = fs.readFileSync(cargoPath, 'utf8');
cargoContent = cargoContent.replace(/^version = ".*"$/m, `version = "${version}"`);
fs.writeFileSync(cargoPath, cargoContent);
console.log(`  Cargo.toml: ${version}`);

// Create version.ts for frontend
const versionTsPath = path.join(__dirname, '..', 'src', 'version.ts');
const versionContent = `export const APP_VERSION = '${version}';
export const APP_VERSION_BASE = '${version}';
export const APP_COMMIT_SHA = '${shortSha}';
`;
fs.writeFileSync(versionTsPath, versionContent);
console.log(`  src/version.ts: ${version} (sha: ${shortSha})`);
