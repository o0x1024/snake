use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

// Import host functions from Aurora
extern "C" {
    fn log(ptr: *const u8, len: usize);
    fn get_timestamp() -> i64;
    fn alloc(size: i32) -> i32;
    fn dealloc(ptr: i32, size: i32);
}

// Plugin metadata
#[derive(Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub functions: Vec<String>,
}

// Export plugin info
#[no_mangle]
pub extern "C" fn get_plugin_info() -> i32 {
    let info = PluginInfo {
        name: "template_plugin".to_string(),
        version: "1.0.0".to_string(),
        functions: vec![
            "scan_target".to_string(),
            "process_data".to_string(),
        ],
    };
    
    let json = serde_json::to_string(&info).unwrap();
    let bytes = json.as_bytes();
    
    // Allocate memory for the result
    let ptr = unsafe { alloc(bytes.len() as i32) };
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
    }
    
    ptr
}

// Main plugin function: scan_target
#[no_mangle]
pub extern "C" fn scan_target(target_ptr: i32, target_len: i32) -> i32 {
    // Read target from memory
    let target_bytes = unsafe {
        std::slice::from_raw_parts(target_ptr as *const u8, target_len as usize)
    };
    let target = String::from_utf8_lossy(target_bytes);
    
    // Log the operation
    let log_msg = format!("Scanning target: {}", target);
    unsafe {
        log(log_msg.as_ptr(), log_msg.len());
    }
    
    // Simulate scanning
    let timestamp = unsafe { get_timestamp() };
    
    let result = serde_json::json!({
        "target": target,
        "scan_time": timestamp,
        "vulnerabilities": [
            {
                "id": "CVE-2023-1234",
                "severity": "HIGH",
                "description": "SQL Injection vulnerability",
                "component": "login.php"
            }
        ],
        "status": "completed"
    });
    
    let result_str = result.to_string();
    let result_bytes = result_str.as_bytes();
    
    // Allocate memory for the result
    let result_ptr = unsafe { alloc(result_bytes.len() as i32) };
    unsafe {
        std::ptr::copy_nonoverlapping(result_bytes.as_ptr(), result_ptr as *mut u8, result_bytes.len());
    }
    
    result_ptr
}

// Helper function: process_data
#[no_mangle]
pub extern "C" fn process_data(data_ptr: i32, data_len: i32) -> i32 {
    let data_bytes = unsafe {
        std::slice::from_raw_parts(data_ptr as *const u8, data_len as usize)
    };
    
    // Process the data (example: count bytes)
    let result = serde_json::json!({
        "processed_bytes": data_bytes.len(),
        "timestamp": unsafe { get_timestamp() },
        "status": "success"
    });
    
    let result_str = result.to_string();
    let result_bytes = result_str.as_bytes();
    
    let result_ptr = unsafe { alloc(result_bytes.len() as i32) };
    unsafe {
        std::ptr::copy_nonoverlapping(result_bytes.as_ptr(), result_ptr as *mut u8, result_bytes.len());
    }
    
    result_ptr
}

// Memory management
#[no_mangle]
pub extern "C" fn free_memory(ptr: i32, len: i32) {
    unsafe {
        dealloc(ptr, len);
    }
}