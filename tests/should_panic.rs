#![no_std]
#![no_main]

use core::panic::PanicInfo;

use hannos::{exit_qemu, sprintln, QemuExitCode};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    assert_eq!(0, 1);
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sprintln!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
