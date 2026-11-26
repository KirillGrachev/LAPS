import { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import {
    addMonths,
    eachDayOfInterval,
    endOfMonth,
    endOfWeek,
    format,
    isSameDay,
    isSameMonth,
    parseISO,
    startOfMonth,
    startOfWeek,
} from 'date-fns';
import { ru as localeRU, enUS as localeEN } from 'date-fns/locale';
import { Calendar as CalendarIcon, ChevronLeft, ChevronRight } from 'lucide-react';
import { useTranslation } from '../../i18n/index.tsx';
import { formatDateTime } from '../../lib/format';
import { Button } from './Button';
import { usePopoverPosition } from './usePopoverPosition';

interface DateTimePickerProps {
    /** Локальная строка `YYYY-MM-DDTHH:mm`. */
    value: string;
    onChange: (value: string) => void;
    disabled?: boolean;
    ariaLabel?: string;
    className?: string;
    /** Применить выбранные дату/время прямо из календаря. */
    onApply?: (value: string) => void;
    /** Показывает «Обновление…» на кнопке применения. */
    applying?: boolean;
}

const pad = (n: number) => n.toString().padStart(2, '0');

const PANEL_WIDTH = 268;
const PANEL_HEIGHT = 430;

function toLocalValue(date: Date, hours: number, minutes: number): string {
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(hours)}:${pad(minutes)}`;
}

/**
 * Кастомный выбор даты и времени в стиле приложения.
 *
 * Попап рендерится через portal с `position: fixed` и клампом по краям
 * окна (при нехватке места снизу - раскрывается вверх): календарь никогда
 * не выходит за пределы формы приложения и не растягивает скролл -
 * поведение нативного контрола, а не веб-страницы.
 */
export function DateTimePicker({
    value,
    onChange,
    disabled,
    ariaLabel,
    className = '',
    onApply,
    applying = false,
}: DateTimePickerProps) {
    const { t, language } = useTranslation();
    const locale = language === 'ru' ? localeRU : localeEN;

    const parsed = useMemo(() => {
        const date = parseISO(value);
        return isNaN(date.getTime()) ? null : date;
    }, [value]);

    const [open, setOpen] = useState(false);
    const [month, setMonth] = useState<Date>(() => (parsed ? startOfMonth(parsed) : startOfMonth(new Date())));
    const [hours, setHours] = useState<number>(parsed ? parsed.getHours() : 12);
    const [minutes, setMinutes] = useState<number>(parsed ? parsed.getMinutes() : 0);
    const rootRef = useRef<HTMLDivElement>(null);
    const popRef = useRef<HTMLDivElement>(null);
    const position = usePopoverPosition(rootRef, open, PANEL_WIDTH, PANEL_HEIGHT);

    useEffect(() => {
        if (!open) return;
        const onPointerDown = (e: MouseEvent) => {
            const target = e.target as Node;
            if (rootRef.current?.contains(target) || popRef.current?.contains(target)) return;
            setOpen(false);
        };
        const onKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') setOpen(false);
        };
        document.addEventListener('mousedown', onPointerDown);
        document.addEventListener('keydown', onKeyDown);
        return () => {
            document.removeEventListener('mousedown', onPointerDown);
            document.removeEventListener('keydown', onKeyDown);
        };
    }, [open]);

    const days = useMemo(() => {
        const start = startOfWeek(startOfMonth(month), { weekStartsOn: 1 });
        const end = endOfWeek(endOfMonth(month), { weekStartsOn: 1 });
        return eachDayOfInterval({ start, end });
    }, [month]);

    const weekdayLabels = useMemo(
        () => days.slice(0, 7).map((day) => format(day, 'EEEEE', { locale })),
        [days, locale],
    );

    const commitDate = (day: Date) => {
        onChange(toLocalValue(day, hours, minutes));
    };

    const commitTime = (nextHours: number, nextMinutes: number) => {
        const base = parsed ?? new Date();
        onChange(toLocalValue(base, nextHours, nextMinutes));
    };

    const onHours = (raw: string) => {
        const next = Math.min(23, Math.max(0, Number(raw) || 0));
        setHours(next);
        commitTime(next, minutes);
    };

    const onMinutes = (raw: string) => {
        const next = Math.min(59, Math.max(0, Number(raw) || 0));
        setMinutes(next);
        commitTime(hours, next);
    };

    const currentValue = parsed ? toLocalValue(parsed, hours, minutes) : value;

    return (
        <div ref={rootRef} className={`relative ${className}`}>
            <button
                type="button"
                aria-label={ariaLabel}
                aria-haspopup="dialog"
                aria-expanded={open}
                disabled={disabled}
                onClick={() => setOpen((v) => !v)}
                className="relative w-full h-[42px] rounded-[10px] border border-border bg-[#e2e8f0] dark:bg-white/[0.08] pl-4 pr-10 text-center text-ui-md font-semibold text-foreground transition-colors focus:border-foreground/30 focus:outline-none disabled:opacity-60 disabled:cursor-not-allowed"
            >
                <span className={`block truncate ${parsed ? '' : 'text-muted-foreground'}`}>
                    {parsed ? formatDateTime(value, language) : t('search.expiration.placeholder')}
                </span>
                <CalendarIcon
                    size={15}
                    className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground opacity-70 pointer-events-none"
                />
            </button>

            {open &&
                position &&
                createPortal(
                    <div
                        ref={popRef}
                        style={{
                            position: position.position,
                            left: position.left,
                            top: position.top,
                            width: position.width,
                        }}
                        className="z-[95] rounded-[12px] bg-[#e2e8f0] dark:bg-[#1b1b1f] border border-border shadow-[0_4px_20px_rgba(0,0,0,0.18)] p-3 flex flex-col gap-3"
                    >
                        <div className="flex items-center justify-between">
                            <button
                                type="button"
                                onClick={() => setMonth((m) => addMonths(m, -1))}
                                className="p-1.5 rounded-lg hover:bg-surface-hover text-muted-foreground hover:text-foreground transition-colors"
                                aria-label="prev"
                            >
                                <ChevronLeft size={15} />
                            </button>
                            <span className="text-ui-label font-black text-foreground uppercase tracking-tight">
                                {format(month, 'LLLL yyyy', { locale })}
                            </span>
                            <button
                                type="button"
                                onClick={() => setMonth((m) => addMonths(m, 1))}
                                className="p-1.5 rounded-lg hover:bg-surface-hover text-muted-foreground hover:text-foreground transition-colors"
                                aria-label="next"
                            >
                                <ChevronRight size={15} />
                            </button>
                        </div>

                        <div className="grid grid-cols-7 gap-0.5">
                            {weekdayLabels.map((label, i) => (
                                <span
                                    key={i}
                                    className="h-6 flex items-center justify-center text-ui-caption font-black text-muted-foreground uppercase"
                                >
                                    {label}
                                </span>
                            ))}
                            {days.map((day) => {
                                const inMonth = isSameMonth(day, month);
                                const selected = parsed ? isSameDay(day, parsed) : false;
                                const today = isSameDay(day, new Date());
                                return (
                                    <button
                                        key={day.toISOString()}
                                        type="button"
                                        onClick={() => commitDate(day)}
                                        className={`h-8 rounded-lg text-ui-sm font-bold transition-colors ${
                                            selected
                                                ? 'bg-primary text-primary-foreground shadow'
                                                : today
                                                  ? 'ring-1 ring-primary/50 text-primary hover:bg-primary/10'
                                                  : inMonth
                                                    ? 'text-foreground hover:bg-surface-hover'
                                                    : 'text-muted-foreground/40 hover:bg-surface-hover'
                                        }`}
                                    >
                                        {format(day, 'd')}
                                    </button>
                                );
                            })}
                        </div>

                        <div className="flex flex-col items-center gap-1.5 pt-2 border-t border-border/60">
                            <span className="text-ui-caption font-black text-muted-foreground uppercase tracking-tight">
                                {t('search.expiration.time')}
                            </span>
                            <div className="flex items-center justify-center gap-2">
                                <input
                                    type="number"
                                    min={0}
                                    max={23}
                                    value={pad(hours)}
                                    onChange={(e) => onHours(e.target.value)}
                                    className="time-input w-12 h-9 rounded-[8px] border border-border text-center text-ui-base font-black text-foreground outline-none focus:border-foreground/30 [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                                    aria-label="HH"
                                />
                                <span className="text-foreground font-black">:</span>
                                <input
                                    type="number"
                                    min={0}
                                    max={59}
                                    value={pad(minutes)}
                                    onChange={(e) => onMinutes(e.target.value)}
                                    className="time-input w-12 h-9 rounded-[8px] border border-border text-center text-ui-base font-black text-foreground outline-none focus:border-foreground/30 [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                                    aria-label="MM"
                                />
                            </div>
                        </div>

                        {onApply && (
                            <Button
                                type="button"
                                variant="primary"
                                disabled={disabled || applying}
                                onClick={() => {
                                    onApply(currentValue);
                                    setOpen(false);
                                }}
                                className="w-full h-[38px] text-ui-base font-bold"
                            >
                                {applying ? t('search.rotation.applyingBtn') : t('search.rotation.applyBtn')}
                            </Button>
                        )}
                    </div>,
                    document.body,
                )}
        </div>
    );
}
