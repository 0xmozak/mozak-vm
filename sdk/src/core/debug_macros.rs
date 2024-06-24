#[macro_export]
#[cfg(feature = "trace")]
macro_rules! debug_scope {
    ($code: block) => {
        // using this helper function to ensure that
        // the code block doesn't mutate or return anything.
        let closure = || $code;
        mozak_sdk::core::debug_macros::debug_code_block(closure);
    };
}
#[macro_export]
#[cfg(not(feature = "trace"))]
macro_rules! debug_scope {
    ($code: block) => {
        // NOOP when tracing is disabled
    };
}

#[macro_export]
#[cfg(feature = "trace")]
macro_rules! trace {
    ($str: expr) => {
        let msg = alloc::format!($str);
        unsafe { mozak_sdk::core::ecall::trace(&msg) };
    };
}

#[macro_export]
#[cfg(not(feature = "trace"))]
macro_rules! trace {
    ($msg: expr) => {
        // NOOP when tracing is disabled
    };
}

#[cfg(feature = "trace")]
pub fn debug_code_block<F>(code: F)
where
    F: Fn(), {
    code();
}
