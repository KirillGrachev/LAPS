import React from 'react';
import { ArrowLeft } from 'lucide-react';
import { TabWrapper } from './TabWrapper';

interface PageLayoutProps {
    id: string;
    title: string;
    subtitle?: string;
    rightElement?: React.ReactNode;
    /** Кнопка «назад» слева от заголовка, как в PhoneBook. */
    onBack?: () => void;
    backLabel?: string;
    children: React.ReactNode;
    className?: string;
    contentClassName?: string;
}

export function PageLayout({
    id,
    title,
    subtitle,
    rightElement,
    onBack,
    backLabel = 'Назад',
    children,
    className = '',
    contentClassName = '',
}: PageLayoutProps) {
    return (
        <TabWrapper id={id} className={`w-full h-full flex flex-col overflow-hidden ${className}`}>
            <div className="bg-surface p-[20px] pl-[56px] lg:pl-[28px] text-foreground flex items-center justify-between shrink-0 transition-colors border-b border-border">
                <div className="flex items-center gap-3 min-w-0">
                    {onBack && (
                        <button
                            type="button"
                            onClick={onBack}
                            aria-label={backLabel}
                            title={backLabel}
                            className="w-9 h-9 -ml-2 rounded-lg shrink-0 flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-foreground/5 transition-colors"
                        >
                            <ArrowLeft size={18} strokeWidth={2.4} />
                        </button>
                    )}
                    <div className="flex flex-col gap-1 min-w-0">
                        <h1 className="text-ui-2xl font-bold tracking-tight text-foreground truncate">{title}</h1>
                        {subtitle && (
                            <p className="text-ui-label font-medium text-muted-foreground truncate">{subtitle}</p>
                        )}
                    </div>
                </div>
                {rightElement && (
                    <div className="hidden sm:flex items-center gap-[10px] uppercase tracking-normal text-ui-base font-bold text-muted-foreground">
                        {rightElement}
                    </div>
                )}
            </div>

            <div className={`flex-1 flex flex-col relative min-h-0 bg-background ${contentClassName}`}>{children}</div>
        </TabWrapper>
    );
}
