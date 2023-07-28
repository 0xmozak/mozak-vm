#![no_std]
#![no_main]
use core::assert_eq;


#[no_mangle]
pub extern "C" fn _start() {
    let a = 10;
    let b = a * 10;
    assert_eq!(b, a * 10);
}
use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub fn __atomic_load_4(arg: *const usize, _ordering: usize) -> usize {
    unsafe { *arg }
}
