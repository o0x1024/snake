import React, { useState, useEffect, createContext, useContext, ReactNode } from 'react';
import i18n from 'i18next';
import { useTranslation } from 'react-i18next';

export type Language = 'zh' | 'en';

interface LanguageContextType {
  language: Language;
  setLanguage: (language: Language) => void;
  t: (key: string) => string;
}

const LanguageContext = createContext<LanguageContextType | undefined>(undefined);

export function LanguageProvider({ children }: { children: ReactNode }) {
  const { t, i18n: i18nextInstance } = useTranslation();
  const [language, setLanguageState] = useState<Language>(() => {
    const saved = localStorage.getItem('language') as Language | null;
    return (saved || (i18n.language as Language) || 'zh');
  });

  // 同步语言到 i18next、localStorage 和 <html lang>
  useEffect(() => {
    const changeLanguage = async () => {
      try {
        await i18nextInstance.changeLanguage(language);
        localStorage.setItem('language', language);
        document.documentElement.setAttribute('lang', language);
        console.log('Language changed to:', language);
      } catch (error) {
        console.error('Failed to change language:', error);
      }
    };
    changeLanguage();
  }, [language, i18nextInstance]);

  const setLanguage = (newLanguage: Language) => {
    console.log('Setting language to:', newLanguage);
    setLanguageState(newLanguage);
  };

  // 为保持接口兼容性，仍然提供t函数
  const translate = (key: string): string => {
    return t(key);
  };

  return React.createElement(
    LanguageContext.Provider,
    { value: { language, setLanguage, t: translate } },
    children
  );
}

export function useLanguage() {
  const context = useContext(LanguageContext);
  if (context === undefined) {
    throw new Error('useLanguage must be used within a LanguageProvider');
  }
  return context;
}