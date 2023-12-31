use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::{null_mut, NonNull},
};

use super::{align_up, Locked};

#[derive(Clone, Copy, Debug)]
struct MyNonNull<ListNode>(NonNull<ListNode>);

impl<T> MyNonNull<T> {
    fn new(ptr: *mut T) -> Option<Self> {
        match NonNull::new(ptr) {
            Some(ptr) => Some(Self(ptr.cast::<T>())),
            None => None,
        }
    }
}

unsafe impl Send for MyNonNull<ListNode> {}

unsafe impl GlobalAlloc for Locked<FixedSizeAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock()._alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.lock()._dealloc(ptr, layout);
    }
}

const BLOCK_SIZES: [usize; 9] = [8, 16, 32, 64, 128, 256, 512, 1024, 2048];
const MIN_BLOCK_SIZE: usize = BLOCK_SIZES[0];
const MAX_BLOCK_SIZE: usize = BLOCK_SIZES[BLOCK_SIZES.len() - 1];

#[derive(Debug)]
pub struct FixedSizeAllocator {
    heap_start: usize,
    heap_size: usize,
    used_memory: usize,
    free_list: [Option<MyNonNull<ListNode>>; BLOCK_SIZES.len()],
}

impl FixedSizeAllocator {
    pub const fn new() -> Self {
        const EMPTY: Option<MyNonNull<ListNode>> = None;
        Self {
            heap_start: 0,
            heap_size: 0,
            used_memory: 0,
            free_list: [EMPTY; BLOCK_SIZES.len()],
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self._init(heap_start, heap_size);
    }

    fn _init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_size = heap_size;
    }

    fn _alloc(&mut self, layout: Layout) -> *mut u8 {
        let size = layout.size();

        // If allocation is large, create many blocks and return ptr to the first
        if size > MAX_BLOCK_SIZE {
            match self._alloc_huge(layout) {
                Some(ptr) => return ptr.as_ptr(),
                None => return null_mut(),
            }
        }

        let idx = Self::get_block_size_index(size);
        let block = match self.remove_block(idx) {
            Some(block_ptr) => block_ptr,
            None => match self.find_index_of_larger_block(idx + 1) {
                Some(larger_block_idx) => {
                    for i in 0..larger_block_idx - idx {
                        self.split_block(larger_block_idx - i);
                    }
                    self.remove_block(idx).expect(
                        "split block function put the new block in the wrong free list slot",
                    )
                }
                None => match self.create_block(size, layout.align()) {
                    Some(block) => block,
                    None => return null_mut(), // out of memory
                },
            },
        };
        block.0.cast::<u8>().as_ptr()
    }

    fn _alloc_huge(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        let size = layout.size();
        let first_block = self.create_block(MAX_BLOCK_SIZE, MAX_BLOCK_SIZE)?;
        let blocks_needed = size / MAX_BLOCK_SIZE + 1;
        for _ in 0..blocks_needed - 1 {
            self.create_block(MAX_BLOCK_SIZE, MAX_BLOCK_SIZE)?;
        }
        return NonNull::new(first_block.0.cast::<u8>().as_ptr());
    }

    fn _dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        if layout.size() > MAX_BLOCK_SIZE {
            self._dealloc_huge(ptr, layout);
        } else {
            self.add_block(ptr.cast::<ListNode>(), layout.size());
        }
    }

    fn _dealloc_huge(&mut self, ptr: *mut u8, layout: Layout) {
        let no_of_blocks = layout.size() / MAX_BLOCK_SIZE + 1;
        for i in 0..no_of_blocks {
            self.add_block(
                unsafe { ptr.add(MAX_BLOCK_SIZE * i) }.cast::<ListNode>(),
                MAX_BLOCK_SIZE,
            )
        }
    }

    fn split_block(&mut self, idx: usize) {
        let size = BLOCK_SIZES[idx
            .checked_sub(1)
            .expect("tried to split block with minimum size")];
        let node = self
            .remove_block(idx)
            .expect("tried to split non existing block");
        let other_node =
            unsafe { (node.0.as_ptr() as *const _ as *mut u8).add(size) }.cast::<ListNode>();
        self.add_block(node.0.as_ptr(), size);
        self.add_block(other_node, size);
    }

    fn find_index_of_larger_block(&self, start_index: usize) -> Option<usize> {
        self.free_list[start_index..]
            .iter()
            .position(|node| node.is_some())
            .and_then(|idx| Some(idx + start_index))
    }

    fn get_block_size_index(size: usize) -> usize {
        match BLOCK_SIZES.iter().position(|v| *v >= size) {
            Some(i) => i,
            None => panic!(
                "cannot allocate blocks larger than {} bytes",
                BLOCK_SIZES.last().unwrap()
            ),
        }
    }

    fn round_up(size: usize) -> usize {
        BLOCK_SIZES[Self::get_block_size_index(size)]
    }

    fn create_block(&mut self, size: usize, align: usize) -> Option<MyNonNull<ListNode>> {
        let mut current_addr = self.heap_start + self.used_memory;
        let block_addr = align_up(current_addr, align);

        // If there is memory between the end of the heap and the aligned address,
        // create padding blocks and add them to the free list
        while block_addr - current_addr >= MIN_BLOCK_SIZE {
            let pad_size = *BLOCK_SIZES
                .iter()
                .filter(|s| **s <= block_addr - current_addr)
                .max()
                .unwrap();
            self.add_block(current_addr as *mut ListNode, pad_size);
            self.used_memory += pad_size;
            current_addr += pad_size;
        }

        // Mark remaining memory after padding as used
        self.used_memory += block_addr - current_addr;

        let block_size = Self::round_up(size);
        if self.used_memory + block_size > self.heap_size {
            return None; // out of memory
        }

        let ptr = MyNonNull::new(block_addr as *mut ListNode);
        self.used_memory += block_size;
        ptr
    }

    fn add_block(&mut self, node_ptr: *mut ListNode, size: usize) {
        let idx = Self::get_block_size_index(size);
        let node = unsafe {
            node_ptr
                .as_mut()
                .expect("tried to add null block to free list")
        };
        (self.free_list[idx], node.next) = (MyNonNull::new(node_ptr), self.free_list[idx]);
    }

    fn remove_block(&mut self, idx: usize) -> Option<MyNonNull<ListNode>> {
        let node_ptr = self.free_list[idx]?;
        unsafe {
            self.free_list[idx] = node_ptr.0.as_ref().next;
        }
        Some(node_ptr)
    }
}

#[derive(Clone, Copy, Debug)]
struct ListNode {
    next: Option<MyNonNull<ListNode>>,
}
