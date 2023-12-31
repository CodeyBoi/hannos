#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(hannos::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use hannos::{
    allocator::{self, HEAP_SIZE},
    hlt_loop,
    memory::{self, BootInfoFrameAllocator},
};
use x86_64::VirtAddr;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    hannos::init();
    let phys_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_memory_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initalization failed");

    test_main();

    hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    hannos::test_panic_handler(info)
}

#[test_case]
fn test_allocation() {
    let x = Box::new(1337);
    let y = Box::new(516);
    assert_eq!(*x, 1337);
    assert_eq!(*y, 516);
}

#[test_case]
fn test_allocate_loads() {
    let n = 1000;
    let mut v = Vec::new();
    for i in 0..n {
        v.push(i);
    }
    assert_eq!(v.iter().sum::<usize>(), (n - 1) * n / 2);
}

#[test_case]
fn test_reuse_allocations() {
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}

#[test_case]
fn test_reuse_allocations_long_lived() {
    let long_lived = Box::new(1);
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1);
}
