use crate::pubkey::PubKey;

pub struct Account {
    /// Public key of the account (address)
    pub id: PubKey,
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
