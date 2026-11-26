import { Clock } from 'lucide-react';
import type { LapsInfo } from '../../types';
import { useTranslation } from '../../i18n/index.tsx';
import { expirationStatus, formatDateTime } from '../../lib/format';

/** Строка «действует до …» - живёт в блоке управления сроком действия. */
export function ExpirationStatusLine({ result }: { result: LapsInfo }) {
    const { t, language } = useTranslation();
    const status = expirationStatus(result.expiresAt);

    if (status.kind === 'notSet') {
        return (
            <span className="text-ui-label font-bold text-muted-foreground">
                {t('search.expirationStatus.resetPending')}
            </span>
        );
    }

    const date = formatDateTime(result.expiresAt, language);
    let color = 'text-emerald-600 dark:text-emerald-400';
    let text = t('search.expirationStatus.expiresAt', { date });
    if (status.kind === 'expired') {
        color = 'text-red-500';
        text = t('search.expirationStatus.expired', { date });
    } else if (status.kind === 'today') {
        color = 'text-amber-500';
        text = t('search.expirationStatus.expiresSoon');
    } else if (status.kind === 'soon') {
        color = 'text-amber-500';
        text = `${t('search.expirationStatus.expiresIn', { days: status.days })}, ${date}`;
    }

    return (
        <span className={`inline-flex items-center gap-1.5 text-ui-label font-bold ${color}`}>
            <Clock size={12} className="shrink-0" />
            {text}
        </span>
    );
}
