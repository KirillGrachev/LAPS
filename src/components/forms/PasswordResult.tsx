import { useEffect, useState } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { Eye, EyeOff, CheckCircle2, Copy, Monitor, Globe, Sparkles } from 'lucide-react';
import { Input } from '../ui/Input';
import { Button } from '../ui/Button';
import type { LapsInfo } from '../../types';
import { useTranslation } from '../../i18n/index.tsx';
import { formatDateTime } from '../../lib/format';

export interface PasswordResultProps {
    result: LapsInfo | null;
    /** true - пароль реально попал в буфер обмена. */
    copyPassword: () => Promise<boolean>;
}

/** Через сколько секунд показанный пароль снова маскируется. */
const AUTO_HIDE_MS = 30_000;

/** Длина маски фиксирована: по числу точек не должно быть видно длину пароля. */
const MASK_LENGTH = 16;

/**
 * Карточка результата: метаинформация о компьютере, пароль
 * (маска/показ/копирование) и диагностика для шифрованного хранения.
 */
export function PasswordResult({ result, copyPassword }: PasswordResultProps) {
    const { t, language } = useTranslation();
    const [isVisible, setIsVisible] = useState(false);
    const [copied, setCopied] = useState(false);

    // Новая карточка (другой компьютер или новый пароль после ротации) -
    // снова маска: нельзя показывать секрет только потому, что у предыдущего
    // пароля нажали «показать». Ключ - пара DN + значение пароля.
    useEffect(() => {
        setIsVisible(false);
        setCopied(false);
    }, [result?.dn, result?.password]);

    // Показанный пароль сам маскируется через 30 секунд.
    useEffect(() => {
        if (!isVisible) return;
        const timer = window.setTimeout(() => setIsVisible(false), AUTO_HIDE_MS);
        return () => window.clearTimeout(timer);
    }, [isVisible]);

    useEffect(() => {
        if (!copied) return;
        const timer = window.setTimeout(() => setCopied(false), 2000);
        return () => window.clearTimeout(timer);
    }, [copied]);

    if (!result) return null;

    const handleCopy = async () => {
        // Не показываем «Готово», если clipboard.writeText упал.
        setCopied(await copyPassword());
    };

    return (
        <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.18, ease: 'easeOut' }}
            className="w-full bg-surface rounded-2xl p-5 sm:p-6 flex flex-col gap-5"
        >
            <div className="flex items-start gap-4 flex-wrap">
                <div className="flex flex-col gap-1 min-w-0">
                    <div className="flex items-center gap-2.5 flex-wrap">
                        <Monitor size={16} className="text-primary shrink-0" />
                        <span className="text-ui-lg font-black text-foreground tracking-tight uppercase truncate">
                            {result.computerName}
                        </span>
                    </div>
                    <div className="flex items-center gap-4 text-ui-sm text-muted-foreground font-medium flex-wrap">
                        {result.dnsHostName && (
                            <span className="inline-flex items-center gap-1">
                                <Globe size={11} className="opacity-60" />
                                {result.dnsHostName}
                            </span>
                        )}
                        {result.operatingSystem && <span>{result.operatingSystem}</span>}
                    </div>
                </div>
            </div>

            {result.decrypted && result.password !== null && (
                <div className="flex items-center gap-2 text-ui-sm font-bold text-primary">
                    <Sparkles size={13} className="shrink-0" />
                    {t('search.encrypted.decryptedNote')}
                </div>
            )}

            {result.flavor === 'windowsEncrypted' && result.password === null && (
                <div className="flex flex-col gap-1 text-ui-sm font-medium text-muted-foreground leading-relaxed">
                    <span>{t('search.encrypted.notDecryptedHint')}</span>
                    {result.decryptError && (
                        <span className="font-mono text-ui-xs text-muted-foreground/60 break-all">
                            {t('search.encrypted.decryptErrorLabel')}: {result.decryptError}
                        </span>
                    )}
                </div>
            )}

            {(result.password !== null || result.flavor === 'windowsEncrypted') && (
                <div className="flex flex-col gap-[8px]">
                    <div className="flex flex-col gap-1.5">
                        <label
                            htmlFor="currentAccount"
                            className="text-[13px] font-bold text-muted-foreground uppercase"
                        >
                            {t('search.labels.account')}
                        </label>
                        <Input
                            id="currentAccount"
                            type="text"
                            className="w-full h-[40px] sm:h-[44px] text-ui-md font-bold text-center"
                            value={result.accountName ?? t('common.unknown')}
                            readOnly
                        />
                    </div>
                    <label
                        htmlFor="currentPassword"
                        className="text-ui-label font-bold text-muted-foreground uppercase tracking-tight transition-colors"
                    >
                        {t('search.passwordLabel')}
                    </label>
                    <div className="flex flex-col sm:flex-row gap-[8px] sm:gap-[10px]">
                        <div className="relative flex-1">
                            <Input
                                id="currentPassword"
                                type="text"
                                className={`w-full h-[40px] sm:h-[44px] text-ui-lg font-mono font-bold tracking-wide text-center ${
                                    result.password === null
                                        ? 'pr-4 text-muted-foreground/70 font-sans text-ui-base select-none'
                                        : 'pr-12'
                                }`}
                                value={
                                    result.password === null
                                        ? t('search.encrypted.notDecrypted')
                                        : isVisible
                                          ? result.password
                                          : '•'.repeat(MASK_LENGTH)
                                }
                                readOnly
                                tabIndex={0}
                            />
                            {result.password !== null && (
                                <button
                                    type="button"
                                    onClick={() => setIsVisible(!isVisible)}
                                    className="absolute right-3 top-1/2 -translate-y-1/2 p-2 text-muted-foreground hover:text-foreground transition-colors"
                                    title={isVisible ? t('app.hide') : t('app.show')}
                                >
                                    {isVisible ? (
                                        <EyeOff size={18} strokeWidth={2.2} />
                                    ) : (
                                        <Eye size={18} strokeWidth={2.2} />
                                    )}
                                </button>
                            )}
                        </div>
                        <Button
                            variant={copied ? 'primary' : 'outline'}
                            type="button"
                            onClick={() => void handleCopy()}
                            disabled={!result.password}
                            className={`relative font-black h-[40px] sm:h-[44px] px-8 transition-colors w-full sm:w-[160px] sm:min-w-[160px] flex-shrink-0 ${
                                copied
                                    ? 'bg-green-600 hover:bg-green-700 border-transparent text-white shadow-lg shadow-green-600/20'
                                    : ''
                            }`}
                        >
                            <AnimatePresence mode="wait" initial={false}>
                                {copied ? (
                                    <motion.div
                                        key="copied"
                                        initial={{ opacity: 0 }}
                                        animate={{ opacity: 1 }}
                                        exit={{ opacity: 0 }}
                                        transition={{ duration: 0.16 }}
                                        className="flex items-center gap-2 justify-center"
                                    >
                                        <CheckCircle2 size={16} strokeWidth={3} />
                                        <span className="whitespace-nowrap">{t('app.ready')}</span>
                                    </motion.div>
                                ) : (
                                    <motion.div
                                        key="copy"
                                        initial={{ opacity: 0 }}
                                        animate={{ opacity: 1 }}
                                        exit={{ opacity: 0 }}
                                        transition={{ duration: 0.16 }}
                                        className="flex items-center gap-2 justify-center"
                                    >
                                        <Copy size={16} strokeWidth={3} />
                                        <span className="whitespace-nowrap">{t('search.copyBtn')}</span>
                                    </motion.div>
                                )}
                            </AnimatePresence>
                        </Button>
                    </div>
                    {result.updatedAt && (
                        <span className="text-ui-xs text-muted-foreground/70 font-medium text-center sm:text-left">
                            {t('search.labels.updated')}: {formatDateTime(result.updatedAt, language)}
                        </span>
                    )}
                </div>
            )}
        </motion.div>
    );
}
