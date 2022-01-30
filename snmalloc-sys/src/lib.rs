#![no_std]
#![allow(non_camel_case_types)]

use {core::ffi::c_void, core::usize};

/// Opaque type for snmalloc allocator
pub enum Alloc {}

extern "C" {
    /// Allocate the memory with the given alignment and size.
    /// On success, it returns a pointer pointing to the required memory address.
    /// On failure, it returns a null pointer.
    /// The client must assure the following things:
    /// - `alignment` is greater than zero
    /// - `alignment` is a power of 2
    /// The program may be forced to abort if the constrains are not full-filled.
    pub fn sn_rust_alloc(alignment: usize, size: usize) -> *mut c_void;

    /// De-allocate the memory at the given address with the given alignment and size.
    /// The client must assure the following things:
    /// - the memory is acquired using the same allocator and the pointer points to the start position.
    /// - `alignment` and `size` is the same as allocation
    /// The program may be forced to abort if the constrains are not full-filled.
    pub fn sn_rust_dealloc(ptr: *mut c_void, alignment: usize, size: usize) -> c_void;

    /// Behaves like rust_alloc, but also ensures that the contents are set to zero before being returned.
    pub fn sn_rust_alloc_zeroed(alignment: usize, size: usize) -> *mut c_void;

    /// Re-allocate the memory at the given address with the given alignment and size.
    /// On success, it returns a pointer pointing to the required memory address.
    /// The memory content within the `new_size` will remains the same as previous.
    /// On failure, it returns a null pointer. In this situation, the previous memory is not returned to the allocator.
    /// The client must assure the following things:
    /// - the memory is acquired using the same allocator and the pointer points to the start position
    /// - `alignment` and `old_size` is the same as allocation
    /// - `alignment` fulfills all the requirements as `rust_alloc`
    /// The program may be forced to abort if the constrains are not full-filled.
    pub fn sn_rust_realloc(
        ptr: *mut c_void,
        alignment: usize,
        old_size: usize,
        new_size: usize,
    ) -> *mut c_void;

    /// Allocate `count` items of `size` length each.
    /// Returns `null` if `count * size` overflows or on out-of-memory.
    /// All items are initialized to zero.
    pub fn sn_calloc(count: usize, size: usize) -> *mut c_void;

    /// Allocate `size` bytes.
    /// Returns pointer to the allocated memory or null if out of memory.
    /// Returns a unique pointer if called with `size` 0.
    pub fn sn_malloc(size: usize) -> *mut c_void;

    /// Re-allocate memory to `newsize` bytes.
    /// Return pointer to the allocated memory or null if out of memory. If null
    /// is returned, the pointer `p` is not freed. Otherwise the original
    /// pointer is either freed or returned as the reallocated result (in case
    /// it fits in-place with the new size).
    /// If `p` is null, it behaves as [`sn_malloc`]. If `newsize` is larger than
    /// the original `size` allocated for `p`, the bytes after `size` are
    /// uninitialized.
    pub fn sn_realloc(p: *mut c_void, newsize: usize) -> *mut c_void;

    /// Free previously allocated memory.
    /// The pointer `p` must have been allocated before (or be null).
    pub fn sn_free(p: *mut c_void);

    /// Return the available bytes in a memory block.
    pub fn sn_malloc_usable_size(p: *const c_void) -> usize;

    /// Allocate a memory area with snmalloc internal API and return a pointer
    /// to initialized allocator.
    pub fn sn_rust_allocator_new() -> *mut Alloc;

    /// Teardown the allocator referenced by the pointer and release the associated
    /// memory area.
    pub fn sn_rust_allocator_drop(alloc: *mut Alloc);

    /// Allocate a memory area via a specific allocator.
    pub fn sn_rust_allocator_allocate(
        alloc: *mut Alloc,
        alignment: usize,
        size: usize,
    ) -> *mut c_void;

    /// Deallocate a memory via a specific allocator.
    pub fn sn_rust_allocator_deallocate(
        alloc: *mut Alloc,
        ptr: *mut c_void,
        alignment: usize,
        size: usize,
    ) -> *mut c_void;

    /// Deallocate a memory via a specific allocator. The memory area will be filled with zero.
    pub fn sn_rust_allocator_allocate_zeroed(
        alloc: *mut Alloc,
        alignment: usize,
        size: usize,
    ) -> *mut c_void;

    /// Grow a memory via a specific allocator.
    pub fn sn_rust_allocator_grow(
        alloc: *mut Alloc,
        ptr: *mut c_void,
        old_alignment: usize,
        old_size: usize,
        new_alignment: usize,
        new_size: usize,
    ) -> *mut c_void;

    /// Grow a memory via a specific allocator. The extra memory area will be filled with zero.
    pub fn sn_rust_allocator_grow_zeroed(
        alloc: *mut Alloc,
        ptr: *mut c_void,
        old_alignment: usize,
        old_size: usize,
        new_alignment: usize,
        new_size: usize,
    ) -> *mut c_void;

    /// Shrink a memory via a specific allocator.
    pub fn sn_rust_allocator_shrink(
        alloc: *mut Alloc,
        ptr: *mut c_void,
        old_alignment: usize,
        old_size: usize,
        new_alignment: usize,
        new_size: usize,
    ) -> *mut c_void;

    /// Check whether we can do realloc inplace.
    pub fn sn_rust_fit_inplace(
        old_alignment: usize,
        old_size: usize,
        new_alignment: usize,
        new_size: usize,
    ) -> bool;

    pub fn sn_rust_round_size(alignment: usize, size: usize) -> usize;

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_zero_allocs_correctly() {
        let ptr = unsafe { sn_rust_alloc_zeroed(8, 1024) } as *mut u8 as *mut [u8; 1024];
        unsafe {
            assert!((*ptr).iter().all(|x| *x == 0));
        };
        unsafe { sn_rust_dealloc(ptr as *mut c_void, 8, 1024) };
    }

    #[test]
    fn it_frees_memory_malloc() {
        let ptr = unsafe { sn_rust_alloc(8, 8) } as *mut u8;
        unsafe {
            *ptr = 127;
            assert_eq!(*ptr, 127)
        };
        unsafe { sn_rust_dealloc(ptr as *mut c_void, 8, 8) };
    }

    #[test]
    fn it_frees_memory_sn_malloc() {
        let ptr = unsafe { sn_malloc(8) } as *mut u8;
        unsafe { sn_free(ptr as *mut c_void) };
    }

    #[test]
    fn it_frees_memory_sn_realloc() {
        let ptr = unsafe { sn_malloc(8) } as *mut u8;
        let ptr = unsafe { sn_realloc(ptr as *mut c_void, 8) } as *mut u8;
        unsafe { sn_free(ptr as *mut c_void) };
    }

    #[test]
    fn it_reallocs_correctly() {
        let mut ptr = unsafe { sn_rust_alloc(8, 8) } as *mut u8;
        unsafe {
            *ptr = 127;
            assert_eq!(*ptr, 127)
        };
        ptr = unsafe { sn_rust_realloc(ptr as *mut c_void, 8, 8, 16) } as *mut u8;
        unsafe { assert_eq!(*ptr, 127) };
        unsafe { sn_rust_dealloc(ptr as *mut c_void, 8, 16) };
    }

    #[test]
    fn it_calculates_usable_size() {
        let ptr = unsafe { sn_malloc(32) } as *mut u8;
        let usable_size = unsafe { sn_malloc_usable_size(ptr as *mut c_void) };
        assert!(
            usable_size >= 32,
            "usable_size should at least equal to the allocated size"
        );
    }

    #[test]
    fn it_creates_and_drops_allocator() {
        unsafe {
            let alloc = sn_rust_allocator_new();
            assert!(!alloc.is_null());
            sn_rust_allocator_drop(alloc);
        }
    }

    #[test]
    fn allocator_allocs_and_deallocs_memory() {
        unsafe {
            let alloc = sn_rust_allocator_new();
            let ptr = sn_rust_allocator_allocate(alloc, 8, 8) as *mut u8;
            *ptr = 127;
            assert_eq!(*ptr, 127);
            sn_rust_allocator_deallocate(alloc, ptr as *mut c_void, 8, 8);
            sn_rust_allocator_drop(alloc);
        }
    }

    #[test]
    fn allocator_grows_memory() {
        unsafe {
            let alloc = sn_rust_allocator_new();
            let ptr = sn_rust_allocator_allocate_zeroed(alloc, 8, 8);
            *(ptr as *mut u8) = 127;
            let ptr = sn_rust_allocator_grow_zeroed(alloc, ptr, 8, 8, 8, 16384) as *mut u8;
            assert_eq!(*ptr, 127);
            let slice = core::slice::from_raw_parts_mut(ptr.add(1), 16384 - 1);
            assert!(slice.iter().all(|x| *x == 0u8));
            sn_rust_allocator_deallocate(alloc, ptr as *mut c_void, 8, 16384);
            sn_rust_allocator_drop(alloc);
        }
    }

    #[test]
    fn it_checks_fit_inplace() {
        unsafe {
            for align in [1usize, 2, 4, 8, 16, 64, 512] {
                for size in [1usize, 23, 99, 100, 512, 1024, 3333, 8192, 8193] {
                    let round_size = sn_rust_round_size(align, size);
                    assert!(sn_rust_fit_inplace(align, size, align, round_size));
                    assert!(!sn_rust_fit_inplace(align, size, align, round_size + 1));
                }
            }
        }
    }
}
