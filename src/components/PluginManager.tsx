import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface Plugin {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  status: 'active' | 'inactive' | 'error' | 'loading';
  category: 'scanner' | 'exploit' | 'post-exploitation' | 'utility';
  permissions: string[];
  lastUsed?: string;
  config?: Record<string, any>;
}

interface PluginManagerProps {
  sessionId: string;
}

export default function PluginManager({ sessionId }: PluginManagerProps) {
  const [plugins, setPlugins] = useState<Plugin[]>([]);
  const [selectedPlugin, setSelectedPlugin] = useState<Plugin | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [showInstall, setShowInstall] = useState(false);
  const [showConfig, setShowConfig] = useState(false);
  const [filterCategory, setFilterCategory] = useState<string>('all');
  const [searchTerm, setSearchTerm] = useState('');

  useEffect(() => {
    loadPlugins();
  }, []);

  const loadPlugins = async () => {
    setIsLoading(true);
    try {
      const pluginList = await invoke('get_plugins', {
        sessionId
      });
      setPlugins(pluginList as Plugin[]);
    } catch (error) {
      console.error('Failed to load plugins:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const togglePlugin = async (pluginId: string, enable: boolean) => {
    try {
      await invoke('toggle_plugin', {
        sessionId,
        pluginId,
        enable
      });
      
      setPlugins(prev => prev.map(plugin => 
        plugin.id === pluginId 
          ? { ...plugin, status: enable ? 'active' : 'inactive' }
          : plugin
      ));
    } catch (error) {
      console.error('Failed to toggle plugin:', error);
    }
  };

  const executePlugin = async (pluginId: string, params?: Record<string, any>) => {
    try {
      const result = await invoke('execute_plugin', {
        sessionId,
        pluginId,
        parameters: params || {}
      });
      
      // Update last used timestamp
      setPlugins(prev => prev.map(plugin => 
        plugin.id === pluginId 
          ? { ...plugin, lastUsed: new Date().toISOString() }
          : plugin
      ));
      
      return result;
    } catch (error) {
      console.error('Failed to execute plugin:', error);
      throw error;
    }
  };

  const installPlugin = async (pluginFile: File) => {
    try {
      const fileData = Array.from(new Uint8Array(await pluginFile.arrayBuffer()));
      await invoke('install_plugin', {
        fileName: pluginFile.name,
        fileData
      });
      
      loadPlugins();
      setShowInstall(false);
    } catch (error) {
      console.error('Failed to install plugin:', error);
    }
  };

  const uninstallPlugin = async (pluginId: string) => {
    const confirmed = confirm('Are you sure you want to uninstall this plugin?');
    if (!confirmed) return;

    try {
      await invoke('uninstall_plugin', {
        pluginId
      });
      
      setPlugins(prev => prev.filter(plugin => plugin.id !== pluginId));
      setSelectedPlugin(null);
    } catch (error) {
      console.error('Failed to uninstall plugin:', error);
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'active': return 'badge-success';
      case 'inactive': return 'badge-ghost';
      case 'error': return 'badge-error';
      case 'loading': return 'badge-warning';
      default: return 'badge-ghost';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'active': return '🟢';
      case 'inactive': return '⚪';
      case 'error': return '🔴';
      case 'loading': return '🟡';
      default: return '⚪';
    }
  };

  const getCategoryIcon = (category: string) => {
    switch (category) {
      case 'scanner': return '🔍';
      case 'exploit': return '💥';
      case 'post-exploitation': return '🎯';
      case 'utility': return '🔧';
      default: return '📦';
    }
  };

  const filteredPlugins = plugins.filter(plugin => {
    const matchesCategory = filterCategory === 'all' || plugin.category === filterCategory;
    const matchesSearch = plugin.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
                         plugin.description.toLowerCase().includes(searchTerm.toLowerCase());
    return matchesCategory && matchesSearch;
  });

  return (
    <div className="h-full min-h-0 flex flex-col lg:flex-row bg-gradient-to-br from-base-100 to-base-200">
      {/* Plugin List Sidebar */}
      <div className="w-full lg:w-80 bg-base-100 shadow-xl border-b lg:border-b-0 lg:border-r border-base-300 flex flex-col min-h-0">
        {/* Elegant Header */}
        <div className="bg-gradient-to-r from-primary to-secondary  text-primary-content">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="avatar placeholder">
                <div className="bg-base-100 text-primary rounded-xl w-12">
                  <span className="text-2xl"></span>
                </div>
              </div>
              <div>
                <h2 className="text-1xl font-bold">Plugin Store</h2>
                <p className="text-sm opacity-90">Manage your tools</p>
              </div>
            </div>
            <button
              onClick={() => setShowInstall(true)}
              className="btn btn-accent btn-circle btn-sm shadow-lg hover:shadow-xl transition-all duration-300"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
            </button>
          </div>
        </div>

        {/* Search and Filter Section */}
        <div className="p-4 bg-base-50 space-y-4">
          <div className="form-control">
            <div className="input-group">
              <span className="bg-base-200">
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
              </span>
              <input
                type="text"
                placeholder="Search plugins..."
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
                className="input input-bordered flex-1 focus:input-primary"
              />
            </div>
          </div>
          
          <select
            value={filterCategory}
            onChange={(e) => setFilterCategory(e.target.value)}
            className="select select-bordered w-full focus:select-primary"
          >
            <option value="all">🌟 All Categories</option>
            <option value="scanner">🔍 Scanner Tools</option>
            <option value="exploit">💥 Exploit Modules</option>
            <option value="post-exploitation">🎯 Post-Exploitation</option>
            <option value="utility">🔧 Utilities</option>
          </select>
        </div>

        {/* Plugin Cards List */}
        <div className="flex-1 min-h-0 overflow-y-auto p-4 space-y-3">
          {isLoading ? (
            <div className="flex flex-col items-center justify-center h-full">
              <span className="loading loading-dots loading-lg text-primary"></span>
              <p className="mt-4 text-base-content/70">Loading awesome plugins...</p>
            </div>
          ) : filteredPlugins.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-center">
              <div className="text-6xl mb-4 opacity-50">🔍</div>
              <h3 className="text-lg font-bold mb-2">No plugins found</h3>
              <p className="text-sm opacity-70">Try adjusting your search criteria</p>
            </div>
          ) : (
            filteredPlugins.map((plugin) => (
              <div
                key={plugin.id}
                onClick={() => setSelectedPlugin(plugin)}
                className={`card bg-base-100 shadow-md hover:shadow-xl transition-all duration-300 cursor-pointer transform hover:-translate-y-1 ${
                  selectedPlugin?.id === plugin.id 
                    ? 'ring-2 ring-primary shadow-primary/20' 
                    : 'hover:bg-base-50'
                }`}
              >
                <div className="card-body p-4">
                  <div className="flex items-start justify-between">
                    <div className="flex items-center gap-3">
                      <div className="avatar placeholder">
                        <div className="bg-gradient-to-br from-primary to-secondary text-primary-content rounded-lg w-10">
                          <span className="text-lg">{getCategoryIcon(plugin.category)}</span>
                        </div>
                      </div>
                      <div className="flex-1">
                        <h3 className="font-bold text-base truncate">{plugin.name}</h3>
                        <p className="text-xs opacity-70">v{plugin.version} • {plugin.author}</p>
                      </div>
                    </div>
                    <div className={`badge ${getStatusBadge(plugin.status)} badge-sm gap-1 shadow-sm`}>
                      <span className="text-xs">{getStatusIcon(plugin.status)}</span>
                    </div>
                  </div>
                  <p className="text-sm opacity-80 mt-2 line-clamp-2 leading-relaxed">
                    {plugin.description}
                  </p>
                  <div className="flex items-center justify-between mt-3">
                    <div className="badge badge-outline badge-xs capitalize">
                      {plugin.category}
                    </div>
                    {plugin.lastUsed && (
                      <div className="text-xs opacity-50">
                        Last used: {new Date(plugin.lastUsed).toLocaleDateString()}
                      </div>
                    )}
                  </div>
                </div>
              </div>
            ))
          )}
        </div>
      </div>

      {/* Plugin Details - Main Content Area */}
      <div className="flex-1 min-h-0 flex flex-col bg-base-100">
        {selectedPlugin ? (
          <>
            {/* Elegant Plugin Header */}
            <div className="bg-gradient-to-r from-primary/10 to-secondary/10 border-b border-base-300">
              <div className="p-6">
                <div className="flex items-center justify-between mb-4">
                  <div className="flex items-center gap-4">
                    <div className="avatar placeholder">
                      <div className="bg-gradient-to-br from-primary to-secondary text-primary-content rounded-2xl w-16">
                        <span className="text-3xl">{getCategoryIcon(selectedPlugin.category)}</span>
                      </div>
                    </div>
                    <div>
                      <h1 className="text-3xl font-bold text-base-content">{selectedPlugin.name}</h1>
                      <div className="flex items-center gap-3 mt-1">
                        <span className="text-base-content/70">v{selectedPlugin.version}</span>
                        <div className="divider divider-horizontal"></div>
                        <span className="text-base-content/70">by {selectedPlugin.author}</span>
                        <div className="divider divider-horizontal"></div>
                        <div className="badge badge-outline capitalize">{selectedPlugin.category}</div>
                      </div>
                    </div>
                  </div>
                  
                  <div className={`badge ${getStatusBadge(selectedPlugin.status)} badge-lg gap-2 shadow-lg`}>
                    <span className="text-lg">{getStatusIcon(selectedPlugin.status)}</span>
                    <span className="font-semibold">{selectedPlugin.status.toUpperCase()}</span>
                  </div>
                </div>

                <p className="text-base-content/80 text-lg leading-relaxed mb-6">
                  {selectedPlugin.description}
                </p>
                
                {/* Action Buttons */}
                <div className="flex gap-3">
                  <button
                    onClick={() => togglePlugin(selectedPlugin.id, selectedPlugin.status !== 'active')}
                    className={`btn btn-lg shadow-lg hover:shadow-xl transition-all duration-300 ${
                      selectedPlugin.status === 'active'
                        ? 'btn-error hover:btn-error-focus'
                        : 'btn-success hover:btn-success-focus'
                    }`}
                  >
                    {selectedPlugin.status === 'active' ? (
                      <>
                        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                        Disable Plugin
                      </>
                    ) : (
                      <>
                        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                        Enable Plugin
                      </>
                    )}
                  </button>
                  
                  <button
                    onClick={() => executePlugin(selectedPlugin.id)}
                    disabled={selectedPlugin.status !== 'active'}
                    className="btn btn-primary btn-lg shadow-lg hover:shadow-xl transition-all duration-300"
                  >
                    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
                    </svg>
                    Execute Plugin
                  </button>
                  
                  <button
                    onClick={() => setShowConfig(true)}
                    className="btn btn-ghost btn-lg shadow-lg hover:shadow-xl transition-all duration-300"
                  >
                    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                    </svg>
                    Configure
                  </button>
                  
                  <div className="dropdown dropdown-end">
                    <label tabIndex={0} className="btn btn-ghost btn-lg btn-circle shadow-lg hover:shadow-xl transition-all duration-300">
                      <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 5v.01M12 12v.01M12 19v.01" />
                      </svg>
                    </label>
                    <ul tabIndex={0} className="dropdown-content menu p-2 shadow-xl bg-base-100 rounded-box w-52">
                      <li>
                        <a onClick={() => uninstallPlugin(selectedPlugin.id)} className="text-error">
                          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                          </svg>
                          Uninstall Plugin
                        </a>
                      </li>
                    </ul>
                  </div>
                </div>
              </div>
            </div>

            {/* Plugin Content */}
            <div className="flex-1 min-h-0 p-6 overflow-y-auto bg-base-50">
              <div className="grid grid-cols-1 xl:grid-cols-2 gap-6 mb-6">
                {/* Information Card */}
                <div className="card bg-base-100 shadow-xl hover:shadow-2xl transition-all duration-300">
                  <div className="card-body">
                    <h2 className="card-title text-xl flex items-center gap-3">
                      <div className="avatar placeholder">
                        <div className="bg-info text-info-content rounded-lg w-8">
                          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                          </svg>
                        </div>
                      </div>
                      Plugin Information
                    </h2>
                    <div className="space-y-4 mt-4">
                      <div className="stats shadow">
                        <div className="stat">
                          <div className="stat-title">Category</div>
                          <div className="stat-value text-lg capitalize">{selectedPlugin.category}</div>
                          <div className="stat-desc">{getCategoryIcon(selectedPlugin.category)} Tool Type</div>
                        </div>
                      </div>
                      <div className="grid grid-cols-2 gap-4">
                        <div className="bg-base-200 rounded-lg p-3">
                          <div className="text-sm opacity-70">Version</div>
                          <div className="font-mono font-bold">{selectedPlugin.version}</div>
                        </div>
                        <div className="bg-base-200 rounded-lg p-3">
                          <div className="text-sm opacity-70">Author</div>
                          <div className="font-semibold">{selectedPlugin.author}</div>
                        </div>
                      </div>
                      {selectedPlugin.lastUsed && (
                        <div className="bg-base-200 rounded-lg p-3">
                          <div className="text-sm opacity-70">Last Used</div>
                          <div className="text-sm">
                            {new Date(selectedPlugin.lastUsed).toLocaleString()}
                          </div>
                        </div>
                      )}
                    </div>
                  </div>
                </div>

                {/* Permissions Card */}
                <div className="card bg-base-100 shadow-xl hover:shadow-2xl transition-all duration-300">
                  <div className="card-body">
                    <h2 className="card-title text-xl flex items-center gap-3">
                      <div className="avatar placeholder">
                        <div className="bg-warning text-warning-content rounded-lg w-8">
                          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                          </svg>
                        </div>
                      </div>
                      Security Permissions
                    </h2>
                    <div className="space-y-3 mt-4">
                      {selectedPlugin.permissions.map((permission, index) => (
                        <div key={index} className="alert alert-warning shadow-sm">
                          <svg className="stroke-current shrink-0 w-5 h-5" fill="none" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L3.268 16.5c-.77.833.192 2.5 1.732 2.5z"></path>
                          </svg>
                          <span className="font-medium">{permission}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                </div>
              </div>

              {/* Plugin Output Terminal */}
              <div className="card bg-base-100 shadow-xl hover:shadow-2xl transition-all duration-300">
                <div className="card-body">
                  <h2 className="card-title text-xl flex items-center gap-3 mb-4">
                    <div className="avatar placeholder">
                      <div className="bg-success text-success-content rounded-lg w-8">
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
                        </svg>
                      </div>
                    </div>
                    Plugin Output Terminal
                    <div className="badge badge-success badge-sm">Live</div>
                  </h2>
                  <div className="mockup-window border bg-base-300 shadow-lg">
                    <div className="flex justify-center px-4 py-16 bg-base-200">
                      <div className="mockup-code bg-neutral text-neutral-content min-h-48 w-full">
                        <pre data-prefix="$" className="text-success"><code>Plugin ready for execution...</code></pre>
                        <pre data-prefix=">" className="text-warning"><code>Waiting for commands...</code></pre>
                        <pre data-prefix=">" className="text-info"><code>Output will appear here when plugin runs</code></pre>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </>
        ) : (
          <div className="hero h-full bg-gradient-to-br from-base-100 to-base-200">
            <div className="hero-content text-center">
              <div className="max-w-md">
                <div className="text-8xl mb-6 animate-bounce">🔌</div>
                <h1 className="text-4xl font-bold mb-4 bg-gradient-to-r from-primary to-secondary bg-clip-text text-transparent">
                  Select a Plugin
                </h1>
                <p className="text-lg opacity-70 leading-relaxed">
                  Choose a plugin from the sidebar to view detailed information, configure settings, and execute powerful security tools.
                </p>
                <div className="mt-6">
                  <button 
                    onClick={() => setShowInstall(true)}
                    className="btn btn-primary btn-lg shadow-lg hover:shadow-xl transition-all duration-300"
                  >
                    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                    </svg>
                    Install New Plugin
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Install Plugin Modal */}
      {showInstall && (
        <div className="modal modal-open">
          <div className="modal-box max-w-2xl">
            <button 
              onClick={() => setShowInstall(false)}
              className="btn btn-sm btn-circle btn-ghost absolute right-2 top-2"
            >
              ✕
            </button>
            
            <div className="text-center mb-6">
              <div className="avatar placeholder mb-4">
                <div className="bg-gradient-to-br from-primary to-secondary text-primary-content rounded-2xl w-16">
                  <span className="text-3xl">📦</span>
                </div>
              </div>
              <h3 className="font-bold text-2xl mb-2">Install New Plugin</h3>
              <p className="text-base-content/70">Add powerful security tools to your arsenal</p>
            </div>
            
            <div className="space-y-6">
              <div className="card bg-gradient-to-br from-primary/5 to-secondary/5 border-2 border-dashed border-primary/30 hover:border-primary/60 transition-all duration-300">
                <div className="card-body items-center text-center">
                  <input
                    type="file"
                    accept=".wasm,.zip"
                    onChange={(e) => {
                      const file = e.target.files?.[0];
                      if (file) installPlugin(file);
                    }}
                    className="file-input file-input-bordered file-input-primary w-full max-w-sm mb-4"
                    id="plugin-upload"
                  />
                  <div className="space-y-2">
                    <h4 className="font-semibold text-lg">Drop your plugin file here</h4>
                    <p className="text-sm opacity-70">
                      Supports <span className="badge badge-outline badge-sm">.wasm</span> and 
                      <span className="badge badge-outline badge-sm ml-1">.zip</span> files
                    </p>
                  </div>
                </div>
              </div>

              <div className="alert alert-info">
                <svg className="stroke-current shrink-0 w-6 h-6" fill="none" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                </svg>
                <div>
                  <h3 className="font-bold">安全提示</h3>
                  <div className="text-xs">只安装来自可信来源的插件。恶意插件可能会危害系统安全。</div>
                </div>
              </div>
            </div>
            
            <div className="modal-action">
              <button
                onClick={() => setShowInstall(false)}
                className="btn btn-ghost btn-lg"
              >
                取消
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Configuration Modal */}
      {showConfig && selectedPlugin && (
        <div className="modal modal-open">
          <div className="modal-box max-w-4xl">
            <button 
              onClick={() => setShowConfig(false)}
              className="btn btn-sm btn-circle btn-ghost absolute right-2 top-2"
            >
              ✕
            </button>
            
            <div className="flex items-center gap-4 mb-6">
              <div className="avatar placeholder">
                <div className="bg-gradient-to-br from-primary to-secondary text-primary-content rounded-2xl w-12">
                  <span className="text-2xl">{getCategoryIcon(selectedPlugin.category)}</span>
                </div>
              </div>
              <div>
                <h3 className="font-bold text-2xl">配置 {selectedPlugin.name}</h3>
                <p className="text-base-content/70">自定义插件参数和行为设置</p>
              </div>
            </div>
            
            <div className="space-y-6">
              <div className="tabs tabs-boxed bg-base-200">
                <a className="tab tab-active">基本设置</a>
                <a className="tab">高级选项</a>
                <a className="tab">安全配置</a>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div className="card bg-base-100 shadow-lg">
                  <div className="card-body">
                    <h4 className="card-title text-lg">执行参数</h4>
                    <div className="space-y-4">
                      <div className="form-control">
                        <label className="label">
                          <span className="label-text">超时时间 (秒)</span>
                        </label>
                        <input type="number" placeholder="30" className="input input-bordered" />
                      </div>
                      <div className="form-control">
                        <label className="label">
                          <span className="label-text">最大内存使用 (MB)</span>
                        </label>
                        <input type="number" placeholder="512" className="input input-bordered" />
                      </div>
                      <div className="form-control">
                        <label className="cursor-pointer label">
                          <span className="label-text">启用详细日志</span>
                          <input type="checkbox" className="toggle toggle-primary" />
                        </label>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="card bg-base-100 shadow-lg">
                  <div className="card-body">
                    <h4 className="card-title text-lg">网络设置</h4>
                    <div className="space-y-4">
                      <div className="form-control">
                        <label className="label">
                          <span className="label-text">代理服务器</span>
                        </label>
                        <input type="text" placeholder="http://proxy:8080" className="input input-bordered" />
                      </div>
                      <div className="form-control">
                        <label className="label">
                          <span className="label-text">用户代理</span>
                        </label>
                        <select className="select select-bordered">
                          <option>默认</option>
                          <option>Chrome</option>
                          <option>Firefox</option>
                          <option>自定义</option>
                        </select>
                      </div>
                      <div className="form-control">
                        <label className="cursor-pointer label">
                          <span className="label-text">启用 SSL 验证</span>
                          <input type="checkbox" className="toggle toggle-primary" defaultChecked />
                        </label>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <div className="alert alert-warning">
                <svg className="stroke-current shrink-0 w-6 h-6" fill="none" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L3.268 16.5c-.77.833.192 2.5 1.732 2.5z"></path>
                </svg>
                <div>
                  <h3 className="font-bold">配置说明</h3>
                  <div className="text-xs">插件配置界面会根据插件的具体架构动态生成。这里显示的是示例配置选项。</div>
                </div>
              </div>
            </div>
            
            <div className="modal-action">
              <button
                onClick={() => setShowConfig(false)}
                className="btn btn-primary btn-lg shadow-lg"
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                </svg>
                保存配置
              </button>
              <button
                onClick={() => setShowConfig(false)}
                className="btn btn-ghost btn-lg"
              >
                取消
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}