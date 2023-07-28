#![no_std]
#![no_main]
use core::mem::transmute;
// use std::ptr::read;
use core::convert::TryInto;
//use rand::Rng;

fn to_f32(v: &[u8]) -> f32 {
    let ptr = v.as_ptr() as *const f32;
    unsafe {
        // [1] dereference
        *ptr
        // [2] alternatively
        //ptr.read()
    }
}

#[no_mangle]
pub extern "C" fn _start() {
    // println!("align_of::<f32>() = {}", std::mem::align_of::<f32>());

    //let mut rng = rand::thread_rng();

    // with a pointer on the stack
    let v: [u8; 7] = [ 0x4A, 0x3A, 0x2a, 0x10, 0x0F, 0xD2, 0x37];
    // with a pointer on the heap
    //let v = Box::new(rng.gen::<[u8;7]>());

    for i in 0..4 {
        let ptr = &v[i..(i+4)];
        let f = to_f32(ptr);

        // max alignment of ptr
        let alignment = 1 << (ptr.as_ptr() as usize).trailing_zeros();
        
        // other ways to convert, as a control check
        let repr = ptr.try_into().expect("");
        let f2 = unsafe { transmute::<[u8; 4], f32>(repr) };
        let f3 = f32::from_le_bytes(repr);

        // println!("{:x?} [{alignment}]: {repr:02x?} : {f} =? {f2} = {f3}", ptr.as_ptr());
    
        // assert_eq!(f, f2);
        // assert_eq!(f, f3);
    }
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
