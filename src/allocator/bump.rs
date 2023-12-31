use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::null_mut,
};

use super::{align_up, Locked};

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut a = self.lock();
        let start = align_up(a.next, layout.align());
        let end = match start.checked_add(layout.size()) {
            Some(end) => end,
            None => return null_mut(),
        };

        if end > a.heap_end {
            null_mut()
        } else {
            a.next = end;
            a.allocations += 1;
            start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut a = self.lock();
        a.allocations -= 1;
        if a.allocations == 0 {
            a.next = a.heap_start;
        }
    }
}
