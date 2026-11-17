import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

/**
 * Единая версия продукта: package.json, Cargo.toml и tauri.conf.json
 * обязаны совпадать. Это ловит забытый bump перед релизом.
 */
function readVersion(path: string, pattern: RegExp): string {
    const content = readFileSync(resolve(process.cwd(), path), 'utf8');
    const match = content.match(pattern);
    if (!match?.[1]) throw new Error(`version not found in ${path}`);
    return match[1];
}

describe('product version', () => {
    it('is identical in package.json, Cargo.toml and tauri.conf.json', () => {
        const packageVersion = JSON.parse(readFileSync(resolve(process.cwd(), 'package.json'), 'utf8'))
            .version as string;
        const cargoVersion = readVersion('src-tauri/Cargo.toml', /^version\s*=\s*"([^"]+)"/m);
        const tauriVersion = JSON.parse(readFileSync(resolve(process.cwd(), 'src-tauri/tauri.conf.json'), 'utf8'))
            .version as string;

        expect(new Set([packageVersion, cargoVersion, tauriVersion]).size).toBe(1);
    });
});
