#![no_main]
#![feature(restricted_std)]

mod core_logic;

use core_logic::{dispatch, MethodArgs, MethodReturns};
use mozak_sdk::sys::call_receive;
use rkyv::Deserialize;

pub fn main() {
    while let Some((msg, _idx)) = call_receive() {
        let archived_args = unsafe { rkyv::archived_root::<MethodArgs>(&msg.args.0[..]) };
        let args: MethodArgs = archived_args.deserialize(&mut rkyv::Infallible).unwrap();
        let archived_ret = unsafe { rkyv::archived_root::<MethodReturns>(&msg.ret.0[..]) };
        let ret: MethodReturns = archived_ret.deserialize(&mut rkyv::Infallible).unwrap();

        assert!(dispatch(args) == ret);
    }
}

// We define `main()` to be the program's entry point.
guest::entry!(main);
