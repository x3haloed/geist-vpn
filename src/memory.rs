//! Memory management bridge between Rust and SoftEtherVPN
//!
//! SoftEtherVPN uses custom memory allocators (Malloc, Free, ZeroMalloc) that need to be
//! properly integrated with Rust's ownership system and safety guarantees.

use crate::bindings::{softether_free, softether_malloc, softether_zero_malloc, UINT};
use crate::error::{Error, Result};
use std::alloc::{GlobalAlloc, Layout};
use std::ptr::NonNull;

/// Safe wrapper around SoftEther's Malloc function
///
/// Returns a Box that will automatically free the memory when dropped.
pub fn malloc_box<T>(value: T) -> Result<Box<T>> {
    #[cfg(not(test))]
    {
        let size = std::mem::size_of::<T>() as UINT;
        let ptr = softether_malloc(size).ok_or_else(|| Error::FfiError {
            message: "SoftEther malloc failed".into(),
        })?;

        unsafe {
            let typed_ptr = ptr.as_ptr() as *mut T;
            std::ptr::write(typed_ptr, value);
            Ok(Box::from_raw(typed_ptr))
        }
    }
    #[cfg(test)]
    {
        // During tests, use Rust's standard allocator
        Ok(Box::new(value))
    }
}

/// Safe wrapper around SoftEther's ZeroMalloc function
///
/// Returns a Box containing a zero-initialized value.
pub fn zero_malloc_box<T>() -> Result<Box<T>> {
    #[cfg(not(test))]
    {
        let byte_count = std::mem::size_of::<T>();
        let size = byte_count as UINT;
        let ptr = softether_zero_malloc(size).ok_or_else(|| Error::FfiError {
            message: "SoftEther zero malloc failed".into(),
        })?;

        unsafe {
            let typed_ptr = ptr.as_ptr() as *mut T;
            std::ptr::write_bytes(typed_ptr as *mut u8, 0, byte_count);
            Ok(Box::from_raw(typed_ptr))
        }
    }
    #[cfg(test)]
    {
        // During tests, use Rust's standard allocator with zeroed memory
        unsafe { Ok(Box::new(std::mem::zeroed())) }
    }
}

/// Allocate raw memory using SoftEther's allocator
///
/// Returns a RawMemory handle that automatically frees when dropped.
pub fn malloc_raw(size: usize) -> Result<RawMemory> {
    #[cfg(not(test))]
    {
        let ptr = softether_malloc(size as UINT).ok_or_else(|| Error::FfiError {
            message: "SoftEther malloc failed".into(),
        })?;

        Ok(RawMemory {
            ptr,
            size,
            is_softether_allocated: true,
        })
    }
    #[cfg(test)]
    {
        // During tests, use Rust's standard allocator
        let layout = Layout::from_size_align(size, std::mem::align_of::<u8>()).unwrap();
        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            return Err(Error::FfiError {
                message: "Rust alloc failed".into(),
            });
        }
        let non_null_ptr = unsafe { NonNull::new_unchecked(ptr as *mut std::ffi::c_void) };

        Ok(RawMemory {
            ptr: non_null_ptr,
            size,
            is_softether_allocated: false,
        })
    }
}

/// Zero-allocate raw memory using SoftEther's allocator
pub fn zero_malloc_raw(size: usize) -> Result<RawMemory> {
    #[cfg(not(test))]
    {
        let ptr = softether_zero_malloc(size as UINT).ok_or_else(|| Error::FfiError {
            message: "SoftEther zero malloc failed".into(),
        })?;

        Ok(RawMemory {
            ptr,
            size,
            is_softether_allocated: true,
        })
    }
    #[cfg(test)]
    {
        // During tests, use Rust's standard allocator
        let layout = Layout::from_size_align(size, std::mem::align_of::<u8>()).unwrap();
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            return Err(Error::FfiError {
                message: "Rust alloc_zeroed failed".into(),
            });
        }
        let non_null_ptr = unsafe { NonNull::new_unchecked(ptr as *mut std::ffi::c_void) };

        Ok(RawMemory {
            ptr: non_null_ptr,
            size,
            is_softether_allocated: false,
        })
    }
}

/// Handle to raw memory allocated with SoftEther's allocator
///
/// Automatically frees the memory when dropped.
pub struct RawMemory {
    ptr: NonNull<std::ffi::c_void>,
    size: usize,
    is_softether_allocated: bool,
}

impl RawMemory {
    /// Get the raw pointer
    pub fn as_ptr(&self) -> *mut std::ffi::c_void {
        self.ptr.as_ptr()
    }

    /// Get the size of the allocation
    pub fn size(&self) -> usize {
        self.size
    }

    /// Convert to a typed pointer (unsafe)
    pub unsafe fn as_typed_ptr<T>(&self) -> *mut T {
        self.ptr.as_ptr() as *mut T
    }
}

impl Drop for RawMemory {
    fn drop(&mut self) {
        if self.is_softether_allocated {
            #[cfg(not(test))]
            softether_free(self.ptr.as_ptr());
        } else {
            #[cfg(test)]
            unsafe {
                let layout =
                    Layout::from_size_align(self.size, std::mem::align_of::<u8>()).unwrap();
                std::alloc::dealloc(self.ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}

/// Custom allocator that uses SoftEther's memory functions
///
/// This can be used with Rust's allocation system when SoftEther
/// memory management is required.
pub struct SoftEtherAlloc;

unsafe impl GlobalAlloc for SoftEtherAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        softether_malloc(layout.size() as UINT)
            .map(|ptr| ptr.as_ptr() as *mut u8)
            .unwrap_or(std::ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        softether_free(ptr as *mut std::ffi::c_void);
    }
}

/// Utility for converting between Rust and SoftEther string formats
pub mod strings {
    use crate::bindings::{to_wide_string, MAX_ACCOUNT_NAME_LEN};
    use crate::error::Result;

    /// Convert a Rust string to a SoftEther wide string buffer
    pub fn rust_to_softether_wide(s: &str) -> Result<[u16; MAX_ACCOUNT_NAME_LEN + 1]> {
        let mut buffer = [0u16; MAX_ACCOUNT_NAME_LEN + 1];
        to_wide_string(s, &mut buffer).map_err(|e| crate::error::Error::EncodingError {
            message: format!("String conversion failed: {}", e),
        })?;
        Ok(buffer)
    }

    /// Convert a SoftEther wide string back to Rust string
    pub fn softether_wide_to_rust(wide_str: &[u16]) -> String {
        let end = wide_str
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(wide_str.len());
        String::from_utf16_lossy(&wide_str[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_conversion() {
        let test_str = "Hello VPN";
        let wide_buffer = strings::rust_to_softether_wide(test_str).unwrap();
        let back_to_rust = strings::softether_wide_to_rust(&wide_buffer);
        assert_eq!(test_str, back_to_rust);
    }

    #[test]
    fn test_raw_memory_allocation() {
        let mem = malloc_raw(64).unwrap();
        assert_eq!(mem.size(), 64);
        assert!(!mem.as_ptr().is_null());
        // Memory is automatically freed when mem goes out of scope
    }
}
