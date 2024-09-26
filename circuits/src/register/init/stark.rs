use crate::unstark::unstark;

// For sanity check, we can constrain the register address column to be in
// a running sum from 0..=31, but since this fixed table is known to
// both prover and verifier, we do not need to do so here.
unstark!(RegisterInitStark, super::columns::RegisterInit<T>);
