import { createContext, useContext, useEffect, useState, ReactNode } from 'react';
import type { LapsInfo } from '../types';
import { useSession } from './SessionContext';
import { clearClipboardIfEquals } from '../lib/clipboard';

interface SearchContextType {
    isSearching: boolean;
    setIsSearching: (val: boolean) => void;
    isRotating: boolean;
    setIsRotating: (val: boolean) => void;
    /** Последняя операция сброса срока - для раздельных индикаторов кнопок. */
    rotationKind: 'now' | 'at' | null;
    setRotationKind: (val: 'now' | 'at' | null) => void;
    result: LapsInfo | null;
    setResult: (val: LapsInfo | null) => void;
    error: string | null;
    setError: (val: string | null) => void;
    lastSearchQuery: string;
    setLastSearchQuery: (val: string) => void;
    /** Поколение сессии: растёт при выходе, чтобы поздние ответы не вернули карточку. */
    sessionEpoch: number;
}

const SearchContext = createContext<SearchContextType | undefined>(undefined);

export function SearchProvider({ children }: { children: ReactNode }) {
    const [isSearching, setIsSearching] = useState(false);
    const [isRotating, setIsRotating] = useState(false);
    const [rotationKind, setRotationKind] = useState<'now' | 'at' | null>(null);
    const [result, setResult] = useState<LapsInfo | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [lastSearchQuery, setLastSearchQuery] = useState('');
    const [sessionEpoch, setSessionEpoch] = useState(0);
    const { session, checking } = useSession();

    /**
     * Выход/истечение сессии: пароль не должен переживать logout.
     * `checking` защищает от очистки на старте, пока статус ещё неизвестен.
     */
    useEffect(() => {
        if (checking || session) return;
        const password = result?.password;
        setSessionEpoch((value) => value + 1);
        if (password) {
            void clearClipboardIfEquals(password).catch(() => {
                // Clipboard может быть недоступен - это best effort.
            });
        }
        setResult(null);
        setError(null);
        setLastSearchQuery('');
    }, [checking, session, result?.password]);

    return (
        <SearchContext.Provider
            value={{
                isSearching,
                setIsSearching,
                isRotating,
                setIsRotating,
                rotationKind,
                setRotationKind,
                result,
                setResult,
                error,
                setError,
                lastSearchQuery,
                setLastSearchQuery,
                sessionEpoch,
            }}
        >
            {children}
        </SearchContext.Provider>
    );
}

export function useSearchContext() {
    const context = useContext(SearchContext);
    if (context === undefined) {
        throw new Error('useSearchContext must be used within a SearchProvider');
    }
    return context;
}
