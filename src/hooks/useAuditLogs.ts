/**
 * Журнал аудита: постраничная загрузка с бэкенда (`list_audit`),
 * обновление и «загрузить ещё».
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import type { AuditEvent, AuditPage } from '../types';
import { tauriInvoke, toIpcError } from '../lib/ipc';
import { system } from '../lib/system';

const PAGE_SIZE = 50;

/**
 * Журнал append-only: между запросами страниц в него могут добавиться
 * события, и offset-пагинация даёт дубли на границе. Схлопываем полностью
 * совпавшие записи (одинаковые события в одну миллисекунду практически
 * невозможны, а потеря строки из-за дубля ключа хуже).
 */
function dedupe(entries: AuditEvent[]): AuditEvent[] {
    const seen = new Set<string>();
    return entries.filter((entry) => {
        const key = JSON.stringify(entry);
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
    });
}

export function useAuditLogs() {
    const [logs, setLogs] = useState<AuditEvent[]>([]);
    const [total, setTotal] = useState(0);
    const [isLoading, setIsLoading] = useState(false);
    const [isLoadingMore, setIsLoadingMore] = useState(false);
    const [error, setError] = useState<string | null>(null);
    /** Поколение запроса: поздний ответ «ещё» не должен дописать старую страницу. */
    const requestId = useRef(0);

    const fetchPage = useCallback(async (offset: number, append: boolean) => {
        const id = ++requestId.current;
        if (append) {
            setIsLoadingMore(true);
        } else {
            setIsLoading(true);
        }
        setError(null);
        try {
            const page = await tauriInvoke<AuditPage>('list_audit', { limit: PAGE_SIZE, offset });
            if (id !== requestId.current) return;
            setTotal(page.total);
            setLogs((prev) => dedupe(append ? [...prev, ...page.entries] : page.entries));
        } catch (err) {
            if (id !== requestId.current) return;
            setError(toIpcError(err).message);
        } finally {
            if (id === requestId.current) {
                setIsLoading(false);
                setIsLoadingMore(false);
            }
        }
    }, []);

    const refresh = useCallback(() => fetchPage(0, false), [fetchPage]);
    const loadMore = useCallback(() => fetchPage(logs.length, true), [fetchPage, logs.length]);

    useEffect(() => {
        void refresh();
    }, [refresh]);

    const openFolder = useCallback(async () => {
        try {
            await system.openAuditFolder();
        } catch (err) {
            setError(toIpcError(err).message);
        }
    }, []);

    return {
        logs,
        total,
        isLoading,
        isLoadingMore,
        error,
        refresh,
        loadMore,
        openFolder,
        hasMore: logs.length < total,
    };
}
