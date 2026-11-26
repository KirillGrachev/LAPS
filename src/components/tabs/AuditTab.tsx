import {
    RefreshCw,
    FolderOpen,
    Eye,
    EyeOff,
    RotateCcw,
    Clipboard,
    Save,
    LogIn,
    LogOut,
    PlugZap,
    CheckCircle2,
    XCircle,
    ScrollText,
    Loader2,
} from 'lucide-react';
import { PageLayout } from '../ui/PageLayout';
import { Button } from '../ui/Button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';
import { useTranslation } from '../../i18n/index.tsx';
import { isTestModeActive } from '../../lib/ipc';
import { useAuditLogs } from '../../hooks/useAuditLogs';
import { formatLogTime } from '../../lib/format';
import type { AuditAction } from '../../types';

const ACTION_ICONS: Record<AuditAction, typeof Eye> = {
    search: Eye,
    search_failed: EyeOff,
    rotate: RotateCcw,
    rotate_failed: RotateCcw,
    copy: Clipboard,
    config_saved: Save,
    login: LogIn,
    logout: LogOut,
    connection_test: PlugZap,
};

/** Цветовой код действия: успех - нейтральный, ошибка - красный, копирование - янтарный. */
function actionClasses(action: AuditAction, code: string): string {
    if (code !== 'OK') return 'text-red-500';
    switch (action) {
        case 'copy':
            return 'text-amber-500';
        case 'rotate':
            return 'text-blue-500';
        case 'login':
        case 'logout':
        case 'config_saved':
        case 'connection_test':
            return 'text-muted-foreground';
        default:
            return 'text-emerald-600 dark:text-emerald-400';
    }
}

export function AuditTab() {
    const { t, language } = useTranslation();
    const { logs, total, isLoading, isLoadingMore, error, refresh, loadMore, openFolder, hasMore } = useAuditLogs();

    const flavorLabels: Record<string, string> = {
        legacy: t('audit.flavors.legacy'),
        windows_plain: t('audit.flavors.windows_plain'),
        windows_encrypted: t('audit.flavors.windows_encrypted'),
        none: t('audit.flavors.none'),
    };

    return (
        <PageLayout
            id="audit"
            title={t('audit.title')}
            subtitle={t('audit.subtitle')}
            rightElement={
                <div className="flex items-center gap-2 normal-case">
                    <Button
                        variant="outline"
                        onClick={() => void refresh()}
                        className="h-[34px] px-3 text-ui-sm font-bold gap-2"
                    >
                        <RefreshCw size={13} className={isLoading ? 'animate-spin' : ''} />
                        {t('audit.refreshBtn')}
                    </Button>
                    <Button
                        variant="outline"
                        onClick={() => void openFolder()}
                        className="h-[34px] px-3 text-ui-sm font-bold gap-2"
                    >
                        <FolderOpen size={13} />
                        {t('audit.openFolderBtn')}
                    </Button>
                </div>
            }
            contentClassName="overflow-y-auto p-4 sm:p-6"
        >
            <div className="w-full max-w-6xl mx-auto flex flex-col gap-4 flex-1 min-h-0">
                {isTestModeActive() && (
                    <div className="rounded-lg border border-amber-500/25 bg-amber-500/5 px-4 py-3 text-ui-sm font-bold text-amber-600 dark:text-amber-400">
                        {t('audit.demoBanner')}
                    </div>
                )}

                {error && (
                    <div className="rounded-lg border border-red-500/25 bg-red-500/5 px-4 py-3 text-ui-base font-bold text-red-500">
                        {error}
                    </div>
                )}

                {isLoading ? (
                    <div className="flex-1 flex items-center justify-center gap-3 py-20 text-muted-foreground">
                        <Loader2 size={18} className="animate-spin" />
                        <span className="text-ui-base font-bold">{t('app.loading')}</span>
                    </div>
                ) : logs.length === 0 ? (
                    <div className="flex-1 flex flex-col items-center justify-center gap-3 py-20 text-muted-foreground">
                        <ScrollText size={36} className="opacity-25" />
                        <span className="text-ui-md font-black uppercase tracking-tight">{t('audit.emptyTitle')}</span>
                        <span className="text-ui-base font-medium">{t('audit.emptyText')}</span>
                    </div>
                ) : (
                    <>
                        <div className="rounded-xl border border-border overflow-hidden bg-surface">
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHead>{t('audit.columns.time')}</TableHead>
                                        <TableHead>{t('audit.columns.actor')}</TableHead>
                                        <TableHead>{t('audit.columns.action')}</TableHead>
                                        <TableHead>{t('audit.columns.target')}</TableHead>
                                        <TableHead>{t('audit.columns.flavor')}</TableHead>
                                        <TableHead className="text-center">{t('audit.columns.result')}</TableHead>
                                        <TableHead>{t('audit.columns.details')}</TableHead>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {logs.map((event, index) => {
                                        const Icon = ACTION_ICONS[event.action] ?? Eye;
                                        const ok = event.code === 'OK';
                                        return (
                                            <TableRow key={`${event.ts}-${index}`}>
                                                <TableCell className="whitespace-nowrap font-mono text-ui-label text-muted-foreground">
                                                    {formatLogTime(event.ts, language)}
                                                </TableCell>
                                                <TableCell className="font-bold text-ui-label whitespace-nowrap">
                                                    {event.actor}
                                                </TableCell>
                                                <TableCell className="whitespace-nowrap">
                                                    <span
                                                        className={`inline-flex items-center gap-1.5 text-ui-label font-bold whitespace-nowrap ${actionClasses(event.action, event.code)}`}
                                                    >
                                                        <Icon size={13} />
                                                        {t(`audit.actions.${event.action}`)}
                                                    </span>
                                                </TableCell>
                                                <TableCell className="font-mono font-bold text-ui-label uppercase whitespace-nowrap">
                                                    {event.target}
                                                </TableCell>
                                                <TableCell className="text-ui-label text-muted-foreground whitespace-nowrap">
                                                    {event.flavor
                                                        ? (flavorLabels[event.flavor] ?? event.flavor)
                                                        : t('common.unknown')}
                                                </TableCell>
                                                <TableCell className="whitespace-nowrap text-center">
                                                    {ok ? (
                                                        <CheckCircle2 size={15} className="text-emerald-500" />
                                                    ) : (
                                                        <span className="inline-flex items-center gap-1.5 text-red-500">
                                                            <XCircle size={15} />
                                                            <span className="font-mono text-ui-xs font-bold">
                                                                {event.code}
                                                            </span>
                                                        </span>
                                                    )}
                                                </TableCell>
                                                <TableCell className="text-ui-sm text-muted-foreground min-w-[220px] max-w-[360px] break-words">
                                                    {event.details ?? t('common.unknown')}
                                                </TableCell>
                                            </TableRow>
                                        );
                                    })}
                                </TableBody>
                            </Table>
                        </div>

                        <div className="flex items-center justify-between gap-4 px-1">
                            <span className="text-ui-sm font-bold text-muted-foreground">
                                {t('audit.shownOf', { shown: logs.length, total })}
                            </span>
                            {hasMore && (
                                <Button
                                    variant="outline"
                                    onClick={() => void loadMore()}
                                    disabled={isLoadingMore}
                                    className="h-[34px] px-4 text-ui-sm font-bold gap-2"
                                >
                                    {isLoadingMore && <Loader2 size={13} className="animate-spin" />}
                                    {t('audit.loadMoreBtn')}
                                </Button>
                            )}
                        </div>
                    </>
                )}
            </div>
        </PageLayout>
    );
}
