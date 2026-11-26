interface SwitchProps {
    checked: boolean;
    onChange: (value: boolean) => void;
    disabled?: boolean;
    id?: string;
    ariaLabel?: string;
    /** Янтарный акцент для тестового режима, синий - для обычных настроек. */
    tone?: 'primary' | 'amber';
}

/**
 * Переключатель-«пилюля» в стилистике телефонного справочника КМАруда:
 * синий в активном состоянии, белый круг-ручка, без обводок.
 */
export function Switch({ checked, onChange, disabled, id, ariaLabel, tone = 'primary' }: SwitchProps) {
    return (
        <button
            type="button"
            role="switch"
            aria-checked={checked}
            aria-label={ariaLabel}
            id={id}
            disabled={disabled}
            onClick={() => onChange(!checked)}
            className={`relative inline-flex w-[44px] h-[24px] shrink-0 items-center rounded-full border transition-colors duration-300 outline-none focus-visible:ring-2 focus-visible:ring-primary/40 ${
                checked
                    ? tone === 'amber'
                        ? 'border-amber-500 bg-amber-500'
                        : 'border-primary bg-primary'
                    : 'border-border bg-[#cbd5e1] dark:bg-white/15'
            } ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}`}
        >
            <span
                className={`inline-block h-[18px] w-[18px] rounded-full bg-white shadow transition-transform duration-300 ${
                    checked ? 'translate-x-[21px]' : 'translate-x-[3px]'
                }`}
            />
        </button>
    );
}
