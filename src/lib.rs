#![no_std]
#![feature(allocator_api)]
//! `snmalloc-rs` provides a wrapper for [`microsoft/snmalloc`](https://github.com/microsoft/snmalloc) to make it usable as a global allocator for rust.
//! snmalloc is a research allocator. Its key design features are:
//! - Memory that is freed by the same thread that allocated it does not require any synchronising operations.
//! - Freeing memory in a different thread to initially allocated it, does not take any locks and instead uses a novel message passing scheme to return the memory to the original allocator, where it is recycled.
//! - The allocator uses large ranges of pages to reduce the amount of meta-data required.
//!
//! The benchmark is available at the [paper](https://github.com/microsoft/snmalloc/blob/master/snmalloc.pdf) of `snmalloc`
//! There are three features defined in this crate:
//! - `debug`: Enable the `Debug` mode in `snmalloc`.
//! - `1mib`: Use the `1mib` chunk configuration.
//! - `cache-friendly`: Make the allocator more cache friendly (setting `CACHE_FRIENDLY_OFFSET` to `64` in building the library).
//!
//! The whole library supports `no_std`.
//!
//! To use `snmalloc-rs` add it as a dependency:
//! ```toml
//! # Cargo.toml
//! [dependencies]
//! snmalloc-rs = "0.1.0"
//! ```
//!
//! To set `SnMalloc` as the global allocator add this to your project:
//! ```rust
//! #[global_allocator]
//! static ALLOC: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;
//! ```
extern crate snmalloc_sys as ffi;

use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ptr::{slice_from_raw_parts_mut, NonNull};

pub struct SnMalloc;

unsafe impl GlobalAlloc for SnMalloc {
    /// Allocate the memory with the given alignment and size.
    /// On success, it returns a pointer pointing to the required memory address.
    /// On failure, it returns a null pointer.
    /// The client must assure the following things:
    /// - `alignment` is greater than zero
    /// - Other constrains are the same as the rust standard library.
    /// The program may be forced to abort if the constrains are not full-filled.
    #[inline(always)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ffi::sn_rust_alloc(layout.align(), layout.size()) as _
    }

    /// De-allocate the memory at the given address with the given alignment and size.
    /// The client must assure the following things:
    /// - the memory is acquired using the same allocator and the pointer points to the start position.
    /// - Other constrains are the same as the rust standard library.
    /// The program may be forced to abort if the constrains are not full-filled.
    #[inline(always)]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ffi::sn_rust_dealloc(ptr as _, layout.align(), layout.size());
    }

    /// Behaves like alloc, but also ensures that the contents are set to zero before being returned.
    #[inline(always)]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ffi::sn_rust_alloc_zeroed(layout.align(), layout.size()) as _
    }

    /// Re-allocate the memory at the given address with the given alignment and size.
    /// On success, it returns a pointer pointing to the required memory address.
    /// The memory content within the `new_size` will remains the same as previous.
    /// On failure, it returns a null pointer. In this situation, the previous memory is not returned to the allocator.
    /// The client must assure the following things:
    /// - the memory is acquired using the same allocator and the pointer points to the start position
    /// - `alignment` fulfills all the requirements as `rust_alloc`
    /// - Other constrains are the same as the rust standard library.
    /// The program may be forced to abort if the constrains are not full-filled.
    #[inline(always)]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ffi::sn_rust_realloc(ptr as _, layout.align(), layout.size(), new_size) as _
    }
}

#[derive(Debug)]
pub struct SnAllocator {
    alloc: *mut ffi::Alloc,
}

impl SnAllocator {
    pub fn new() -> Self {
        unsafe {
            SnAllocator {
                alloc: ffi::sn_rust_allocator_new(),
            }
        }
    }
}

impl Drop for SnAllocator {
    fn drop(&mut self) {
        unsafe {
            ffi::sn_rust_allocator_drop(self.alloc);
        }
    }
}

unsafe fn construct_alloc_result(
    ptr: *mut core::ffi::c_void,
    layout: &Layout,
) -> Result<NonNull<[u8]>, AllocError> {
    if ptr.is_null() {
        Err(AllocError)
    } else {
        let fat_ptr = slice_from_raw_parts_mut(ptr as *mut u8, layout.size());
        Ok(NonNull::from(&*fat_ptr))
    }
}

unsafe impl Allocator for SnAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        unsafe {
            let ptr = ffi::sn_rust_allocator_allocate(self.alloc, layout.align(), layout.size());
            construct_alloc_result(ptr, &layout)
        }
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        unsafe {
            let ptr =
                ffi::sn_rust_allocator_allocate_zeroed(self.alloc, layout.align(), layout.size());
            construct_alloc_result(ptr, &layout)
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        ffi::sn_rust_allocator_deallocate(
            self.alloc,
            ptr.as_ptr() as _,
            layout.align(),
            layout.size(),
        );
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        let new_ptr = ffi::sn_rust_allocator_grow(
            self.alloc,
            ptr.as_ptr() as _,
            old_layout.align(),
            old_layout.size(),
            new_layout.align(),
            new_layout.size(),
        );
        construct_alloc_result(new_ptr, &new_layout)
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        let new_ptr = ffi::sn_rust_allocator_grow_zeroed(
            self.alloc,
            ptr.as_ptr() as _,
            old_layout.align(),
            old_layout.size(),
            new_layout.align(),
            new_layout.size(),
        );
        construct_alloc_result(new_ptr, &new_layout)
    }

    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        let new_ptr = ffi::sn_rust_allocator_shrink(
            self.alloc,
            ptr.as_ptr() as _,
            old_layout.align(),
            old_layout.size(),
            new_layout.align(),
            new_layout.size(),
        );
        construct_alloc_result(new_ptr, &new_layout)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    #[test]
    fn it_frees_allocated_memory() {
        unsafe {
            let layout = Layout::from_size_align(8, 8).unwrap();
            let alloc = SnMalloc;

            let ptr = alloc.alloc(layout);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_zero_allocated_memory() {
        unsafe {
            let layout = Layout::from_size_align(8, 8).unwrap();
            let alloc = SnMalloc;

            let ptr = alloc.alloc_zeroed(layout);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_reallocated_memory() {
        unsafe {
            let layout = Layout::from_size_align(8, 8).unwrap();
            let alloc = SnMalloc;

            let ptr = alloc.alloc(layout);
            let ptr = alloc.realloc(ptr, layout, 16);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn allocator_supports_vector() {
        let allocator = SnAllocator::new();
        let mut vec = std::vec::Vec::new_in(&allocator);
        let mut sum: usize = 0;
        for i in 1..512usize {
            vec.push(i);
            sum += i * i;
        }

        let res = vec.into_iter().flat_map(|x| {
            let mut v = std::vec::Vec::new_in(&allocator);
            for _ in 0..x {
                v.push(x);
            }
            v
        }).sum();

        assert_eq!(sum, res);
    }
}
