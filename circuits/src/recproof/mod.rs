use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::generator::{GeneratedValues, SimpleGenerator};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartitionWitness, Witness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CommonCircuitData;
use plonky2::util::serialization::{Buffer, IoResult, Read, Write};

pub mod summarized;

/// A generator for testing if a value equals zero
#[derive(Debug, Default)]
struct NonzeroTestGenerator {
    to_test: Target,
    result: BoolTarget,
}

impl<F: RichField + Extendable<D>, const D: usize> SimpleGenerator<F, D> for NonzeroTestGenerator {
    fn id(&self) -> String { "NonzeroTestGenerator".to_string() }

    fn dependencies(&self) -> Vec<Target> { vec![self.to_test] }

    fn run_once(&self, witness: &PartitionWitness<F>, out_buffer: &mut GeneratedValues<F>) {
        let to_test_value = witness.get_target(self.to_test);
        out_buffer.set_bool_target(self.result, to_test_value.is_nonzero());
    }

    fn serialize(&self, dst: &mut Vec<u8>, _common_data: &CommonCircuitData<F, D>) -> IoResult<()> {
        dst.write_target(self.to_test)?;
        dst.write_target_bool(self.result)
    }

    fn deserialize(src: &mut Buffer, _common_data: &CommonCircuitData<F, D>) -> IoResult<Self> {
        let to_test = src.read_target()?;
        let result = src.read_target_bool()?;
        Ok(Self { to_test, result })
    }
}

fn is_nonzero<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    to_test: Target,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    // `result = to_test != 0`, meaning it's 0 for `to_test == 0` or 1 for all other
    // to_test we'll represent this as `result = 0 | 1`
    // note that this can be falsely proved so we have to put some constraints below
    // to ensure it
    let result = builder.add_virtual_bool_target_safe();
    builder.add_simple_generator(NonzeroTestGenerator { to_test, result });

    // Enforce the result through arithmetic
    let neg = builder.not(result); // neg = 1 | 0
    let denom = builder.add(to_test, neg.target); // denom = 1 | to_test
    let div = builder.div(to_test, denom); // div = 0 | 1

    builder.connect(result.target, div);

    result
}
