import { describe, expect, it } from 'vitest';
import { expirationStatus, formatDateTime, formatLogTime } from '../format';

/**
 * Даты без смещения `parseISO` трактует как локальные - тесты не зависят
 * от часового пояса машины (в CI обычно UTC, у разработчика может быть
 * любой). Смещение в реальных данных приходит с бэкенда и проверяется
 * отдельным тестом ниже.
 */
const NOW = new Date('2026-10-05T12:00:00');

describe('expirationStatus', () => {
    it('reports missing expiration as reset-pending', () => {
        expect(expirationStatus(null, NOW)).toEqual({ kind: 'notSet' });
    });

    it('detects expired passwords', () => {
        const status = expirationStatus('2026-10-01T12:00:00', NOW);
        expect(status.kind).toBe('expired');
    });

    it('detects same-day and soon expirations', () => {
        expect(expirationStatus('2026-10-05T18:00:00', NOW).kind).toBe('today');
        const soon = expirationStatus('2026-10-07T12:00:00', NOW);
        expect(soon).toEqual({ kind: 'soon', days: 2 });
    });

    it('reports far future as future', () => {
        expect(expirationStatus('2026-12-01T00:00:00', NOW)).toEqual({
            kind: 'future',
            days: 57,
        });
    });
});

describe('formatDateTime', () => {
    it('formats a local ISO string', () => {
        expect(formatDateTime('2026-10-07T12:00:00', 'ru')).toBe('07.10.2026 12:00');
    });

    it('converts a UTC instant to local time', () => {
        const local = new Date(2026, 9, 7, 12, 0, 0);
        const formatted = formatDateTime(local.toISOString(), 'ru');
        expect(formatted).toBe('07.10.2026 12:00');
    });

    it('passes broken values through', () => {
        expect(formatDateTime('мусор', 'ru')).toBe('мусор');
        expect(formatDateTime(null, 'ru')).toBe('');
    });
});

describe('formatLogTime', () => {
    it('uses full-year business format', () => {
        const local = new Date(2026, 9, 5, 14, 0, 43);
        expect(formatLogTime(local.toISOString(), 'ru')).toBe('05.10.2026 14:00:43');
    });

    it('passes broken values through', () => {
        expect(formatLogTime('не дата', 'ru')).toBe('не дата');
    });
});
