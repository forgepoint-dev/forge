#[no_mangle]
pub extern "C" fn run() -> i32 {
    let s = "Hello from the simplified C-style ABI issues extension!";
    let mut bytes = s.as_bytes().to_vec();
    bytes.push(0); // Add a null terminator for C-style strings
    let ptr = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    ptr as i32
}