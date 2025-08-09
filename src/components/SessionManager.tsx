import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useLanguage } from '../utils/LanguageManager';
import { useTranslation } from 'react-i18next';
import SessionWorkspace from './SessionWorkspace';
import PluginManager from './PluginManager';
import WebshellGenerator from './WebshellGenerator';
import { SessionTab } from '../App';

interface Session {
  id: string;
  target: string;
  status: 'active' | 'inactive' | 'error';
  lastContact: string;
  encryption: string;
  uptime: number;
}

interface SessionManagerProps {
  sessionTabs: SessionTab[];
  activeTab?: SessionTab;
  onOpenSession: (sessionId: string, sessionTitle: string) => void;
  onCloseTab: (tabId: string) => void;
  onSwitchTab: (tabId: string) => void;
}

export default function SessionManager({
  sessionTabs,
  activeTab,
  onOpenSession,
  onCloseTab,
  onSwitchTab
}: SessionManagerProps) {
  const { t } = useLanguage();
  const { t: translate } = useTranslation();
  const [sessions, setSessions] = useState<Session[]>([]);
  const [currentView, setCurrentView] = useState<'sessions' | 'plugins' | 'generator'>('sessions');
  const [isLoading, setIsLoading] = useState(true);
  const [showNewSession, setShowNewSession] = useState(false);
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('list');
  const [newSessionConfig, setNewSessionConfig] = useState({
    target: '',
    encryption: 'none',
    proxy: '',
    secret: ''
  });
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);

  // 获取翻译函数
  const getTranslation = (key: string): string => {
    return translate(key) !== key ? translate(key) : t(key);
  };

  useEffect(() => {
    loadSessions();
    const interval = setInterval(loadSessions, 5000); // Refresh every 5 seconds
    return () => clearInterval(interval);
  }, []);

  const loadSessions = async () => {
    try {
      const sessionList = await invoke('get_active_sessions', {
        token: 'dev-token-1234'
      });
      setSessions(sessionList as Session[]);
    } catch (error) {
      console.error('Failed to load sessions:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const createSession = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      if (editingSessionId) {
        // 编辑现有会话
        await invoke('update_session', {
          token: 'dev-token-1234',
          sessionId: editingSessionId,
          target: newSessionConfig.target || null,
          encryption: newSessionConfig.encryption || null,
        });

        // 如果提供了新密码，重新配置webshell
        if (newSessionConfig.secret && newSessionConfig.secret.trim()) {
          try {
            await invoke('configure_webshell', {
              session_id: editingSessionId,
              sessionId: editingSessionId,
              config: {
                endpoint: newSessionConfig.target,
                password: newSessionConfig.secret,
                charset: null,
                timeout_ms: null,
              },
            });
          } catch (cfgErr) {
            console.error('Failed to configure webshell driver:', cfgErr);
            alert('会话更新成功，但webshell配置失败: ' + cfgErr);
          }
        }
        alert('会话更新成功！');
      } else {
        // 创建新会话
        const newId = await invoke('create_session', {
          token: 'dev-token-1234',
          config: {
            ...newSessionConfig,
            secret: newSessionConfig.secret ? newSessionConfig.secret : undefined,
          }
        }) as string;

        // Configure driver for immediate use (secret kept only in backend memory)
        if (newSessionConfig.secret && newSessionConfig.secret.trim()) {
          try {
            await invoke('configure_webshell', {
              session_id: newId,
              sessionId: newId,
              config: {
                endpoint: newSessionConfig.target,
                password: newSessionConfig.secret,
                charset: null,
                timeout_ms: null,
              },
            });
          } catch (cfgErr) {
            console.error('Failed to configure webshell driver:', cfgErr);
          }
        }
      }

      setShowNewSession(false);
      setEditingSessionId(null);
      setNewSessionConfig({ target: '', encryption: 'aes-256-gcm', proxy: '', secret: '' });
      loadSessions();
    } catch (error) {
      console.error('Failed to create/update session:', error);
      alert('操作失败: ' + error);
    }
  };

  const connectToSession = (session: Session) => {
    const sessionTitle = `${session.target} (${session.id.slice(0, 8)})`;
    onOpenSession(session.id, sessionTitle);
  };

  const terminateSession = async (sessionId: string) => {
    if (!confirm(getTranslation('confirmTerminate') || 'Are you sure you want to terminate this session?')) return;

    try {
      await invoke('terminate_session', {
        token: 'dev-token-1234',
        session_id: sessionId
      });
      loadSessions();
    } catch (error) {
      console.error('Failed to terminate session:', error);
    }
  };

  const editSession = async (sessionId: string) => {
    // 找到要编辑的session
    const sessionToEdit = sessions.find(s => s.id === sessionId);
    if (!sessionToEdit) return;

    // 设置编辑模式的配置
    setNewSessionConfig({
      target: sessionToEdit.target,
      encryption: sessionToEdit.encryption,
      proxy: '', // 代理信息可能不在session对象中
      secret: '' // 密码不显示
    });
    setEditingSessionId(sessionId);
    setShowNewSession(true);
  };

  const deleteSession = async (sessionId: string) => {
    console.log('删除会话开始:', sessionId);
    
    // if (!confirm('确定要删除这个会话吗？这将永久删除会话记录。')) {
    //   console.log('用户取消删除操作');
    //   return;
    // }
  
    try {
      console.log('开始删除会话:', sessionId);
      
      // 关闭对应的标签页
      const tabToClose = sessionTabs.find(tab => tab.sessionId === sessionId);
      if (tabToClose) {
        console.log('关闭标签页:', tabToClose.id);
        onCloseTab(tabToClose.id);
      }
  
      // 删除会话
      console.log('调用后端删除API...');
      const result = await invoke('delete_session', {
        token: 'dev-token-1234',
        sessionId: sessionId
      });
      
      console.log('删除API调用成功:', result);
      alert('会话删除成功！');
      
      // 重新加载会话列表
      console.log('重新加载会话列表...');
      await loadSessions();
      console.log('会话列表重新加载完成');
      
    } catch (error) {
      console.error('删除会话失败 - 详细错误:', error);
      console.error('错误类型:', typeof error);
      console.error('错误字符串:', String(error));
      
      // 尝试解析错误信息
      let errorMessage = '未知错误';
      if (typeof error === 'string') {
        errorMessage = error;
      } else if (error && typeof error === 'object') {
        errorMessage = String(error);
      }
      
      alert(`删除会话失败: ${errorMessage}`);
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'active': return 'badge-success';
      case 'inactive': return 'badge-warning';
      case 'error': return 'badge-error';
      default: return 'badge-ghost';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'active': return (
        <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
        </svg>
      );
      case 'inactive': return (
        <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clipRule="evenodd" />
        </svg>
      );
      case 'error': return (
        <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
        </svg>
      );
      default: return (
        <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16z" clipRule="evenodd" />
        </svg>
      );
    }
  };

  const formatUptime = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${minutes}m`;
  };

  const renderCardActions = (session: Session) => (
    <div className="dropdown dropdown-end">
      <label tabIndex={0} className="btn btn-ghost btn-xs btn-circle">
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 5v.01M12 12v.01M12 19v.01" />
        </svg>
      </label>
      <ul tabIndex={0} className="dropdown-content menu p-2 shadow bg-base-100 rounded-box w-52">
        <li>
          <button onClick={(e) => { e.stopPropagation(); connectToSession(session); }}>
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 9l3 3-3 3m5 0h3" /></svg>
            {getTranslation('connect') || 'Connect'}
          </button>
        </li>
        <li>
          <button className="text-error" onClick={(e) => { e.stopPropagation(); deleteSession(session.id); }}>
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1-1H8a1 1 0 00-1 1v3M4 7h16" /></svg>
            删除会话
          </button>
        </li>
        <li>
          <button onClick={(e) => { e.stopPropagation(); editSession(session.id); }}>
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" /></svg>
            编辑会话
          </button>
        </li>
      </ul>
    </div>
  );

  const renderCardInfo = (session: Session) => (
    <div className="space-y-2">
      <div className="flex items-center space-x-2"><span className="text-sm opacity-70">{getTranslation('target')}:</span><code className="text-sm bg-base-200 px-2 py-1 rounded">{session.target}</code></div>
      <div className="flex items-center space-x-2"><span className="text-sm opacity-70">{getTranslation('encryption')}:</span><span className="badge badge-success badge-sm">{session.encryption}</span></div>
      <div className="flex items-center space-x-2"><span className="text-sm opacity-70">{getTranslation('lastContact')}:</span><span className="text-sm">{session.lastContact}</span></div>
      <div className="flex items-center space-x-2"><span className="text-sm opacity-70">{getTranslation('uptime')}:</span><span className="text-sm font-mono">{formatUptime(session.uptime)}</span></div>
    </div>
  );

  const renderCardButtons = (session: Session) => (
    <div className="card-actions justify-end mt-2">
      <button
        className="btn btn-primary btn-sm"
        onClick={(e) => { e.stopPropagation(); connectToSession(session); }}
      >
        <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
        </svg>
        {getTranslation('connect') || 'Connect'}
      </button>
    </div>
  );

  if (isLoading) {
    return (
      <div className="w-full h-full grid place-items-center">
        <div className="text-center">
          <span className="loading loading-dots loading-lg text-primary"></span>
          <p className="mt-3 text-sm opacity-70">{getTranslation('loadingSessions') || 'Loading sessions...'}</p>
        </div>
      </div>
    );
  }

  // 如果有活动的session tab，显示工作区
  if (activeTab) {
    return (
      <div className="h-full min-h-0 flex flex-col">
        {/* Active Session Workspace */}
        <div className="flex-1 min-h-0">
          <SessionWorkspace
            sessionTab={activeTab}
            onClose={onCloseTab}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="h-full min-h-0 flex flex-col">
      <div className="flex justify-between items-center mb-2">
        <div className="flex items-center gap-2">
          <span className="text-sm opacity-70">{getTranslation('total') || 'Total'}: {sessions.length}</span>
          <button
            className="btn btn-ghost btn-xs"
            title={viewMode === 'grid' ? getTranslation('switchToList') || 'Switch to List' : getTranslation('switchToGrid') || 'Switch to Grid'}
            onClick={() => setViewMode(viewMode === 'grid' ? 'list' : 'grid')}
          >
            {viewMode === 'grid' ? (
              <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" />
              </svg>
            ) : (
              <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="3" width="7" height="7" /><rect x="14" y="3" width="7" height="7" /><rect x="14" y="14" width="7" height="7" /><rect x="3" y="14" width="7" height="7" />
              </svg>
            )}
          </button>
        </div>
        <div className="flex gap-2">
          {/* <button
            onClick={() => setCurrentView('plugins')}
            className="btn btn-ghost btn-xs md:btn-sm"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
            </svg>
            {getTranslation('pluginManager')}
          </button> */}
          {/* <button
            onClick={() => setCurrentView('generator')}
            className="btn btn-ghost btn-xs md:btn-sm"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
            </svg>
            {getTranslation('webshellGenerator')}
          </button> */}
          <button
            onClick={() => setShowNewSession(true)}
            className="btn btn-primary btn-xs md:btn-sm"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
            </svg>
            {getTranslation('newSession')}
          </button>
        </div>
      </div>

      {/* Navigation Tabs */}
      <div className="tabs tabs-lifted bg-base-100 shadow mb-2 overflow-x-auto shrink-0">
        <button
          onClick={() => setCurrentView('sessions')}
          className={`tab tab-lg ${currentView === 'sessions' ? 'tab-active' : ''}`}
        >
          <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4M7.835 4.697a3.42 3.42 0 001.946-.806 3.42 3.42 0 014.438 0 3.42 3.42 0 001.946.806 3.42 3.42 0 013.138 3.138 3.42 3.42 0 00.806 1.946 3.42 3.42 0 010 4.438 3.42 3.42 0 00-.806 1.946 3.42 3.42 0 01-3.138 3.138 3.42 3.42 0 00-1.946.806 3.42 3.42 0 01-4.438 0 3.42 3.42 0 00-1.946-.806 3.42 3.42 0 01-3.138-3.138 3.42 3.42 0 00-.806-1.946 3.42 3.42 0 010-4.438 3.42 3.42 0 00.806-1.946 3.42 3.42 0 013.138-3.138z" />
          </svg>
          {getTranslation('sessions')}
        </button>
        <button
          onClick={() => setCurrentView('plugins')}
          className={`tab tab-lg ${currentView === 'plugins' ? 'tab-active' : ''}`}
        >
          <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
          {getTranslation('plugins')}
        </button>
        <button
          onClick={() => setCurrentView('generator')}
          className={`tab tab-lg ${currentView === 'generator' ? 'tab-active' : ''}`}
        >
          <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
          {getTranslation('webshellGenerator')}
        </button>
      </div>

      {/* Content Area */}
      <div className="flex-1 min-h-0 overflow-hidden">
        <div className="h-full min-h-0 flex flex-col">
          {currentView === 'sessions' && (
            <>
              {/* 已连接的Session快速访问 */}
              {sessionTabs.length > 0 && (
                <div className="bg-primary/10 border border-primary/20 rounded-lg p-3 mb-4">
                  <div className="flex items-center justify-between mb-2">
                    <h3 className="text-sm font-medium text-primary flex items-center gap-2">
                      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
                      </svg>
                      已连接的会话
                    </h3>
                    <span className="text-xs opacity-70">{sessionTabs.length} 个活动连接</span>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    {sessionTabs.map((tab) => (
                      <div key={tab.id} className="flex items-center gap-1">
                        <button
                          onClick={() => onSwitchTab(tab.id)}
                          className="btn btn-sm btn-primary flex items-center gap-2"
                        >
                          <div className="w-2 h-2 bg-success rounded-full"></div>
                          {tab.title}
                          <span className="text-xs opacity-70">({tab.sessionId.slice(0, 8)}...)</span>
                        </button>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            onCloseTab(tab.id);
                          }}
                          className="btn btn-sm btn-error btn-outline flex items-center gap-1"
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
              )}
              {/* Session Grid/List */}
              {viewMode === 'grid' ? (
                <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4 gap-2 md:gap-3 xl:gap-4 mb-3 md:mb-4 xl:mb-6 flex-1 min-h-0 overflow-auto pr-1">
                  {sessions.map((session) => (
                    <div
                      key={session.id}
                      className="card bg-base-100 border border-base-200 hover:border-primary/40 shadow-sm hover:shadow-md transition-all"
                    >
                      <div className="card-body p-4 md:p-5">
                        <div className="flex items-center justify-between mb-4">
                          <div className={`badge ${getStatusBadge(session.status)} gap-2`}>
                            {getStatusIcon(session.status)}
                            {session.status.toUpperCase()}
                          </div>
                          {renderCardActions(session)}
                        </div>
                        {renderCardInfo(session)}
                        {renderCardButtons(session)}
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="overflow-auto flex-1 min-h-0">
                  <table className="table table-zebra w-full">
                    <thead>
                      <tr>
                        <th>{getTranslation('status')}</th>
                        <th>{getTranslation('sessionId')}</th>
                        <th>{getTranslation('target')}</th>
                        <th>{getTranslation('encryption')}</th>
                        <th>{getTranslation('lastContact')}</th>
                        <th>{getTranslation('uptime')}</th>
                        <th className="text-right">{getTranslation('actions') || 'Actions'}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {sessions.map((s) => (
                        <tr key={s.id} className="hover:bg-base-200">
                          <td>
                            <div className={`badge ${getStatusBadge(s.status)} gap-2`}>{getStatusIcon(s.status)}{s.status.toUpperCase()}</div>
                          </td>
                          <td><code className="text-xs">{s.id}</code></td>
                          <td><code className="text-xs">{s.target}</code></td>
                          <td>{s.encryption}</td>
                          <td>{s.lastContact}</td>
                          <td className="font-mono text-sm">{formatUptime(s.uptime)}</td>
                          <td>
                            <div className="flex justify-end gap-2">
                              <button
                                className="btn btn-primary btn-xs"
                                onClick={(e) => { e.stopPropagation(); connectToSession(s); }}
                              >
                                {getTranslation('connect') || 'Connect'}
                              </button>
                              <button className="btn btn-secondary btn-xs" onClick={(e) => { e.stopPropagation(); editSession(s.id); }}>编辑</button>
                              <button className="btn btn-error btn-xs" onClick={(e) => { e.stopPropagation(); deleteSession(s.id); }}>删除</button>
                            </div>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}

              {sessions.length === 0 && (
                <div className="hero bg-base-100 rounded-box shadow-lg">
                  <div className="hero-content text-center">
                    <div className="max-w-md">
                      <div className="text-6xl mb-4">🔌</div>
                      <h1 className="text-3xl font-bold">No Active Sessions</h1>
                      <p className="py-6">Create a new session to start managing your webshell connections</p>
                      <button
                        onClick={() => setShowNewSession(true)}
                        className="btn btn-primary btn-lg"
                      >
                        <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                        </svg>
                        Create First Session
                      </button>
                    </div>
                  </div>
                </div>
              )}
            </>
          )}
          {currentView === 'generator' && (
            <div className="flex-1 min-h-0 overflow-auto">
              <div className="max-w-5xl mx-auto">
                <WebshellGenerator />
              </div>
            </div>
          )}

          {currentView === 'plugins' && (
            <div className="h-full min-h-0">
              <PluginManager sessionId={''} />
            </div>
          )}
        </div>
      </div>

      {/* New Session Modal */}
      {showNewSession && (
        <div className="modal modal-open">
          <div className="modal-box">
            <h3 className="font-bold text-lg mb-4 flex items-center">
              <svg className="w-6 h-6 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
              {editingSessionId ? '编辑会话' : '创建新会话'}
            </h3>
            <form onSubmit={createSession} className="space-y-4">
              <div className="form-control">
                <label className="label">
                  <span className="label-text">Target URL</span>
                </label>
                <input
                  type="url"
                  value={newSessionConfig.target}
                  onChange={(e) => setNewSessionConfig({ ...newSessionConfig, target: e.target.value })}
                  className="input input-bordered w-full"
                  placeholder="https://target.example.com/shell.php"
                  required
                />
              </div>

              <div className="form-control">
                <label className="label">
                  <span className="label-text">Encryption Method</span>
                </label>
                <select
                  value={newSessionConfig.encryption}
                  onChange={(e) => setNewSessionConfig({ ...newSessionConfig, encryption: e.target.value })}
                  className="select select-bordered w-full"
                >
                  <option value="none">None</option>
                  <option value="aes-256-gcm">AES-256-GCM</option>
                  <option value="chacha20-poly1305">ChaCha20-Poly1305</option>
                  <option value="salsa20">Salsa20</option>
                </select>
              </div>
              <div className="form-control">
                <label className="label">
                  <span className="label-text">Secret (Optional)</span>
                </label>
                <input
                  type="password"
                  value={newSessionConfig.secret}
                  onChange={(e) => setNewSessionConfig({ ...newSessionConfig, secret: e.target.value })}
                  className="input input-bordered w-full"
                  placeholder="shared secret used by remote endpoint"
                />
              </div>

              <div className="form-control">
                <label className="label">
                  <span className="label-text">Proxy (Optional)</span>
                </label>
                <input
                  type="text"
                  value={newSessionConfig.proxy}
                  onChange={(e) => setNewSessionConfig({ ...newSessionConfig, proxy: e.target.value })}
                  className="input input-bordered w-full"
                  placeholder="socks5://127.0.0.1:9050"
                />
              </div>



              <div className="modal-action">
                <button type="submit" className="btn btn-primary">
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                  {editingSessionId ? '保存修改' : '创建会话'}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setShowNewSession(false);
                    setEditingSessionId(null);
                    setNewSessionConfig({ target: '', encryption: 'none', proxy: '', secret: '' });
                  }}
                  className="btn btn-ghost"
                >
                  取消
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}