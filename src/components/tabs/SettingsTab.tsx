import { useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';
import { ChevronRight, ShieldAlert } from 'lucide-react';
import { PageLayout } from '../ui/PageLayout';
import { Input } from '../ui/Input';
import { Button } from '../ui/Button';
import { Select } from '../ui/Select';
import { Switch } from '../ui/Switch';
import { useTheme } from '../../context/ThemeContext';
import { useConfig } from '../../context/ConfigContext';
import { useSession } from '../../context/SessionContext';
import { useTranslation } from '../../i18n/index.tsx';
import { tauriInvoke, toIpcError, dropAllSessions } from '../../lib/ipc';
import { system } from '../../lib/system';
import type { ConnectionTest, SaveConfigRequest, Theme } from '../../types';

const CLIPBOARD_OPTIONS = [0, 15, 30, 60, 120];

type SettingsView = 'menu' | 'ad' | 'security' | 'interface';

/** Строка меню настроек: без декоративных иконок, только текст и переход. */
function MenuRow({ title, onClick }: { title: string; onClick: () => void }) {
    return (
        <button
            type="button"
            onClick={onClick}
            className="w-full flex items-center gap-4 rounded-2xl border border-border bg-surface px-4 py-4 hover:bg-surface-hover transition-all duration-300 text-left shadow-sm"
        >
            <span className="flex-1 text-ui-md font-bold text-foreground truncate">{title}</span>
            <ChevronRight size={16} className="text-muted-foreground/50 shrink-0" />
        </button>
    );
}

/** Строка-переключатель в стилистике справочника: текст слева, switch справа. */
function ToggleRow({
    id,
    checked,
    onChange,
    label,
    hint,
    tone = 'primary',
    alert = false,
    disabled,
}: {
    id: string;
    checked: boolean;
    onChange: (value: boolean) => void;
    label: string;
    hint?: string;
    tone?: 'primary' | 'amber';
    alert?: boolean;
    disabled?: boolean;
}) {
    return (
        <div className="flex items-start justify-between gap-6 w-full">
            <div className="flex flex-col gap-1 min-w-0">
                <label
                    htmlFor={id}
                    className="flex items-center gap-1.5 text-ui-label text-foreground font-bold cursor-pointer select-none leading-snug"
                >
                    {alert && <ShieldAlert size={14} className="text-amber-500 shrink-0" />}
                    {label}
                </label>
                {hint && <span className="text-ui-xs text-muted-foreground font-medium leading-snug">{hint}</span>}
            </div>
            <div className="pt-0.5">
                <Switch id={id} checked={checked} onChange={onChange} disabled={disabled} tone={tone} />
            </div>
        </div>
    );
}

/** Поле с подписью и подсказкой в стилистике справочника: всё слева. */
function Field({ id, label, hint, children }: { id: string; label: string; hint?: string; children: React.ReactNode }) {
    return (
        <div className="flex flex-col gap-1.5 w-full">
            <label htmlFor={id} className="text-[13px] font-bold text-muted-foreground uppercase ml-1">
                {label}
            </label>
            {children}
            {hint && <span className="text-[13px] text-muted-foreground ml-1 leading-snug">{hint}</span>}
        </div>
    );
}

interface SettingsTabProps {
    /** Режим до входа: показывается кнопка возврата ко входу. */
    preAuth?: boolean;
    onBackToLogin?: () => void;
}

/**
 * Хаб настроек со страницами разделов.
 *
 * Автосохранение: черновик уехал от сохранённой конфигурации -
 * дебаунс 600 мс и тихий save; первый рендер не сохраняем.
 * my-auto: короткий контент (меню разделов) центрируется по
 * вертикали страницы, как в справочнике; длинный - скроллится
 * от верха без артефактов.
 */
export function SettingsTab({ preAuth = false, onBackToLogin }: SettingsTabProps) {
    const { theme } = useTheme();
    const { config, saveConfig } = useConfig();
    const { requireReauth } = useSession();
    const { t, language, setLanguage } = useTranslation();

    const [view, setView] = useState<SettingsView>('menu');
    const [draft, setDraft] = useState<SaveConfigRequest>({ ...config });
    const [dirty, setDirty] = useState(false);
    const [isTesting, setIsTesting] = useState(false);
    const [testResult, setTestResult] = useState<ConnectionTest | null>(null);
    const [testError, setTestError] = useState<string | null>(null);
    const [autoSavedAt, setAutoSavedAt] = useState<number | null>(null);
    const [testModeBusy, setTestModeBusy] = useState(false);
    /** Черновик на момент завершения save: отличает «сохранено всё» от «печатали дальше». */
    const draftRef = useRef<SaveConfigRequest>(draft);

    useEffect(() => {
        draftRef.current = draft;
    }, [draft]);

    // Внешнее изменение конфигурации принимаем, только если в черновике нет
    // несохранённых правок: иначе ответ save затирал новые символы.
    useEffect(() => {
        if (!dirty) setDraft({ ...config });
    }, [config, dirty]);

    const patch = (partial: Partial<SaveConfigRequest>) => {
        setDraft((prev) => ({ ...prev, ...partial }));
        setDirty(true);
    };

    /** Схема LDAP: недописанный адрес не отправляем - это даёт ложные тосты. */
    const hasCompleteLdapUrl = (value: string) => {
        const url = value.trim();
        return url === '' || /^ldaps?:\/\//i.test(url);
    };

    useEffect(() => {
        if (!dirty) return;
        if (JSON.stringify(draft) === JSON.stringify(config)) {
            setDirty(false);
            return;
        }
        if (!hasCompleteLdapUrl(draft.ldapUrl)) return;
        const timer = window.setTimeout(async () => {
            const saved = await saveConfig(draft, { silent: true });
            if (!saved) return;
            setAutoSavedAt(Date.now());
            // Пока шёл save, пользователь мог продолжить печатать.
            if (JSON.stringify(draftRef.current) === JSON.stringify(saved)) {
                setDirty(false);
            }
        }, 600);
        return () => window.clearTimeout(timer);
    }, [draft, dirty, config, saveConfig]);

    /**
     * Тестовый режим применяется немедленно: конфигурация сохраняется,
     * сессии обоих транспортов (реальная и демо) сбрасываются - вход
     * выполняется заново уже под актуальный источник данных.
     * Сохранение не удалось - откатываем переключатель.
     */
    const handleTestMode = async (value: boolean) => {
        if (testModeBusy || value === config.testMode) return;
        setTestModeBusy(true);
        patch({ testMode: value });
        const saved = await saveConfig({ ...draft, testMode: value }, { silent: true });
        setTestModeBusy(false);
        if (!saved) {
            patch({ testMode: config.testMode });
            return;
        }
        // Даём ручке переключателя доиграть, иначе экран входа появляется
        // в тот же кадр и движение выглядит резким.
        await new Promise((resolve) => window.setTimeout(resolve, 220));
        await dropAllSessions();
        requireReauth();
    };

    const handleTest = async () => {
        setIsTesting(true);
        setTestResult(null);
        setTestError(null);
        try {
            // Проверка должна идти по текущим полям, а не по прошлому config.json.
            const saved = await saveConfig({ ...draftRef.current }, { silent: true });
            if (!saved) {
                setTestError(t('settings.saveError'));
                return;
            }
            setDirty(false);
            const result = await tauriInvoke<ConnectionTest>('test_ldap_connection');
            setTestResult(result);
        } catch (err) {
            setTestError(toIpcError(err).message);
        } finally {
            setIsTesting(false);
        }
    };

    const authModeLabel = (mode: ConnectionTest['authMode']) =>
        mode === 'integrated'
            ? t('settings.ad.authModeIntegrated')
            : mode === 'account'
              ? t('settings.ad.authModeAccount')
              : t('settings.ad.authModeAnonymous');

    const themeButtons: { value: Theme; label: string }[] = [
        { value: 'light', label: t('settings.interface.themes.light') },
        { value: 'dark', label: t('settings.interface.themes.dark') },
        { value: 'system', label: t('settings.interface.themes.system') },
    ];

    const viewTitles: Record<SettingsView, string> = {
        menu: t('settings.title'),
        ad: t('settings.ad.title'),
        security: t('settings.security.title'),
        interface: t('settings.interface.title'),
    };

    return (
        <PageLayout
            id="settings"
            title={viewTitles[view]}
            onBack={view !== 'menu' ? () => setView('menu') : preAuth ? onBackToLogin : undefined}
            backLabel={view !== 'menu' ? t('settings.back') : t('auth.title')}
            rightElement={
                autoSavedAt ? (
                    <span className="text-ui-xs font-black text-emerald-600 dark:text-emerald-400 uppercase tracking-tight normal-case">
                        {t('settings.ad.autoSaved')}
                    </span>
                ) : undefined
            }
            contentClassName="overflow-y-auto scrollbar-hide p-4 sm:p-6"
        >
            <div className="shrink-0 w-full max-w-[600px] mx-auto my-auto flex flex-col gap-5 py-6">
                {view === 'menu' && (
                    <div className="flex flex-col gap-3">
                        <MenuRow title={t('settings.ad.title')} onClick={() => setView('ad')} />
                        <MenuRow title={t('settings.security.title')} onClick={() => setView('security')} />
                        <MenuRow title={t('settings.interface.title')} onClick={() => setView('interface')} />
                    </div>
                )}

                {view === 'ad' && (
                    <div className="flex flex-col gap-5">
                        <div className="bg-surface border border-border rounded-[16px] p-[16px] shadow-sm flex items-center justify-between gap-[16px]">
                            <div className="flex flex-col gap-0.5 min-w-0 pr-4">
                                <span className="text-ui-label font-bold text-foreground leading-snug">
                                    {t('settings.ad.testModeLabel')}
                                </span>
                                <span className="text-ui-xs text-muted-foreground leading-snug">
                                    {t('settings.ad.testModeDesc')}
                                </span>
                            </div>
                            <Switch
                                id="testMode"
                                checked={draft.testMode}
                                onChange={(value) => void handleTestMode(value)}
                                disabled={testModeBusy}
                                ariaLabel={t('settings.ad.testModeLabel')}
                                tone="amber"
                            />
                        </div>

                        <div className="flex flex-col gap-3 bg-surface border border-border rounded-[16px] p-[16px] shadow-sm">
                            <Field id="ldapUrl" label={t('settings.ad.ldapUrlLabel')}>
                                <Input
                                    id="ldapUrl"
                                    type="text"
                                    value={draft.ldapUrl}
                                    onChange={(e) => patch({ ldapUrl: e.target.value })}
                                    placeholder={t('settings.ad.ldapUrlPlaceholder')}
                                    className="w-full h-[40px]"
                                    autoComplete="off"
                                    spellCheck={false}
                                />
                            </Field>

                            <Field id="baseDn" label={t('settings.ad.baseDnLabel')} hint={t('settings.ad.baseDnHint')}>
                                <Input
                                    id="baseDn"
                                    type="text"
                                    value={draft.baseDn}
                                    onChange={(e) => patch({ baseDn: e.target.value })}
                                    placeholder={t('settings.ad.baseDnPlaceholder')}
                                    className="w-full h-[40px]"
                                    autoComplete="off"
                                    spellCheck={false}
                                />
                            </Field>

                            <div className="flex flex-col gap-4 pt-3 border-t border-border/60">
                                <ToggleRow
                                    id="invalidTls"
                                    checked={draft.allowInvalidTls}
                                    onChange={(value) => patch({ allowInvalidTls: value })}
                                    label={t('settings.ad.invalidTlsLabel')}
                                    hint={t('settings.ad.invalidTlsHint')}
                                    tone="amber"
                                    alert
                                />
                            </div>

                            <div className="flex flex-col items-center gap-3 pt-3 border-t border-border/60">
                                <Button
                                    type="button"
                                    variant="primary"
                                    onClick={() => void handleTest()}
                                    disabled={isTesting || (!draft.ldapUrl && !draft.testMode)}
                                    className="h-11 min-w-[220px] px-6 text-ui-md font-bold"
                                >
                                    {isTesting ? t('settings.ad.testingBtn') : t('settings.ad.testBtn')}
                                </Button>

                                {testError && (
                                    <div className="w-full rounded-lg bg-red-500/[0.08] px-4 py-3 flex flex-col gap-1">
                                        <span className="text-ui-label font-black text-red-500">
                                            {t('settings.ad.testFailedTitle')}
                                        </span>
                                        <span className="text-ui-sm text-muted-foreground font-medium break-words">
                                            {testError}
                                        </span>
                                    </div>
                                )}

                                {testResult && (
                                    <div className="w-full rounded-lg bg-emerald-500/[0.08] px-4 py-3 flex flex-col items-center gap-1.5 text-center">
                                        <span className="text-ui-label font-black text-emerald-600 dark:text-emerald-400">
                                            {t('settings.ad.testOkTitle')}
                                        </span>
                                        <span className="text-ui-sm text-muted-foreground font-bold">
                                            {t('settings.ad.testOkText', {
                                                ms: testResult.bindMs,
                                                server: testResult.serverDnsName ?? '?',
                                            })}
                                            {', '}
                                            {authModeLabel(testResult.authMode)}
                                        </span>
                                        {testResult.defaultNamingContext && !draft.baseDn && (
                                            <button
                                                type="button"
                                                onClick={() => patch({ baseDn: testResult.defaultNamingContext ?? '' })}
                                                className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-primary/10 text-primary text-ui-xs font-black hover:bg-primary/15 transition-colors"
                                            >
                                                {t('settings.ad.useAsBaseDnBtn')}: {testResult.defaultNamingContext}
                                            </button>
                                        )}
                                    </div>
                                )}
                            </div>
                        </div>
                    </div>
                )}

                {view === 'security' && (
                    <div className="flex flex-col gap-5 bg-surface rounded-2xl p-6">
                        <ToggleRow
                            id="audit"
                            checked={draft.auditEnabled}
                            onChange={(value) => patch({ auditEnabled: value })}
                            label={t('settings.security.auditLabel')}
                        />

                        <div className="flex items-start justify-between gap-6 w-full pt-3 border-t border-border/60">
                            <div className="flex flex-col gap-1 min-w-0">
                                <label
                                    htmlFor="clipboardClear"
                                    className="text-ui-label text-foreground font-bold leading-snug"
                                >
                                    {t('settings.security.clipboardLabel')}
                                </label>
                                <span className="text-ui-xs text-muted-foreground font-medium leading-snug">
                                    {t('settings.security.clipboardHint')}
                                </span>
                            </div>
                            <Select
                                id="clipboardClear"
                                className="w-[180px] shrink-0"
                                value={draft.clipboardClearSeconds}
                                options={CLIPBOARD_OPTIONS.map((seconds) => ({
                                    value: seconds,
                                    label:
                                        seconds === 0
                                            ? t('settings.security.clipboardOff')
                                            : t('settings.security.clipboardSeconds', { seconds }),
                                }))}
                                onChange={(value) => patch({ clipboardClearSeconds: value })}
                            />
                        </div>

                        <div className="w-full flex justify-center pt-3 border-t border-border/60">
                            <button
                                type="button"
                                onClick={() => {
                                    void system.openAuditFolder().catch((err) => {
                                        toast.error(toIpcError(err).message, { id: 'audit-folder-error' });
                                    });
                                }}
                                className="inline-flex items-center gap-2 px-4 py-2 text-ui-xs font-bold uppercase tracking-tight text-muted-foreground hover:text-primary transition-colors bg-[#e6ebf2] dark:bg-white/[0.08] rounded-lg"
                            >
                                {t('settings.security.openAuditFolderBtn')}
                            </button>
                        </div>
                    </div>
                )}

                {view === 'interface' && (
                    <div className="flex flex-col gap-5 bg-surface rounded-2xl p-6">
                        <div className="flex flex-col gap-3">
                            <span className="text-ui-label font-black text-muted-foreground uppercase tracking-tight">
                                {t('settings.interface.themeLabel')}
                            </span>
                            <div className="flex items-center justify-center gap-3">
                                {themeButtons.map(({ value, label }) => (
                                    <button
                                        key={value}
                                        type="button"
                                        onClick={() => patch({ theme: value })}
                                        className={`w-[92px] p-3 rounded-xl flex flex-col items-center gap-2 transition-all duration-300 border-none ${
                                            theme === value
                                                ? 'bg-primary text-primary-foreground scale-105'
                                                : 'bg-[#e6ebf2] dark:bg-white/[0.08] text-muted-foreground hover:bg-[#dde4ed] dark:hover:bg-white/[0.13]'
                                        }`}
                                    >
                                        <span className="text-ui-caption font-bold uppercase tracking-tight">
                                            {label}
                                        </span>
                                    </button>
                                ))}
                            </div>
                        </div>

                        <div className="flex flex-col gap-3 pt-3 border-t border-border/60">
                            <span className="text-ui-label font-black text-muted-foreground uppercase tracking-tight">
                                {t('settings.interface.languageLabel')}
                            </span>
                            <div className="flex items-center justify-center gap-3">
                                {(['ru', 'en'] as const).map((lang) => (
                                    <button
                                        key={lang}
                                        type="button"
                                        onClick={() => {
                                            void setLanguage(lang);
                                            patch({ language: lang });
                                        }}
                                        className={`w-[92px] p-3 rounded-xl flex flex-col items-center gap-1.5 transition-all duration-300 border-none ${
                                            language === lang
                                                ? 'bg-primary text-primary-foreground scale-105'
                                                : 'bg-[#e6ebf2] dark:bg-white/[0.08] text-muted-foreground hover:bg-[#dde4ed] dark:hover:bg-white/[0.13]'
                                        }`}
                                    >
                                        <span className="text-ui-base font-black">{lang.toUpperCase()}</span>
                                        <span className="text-ui-caption font-bold uppercase tracking-tight">
                                            {t(`settings.interface.${lang}`)}
                                        </span>
                                    </button>
                                ))}
                            </div>
                        </div>
                    </div>
                )}
            </div>
        </PageLayout>
    );
}
