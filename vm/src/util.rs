pub fn init_arr<I: Copy, A: Default + Copy>(entries: &[(I, A)]) -> [A; 32]
where
    usize: From<I>,
{
    let mut arr = [A::default(); 32];
    for (i, entry) in entries {
        arr[usize::from(*i)] = *entry;
    }
    arr
}
