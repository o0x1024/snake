import { useState } from 'react';
import { useLanguage } from '../utils/LanguageManager';
import { useTranslation } from 'react-i18next';
import CommandExecutor from './CommandExecutor';
import FileManager from './FileManager';
import { SessionTab } from '../App';

interface SessionWorkspaceProps {
  sessionTab: SessionTab;
  onClose: (tabId: string) => void;
}

export default function SessionWorkspace({ sessionTab, onClose }: SessionWorkspaceProps) {
  const { t } = useLanguage();
  const { t: translate } = useTranslation();
  const [activeTab, setActiveTab] = useState<'console' | 'files'>('console');

  // 获取翻译函数
  const getTranslation = (key: string): string => {
    return translate(key) !== key ? translate(key) : t(key);
  };

  return (
    <div className="h-full min-h-0 flex flex-col bg-base-100 rounded-lg shadow-lg">
      {/* Session Header */}
      <div className="flex items-center justify-between p-4 border-b border-base-200">
        <div className="flex items-center space-x-3">
          <div className="w-3 h-3 bg-success rounded-full animate-pulse"></div>
          <h2 className="text-lg font-semibold">{sessionTab.title}</h2>
          <code className="text-xs bg-base-200 px-2 py-1 rounded">{sessionTab.sessionId}</code>
        </div>
        <button
          onClick={() => onClose(sessionTab.id)}
          className="btn btn-ghost btn-sm btn-circle"
          title={getTranslation('close') || 'Close'}
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Tab Navigation */}
      <div className="tabs tabs-lifted bg-base-100 px-4">
        <button
          onClick={() => setActiveTab('console')}
          className={`tab tab-lg ${activeTab === 'console' ? 'tab-active' : ''}`}
        >
          <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
          </svg>
          {getTranslation('console')}
        </button>
        <button
          onClick={() => setActiveTab('files')}
          className={`tab tab-lg ${activeTab === 'files' ? 'tab-active' : ''}`}
        >
          <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
          </svg>
          {getTranslation('files')}
        </button>
      </div>

      {/* Content Area */}
      <div className="flex-1 min-h-0 overflow-hidden p-4">
        {activeTab === 'console' && (
          <div className="h-full min-h-0">
            <CommandExecutor sessionId={sessionTab.sessionId} />
          </div>
        )}

        {activeTab === 'files' && (
          <div className="h-full min-h-0">
            <FileManager sessionId={sessionTab.sessionId} />
          </div>
        )}
      </div>
    </div>
  );
}