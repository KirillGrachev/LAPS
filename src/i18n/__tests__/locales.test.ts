import { describe, expect, it } from 'vitest';
import { en } from '../en';
import { ru } from '../ru';

/**
 * Полнота локалей: набор ключей `ru` и `en` должен совпадать.
 * Иначе английский/русский интерфейс тихо уезжает в fallback.
 */
function keyPaths(value: unknown, prefix = ''): string[] {
    if (value === null || typeof value !== 'object') return [prefix];
    return Object.entries(value as Record<string, unknown>).flatMap(([key, child]) =>
        keyPaths(child, prefix ? `${prefix}.${key}` : key),
    );
}

describe('i18n locales', () => {
    it('ru and en expose the same keys', () => {
        const ruKeys = keyPaths(ru).sort();
        const enKeys = keyPaths(en).sort();
        expect(ruKeys).toEqual(enKeys);
    });

    it('has no empty translations', () => {
        for (const [locale, tree] of [
            ['ru', ru],
            ['en', en],
        ] as const) {
            const empty = keyPaths(tree).filter((path) => {
                let node: unknown = tree;
                for (const part of path.split('.')) {
                    node = (node as Record<string, unknown>)[part];
                }
                return typeof node === 'string' && node.trim() === '';
            });
            expect(empty, `${locale}: пустые ключи`).toEqual([]);
        }
    });
});
