import { tauriInvoke } from './ipc';
import { system } from './system';

/**
 * Буфер обмена в Tauri-сборке идёт через Rust: WebView2 иначе показывает
 * запрос «сайт хочет просматривать буфер обмена». В браузерном dev-режиме
 * остаётся нативный Clipboard API.
 */
export async function writeClipboardText(text: string): Promise<void> {
    if (system.isTauri()) {
        await tauriInvoke('write_clipboard_text', { text });
        return;
    }
    await navigator.clipboard.writeText(text);
}

/**
 * Очищает буфер, только если там лежит ожидаемое значение.
 *
 * В Tauri содержимое буфера не покидает Rust: команда возвращает лишь факт
 * очистки. В браузерном режиме сравнение делается через Clipboard API;
 * ошибка доступа пробрасывается, чтобы вызывающий код показал предупреждение.
 */
export async function clearClipboardIfEquals(expected: string): Promise<boolean> {
    if (system.isTauri()) {
        return await tauriInvoke<boolean>('clear_clipboard_if_equals', { expected });
    }
    const current = await navigator.clipboard.readText();
    if (current !== expected) return false;
    await navigator.clipboard.writeText('');
    return true;
}
