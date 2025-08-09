import { useEffect, useState } from "react";
import { ThemeProvider } from "./utils/ThemeManager";
import { LanguageProvider } from "./utils/LanguageManager";
import Navbar from "./components/Navbar";
import SessionManager from "./components/SessionManager";
import "./App.css";
import './utils/i18n'; // 初始化i18n

export interface SessionTab {
  id: string;
  sessionId: string;
  title: string;
  isActive: boolean;
}

function App() {
  const [sessionTabs, setSessionTabs] = useState<SessionTab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);

  useEffect(() => {
  }, []);

  const openSessionTab = (sessionId: string, sessionTitle: string) => {
    const tabId = `session-${sessionId}-${Date.now()}`;
    const newTab: SessionTab = {
      id: tabId,
      sessionId,
      title: sessionTitle,
      isActive: true
    };
    
    setSessionTabs(prev => [
      ...prev.map(tab => ({ ...tab, isActive: false })),
      newTab
    ]);
    setActiveTabId(tabId);
  };

  const closeSessionTab = (tabId: string) => {
    setSessionTabs(prev => {
      const filtered = prev.filter(tab => tab.id !== tabId);
      if (filtered.length > 0 && activeTabId === tabId) {
        const newActiveTab = filtered[filtered.length - 1];
        newActiveTab.isActive = true;
        setActiveTabId(newActiveTab.id);
      } else if (filtered.length === 0) {
        setActiveTabId(null);
      }
      return filtered;
    });
  };

  const switchToTab = (tabId: string) => {
    if (tabId === '') {
      // 返回会话列表，隐藏所有活动session但不关闭连接
      setSessionTabs(prev => prev.map(tab => ({
        ...tab,
        isActive: false
      })));
      setActiveTabId(null);
    } else {
      setSessionTabs(prev => prev.map(tab => ({
        ...tab,
        isActive: tab.id === tabId
      })));
      setActiveTabId(tabId);
    }
  };

  const activeTab = sessionTabs.find(tab => tab.id === activeTabId);

  // 直接显示主界面
  return (
    <ThemeProvider>
      <LanguageProvider>
        <div className="h-screen w-screen flex flex-col bg-base-200">
          <div className="shrink-0">
            <Navbar 
            />
          </div>
          {/* 管理会话按钮区域 - 放在活动会话前面 */}
          {activeTab && (
            <div className="shrink-0 px-2 sm:px-3 md:px-4 lg:px-6 pt-2">
              <div className="flex items-center bg-base-100 rounded-lg p-3 border border-base-300">
                <div className="flex items-center gap-3 flex-1 min-w-0">
                  <button
                    onClick={() => switchToTab('')}
                    className="btn btn-outline btn-primary btn-sm"
                  >
                    <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M19 12H5M12 19l-7-7 7-7" />
                    </svg>
                    管理会话
                  </button>
                  <div className="divider divider-horizontal"></div>
                  {sessionTabs.length > 1 ? (
                    <div className="flex items-center gap-2 overflow-x-auto flex-1 min-w-0" style={{ scrollbarWidth: 'thin' }}>
                      <span className="text-sm font-medium whitespace-nowrap">活动会话:</span>
                      <div className="flex gap-1">
                        {sessionTabs.map((tab) => (
                          <div
                            key={tab.id}
                            className={`btn btn-sm flex items-center gap-1 shrink-0 p-0 ${tab.isActive ? 'btn-primary' : 'btn-ghost'}`}
                            title={tab.title}
                          >
                            <button
                              onClick={() => switchToTab(tab.id)}
                              className="flex items-center gap-1 px-2 py-1 flex-1 min-w-0 hover:bg-transparent"
                            >
                              <div className="w-2 h-2 bg-success rounded-full shrink-0"></div>
                              <span className="truncate text-xs">{tab.title}</span>
                            </button>
                            <button
                              onClick={(e) => { e.stopPropagation(); closeSessionTab(tab.id); }}
                              className="flex items-center justify-center w-5 h-5 shrink-0 hover:bg-base-300 rounded-sm transition-colors opacity-60 hover:opacity-100"
                              title="关闭标签"
                            >
                              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                              </svg>
                            </button>
                          </div>
                        ))}
                      </div>
                    </div>
                  ) : (
                    <div className="flex items-center gap-2">
                      <div className="w-2 h-2 bg-success rounded-full animate-pulse"></div>
                      <span className="text-sm font-medium">活动会话:</span>
                      <span className="text-sm">{activeTab.title}</span>
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
          <main className="flex-1 min-h-0 overflow-hidden p-2 sm:p-3 md:p-4 lg:px-6">
            <div className="h-full min-h-0">
              <SessionManager 
                sessionTabs={sessionTabs}
                activeTab={sessionTabs.find(tab => tab.isActive)}
                onOpenSession={openSessionTab}
                onCloseTab={closeSessionTab}
                onSwitchTab={switchToTab}
              />
            </div>
          </main>
        </div>
      </LanguageProvider>
    </ThemeProvider>
  );
}

export default App;