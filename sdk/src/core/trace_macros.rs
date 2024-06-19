#[macro_export]
#[cfg(feature = "trace")]
macro_rules! trace_scope {
    ($code: block) => {
        unsafe { mozak_sdk::core::trace::GLOBAL_TRACER.trace(|| $code) };
        // tracer is disabled at the end of this scope, thus `trace` macro
        // won't work outside the scope.
    };
}
#[macro_export]
#[cfg(not(feature = "trace"))]
macro_rules! trace_scope {
    ($code: block) => {
        // NOOP when tracing is disabled
    };
}

#[macro_export]
#[cfg(feature = "trace")]
macro_rules! trace {
    ($str: expr) => {
        let msg = format!($str);
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
