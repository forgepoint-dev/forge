//! Example WASM extension for the Forge GraphQL server

use serde::{Deserialize, Serialize};
use std::slice;
use std::mem;

/// Extension configuration
#[derive(Debug, Deserialize)]
struct ExtConfig {
    name: String,
    version: String,
    database_path: String,
    custom_config: Option<String>,
}

/// Field resolution request
#[derive(Debug, Deserialize)]
struct FieldResolveRequest {
    field_name: String,
    parent_type: String,
    arguments: serde_json::Value,
    context: serde_json::Value,
    parent: Option<serde_json::Value>,
}

/// Field resolution response
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum FieldResolveResponse {
    #[serde(rename = "success")]
    Success { data: serde_json::Value },
    #[serde(rename = "error")]
    Error { message: String },
}

static mut EXTENSION_NAME: String = String::new();

/// Allocate memory for data exchange
#[no_mangle]
pub extern "C" fn alloc(size: u32) -> *mut u8 {
    let mut buf = Vec::with_capacity(size as usize);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr
}

/// Deallocate memory
#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, size: u32) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, size as usize, size as usize);
    }
}

/// Initialize the extension
#[no_mangle]
pub extern "C" fn init(ptr: *const u8, len: u32) -> u32 {
    let input = unsafe {
        let slice = slice::from_raw_parts(ptr, len as usize);
        std::str::from_utf8_unchecked(slice)
    };

    match serde_json::from_str::<ExtConfig>(input) {
        Ok(config) => {
            unsafe {
                EXTENSION_NAME = config.name.clone();
            }
            log(&format!("Extension {} initialized", config.name));
            0 // Success
        }
        Err(e) => {
            log(&format!("Failed to parse config: {}", e));
            1 // Error
        }
    }
}

/// Get the GraphQL schema
#[no_mangle]
pub extern "C" fn get_schema() -> *mut u8 {
    let schema = r#"
type User {
  id: ID!
  name: String!
  email: String!
}

extend type Query {
  user(id: ID!): User
  users: [User!]!
}
"#;

    return_string(schema)
}

/// Resolve a GraphQL field
#[no_mangle]
pub extern "C" fn resolve_field(ptr: *const u8, len: u32) -> *mut u8 {
    let input = unsafe {
        let slice = slice::from_raw_parts(ptr, len as usize);
        std::str::from_utf8_unchecked(slice)
    };

    let request: FieldResolveRequest = match serde_json::from_str(input) {
        Ok(r) => r,
        Err(e) => {
            let response = FieldResolveResponse::Error {
                message: format!("Failed to parse request: {}", e),
            };
            let json = serde_json::to_string(&response).unwrap();
            return return_string(&json);
        }
    };

    log(&format!("Resolving field: {}", request.field_name));

    let response = match request.field_name.as_str() {
        "user" => {
            // Extract the id argument
            let id = request.arguments
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("1");

            let user = serde_json::json!({
                "id": id,
                "name": format!("User {}", id),
                "email": format!("user{}@example.com", id),
            });

            FieldResolveResponse::Success { data: user }
        }
        "users" => {
            let users = serde_json::json!([
                {
                    "id": "1",
                    "name": "Alice",
                    "email": "alice@example.com"
                },
                {
                    "id": "2",
                    "name": "Bob",
                    "email": "bob@example.com"
                },
                {
                    "id": "3",
                    "name": "Charlie",
                    "email": "charlie@example.com"
                }
            ]);

            FieldResolveResponse::Success { data: users }
        }
        _ => FieldResolveResponse::Error {
            message: format!("Unknown field: {}", request.field_name),
        },
    };

    let json = serde_json::to_string(&response).unwrap();
    return_string(&json)
}

/// Helper to return a string from WASM
fn return_string(s: &str) -> *mut u8 {
    let bytes = s.as_bytes();
    let len = bytes.len() as u32;
    
    // Allocate memory for length + string
    let result_ptr = alloc(4 + len);
    
    unsafe {
        // Write length as little-endian u32
        let len_bytes = len.to_le_bytes();
        std::ptr::copy_nonoverlapping(len_bytes.as_ptr(), result_ptr, 4);
        
        // Write string data
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), result_ptr.offset(4), bytes.len());
    }
    
    result_ptr
}

/// Log a message (calls host function)
fn log(msg: &str) {
    unsafe {
        host_log(msg.as_ptr(), msg.len() as u32);
    }
}

// Host imports
extern "C" {
    fn host_log(ptr: *const u8, len: u32);
}
