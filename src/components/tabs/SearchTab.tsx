import { PageLayout } from '../ui/PageLayout';
import { useTranslation } from '../../i18n/index.tsx';
import { useLapsSearch } from '../../hooks/useLapsSearch';
import { SearchForm } from '../forms/SearchForm';
import { PasswordResult } from '../forms/PasswordResult';
import { ExpirationForm } from '../forms/ExpirationForm';

export function SearchTab() {
    const { t } = useTranslation();
    const {
        isSearching,
        isRotating,
        rotationKind,
        result,
        lastSearchQuery,
        error,
        handleSearch,
        handleRotate,
        copyPassword,
        clearResult,
    } = useLapsSearch();

    return (
        <PageLayout id="search" title={t('search.title')} contentClassName="p-4 sm:p-6 md:p-10 lg:p-12 overflow-y-auto">
            <div className="w-full max-w-4xl mx-auto flex flex-col gap-6 md:gap-8 my-auto py-4">
                <SearchForm
                    initialQuery={lastSearchQuery}
                    onSubmit={(query) => void handleSearch(query)}
                    isSearching={isSearching}
                    isRotating={isRotating}
                    hasResult={!!result}
                    onClear={clearResult}
                    translate={t}
                />

                {error && (
                    <div className="w-full rounded-lg bg-red-500/[0.07] px-4 py-2.5 text-center text-ui-label font-bold text-red-500">
                        {error}
                    </div>
                )}

                {result && (
                    <div className="w-full h-[1px] bg-border dark:bg-white/10 max-w-[120px] sm:max-w-xs md:max-w-md mx-auto transition-colors" />
                )}

                <PasswordResult result={result} copyPassword={copyPassword} />

                <ExpirationForm
                    result={result}
                    isRotating={isRotating}
                    rotationKind={rotationKind}
                    onRotate={(mode, localDateTime) => void handleRotate(mode, localDateTime)}
                />
            </div>
        </PageLayout>
    );
}
