#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(custom_test_frameworks)]
#![test_runner(hannos::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::panic::PanicInfo;

use bootloader::{entry_point, BootInfo};
use hannos::{
    allocator,
    memory::{self, BootInfoFrameAllocator},
    println,
    shell::Shell,
    task::{executor::Executor, keyboard::process_keypresses, Task},
};
use x86_64::VirtAddr;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    hannos::init();

    let phys_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_memory_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");
    println!("Boot successful!");

    #[cfg(test)]
    test_main();

    let mut exec = Executor::new();
    let mut shell = Shell::new();
    exec.spawn(Task::new(process_keypresses(move |key| {
        shell.handle_keypress(key)
    })));
    exec.run();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    hannos::println!("{}", info);
    hannos::hlt_loop();
}

/// This function is called on panic in testing mode.
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    hannos::test_panic_handler(info)
}
