use core::{
    alloc::{GlobalAlloc, Layout},
    mem::size_of,
    ptr::null_mut,
};

use super::{align_up, Locked};

pub const BLOCK_SIZE: usize = 16;
pub const MAX_ORDER: u8 = 24;
pub const HEAP_SIZE: usize = BLOCK_SIZE * (1 << MAX_ORDER);

unsafe impl GlobalAlloc for Locked<BuddyAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let order = BuddyAllocator::order(layout.size());
        if order > MAX_ORDER {
            return null_mut();
        }

        let a = self.lock();
        match a.get_block_of_min_order(order) {
            Some(block) => {
                let block = block
                    .as_mut()
                    .expect("got null addr instead of block pointer");
                block.is_free = false;
                block.get_memory_ptr(layout.align()) as *mut u8
            }
            None => null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        let block = (ptr as *mut Block)
            .as_mut()
            .expect("got invalid block pointer during dealloc");
        block.is_free = true;
        // TODO: Merge neighborning blocks
    }
}

pub struct BuddyAllocator {
    heap_start: usize,
    heap_order: u8,
}

impl BuddyAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_order: 0,
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.init_inner(heap_start, heap_size);
    }

    fn init_inner(&mut self, heap_start: usize, heap_size: usize) {
        let order = Self::order(heap_size).min(MAX_ORDER);
        self.heap_order = order;
        self.heap_start = heap_start;
        let block = heap_start as *mut Block;
        unsafe {
            block.write(Block {
                order,
                is_free: true,
            });
        }
    }

    fn heap_size(&self) -> usize {
        BLOCK_SIZE << self.heap_order
    }

    fn get_block_of_min_order(&self, order: u8) -> Option<*mut Block> {
        let mut block = self.heap_start as *mut Block;
        loop {
            unsafe {
                if (*block).is_free && (*block).order >= order {
                    while (*block).order > order {
                        (*block).split();
                    }
                    break Some(block as *mut Block);
                } else if let Some(next_block) = self.next_block(
                    block
                        .as_ref()
                        .expect("invalid null pointer to buddy allocator block"),
                ) {
                    block = next_block;
                } else {
                    break None;
                }
            }
        }
    }

    fn next_block(&self, block: &Block) -> Option<*mut Block> {
        let next_block = block.next_free_addr();
        if next_block as usize - self.heap_start >= self.heap_size() {
            None
        } else {
            Some(next_block as *mut Block)
        }
    }

    const fn order(size: usize) -> u8 {
        let mut current_size = BLOCK_SIZE;
        let mut order = 0;
        while current_size - size_of::<Block>() < size {
            current_size = current_size << 1;
            order += 1;
        }
        order
    }
}

#[derive(Debug)]
#[repr(C)]
struct Block {
    order: u8,
    is_free: bool,
}

impl Block {
    fn size(&self) -> usize {
        BLOCK_SIZE << self.order
    }

    fn get_memory_ptr(&self, align: usize) -> usize {
        unsafe { align_up((self as *const Self).add(1) as usize, align) }
    }

    fn next_free_addr(&self) -> usize {
        self as *const _ as usize + self.size()
    }

    fn split(&mut self) {
        if !self.is_free {
            panic!("tried to split non-free memory block");
        }
        let order = match self.order.checked_sub(1) {
            Some(v) => v,
            None => panic!("tried to split memory block with order 0"),
        };
        self.order = order;
        let next_block = Block {
            order,
            is_free: true,
        };
        let addr = self.next_free_addr() as *mut Block;
        unsafe {
            addr.write(next_block);
        }
    }
}
