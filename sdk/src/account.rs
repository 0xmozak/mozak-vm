use crate::pubkey::PubKey;

#[allow(dead_code)] // TODO: Used soon
pub struct Account {
    /// Public key of the account (address)
    id: PubKey,
}

pub fn add(left: usize, right: usize) -> usize { left + right }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
