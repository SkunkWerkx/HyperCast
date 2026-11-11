//! The text door's length limit: valid text a span can't carry (past `u32::MAX` bytes) is
//! `OutOfRange` at both public entry points, so a binding that hands back the whole input never
//! narrows its length, and text of exactly `u32::MAX` bytes still reads.
#![cfg(target_pointer_width = "64")]

use hypercast::{Detail, Reason, cast_text, detailed};

/// `len` zeroed bytes, freed on drop. NUL is valid UTF-8. Panics if the allocator refuses: a
/// test that cannot build its input fails, never passes.
///
/// Only glibc's `calloc` maps an allocation this size lazily, reads of it mapping the shared
/// zero page, so these inputs cost address space rather than memory only on Linux (see
/// `constraints_raw.rs`); elsewhere, and under qemu-user (s390x), the tests are ignored.
struct Zeroed {
    ptr: *mut u8,
    layout: std::alloc::Layout,
}

impl Zeroed {
    fn new(len: usize) -> Zeroed {
        let layout = std::alloc::Layout::array::<u8>(len).expect("a layout");
        // SAFETY: the layout has a non-zero size.
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        assert!(!ptr.is_null(), "the allocator refused {len} zeroed bytes");
        Zeroed { ptr, layout }
    }

    fn bytes(&self) -> &[u8] {
        // SAFETY: `ptr` is valid for `layout.size()` initialized (zeroed) bytes until drop.
        unsafe { std::slice::from_raw_parts(self.ptr, self.layout.size()) }
    }
}

impl Drop for Zeroed {
    fn drop(&mut self) {
        // SAFETY: allocated by `Zeroed::new` with this layout.
        unsafe { std::alloc::dealloc(self.ptr, self.layout) }
    }
}

#[test]
#[cfg_attr(
    any(not(target_os = "linux"), target_arch = "s390x"),
    ignore = "4 GiB inputs cost real memory or emulated time off native Linux (see `Zeroed`)"
)]
fn text_past_u32_is_out_of_range() {
    let big = Zeroed::new(u32::MAX as usize + 5);
    // A success maps to its length: formatting 4 GiB of text would exhaust memory.
    let fault = cast_text(big.bytes()).map(str::len).unwrap_err();
    assert_eq!((fault.reason, fault.offset, fault.len), (Reason::OutOfRange, 0, u32::MAX));
    let failure = detailed::cast_text(big.bytes()).map(str::len).unwrap_err();
    assert_eq!((failure.reason, failure.detail), (Reason::OutOfRange, Detail::TooLong));
    assert_eq!((failure.offset, failure.len), (0, u32::MAX));
}

#[test]
#[cfg_attr(
    any(not(target_os = "linux"), target_arch = "s390x"),
    ignore = "4 GiB inputs cost real memory or emulated time off native Linux (see `Zeroed`)"
)]
fn text_of_u32_max_reads() {
    let big = Zeroed::new(u32::MAX as usize);
    assert_eq!(cast_text(big.bytes()).map(str::len), Ok(u32::MAX as usize));
    assert_eq!(detailed::cast_text(big.bytes()).map(str::len), Ok(u32::MAX as usize));
}
