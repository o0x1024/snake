import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface CommandHistory {
  id: string;
  command: string;
  output: string;
  timestamp: string;
  status: 'success' | 'error' | 'running';
  sessionId: string;
}

interface CommandExecutorProps {
  sessionId: string;
}

// 全局会话历史记录缓存
const sessionHistoryCache = new Map<string, CommandHistory[]>();
const sessionDirectoryCache = new Map<string, string>();
const sessionSystemInfoCache = new Map<string, { username: string; hostname: string }>();

export default function CommandExecutor({ sessionId }: CommandExecutorProps) {
  const [command, setCommand] = useState('');
  const [history, setHistory] = useState<CommandHistory[]>([]);
  const [isExecuting, setIsExecuting] = useState(false);
  const [currentDirectory, setCurrentDirectory] = useState('/');
  const [showSecretModal, setShowSecretModal] = useState(false);
  const [secretInput, setSecretInput] = useState('');
  const [secretSaving, setSecretSaving] = useState(false);
  const [pending, setPending] = useState<{ id: string; cmd: string } | null>(null);
  const [username, setUsername] = useState('user');
  const [hostname, setHostname] = useState('localhost');
  const terminalRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [isInitialized, setIsInitialized] = useState(false);

  useEffect(() => {
    // 从缓存中恢复会话状态
    const cachedHistory = sessionHistoryCache.get(sessionId);
    const cachedDirectory = sessionDirectoryCache.get(sessionId);
    const cachedSystemInfo = sessionSystemInfoCache.get(sessionId);
    
    if (cachedHistory) {
      setHistory(cachedHistory);
    } else {
      setHistory([]);
      loadCommandHistory();
    }
    
    if (cachedDirectory) {
      setCurrentDirectory(cachedDirectory);
    } else {
      setCurrentDirectory('/');
    }
    
    if (cachedSystemInfo) {
      setUsername(cachedSystemInfo.username);
      setHostname(cachedSystemInfo.hostname);
    } else {
      setUsername('user');
      setHostname('localhost');
      loadSystemInfo();
    }
    
    setIsInitialized(true);
    inputRef.current?.focus();
  }, [sessionId]);

  useEffect(() => {
    if (terminalRef.current) {
      terminalRef.current.scrollTop = terminalRef.current.scrollHeight;
    }
  }, [history]);

  const loadCommandHistory = async () => {
    try {
      const commandHistory = await invoke('get_command_history', {
        sessionId: sessionId,
        limit: 100, // 限制加载最近100条记录
      });
      const historyData = (commandHistory as any[]).map(item => ({
        id: item.id,
        command: item.command,
        output: item.output,
        timestamp: item.timestamp,
        status: item.status as 'success' | 'error' | 'running',
        sessionId: item.session_id
      }));
      setHistory(historyData);
      // 保存到缓存
      sessionHistoryCache.set(sessionId, historyData);
    } catch (error) {
      console.error('Failed to load command history:', error);
    }
  };

  const loadSystemInfo = async () => {
    try {
      // 获取会话信息以确定是否有远程端点
      const sessions = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessions.find(s => s.id === sessionId);
      const endpoint = (window as any).__wsEndpoint || found?.target;
      
      let whoamiResult: any;
      let hostnameResult: any;
      
      if (endpoint) {
        // 优先使用远程执行获取目标机器的信息
        try {
          whoamiResult = await invoke('ws_execute', {
            session_id: sessionId,
            sessionId,
            endpoint,
            command: 'whoami'
          }) as any;
          
          hostnameResult = await invoke('ws_execute', {
            session_id: sessionId,
            sessionId,
            endpoint,
            command: 'hostname'
          }) as any;
        } catch (err) {
          // 如果远程执行失败，回退到本地执行（用于演示）
          console.log('Remote execution failed, falling back to local execution');
          whoamiResult = await invoke('execute_command', {
            session_id: sessionId,
            sessionId,
            command: 'whoami',
            commandId: 'system_whoami'
          }) as any;
          
          hostnameResult = await invoke('execute_command', {
            session_id: sessionId,
            sessionId,
            command: 'hostname',
            commandId: 'system_hostname'
          }) as any;
        }
      } else {
        // 没有远程端点，使用本地执行
        whoamiResult = await invoke('execute_command', {
          session_id: sessionId,
          sessionId,
          command: 'whoami',
          commandId: 'system_whoami'
        }) as any;
        
        hostnameResult = await invoke('execute_command', {
          session_id: sessionId,
          sessionId,
          command: 'hostname',
          commandId: 'system_hostname'
        }) as any;
      }
      
      let username = 'user';
      let hostname = 'localhost';
      
      // 处理远程执行和本地执行的不同响应格式
      const whoamiOutput = whoamiResult?.output || whoamiResult?.stdout || '';
      const hostnameOutput = hostnameResult?.output || hostnameResult?.stdout || '';
      
      if (whoamiOutput) {
        username = whoamiOutput.trim();
        setUsername(username);
      }
      
      if (hostnameOutput) {
        hostname = hostnameOutput.trim();
        setHostname(hostname);
      }
      
      // 保存到缓存
      sessionSystemInfoCache.set(sessionId, { username, hostname });
    } catch (error) {
      console.log('Using default system info');
      // 使用默认值
    }
  };

  const executeCommand = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!command.trim() || isExecuting) return;

    const commandId = Date.now().toString();
    const cmdText = command.trim();
    const newCommand: CommandHistory = {
      id: commandId,
      command: cmdText,
      output: '',
      timestamp: new Date().toISOString(),
      status: 'running',
      sessionId
    };

    setHistory(prev => [...prev, newCommand]);
    setIsExecuting(true);
    setCommand('');    try {

      // Prefer remote ws_execute if endpoint configured
      const sessions = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessions.find(s => s.id === sessionId);
      const endpoint = (window as any).__wsEndpoint || found?.target;
      let result: any;
      if (endpoint) {
        // Assume backend memory holds the secret after creation or manual configuration
        try {
          result = await invoke('ws_execute', { session_id: sessionId, sessionId, endpoint, command: cmdText });
        } catch (err) {
          const message = String(err ?? 'unknown error');
          if (message.includes('No secret configured for session')) {
            setPending({ id: commandId, cmd: cmdText });
            setShowSecretModal(true);
            setIsExecuting(false);
            return;
          }
          throw err;
        }
      } else {
        result = await invoke('execute_command', {
          session_id: sessionId,
          sessionId,
          command: cmdText,
          command_id: commandId,
          commandId
        });
      }

      const commandResult = result as any;
      
      setHistory(prev => {
        const updatedHistory = prev.map(cmd => 
          cmd.id === commandId 
            ? {
                ...cmd,
                 output: (commandResult.output ?? commandResult.stdout ?? '').toString(),
                 status: (commandResult.exitCode ?? commandResult.exit_code ?? 0) === 0 ? 'success' as const : 'error' as const
              }
            : cmd
        );
        // 更新缓存
        sessionHistoryCache.set(sessionId, updatedHistory);
        return updatedHistory;
      });

      const newDirectory = commandResult.directory ?? commandResult.cwd ?? currentDirectory;
      setCurrentDirectory(newDirectory);
      sessionDirectoryCache.set(sessionId, newDirectory);
    } catch (error) {
      setHistory(prev => prev.map(cmd => 
        cmd.id === commandId 
          ? {
              ...cmd,
              output: `Error: ${error}`,
              status: 'error'
            }
          : cmd
      ));
    } finally {
      setIsExecuting(false);
    }
  };

  const saveSecretAndRun = async () => {
    if (!pending || !secretInput.trim()) return;
    setSecretSaving(true);
    try {
      await invoke('update_session_secret', { token: 'dev-token-1234', session_id: sessionId, sessionId, secret: secretInput.trim() });
      const sessions = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessions.find((s: any) => s.id === sessionId);
      const endpoint = (window as any).__wsEndpoint || found?.target;
      if (!endpoint) throw new Error('No endpoint for this session');

      // Retry the pending command
      const result = await invoke('ws_execute', { session_id: sessionId, sessionId, endpoint, command: pending.cmd });
      const commandResult = result as any;
      setHistory(prev => prev.map(cmd => 
        cmd.id === pending.id 
          ? {
              ...cmd,
              output: (commandResult.output ?? commandResult.stdout ?? '').toString(),
              status: (commandResult.exitCode ?? commandResult.exit_code ?? 0) === 0 ? 'success' : 'error'
            }
          : cmd
      ));
      setCurrentDirectory(commandResult.directory ?? commandResult.cwd ?? currentDirectory);
      setShowSecretModal(false);
      setSecretInput('');
      setPending(null);
    } catch (e) {
      // Mark pending as error
      if (pending) {
        setHistory(prev => prev.map(cmd => 
          cmd.id === pending.id 
            ? { ...cmd, output: `Error: ${e}`, status: 'error' }
            : cmd
        ));
      }
      console.error('Failed to configure secret or run command:', e);
      setShowSecretModal(false);
      setPending(null);
    } finally {
      setSecretSaving(false);
    }
  };

  const clearHistory = async () => {
    try {
      await invoke('clear_command_history', {
        session_id: sessionId,
      });
      setHistory([]);
      // 清除缓存
      sessionHistoryCache.delete(sessionId);
    } catch (error) {
      console.error('Failed to clear command history:', error);
      // 即使数据库清除失败，也清除本地显示
      setHistory([]);
      sessionHistoryCache.delete(sessionId);
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'success': return '✅';
      case 'error': return '❌';
      case 'running': return '⏳';
      default: return '⚪';
    }
  };

  const formatTimestamp = (timestamp: string) => {
    return new Date(timestamp).toLocaleTimeString();
  };

  return (
    <div className="h-full min-h-0 flex flex-col bg-gray-900 rounded-xl shadow-2xl border border-gray-700 overflow-hidden" style={{ fontFamily: 'SF Mono, Monaco, Inconsolata, Roboto Mono, Consolas, Courier New, monospace' }}>
      {/* Terminal Header */}
      <div className="bg-gray-800 px-4 py-3 border-b border-gray-700">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <div className="flex items-center space-x-2">
              <svg className="w-5 h-5 text-green-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="4" width="18" height="14" rx="2"/>
                <path d="M8 21h8"/>
              </svg>
              <span className="text-gray-300 font-medium text-sm">Terminal — Session {sessionId}</span>
            </div>
          </div>
          <div className="flex items-center space-x-2">
            <div className="px-2 py-1 bg-gray-700 rounded text-xs text-green-400 font-mono">
              📁 {currentDirectory}
            </div>
            <button 
              onClick={clearHistory} 
              className="w-6 h-6 rounded hover:bg-gray-700 transition-colors flex items-center justify-center text-gray-400 hover:text-red-400"
              title="Clear History"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          </div>
        </div>
      </div>

      {/* Terminal Body */}
      <div className="flex-1 min-h-0 bg-black">
        <div ref={terminalRef} className="h-full overflow-y-auto px-4 py-2">
          {history.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-center p-8">
              <div className="text-4xl mb-4 text-green-400">🍺</div>
              <div className="text-green-400 font-mono text-lg mb-2">Homebrew Terminal Ready</div>
              <div className="text-gray-500 font-mono text-sm mb-4">Last login: {new Date().toLocaleString()} on ttys000</div>
              <div className="text-gray-400 font-mono text-xs">
                Type a command to get started...
              </div>
            </div>
          ) : (
            <div className="space-y-1">
              {history.map((cmd) => (
                <div key={cmd.id} className="font-mono text-sm">
                  {/* Command Line */}
                  <div className="flex items-center space-x-2 mb-1">
                    <span className="text-green-400 font-bold">{username}</span>
                     <span className="text-gray-500">@</span>
                     <span className="text-blue-400 font-bold">{hostname}</span>
                    <span className="text-gray-500">:</span>
                    <span className="text-purple-400">{currentDirectory}</span>
                    <span className="text-green-400 font-bold">$</span>
                    <span className="text-white">{cmd.command}</span>
                    {cmd.status === 'running' && (
                      <span className="text-yellow-400 animate-pulse">⏳</span>
                    )}
                  </div>
                  
                  {/* Command Output */}
                  {cmd.output && (
                    <div className={`mb-2 pl-2 border-l-2 ${
                      cmd.status === 'error' 
                        ? 'border-red-500 text-red-400' 
                        : 'border-gray-600 text-gray-300'
                    }`}>
                      <pre className="whitespace-pre-wrap font-mono text-xs leading-relaxed">{cmd.output}</pre>
                    </div>
                  )}
                  
                  {cmd.status === 'running' && !cmd.output && (
                    <div className="flex items-center space-x-2 text-yellow-400 pl-2 mb-2">
                      <div className="w-2 h-2 bg-yellow-400 rounded-full animate-pulse"></div>
                      <span className="text-xs font-mono">Executing...</span>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Command Input Section */}
      <div className="bg-black border-t border-gray-700 px-4 py-3">
        <form onSubmit={executeCommand}>
          <div className="flex items-center space-x-2 font-mono text-sm">
            <span className="text-green-400 font-bold">{username}</span>
             <span className="text-gray-500">@</span>
             <span className="text-blue-400 font-bold">{hostname}</span>
            <span className="text-gray-500">:</span>
            <span className="text-purple-400">{currentDirectory}</span>
            <span className="text-green-400 font-bold">$</span>
            <input
              ref={inputRef}
              type="text"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              placeholder=""
              className="flex-1 bg-transparent text-white outline-none border-none font-mono text-sm ml-2"
              disabled={isExecuting}
              style={{ caretColor: '#10b981' }}
            />
            {isExecuting && (
              <div className="flex items-center space-x-1 text-yellow-400">
                <div className="w-1 h-1 bg-yellow-400 rounded-full animate-pulse"></div>
                <div className="w-1 h-1 bg-yellow-400 rounded-full animate-pulse" style={{ animationDelay: '0.2s' }}></div>
                <div className="w-1 h-1 bg-yellow-400 rounded-full animate-pulse" style={{ animationDelay: '0.4s' }}></div>
              </div>
            )}
          </div>
        </form>
      </div>

      {/* Secret Configuration Modal */}
      {showSecretModal && (
        <div className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50">
          <div className="bg-gray-900 rounded-lg border border-gray-700 p-6 max-w-md w-full mx-4">
            <h3 className="text-green-400 font-mono text-lg font-bold mb-4">🔐 Session Authentication</h3>
            <p className="text-gray-300 font-mono text-sm mb-4">
              This session requires authentication. Please enter the secret key.
            </p>
            <input
              type="password"
              value={secretInput}
              onChange={(e) => setSecretInput(e.target.value)}
              placeholder="Enter secret key..."
              className="w-full bg-black text-white border border-gray-600 rounded px-3 py-2 font-mono text-sm mb-4 focus:outline-none focus:border-green-400"
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  saveSecretAndRun();
                }
              }}
            />
            <div className="flex justify-end space-x-3">
              <button
                onClick={() => { setShowSecretModal(false); setPending(null); }}
                className="px-4 py-2 text-gray-400 hover:text-white font-mono text-sm transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={saveSecretAndRun}
                className="px-4 py-2 bg-green-600 hover:bg-green-500 text-white font-mono text-sm rounded transition-colors disabled:opacity-50"
                disabled={!secretInput.trim() || secretSaving}
              >
                {secretSaving ? 'Saving...' : 'Save & Execute'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}