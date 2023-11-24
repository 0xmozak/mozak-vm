// TODO: Maybe we should move cli_benches elsewhere later.

#[cfg(any(feature = "test", test))]
pub mod sample;

#[cfg(any(feature = "test", test))]
pub mod fibo_with_inp;

#[cfg(any(feature = "test", test))]
pub mod xor;

#[cfg(any(feature = "test", test))]
pub mod benches;

#[cfg(any(feature = "test", test))]
pub mod nop;
