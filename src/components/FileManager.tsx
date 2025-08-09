import { useState, useEffect } from 'react';
import type { ReactElement } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface FileItem {
  name: string;
  path: string;
  type: 'file' | 'directory';
  size: number;
  permissions: string;
  modified: string;
  owner: string;
  isHidden: boolean;
}

interface FileManagerProps {
  sessionId: string;
}

export default function FileManager({ sessionId }: FileManagerProps) {
  const [currentPath, setCurrentPath] = useState('/');
  const [files, setFiles] = useState<FileItem[]>([]);
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(true);
  const [showHidden, setShowHidden] = useState(false);
  const [sortBy, setSortBy] = useState<'name' | 'size' | 'modified'>('name');
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('asc');
  const [showUpload, setShowUpload] = useState(false);
  const [uploadProgress, setUploadProgress] = useState(0);
  const [showSecretModal, setShowSecretModal] = useState(false);
  const [secretInput, setSecretInput] = useState('');
  const [secretSaving, setSecretSaving] = useState(false);
  // File operations states
  const [showFileViewer, setShowFileViewer] = useState(false);
  const [showFileEditor, setShowFileEditor] = useState(false);
  const [showRenameModal, setShowRenameModal] = useState(false);
  const [showCreateDirModal, setShowCreateDirModal] = useState(false);
  const [showCreateFileModal, setShowCreateFileModal] = useState(false);
  const [currentFileContent, setCurrentFileContent] = useState('');
  const [currentFileName, setCurrentFileName] = useState('');
  const [currentFilePath, setCurrentFilePath] = useState('');
  const [newFileName, setNewFileName] = useState('');
  const [newDirName, setNewDirName] = useState('');
  const [newFileContent, setNewFileContent] = useState('');
  const [isEditing, setIsEditing] = useState(false);
  const [selectedUploadFile, setSelectedUploadFile] = useState<File | null>(null);
  const [showUploadConfirm, setShowUploadConfirm] = useState(false);
  // Folder tree states
  const [tree, setTree] = useState<Record<string, { loaded: boolean; children: { name: string; path: string }[] }>>({});
  const [expanded, setExpanded] = useState<Set<string>>(new Set(['/']));
  const [selectedPath, setSelectedPath] = useState<string>('/');

  // Load current directory contents whenever path changes
  useEffect(() => {
    void loadDirectory(currentPath);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentPath]);

  // Ensure root children are present; re-evaluate when showHidden changes
  useEffect(() => {
    void ensureChildren('/', showHidden);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showHidden]);

  // When a node is expanded, lazy load its children if not loaded
  useEffect(() => {
    expanded.forEach((p) => {
      if (!tree[p]?.loaded) {
        ensureChildren(p, showHidden);
      }
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [expanded, showHidden]);

  // Fetch listing (remote preferred), reused by grid and tree
  const fetchListing = async (path: string): Promise<FileItem[]> => {
    // Prefer remote listing via driver using session target as endpoint
    const sessionInfo = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
    const found = sessionInfo.find(s => s.id === sessionId);
    const remoteEndpoint = (window as any).__wsEndpoint || found?.target;
    if (remoteEndpoint) {
      const remote = await invoke('ws_list', { session_id: sessionId, sessionId, endpoint: remoteEndpoint, path });
      const mapped = (remote as any[]).map((e) => ({
        name: e.name,
        path: e.path,
        type: e.type === 'directory' ? 'directory' : 'file',
        size: e.type === 'directory' ? 0 : (e.size || e.file_size || 0),
        permissions: e.perm || e.permissions || '',
        modified: e.mtime || e.modified || e.last_modified || '',
        owner: 'remote',
        isHidden: e.hidden ?? false,
      }));
      return mapped as FileItem[];
    } else {
      const fileList = await invoke('list_directory', {
        session_id: sessionId,
        sessionId,
        path,
        show_hidden: showHidden,
        showHidden,
      });
      return fileList as FileItem[];
    }
  };

  const loadDirectory = async (path: string) => {
    setIsLoading(true);
    try {
      const list = await fetchListing(path);
      setFiles(list);
    } catch (error) {
      const message = String(error ?? 'unknown error');
      console.error('Failed to load directory:', message);
      if (message.includes('No secret configured for session')) {
        setShowSecretModal(true);
      }
    } finally {
      setIsLoading(false);
    }
  };

  const ensureChildren = async (path: string, hidden: boolean): Promise<{ name: string; path: string }[]> => {
    if (tree[path]?.loaded) {
      return tree[path]!.children;
    }
    // Optimistically mark as loading to show spinner
    setTree(prev => ({
      ...prev,
      [path]: prev[path] ? { ...prev[path], loaded: false } : { loaded: false, children: [] },
    }));
    try {
      const list = await fetchListing(path);
      const dirs = list.filter((f) => f.type === 'directory' && (hidden || !f.isHidden));
      setTree(prev => ({
        ...prev,
        [path]: { loaded: true, children: dirs.map(d => ({ name: d.name, path: d.path })) },
      }));
      return dirs.map(d => ({ name: d.name, path: d.path }));
    } catch (e) {
      console.error('Failed to load tree children:', e);
      // keep node as loaded to avoid infinite spinner
      setTree(prev => ({ ...prev, [path]: { loaded: true, children: prev[path]?.children ?? [] } }));
      return tree[path]?.children ?? [];
    }
  };

  // removed subtree prefetch; children are loaded only on demand

  const toggleExpand = async (path: string) => {
    const next = new Set(expanded);
    if (next.has(path)) {
      // collapse: remove self and all descendants
      next.forEach(p => {
        if (p !== path && p.startsWith(path.endsWith('/') ? path : path + '/')) {
          next.delete(p);
        }
      });
      next.delete(path);
      setExpanded(next);
    } else {
      setExpanded(prev => new Set(prev).add(path));
      await ensureChildren(path, showHidden);
    }
  };

  const renderTreeNode = (path: string, depth: number): ReactElement => {
    const isRoot = path === '/';
    const nodeName = isRoot ? '根目录' : path.split('/').filter(Boolean).slice(-1)[0];
    const isOpen = expanded.has(path);
    const isSelected = selectedPath === path;
    const nodeState = tree[path];
    const children = nodeState?.children ?? [];
    const hasChildren = children.length > 0 || isRoot;
    return (
      <div key={path}>
        <div
          className={`flex items-center gap-1 cursor-pointer px-2 py-1 rounded ${isSelected ? 'bg-primary/10 text-primary' : 'hover:bg-base-200'}`}
          style={{ paddingLeft: depth * 12 }}
          onClick={() => { setSelectedPath(path); navigateToPath(path); }}
        >
          <button
            className="btn btn-ghost btn-xs btn-circle"
            onClick={(e) => { e.stopPropagation(); if (hasChildren) toggleExpand(path); }}
          >
            {hasChildren ? (
              isOpen ? (
                <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M6 9l6 6 6-6"/></svg>
              ) : (
                <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M9 18l6-6-6-6"/></svg>
              )
            ) : (
              <span className="w-4 h-4"/>
            )}
          </button>
          <svg className={`w-4 h-4 ${isSelected ? 'text-primary' : 'text-base-content/70'}`} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 7h5l2 3h11v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"/>
          </svg>
          <span className="text-sm font-medium truncate">{nodeName}</span>
        </div>
        {isOpen && !nodeState?.loaded && (
          <div className="pl-6 py-1 text-xs opacity-70 flex items-center gap-2">
            <span className="loading loading-spinner loading-xs"/>
            Loading...
          </div>
        )}
        {isOpen && nodeState?.loaded && children.map((c) => renderTreeNode(c.path, depth + 1))}
      </div>
    );
  };

  const renderTree = () => renderTreeNode('/', 0);

  const saveSecretAndReload = async () => {
    if (!secretInput.trim()) return;
    setSecretSaving(true);
    try {
      await invoke('update_session_secret', { token: 'dev-token-1234', session_id: sessionId, sessionId, secret: secretInput.trim() });
      setShowSecretModal(false);
      setSecretInput('');
      // reload current directory
      loadDirectory(currentPath);
    } catch (e) {
      console.error('Failed to save secret:', e);
      alert('Failed to configure secret. Please check and try again.');
    } finally {
      setSecretSaving(false);
    }
  };

  const navigateToPath = (path: string) => {
    setCurrentPath(path);
    setSelectedPath(path);
    setSelectedFiles(new Set());
    // 展开并填充被导航到的目录的子目录
    setExpanded(prev => {
      const next = new Set(prev);
      next.add(path);
      return next;
    });
    // 懒加载子目录（不阻塞 UI）
    void ensureChildren(path, showHidden);
  };

  const navigateUp = () => {
    const parentPath = currentPath.split('/').slice(0, -1).join('/') || '/';
    navigateToPath(parentPath);
  };

  const handleFileClick = (file: FileItem) => {
    if (file.type === 'directory') {
      navigateToPath(file.path);
    } else {
      // Toggle selection for files
      const newSelected = new Set(selectedFiles);
      if (newSelected.has(file.path)) {
        newSelected.delete(file.path);
      } else {
        newSelected.add(file.path);
      }
      setSelectedFiles(newSelected);
    }
  };

  const downloadFile = async (filePath: string) => {
    try {
      const sessionInfo = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessionInfo.find(s => s.id === sessionId);
      const remoteEndpoint = (window as any).__wsEndpoint || found?.target;
      
      if (remoteEndpoint) {
        const fileData = await invoke('download_file_with_endpoint', {
          session_id: sessionId,
          sessionId,
          endpoint: remoteEndpoint,
          remote_path: filePath,
          remotePath: filePath,
        }) as number[];
        
        // 创建下载链接并触发下载，让浏览器处理保存位置选择
        const blob = new Blob([new Uint8Array(fileData)]);
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = filePath.split('/').pop() || 'download';
        a.style.display = 'none';
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
        
        // 提示用户文件已开始下载
        alert('文件下载已开始，请检查浏览器的下载文件夹');
      } else {
        // 对于本地文件，使用后端的下载功能
        await invoke('download_file', {
          session_id: sessionId,
          sessionId,
          remote_path: filePath,
          remotePath: filePath,
        });
        
        alert('文件下载完成');
      }
    } catch (error) {
      console.error('Failed to download file:', error);
      alert('下载文件失败: ' + String(error));
    }
  };

  const viewFile = async (filePath: string) => {
    try {
      const sessionInfo = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessionInfo.find(s => s.id === sessionId);
      const remoteEndpoint = (window as any).__wsEndpoint || found?.target;
      
      const content = await invoke('read_file', {
        state: undefined,
        session_id: sessionId,
        sessionId,
        endpoint: remoteEndpoint,
        file_path: filePath,
        filePath,
      }) as string;
      
      setCurrentFileContent(content);
      setCurrentFileName(filePath.split('/').pop() || '');
      setCurrentFilePath(filePath);
      setShowFileViewer(true);
    } catch (error) {
      console.error('Failed to view file:', error);
      alert('查看文件失败: ' + String(error));
    }
  };

  const editFile = async (filePath: string) => {
    try {
      const sessionInfo = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessionInfo.find(s => s.id === sessionId);
      const remoteEndpoint = (window as any).__wsEndpoint || found?.target;
      
      const content = await invoke('read_file', {
        state: undefined,
        session_id: sessionId,
        sessionId,
        endpoint: remoteEndpoint,
        file_path: filePath,
        filePath,
      }) as string;
      
      setCurrentFileContent(content);
      setCurrentFileName(filePath.split('/').pop() || '');
      setCurrentFilePath(filePath);
      setIsEditing(true);
      setShowFileEditor(true);
    } catch (error) {
      console.error('Failed to edit file:', error);
      alert('编辑文件失败: ' + String(error));
    }
  };

  const saveFile = async () => {
    try {
      const sessionInfo = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessionInfo.find(s => s.id === sessionId);
      const remoteEndpoint = (window as any).__wsEndpoint || found?.target;
      
      await invoke('write_file', {
        state: undefined,
        session_id: sessionId,
        sessionId,
        endpoint: remoteEndpoint,
        file_path: currentFilePath,
        filePath: currentFilePath,
        content: currentFileContent,
      });
      
      setShowFileEditor(false);
      setIsEditing(false);
      alert('文件保存成功!');
      loadDirectory(currentPath);
    } catch (error) {
      console.error('Failed to save file:', error);
      alert('保存文件失败: ' + String(error));
    }
  };

  const renameFile = async () => {
    if (!newFileName.trim()) return;
    
    try {
      const sessionInfo = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessionInfo.find(s => s.id === sessionId);
      const remoteEndpoint = (window as any).__wsEndpoint || found?.target;
      
      const newPath = currentFilePath.replace(/[^/]*$/, newFileName.trim());
      
      await invoke('rename_file', {
        state: undefined,
        session_id: sessionId,
        sessionId,
        endpoint: remoteEndpoint,
        old_path: currentFilePath,
        oldPath: currentFilePath,
        new_path: newPath,
        newPath,
      });
      
      setShowRenameModal(false);
      setNewFileName('');
      loadDirectory(currentPath);
    } catch (error) {
      console.error('Failed to rename file:', error);
      alert('重命名失败: ' + String(error));
    }
  };

  const createDirectory = async () => {
    if (!newDirName.trim()) return;
    
    try {
      const sessionInfo = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessionInfo.find(s => s.id === sessionId);
      const remoteEndpoint = (window as any).__wsEndpoint || found?.target;
      
      const dirPath = currentPath.endsWith('/') ? currentPath + newDirName.trim() : currentPath + '/' + newDirName.trim();
      
      await invoke('create_directory', {
        state: undefined,
        session_id: sessionId,
        sessionId,
        endpoint: remoteEndpoint,
        dir_path: dirPath,
        dirPath,
      });
      
      setShowCreateDirModal(false);
      setNewDirName('');
      loadDirectory(currentPath);
    } catch (error) {
      console.error('Failed to create directory:', error);
      alert('创建目录失败: ' + String(error));
    }
  };

  const copyFile = async (sourcePath: string) => {
    const destName = prompt('请输入复制后的文件名:', sourcePath.split('/').pop());
    if (!destName) return;
    
    try {
      const sessionInfo = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessionInfo.find(s => s.id === sessionId);
      const remoteEndpoint = (window as any).__wsEndpoint || found?.target;
      
      const destPath = currentPath.endsWith('/') ? currentPath + destName : currentPath + '/' + destName;
      
      await invoke('copy_file', {
        state: undefined,
        session_id: sessionId,
        sessionId,
        endpoint: remoteEndpoint,
        source_path: sourcePath,
        sourcePath,
        dest_path: destPath,
        destPath,
      });
      
      loadDirectory(currentPath);
    } catch (error) {
      console.error('Failed to copy file:', error);
      alert('复制文件失败: ' + String(error));
    }
  };

  const deleteFiles = async () => {
    if (selectedFiles.size === 0) return;
    
    const confirmed = confirm(`Delete ${selectedFiles.size} selected item(s)?`);
    if (!confirmed) return;

    try {
      await invoke('delete_files', {
        session_id: sessionId,
        sessionId,
        paths: Array.from(selectedFiles)
      });
      setSelectedFiles(new Set());
      loadDirectory(currentPath);
    } catch (error) {
      console.error('Failed to delete files:', error);
    }
  };

  const uploadFile = async (file: File) => {
    try {
      setUploadProgress(0);
      const sessionInfo = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessionInfo.find(s => s.id === sessionId);
      const remoteEndpoint = (window as any).__wsEndpoint || found?.target;
      
      await invoke('upload_file', {
        session_id: sessionId,
        sessionId,
        endpoint: remoteEndpoint,
        file_name: file.name,
        fileName: file.name,
        remote_path: currentPath,
        remotePath: currentPath,
        file_data: Array.from(new Uint8Array(await file.arrayBuffer())),
        fileData: Array.from(new Uint8Array(await file.arrayBuffer()))
      });
      loadDirectory(currentPath);
      setShowUpload(false);
      setShowUploadConfirm(false);
      setSelectedUploadFile(null);
      alert('文件上传成功!');
    } catch (error) {
      console.error('Failed to upload file:', error);
      alert('上传文件失败: ' + String(error));
    }
  };

  const handleFileSelect = (file: File) => {
    setSelectedUploadFile(file);
    setShowUploadConfirm(true);
  };

  const confirmUpload = () => {
    if (selectedUploadFile) {
      uploadFile(selectedUploadFile);
    }
  };

  const createFile = async () => {
    if (!newFileName.trim()) return;
    
    try {
      const sessionInfo = await invoke('get_active_sessions', { token: 'dev-token-1234' }) as any[];
      const found = sessionInfo.find(s => s.id === sessionId);
      const remoteEndpoint = (window as any).__wsEndpoint || found?.target;
      
      const filePath = currentPath.endsWith('/') ? currentPath + newFileName.trim() : currentPath + '/' + newFileName.trim();
      
      await invoke('write_file', {
        state: undefined,
        session_id: sessionId,
        sessionId,
        endpoint: remoteEndpoint,
        file_path: filePath,
        filePath,
        content: newFileContent,
      });
      
      setShowCreateFileModal(false);
      setNewFileName('');
      setNewFileContent('');
      loadDirectory(currentPath);
      alert('文件创建成功!');
    } catch (error) {
      console.error('Failed to create file:', error);
      alert('创建文件失败: ' + String(error));
    }
  };

  const formatFileSize = (bytes: number) => {
    const units = ['B', 'KB', 'MB', 'GB'];
    let size = bytes;
    let unitIndex = 0;
    
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }
    
    return `${size.toFixed(1)} ${units[unitIndex]}`;
  };

  const formatDate = (dateString: string) => {
    return new Date(dateString).toLocaleString();
  };

  const getFileIcon = (file: FileItem) => {
    const iconFolder = (
      <svg className="w-full h-full" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M3 7h5l2 3h11v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"/>
      </svg>
    );
    const iconFile = (
      <svg className="w-full h-full" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z"/>
        <path d="M14 2v6h6"/>
      </svg>
    );
    const iconCode = (
      <svg className="w-full h-full" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M16 18l6-6-6-6"/><path d="M8 6l-6 6 6 6"/>
      </svg>
    );
    const iconImage = (
      <svg className="w-full h-full" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect x="3" y="3" width="18" height="18" rx="2"/>
        <circle cx="8.5" cy="8.5" r="1.5"/>
        <path d="M21 15l-5-5L5 21"/>
      </svg>
    );
    const iconBox = (
      <svg className="w-full h-full" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 16V8a2 2 0 0 0-1-1.73L13 2.27a2 2 0 0 0-2 0L4 6.27A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
        <path d="M3.27 6.96L12 12l8.73-5.04"/>
      </svg>
    );
    const iconCog = (
      <svg className="w-full h-full" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="3"/>
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06A1.65 1.65 0 0 0 15 19.4a1.65 1.65 0 0 0-1 .6 1.65 1.65 0 0 0-.35 1.05V22a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 8 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.6 15a1.65 1.65 0 0 0-1-.6A1.65 1.65 0 0 0 2.55 13H2a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 3.6 8a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-.6A1.65 1.65 0 0 0 10.35 3V2a2 2 0 1 1 4 0v.09A1.65 1.65 0 0 0 15 4.6a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9c.36 0 .7.1 1 .26"/>
      </svg>
    );

    if (file.type === 'directory') return iconFolder;

    const ext = file.name.split('.').pop()?.toLowerCase();
    switch (ext) {
      case 'txt': case 'log':
        return iconFile;
      case 'js': case 'ts': case 'py': case 'php':
        return iconCode;
      case 'jpg': case 'png': case 'gif': case 'jpeg': case 'webp':
        return iconImage;
      case 'zip': case 'tar': case 'gz': case '7z': case 'rar':
        return iconBox;
      case 'exe': case 'bin': case 'sh':
        return iconCog;
      default:
        return iconFile;
    }
  };

  const sortedFiles = [...files].sort((a, b) => {
    // Directories first
    if (a.type !== b.type) {
      return a.type === 'directory' ? -1 : 1;
    }
    
    let comparison = 0;
    switch (sortBy) {
      case 'name':
        comparison = a.name.localeCompare(b.name);
        break;
      case 'size':
        comparison = a.size - b.size;
        break;
      case 'modified':
        comparison = new Date(a.modified).getTime() - new Date(b.modified).getTime();
        break;
    }
    
    return sortOrder === 'asc' ? comparison : -comparison;
  });

  const filteredFiles = sortedFiles.filter(file => showHidden || !file.isHidden);

  return (
    <div className="h-full min-h-0 flex flex-col bg-base-100 rounded-lg border border-base-300 overflow-hidden">
      {/* Elegant Header */}
      <div className="p-2 md:p-3 border-b border-base-300 bg-base-100/80 backdrop-blur supports-[backdrop-filter]:bg-base-100/60">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <div className="w-5 h-5 rounded-md bg-secondary text-secondary-content grid place-items-center">
              <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M3 7h5l2 3h11v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"/>
              </svg>
            </div>
            <div>
              <h2 className="text-base md:text-xm font-semibold">文件管理器</h2>
              {/* <p className="text-xs opacity-70">浏览和管理远程文件系统</p> */}
            </div>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => setShowUpload(true)}
              className="btn btn-accent btn-xs shadow-lg hover:shadow-xl transition-all duration-300"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
              </svg>
              上传
            </button>
            <button
              onClick={() => setShowCreateDirModal(true)}
              className="btn btn-info btn-xs shadow-lg hover:shadow-xl transition-all duration-300"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
              新建文件夹
            </button>
            <button
              onClick={() => setShowCreateFileModal(true)}
              className="btn btn-success btn-xs shadow-lg hover:shadow-xl transition-all duration-300"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
              新建文件
            </button>
            <button
              onClick={() => {
                const filePath = Array.from(selectedFiles)[0];
                const file = files.find(f => f.path === filePath);
                if (file && file.type === 'file') {
                  viewFile(filePath);
                }
              }}
              disabled={selectedFiles.size !== 1 || !files.find(f => f.path === Array.from(selectedFiles)[0])?.type || files.find(f => f.path === Array.from(selectedFiles)[0])?.type === 'directory'}
              className="btn btn-primary btn-xs shadow-lg hover:shadow-xl transition-all duration-300"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
              </svg>
              查看
            </button>
            <button
              onClick={() => {
                const filePath = Array.from(selectedFiles)[0];
                const file = files.find(f => f.path === filePath);
                if (file && file.type === 'file') {
                  editFile(filePath);
                }
              }}
              disabled={selectedFiles.size !== 1 || !files.find(f => f.path === Array.from(selectedFiles)[0])?.type || files.find(f => f.path === Array.from(selectedFiles)[0])?.type === 'directory'}
              className="btn btn-warning btn-xs shadow-lg hover:shadow-xl transition-all duration-300"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
              </svg>
              编辑
            </button>
            <button
              onClick={() => downloadFile(Array.from(selectedFiles)[0])}
              disabled={selectedFiles.size !== 1}
              className="btn btn-success btn-xs shadow-lg hover:shadow-xl transition-all duration-300"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 10v6m0 0l-3-3m3 3l3-3m2 8H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
              下载
            </button>
            <button
              onClick={() => {
                const filePath = Array.from(selectedFiles)[0];
                setCurrentFilePath(filePath);
                setNewFileName(filePath.split('/').pop() || '');
                setShowRenameModal(true);
              }}
              disabled={selectedFiles.size !== 1}
              className="btn btn-secondary btn-xs shadow-lg hover:shadow-xl transition-all duration-300"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
              </svg>
              重命名
            </button>
            <button
              onClick={() => copyFile(Array.from(selectedFiles)[0])}
              disabled={selectedFiles.size !== 1}
              className="btn btn-neutral btn-xs shadow-lg hover:shadow-xl transition-all duration-300"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
              </svg>
              复制
            </button>
            <button
              onClick={deleteFiles}
              disabled={selectedFiles.size === 0}
              className="btn btn-error btn-xs shadow-lg hover:shadow-xl transition-all duration-300"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
              删除
            </button>
          </div>
        </div>
      </div>

      {/* Navigation and Controls */}
      <div className="bg-base-100 shadow-lg border-b border-base-300 shrink-0">
        {/* Path Navigation */}
        <div className="p-1 border-b border-base-200">
          <div className="flex items-center gap-3">
            <button
              onClick={navigateUp}
              disabled={currentPath === '/'}
              className="btn btn-circle btn-xs btn-ghost hover:btn-primary transition-all duration-300"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
              </svg>
            </button>
            <div className="flex-1">
              <div className="breadcrumbs text-sm">
                <ul>
                  {currentPath.split('/').filter(Boolean).reduce((acc, part, index, arr) => {
                    const path = '/' + arr.slice(0, index + 1).join('/');
                    acc.push(
                      <li key={path}>
                        <a 
                          onClick={() => navigateToPath(path)}
                          className="hover:text-primary cursor-pointer transition-colors"
                        >
                          {part}
                        </a>
                      </li>
                    );
                    return acc;
                  }, [<li key="root"><a onClick={() => navigateToPath('/')} className="hover:text-primary cursor-pointer transition-colors">
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="m2.25 12 8.954-8.955c.44-.439 1.152-.439 1.591 0L21.75 12M4.5 9.75v10.125c0 .621.504 1.125 1.125 1.125H9.75v-4.875c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125V21h4.125c.621 0 1.125-.504 1.125-1.125V9.75M8.25 21h8.25" />
                    </svg>
                    根目录
                    </a></li>])}
                </ul>
              </div>
            </div>
            <div className="badge badge-outline font-mono text-xs">
              {currentPath}
            </div>
          </div>
        </div>

        {/* Controls */}
        <div className="p-1">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-6">
              <div className="form-control">
                <label className="label cursor-pointer gap-3">
                  <input
                    type="checkbox"
                    checked={showHidden}
                    onChange={(e) => setShowHidden(e.target.checked)}
                    className="checkbox checkbox-primary checkbox-xs"
                  />
                  <span className="label-text font-medium text-sm opacity-70">显示隐藏文件</span>
                </label>
              </div>
              
              <div className="flex items-center gap-3">
                <span className="text-sm font-medium opacity-70">排序方式:</span>
                <div className="join">
                  <select
                    value={sortBy}
                    onChange={(e) => setSortBy(e.target.value as any)}
                    className="select select-xs select-bordered join-item"
                  >
                    <option value="name">名称</option>
                    <option value="size">大小</option>
                    <option value="modified">修改时间</option>
                  </select>
                  <button
                    onClick={() => setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc')}
                    className="btn btn-ghost btn-sm join-item hover:btn-primary transition-all duration-300"
                  >
                    {sortOrder === 'asc' ? (
                      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
                      </svg>
                    ) : (
                      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                      </svg>
                    )}
                  </button>
                </div>
              </div>
            </div>
            
            {selectedFiles.size > 0 && (
              <div className="flex items-center gap-2">
                <div className="badge badge-primary badge-lg gap-2 shadow-lg">
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  已选择 {selectedFiles.size} 个项目
                </div>
                <button
                  onClick={() => setSelectedFiles(new Set())}
                  className="btn btn-ghost btn-sm btn-circle"
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Split Pane: Tree + File List */}
      <div className="flex-1 min-h-0 bg-base-50 flex overflow-hidden">
        {/* Left: Folder Tree */}
        <aside className="w-64 border-r border-base-300 overflow-auto hidden md:block">
          <div className="p-2 text-sm font-semibold opacity-70">目录</div>
          <div className="pb-4">
            {renderTree()}
          </div>
        </aside>
        {/* Right: File List */}
        <div className="flex-1 min-h-0 overflow-y-auto">
        {isLoading ? (
          <div className="hero h-full bg-gradient-to-br from-base-100 to-base-200">
            <div className="hero-content text-center">
              <div className="max-w-md">
                <div className="relative">
                  <span className="loading loading-spinner loading-lg text-primary"></span>
                  <div className="absolute inset-0 loading loading-spinner loading-lg text-secondary opacity-50 animate-pulse"></div>
                </div>
                <h3 className="text-xl font-bold mt-6 mb-2">正在加载目录</h3>
                <p className="opacity-70">请稍候，正在获取文件列表...</p>
              </div>
            </div>
          </div>
        ) : filteredFiles.length === 0 ? (
          <div className="hero h-full bg-gradient-to-br from-base-100 to-base-200">
            <div className="hero-content text-center">
              <div className="max-w-md">
                          <div className="mb-6 animate-bounce">
                            <svg className="w-16 h-16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                              <path d="M3 7h5l2 3h11v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"/>
                            </svg>
                          </div>
                <h3 className="text-2xl font-bold mb-3">目录为空</h3>
                <p className="opacity-70 mb-6">此目录中没有找到任何文件或文件夹</p>
                <button
                  onClick={() => setShowUpload(true)}
                  className="btn btn-primary btn-wide shadow-lg hover:shadow-xl transition-all duration-300"
                >
                  <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                  </svg>
                  上传第一个文件
                </button>
              </div>
            </div>
          </div>
        ) : (
          <div className="p-2">
            <div className="grid gap-2">
              {filteredFiles.map((file) => (
                <div
                  key={file.path}
                  onClick={() => handleFileClick(file)}
                  className={`card bg-base-100 shadow-sm hover:shadow-lg cursor-pointer transition-all duration-300 border-2 ${
                    selectedFiles.has(file.path) 
                      ? 'border-primary bg-primary/5 shadow-primary/20' 
                      : 'border-transparent hover:border-base-300'
                  }`}
                >
                  <div className="card-body p-1">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-4 flex-1 min-w-0">
                        <div className="avatar placeholder">
                          <div className={`w-8 h-8 rounded-xl ${
                            file.type === 'directory' 
                              ? 'bg-primary/10 text-primary' 
                              : 'bg-secondary/10 text-secondary'
                          }`}>
                            <span className="w-6 h-6 text-base-content/80">{getFileIcon(file)}</span>
                          </div>
                        </div>
                        
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2 mb-1">
                            <h3 className={`font-semibold truncate ${
                              file.isHidden ? 'opacity-50' : ''
                            }`}>
                              {file.name}
                            </h3>
                            {file.type === 'directory' && (
                              <div className="badge badge-primary badge-sm">文件夹</div>
                            )}
                            {file.isHidden && (
                              <div className="badge badge-ghost badge-sm">隐藏</div>
                            )}
                          </div>
                          
                          <div className="flex items-center gap-4 text-sm opacity-70">
                            <span className="flex items-center gap-1">
                              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                              </svg>
                              {file.owner}
                            </span>
                            <span className="flex items-center gap-1">
                              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                              </svg>
                              {file.permissions}
                            </span>
                          </div>
                        </div>
                      </div>
                      
                      <div className="text-right">
                        <div className="font-mono text-sm font-semibold mb-1">
                          {file.type === 'file' ? formatFileSize(file.size) : '—'}
                        </div>
                        <div className="text-xs opacity-70">
                          {formatDate(file.modified)}
                        </div>
                      </div>
                      
                      {selectedFiles.has(file.path) && (
                        <div className="ml-3">
                          <div className="w-6 h-6 bg-primary rounded-full flex items-center justify-center">
                            <svg className="w-4 h-4 text-primary-content" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                            </svg>
                          </div>
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
        </div>
      </div>

      {/* Upload Modal */}
      {showUpload && (
        <div className="modal modal-open">
          <div className="modal-box max-w-2xl bg-gradient-to-br from-base-100 to-base-200">
            <div className="flex items-center justify-between mb-6">
              <div className="flex items-center gap-3">
                <div className="avatar placeholder">
                  <div className="bg-primary text-primary-content rounded-xl w-12">
                    <span className="text-2xl">📤</span>
                  </div>
                </div>
                <div>
                  <h3 className="font-bold text-xl">上传文件</h3>
                  <p className="text-sm opacity-70">上传到: {currentPath}</p>
                </div>
              </div>
              <button
                onClick={() => setShowUpload(false)}
                className="btn btn-ghost btn-circle"
              >
                <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
            
            <div className="space-y-6">
              <div className="card bg-base-100 shadow-lg border-2 border-dashed border-primary/30 hover:border-primary transition-all duration-300">
                <div className="card-body p-8 text-center">
                          <div className="mb-4 animate-bounce">
                            <svg className="w-12 h-12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                              <path d="M20 17.58A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 4 16.25"/>
                            </svg>
                          </div>
                  <h4 className="text-lg font-semibold mb-2">选择要上传的文件</h4>
                  <p className="text-sm opacity-70 mb-6">支持所有文件类型，拖拽或点击选择</p>
                  
                  <input
                    type="file"
                    onChange={(e) => {
                      const file = e.target.files?.[0];
                      if (file) handleFileSelect(file);
                    }}
                    className="file-input file-input-bordered file-input-primary w-full max-w-sm shadow-lg"
                    id="file-upload"
                  />
                </div>
              </div>
              
              {uploadProgress > 0 && (
                <div className="card bg-base-100 shadow-lg">
                  <div className="card-body p-6">
                    <div className="flex items-center justify-between mb-3">
                      <div className="flex items-center gap-2">
                        <span className="loading loading-spinner loading-sm text-primary"></span>
                        <span className="font-medium">正在上传文件...</span>
                      </div>
                      <span className="font-mono text-sm font-bold">{uploadProgress}%</span>
                    </div>
                    <progress 
                      className="progress progress-primary w-full h-3 shadow-inner" 
                      value={uploadProgress} 
                      max="100"
                    ></progress>
                    <div className="text-xs opacity-70 mt-2 text-center">
                      请保持网络连接稳定，不要关闭此窗口
                    </div>
                  </div>
                </div>
              )}

              <div className="alert alert-info shadow-lg">
                <svg className="stroke-current shrink-0 w-6 h-6" fill="none" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                </svg>
                <div>
                  <h3 className="font-bold">上传提示</h3>
                  <div className="text-xs">
                    • 大文件上传可能需要较长时间，请耐心等待<br/>
                    • 上传完成后文件将自动显示在当前目录中<br/>
                    • 如果同名文件已存在，将会被覆盖
                  </div>
                </div>
              </div>
            </div>
            
            <div className="modal-action">
              <button
                onClick={() => setShowUpload(false)}
                className="btn btn-ghost shadow-lg hover:shadow-xl transition-all duration-300"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
                取消
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Secret Modal */}
      {showSecretModal && (
        <div className="modal modal-open">
          <div className="modal-box max-w-md">
            <h3 className="font-bold text-lg mb-2">Configure Secret</h3>
            <p className="text-sm opacity-70 mb-4">This session requires a secret to access remote file system.</p>
            <div className="form-control">
              <label className="label"><span className="label-text">Secret</span></label>
              <input
                type="password"
                className="input input-bordered w-full"
                value={secretInput}
                onChange={(e) => setSecretInput(e.target.value)}
                placeholder="Enter session secret"
              />
            </div>
            <div className="modal-action">
              <button className="btn btn-ghost" onClick={() => setShowSecretModal(false)}>Cancel</button>
              <button className={`btn btn-primary ${secretSaving ? 'btn-disabled' : ''}`} onClick={saveSecretAndReload}>
                {secretSaving ? 'Saving...' : 'Save & Retry'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 文件查看模态框 */}
        {showFileViewer && (
          <div className="modal modal-open">
            <div className="modal-box max-w-4xl max-h-[80vh]">
              <h3 className="font-bold text-lg mb-4">查看文件: {currentFileName}</h3>
              <div className="bg-base-200 p-4 rounded-lg max-h-96 overflow-auto">
                <pre className="whitespace-pre-wrap text-sm">{currentFileContent}</pre>
              </div>
              <div className="modal-action">
                <button
                  onClick={() => setShowFileViewer(false)}
                  className="btn btn-ghost"
                >
                  关闭
                </button>
              </div>
            </div>
          </div>
        )}
  
        {/* 文件编辑模态框 */}
        {showFileEditor && (
          <div className="modal modal-open">
            <div className="modal-box max-w-4xl max-h-[80vh]">
              <h3 className="font-bold text-lg mb-4">编辑文件: {currentFileName}</h3>
              <textarea
                value={currentFileContent}
                onChange={(e) => setCurrentFileContent(e.target.value)}
                className="textarea textarea-bordered w-full h-96 font-mono text-sm"
                placeholder="文件内容..."
              />
              <div className="modal-action">
                <button
                  onClick={() => setShowFileEditor(false)}
                  className="btn btn-ghost"
                >
                  取消
                </button>
                <button
                  onClick={saveFile}
                  className="btn btn-primary"
                  disabled={!isEditing}
                >
                  保存
                </button>
              </div>
            </div>
          </div>
        )}

      {/* 重命名模态框 */}
      {showRenameModal && (
        <div className="modal modal-open">
          <div className="modal-box">
            <h3 className="font-bold text-lg mb-4">重命名</h3>
            <div className="form-control">
              <label className="label">
                <span className="label-text">新名称</span>
              </label>
              <input
                type="text"
                value={newFileName}
                onChange={(e) => setNewFileName(e.target.value)}
                className="input input-bordered w-full"
                placeholder="输入新名称..."
              />
            </div>
            <div className="modal-action">
              <button
                onClick={() => setShowRenameModal(false)}
                className="btn btn-ghost"
              >
                取消
              </button>
              <button
                onClick={renameFile}
                className="btn btn-primary"
                disabled={!newFileName.trim()}
              >
                重命名
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 上传确认对话框 */}
      {showUploadConfirm && selectedUploadFile && (
        <div className="modal modal-open">
          <div className="modal-box">
            <h3 className="font-bold text-lg mb-4">确认上传文件</h3>
            <div className="space-y-4">
              <div className="alert alert-info">
                <svg className="stroke-current shrink-0 w-6 h-6" fill="none" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                </svg>
                <div>
                  <h3 className="font-bold">文件信息</h3>
                  <div className="text-sm">
                    <p><strong>文件名:</strong> {selectedUploadFile.name}</p>
                    <p><strong>大小:</strong> {(selectedUploadFile.size / 1024).toFixed(2)} KB</p>
                    <p><strong>上传到:</strong> {currentPath}</p>
                  </div>
                </div>
              </div>
              <p className="text-sm opacity-70">确定要上传这个文件吗？如果同名文件已存在，将会被覆盖。</p>
            </div>
            <div className="modal-action">
              <button
                onClick={() => {
                  setShowUploadConfirm(false);
                  setSelectedUploadFile(null);
                }}
                className="btn btn-ghost"
              >
                取消
              </button>
              <button
                onClick={confirmUpload}
                className="btn btn-primary"
              >
                确认上传
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 创建目录模态框 */}
      {showCreateDirModal && (
        <div className="modal modal-open">
          <div className="modal-box">
            <h3 className="font-bold text-lg mb-4">创建新文件夹</h3>
            <div className="form-control">
              <label className="label">
                <span className="label-text">文件夹名称</span>
              </label>
              <input
                type="text"
                value={newDirName}
                onChange={(e) => setNewDirName(e.target.value)}
                className="input input-bordered w-full"
                placeholder="输入文件夹名称..."
              />
            </div>
            <div className="modal-action">
              <button
                onClick={() => setShowCreateDirModal(false)}
                className="btn btn-ghost"
              >
                取消
              </button>
              <button
                onClick={createDirectory}
                className="btn btn-primary"
                disabled={!newDirName.trim()}
              >
                创建
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 创建文件模态框 */}
      {showCreateFileModal && (
        <div className="modal modal-open">
          <div className="modal-box max-w-2xl">
            <h3 className="font-bold text-lg mb-4">创建新文件</h3>
            <div className="space-y-4">
              <div className="form-control">
                <label className="label">
                  <span className="label-text">文件名</span>
                </label>
                <input
                  type="text"
                  value={newFileName}
                  onChange={(e) => setNewFileName(e.target.value)}
                  className="input input-bordered w-full"
                  placeholder="输入文件名（包含扩展名）..."
                />
              </div>
              <div className="form-control">
                <label className="label">
                  <span className="label-text">文件内容</span>
                </label>
                <textarea
                  value={newFileContent}
                  onChange={(e) => setNewFileContent(e.target.value)}
                  className="textarea textarea-bordered w-full h-40 font-mono text-sm"
                  placeholder="输入文件内容（可选）..."
                />
              </div>
              <div className="text-sm opacity-70">
                <p><strong>创建位置:</strong> {currentPath}</p>
              </div>
            </div>
            <div className="modal-action">
              <button
                onClick={() => {
                  setShowCreateFileModal(false);
                  setNewFileName('');
                  setNewFileContent('');
                }}
                className="btn btn-ghost"
              >
                取消
              </button>
              <button
                onClick={createFile}
                className="btn btn-primary"
                disabled={!newFileName.trim()}
              >
                创建文件
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}