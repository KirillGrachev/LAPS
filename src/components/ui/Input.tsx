import { InputHTMLAttributes, forwardRef } from 'react';

export type InputProps = InputHTMLAttributes<HTMLInputElement>;

/**
 * Поле ввода как в PhoneBook: серый фон и рамка, без осветления
 * при наведении/фокусе. Фокус обозначается только рамкой.
 */
export const Input = forwardRef<HTMLInputElement, InputProps>(({ className = '', readOnly, ...props }, ref) => {
    const baseClasses =
        'bg-[#e2e8f0] dark:bg-white/[0.08] px-3 py-2 border border-border rounded-[10px] text-[15px] font-normal outline-none transition-colors duration-200 focus:border-foreground/30 disabled:opacity-60 disabled:cursor-not-allowed text-[#1e293b] dark:text-white placeholder:text-muted-foreground';

    return <input ref={ref} readOnly={readOnly} className={`${baseClasses} ${className}`} {...props} />;
});

Input.displayName = 'Input';
