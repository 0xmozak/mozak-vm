fn main() {
    #[cfg(feature = "native")]
    {
        extern crate build_scripts;
        build_scripts::self_prog_id::dump_self_prog_id("walletbin");
    }
}
