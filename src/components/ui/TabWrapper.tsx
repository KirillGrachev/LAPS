import { motion } from 'motion/react';
import { ReactNode } from 'react';

interface TabWrapperProps {
    children: ReactNode;
    id: string;
    className?: string;
}

/**
 * Обёртка вкладки с мягким появлением. Вход в приложение (LoginScreen)
 * сюда не входит: там анимация «подъёма» намеренно убрана.
 */
export function TabWrapper({ children, id, className = '' }: TabWrapperProps) {
    return (
        <motion.div
            key={id}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.22, ease: 'easeOut' }}
            className={`bg-background overflow-hidden flex flex-col ${className}`}
        >
            {children}
        </motion.div>
    );
}
