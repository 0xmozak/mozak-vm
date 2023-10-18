/// Implement Display by converting the Identifier to a String
macro_rules! derive_display_stark_name {
    ($s: ident) => {
        impl<F, const D: usize> Display for $s<F, D> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", std::stringify!($s))
            }
        }
    };
}

pub(crate) use derive_display_stark_name;
