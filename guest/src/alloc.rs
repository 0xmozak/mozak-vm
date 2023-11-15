#[no_mangle]
pub extern "C" fn alloc_aligned(bytes: usize, align: usize) -> *mut u8 {
    extern "C" {
        // This symbol is defined by the loader and marks the end
        // of all elf sections, so this is where we start our
        // heap.
        //
        // This is generated automatically by the linker; see
        // https://lld.llvm.org/ELF/linker_script.html#sections-command
        #[link_name = "_mozak_heap_start"]
        static _mozak_heap_start: u8;
    }

    // Pointer to next heap address to use, or 0 if the heap has not yet been
    // initialized.
    static mut HEAP_POS: usize = 0;

    // SAFETY: Single threaded, so nothing else can touch this while we're working.
    let mut heap_pos = unsafe { HEAP_POS };

    if heap_pos == 0 {
        heap_pos = unsafe { (&_mozak_heap_start) as *const u8 as usize };
    }

    let offset = heap_pos & (align - 1);
    if offset != 0 {
        heap_pos += align - offset;
    }

    let ptr = heap_pos as *mut u8;
    heap_pos += bytes;

    unsafe { HEAP_POS = heap_pos };
    ptr
}

use core::alloc::{GlobalAlloc, Layout};

struct BumpPointerAlloc;

unsafe impl GlobalAlloc for BumpPointerAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        alloc_aligned(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {
        // BumpPointerAlloc never deallocates memory
    }
}

#[global_allocator]
static HEAP: BumpPointerAlloc = BumpPointerAlloc;
