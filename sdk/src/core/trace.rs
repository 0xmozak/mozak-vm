use once_cell::unsync::Lazy;

pub struct Tracer {
    enabled: bool,
}

pub static mut GLOBAL_TRACER: Lazy<Tracer> = Lazy::new(Tracer::new);

impl Default for Tracer {
    fn default() -> Self { Self::new() }
}

impl Tracer {
    #[must_use]
    pub fn new() -> Self { Self { enabled: false } }

    #[must_use]
    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn trace<F>(&mut self, code: F)
    where
        F: FnOnce(), {
        // enable the tracer, effectively enabling the `trace` ecalls,
        // if any, which are present in `code`
        self.enabled = true;
        code();
        self.enabled = false;
    }
}
