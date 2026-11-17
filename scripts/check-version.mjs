/**
 * Version consistency gate for the release pipeline.
 *
 * The product version must be identical in package.json, src-tauri/Cargo.toml
 * and src-tauri/tauri.conf.json. The Vitest suite checks the same invariant for
 * pull requests; this script keeps the release job independent of npm install.
 */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

function readVersion(path, pattern) {
    const content = readFileSync(resolve(process.cwd(), path), 'utf8');
    const match = content.match(pattern);
    if (!match?.[1]) {
        throw new Error(`version not found in ${path}`);
    }
    return match[1];
}

const packageVersion = JSON.parse(readFileSync(resolve(process.cwd(), 'package.json'), 'utf8')).version;
const cargoVersion = readVersion('src-tauri/Cargo.toml', /^version\s*=\s*"([^"]+)"/m);
const tauriVersion = JSON.parse(readFileSync(resolve(process.cwd(), 'src-tauri/tauri.conf.json'), 'utf8')).version;

const versions = new Set([packageVersion, cargoVersion, tauriVersion]);
if (versions.size !== 1) {
    console.error(
        `version mismatch: package.json=${packageVersion}, Cargo.toml=${cargoVersion}, tauri.conf.json=${tauriVersion}`,
    );
    process.exit(1);
}

console.log(`version ${packageVersion}`);
