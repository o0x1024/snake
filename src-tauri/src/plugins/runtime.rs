use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use wasmtime::{Engine, Module, Store, Instance, Func, Caller};

use crate::error::{AuroraResult, PluginError};

pub struct PluginRuntime {
    engine: Engine,
    modules: Arc<RwLock<HashMap<String, Module>>>,
    instances: Arc<RwLock<HashMap<String, Instance>>>,
}

impl PluginRuntime {
    pub fn new() -> AuroraResult<Self> {
        let engine = Engine::default();
        
        Ok(Self {
            engine,
            modules: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn load_plugin(&self, name: String, wasm_bytes: &[u8]) -> AuroraResult<()> {
        let module = Module::from_binary(&self.engine, wasm_bytes)
            .map_err(|e| PluginError::LoadFailed(e.to_string()))?;

        let mut modules = self.modules.write().await;
        modules.insert(name, module);

        Ok(())
    }

    pub async fn instantiate_plugin(&self, name: &str) -> AuroraResult<()> {
        let modules = self.modules.read().await;
        let module = modules.get(name)
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;

        let mut store = Store::new(&self.engine, ());
        
        // Define host functions that plugins can call
        let log_func = Func::wrap(&mut store, |caller: Caller<'_, ()>, ptr: i32, len: i32| {
            // This would extract memory from the caller and log the message
            println!("Plugin log: ptr={}, len={}", ptr, len);
        });

        let instance = Instance::new(&mut store, module, &[log_func.into()])
            .map_err(|e| PluginError::LoadFailed(e.to_string()))?;

        let mut instances = self.instances.write().await;
        instances.insert(name.to_string(), instance);

        Ok(())
    }

    pub async fn execute_plugin_function(
        &self,
        plugin_name: &str,
        function_name: &str,
        args: &[i32],
    ) -> AuroraResult<Vec<i32>> {
        let instances = self.instances.read().await;
        let instance = instances.get(plugin_name)
            .ok_or_else(|| PluginError::NotFound(plugin_name.to_string()))?;

        let mut store = Store::new(&self.engine, ());
        
        let func = instance.get_func(&mut store, function_name)
            .ok_or_else(|| PluginError::ExecutionFailed(
                format!("Function '{}' not found in plugin '{}'", function_name, plugin_name)
            ))?;

        // Convert args to wasmtime values
        let wasm_args: Vec<wasmtime::Val> = args.iter()
            .map(|&arg| wasmtime::Val::I32(arg))
            .collect();

        let mut results = vec![wasmtime::Val::I32(0); func.ty(&store).results().len()];
        
        func.call(&mut store, &wasm_args, &mut results)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // Convert results back to i32
        let int_results: Vec<i32> = results.iter()
            .filter_map(|val| {
                if let wasmtime::Val::I32(i) = val {
                    Some(*i)
                } else {
                    None
                }
            })
            .collect();

        Ok(int_results)
    }

    pub async fn unload_plugin(&self, name: &str) -> AuroraResult<()> {
        let mut modules = self.modules.write().await;
        let mut instances = self.instances.write().await;
        
        modules.remove(name);
        instances.remove(name);
        
        Ok(())
    }

    pub async fn list_loaded_plugins(&self) -> AuroraResult<Vec<String>> {
        let modules = self.modules.read().await;
        Ok(modules.keys().cloned().collect())
    }

    pub async fn get_plugin_info(&self, name: &str) -> AuroraResult<PluginInfo> {
        let modules = self.modules.read().await;
        let instances = self.instances.read().await;
        
        let has_module = modules.contains_key(name);
        let has_instance = instances.contains_key(name);
        
        if !has_module {
            return Err(PluginError::NotFound(name.to_string()).into());
        }

        Ok(PluginInfo {
            name: name.to_string(),
            loaded: has_module,
            instantiated: has_instance,
            functions: vec![], // Would need to extract from module
        })
    }
}

#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub loaded: bool,
    pub instantiated: bool,
    pub functions: Vec<String>,
}

// Host functions that plugins can call
pub struct HostFunctions;

impl HostFunctions {
    pub fn log_message(message: &str) {
        tracing::info!("Plugin log: {}", message);
    }

    pub fn get_system_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    pub fn allocate_memory(size: usize) -> *mut u8 {
        let layout = std::alloc::Layout::from_size_align(size, 1).unwrap();
        unsafe { std::alloc::alloc(layout) }
    }

    pub unsafe fn deallocate_memory(ptr: *mut u8, size: usize) {
        let layout = std::alloc::Layout::from_size_align(size, 1).unwrap();
        std::alloc::dealloc(ptr, layout);
    }
}