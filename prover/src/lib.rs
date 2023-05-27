use p3_field::PrimeField;

pub trait Prover {
    type BaseField: PrimeField;
    // fn run(&mut self, program: Program);
    fn prove(&self);
    fn verify();
}

