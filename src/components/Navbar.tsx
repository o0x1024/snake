import { useState, useRef, useEffect } from 'react';
import { useTheme } from '../utils/ThemeManager';
import { useLanguage } from '../utils/LanguageManager';
import { useTranslation } from 'react-i18next';

interface NavbarProps {
  isInSessionWorkspace?: boolean;
  onReturnToSessionList?: () => void;
}

export default function Navbar( ) {
  const { theme, toggleTheme } = useTheme();
  const { language, setLanguage, t } = useLanguage();
  const { t: translate } = useTranslation(); // i18next hook
  const [isDropdownOpen, setIsDropdownOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const langDropdownRef = useRef<HTMLDivElement>(null);
  const settingsDropdownRef = useRef<HTMLDivElement>(null);

  // 使用i18next翻译函数作为备选
  const getTranslation = (key: string): string => {
    return translate(key) !== key ? translate(key) : t(key);
  };

  // 处理点击外部区域关闭下拉菜单
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (langDropdownRef.current && !langDropdownRef.current.contains(event.target as Node)) {
        setIsDropdownOpen(false);
      }
      if (settingsDropdownRef.current && !settingsDropdownRef.current.contains(event.target as Node)) {
        setIsSettingsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, []);

  // 切换语言
  const handleLanguageChange = (lang: 'zh' | 'en') => {
    setLanguage(lang);
    setIsDropdownOpen(false);
  };

  // 切换主题
  const handleThemeToggle = () => {
    toggleTheme();
    setIsSettingsOpen(false);
  };

  return (
    <div className="navbar min-h-0 py-1 bg-base-100/80 backdrop-blur supports-[backdrop-filter]:bg-base-100/70 border-b border-base-300">
      <div className="navbar-start">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-md  text-primary-content flex items-center justify-center overflow-hidden">
            <img src="/tauri.png" alt="app-logo" className="w-5 h-5" />
          </div>
          <div className="leading-tight">
            <h1 className="text-sm md:text-base font-semibold">{getTranslation('appTitle')}</h1>
            <p className="text-xs opacity-70">{getTranslation('appSubtitle')}</p>
          </div>
        </div>
      </div>

      <div className="navbar-end">
        <div className="flex items-center gap-1 md:gap-1.5">
          {/* 管理会话按钮 - 只在会话工作区显示 */}

          {/* Language Selector */}
          <div className="dropdown dropdown-end" ref={langDropdownRef}>
            <label
              tabIndex={0} 
              className="btn btn-ghost btn-xs md:btn-sm"
              onClick={(e) => {
                e.stopPropagation();
                setIsDropdownOpen(!isDropdownOpen);
              }}
            >
              <svg className="w-4 h-4 mr-1" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="10"/>
                <path d="M2 12h20M12 2a15.3 15.3 0 010 20"/>
              </svg>
              {language === 'zh' ? '中文' : 'English'}
            </label>
            {isDropdownOpen && (
              <ul 
                tabIndex={0} 
                className="dropdown-content menu p-2 shadow bg-base-100 rounded-box w-32 z-50"
              >
                <li>
                  <button
                    onClick={() => handleLanguageChange('zh')}
                    className={language === 'zh' ? 'active' : ''}
                  >
                    中文
                  </button>
                </li>
                <li>
                  <button
                    onClick={() => handleLanguageChange('en')}
                    className={language === 'en' ? 'active' : ''}
                  >
                    English
                  </button>
                </li>
              </ul>
            )}
          </div>

          {/* Theme Toggle */}
          <button
            onClick={handleThemeToggle}
            className="btn btn-ghost btn-xs md:btn-sm"
          >
            {theme === 'light' ? (
              <>
                <svg className="w-4 h-4 md:w-5 md:h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/>
                </svg>
                <span className="ml-1">{getTranslation('switchToDark')}</span>
              </>
            ) : (
              <>
                <svg className="w-4 h-4 md:w-5 md:h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="12" cy="12" r="5"/>
                  <path d="M12 1v2m0 18v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2m18 0h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/>
                </svg>
                <span className="ml-1">{getTranslation('switchToLight')}</span>
              </>
            )}
          </button>

          {/* Settings Dropdown */}
          <div className="dropdown dropdown-end" ref={settingsDropdownRef}>
            <label
              tabIndex={0} 
              className="btn btn-ghost btn-xs md:btn-sm"
              onClick={(e) => {
                e.stopPropagation();
                setIsSettingsOpen(!isSettingsOpen);
              }}
            >
              <svg className="w-4 h-4 md:w-5 md:h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="3"/>
                <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 01-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V22a2 2 0 01-4 0v-.09A1.65 1.65 0 008 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.6 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 8a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 112.83-2.83l.06.06A1.65 1.65 0 008 4.6a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09A1.65 1.65 0 0016 4.6a1.65 1.65 0 001.82-.33l.06-.06a2 2 0 112.83 2.83l-.06.06A1.65 1.65 0 0019.4 9c.36 0 .7.1 1 .26"/>
              </svg>
            </label>
            {isSettingsOpen && (
              <ul 
                tabIndex={0} 
                className="dropdown-content menu p-2 shadow bg-base-100 rounded-box w-52 z-50"
              >
                <li>
                  <button onClick={handleThemeToggle}>
                    {theme === 'light' ? (
                      <>
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
                        </svg>
                        {getTranslation('switchToDark')}
                      </>
                    ) : (
                      <>
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
                        </svg>
                        {getTranslation('switchToLight')}
                      </>
                    )}
                  </button>
                </li>
                <li>
                  <button onClick={() => setIsSettingsOpen(false)}>
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    {getTranslation('help')}
                  </button>
                </li>
                <li>
                  <button onClick={() => setIsSettingsOpen(false)}>
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    {getTranslation('about')}
                  </button>
                </li>
              </ul>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}