#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(hannos::test_runner)]
#![reexport_test_harness_main = "test_main"]

use hannos::println;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    test_main();
    loop {}
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    hannos::test_panic_handler(info)
}

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}
