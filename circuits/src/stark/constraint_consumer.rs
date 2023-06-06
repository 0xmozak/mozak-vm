
use plonky2::field::packed::PackedField;

pub struct ConstraintConsumer<P: PackedField> {
    /// Random values used to combine multiple constraints into one.
    pub alphas: Vec<P::Scalar>,

    /// Running sums of constraints that have been emitted so far, scaled by
    /// powers of alpha.
    // TODO(JN): This is pub so it can be used in a test. Once we have an API for accessing this
    // result, it should be made private.
    pub constraint_accs: Vec<P>,

    /// The evaluation of `X - g^(n-1)`.
    z_last: P,

    /// The evaluation of the Lagrange basis polynomial which is nonzero at the
    /// point associated with the first trace row, and zero at other points
    /// in the subgroup.
    lagrange_basis_first: P,

    /// The evaluation of the Lagrange basis polynomial which is nonzero at the
    /// point associated with the last trace row, and zero at other points
    /// in the subgroup.
    lagrange_basis_last: P,
}

impl<P: PackedField> ConstraintConsumer<P> {
    pub fn new(
        alphas: Vec<P::Scalar>,
        z_last: P,
        lagrange_basis_first: P,
        lagrange_basis_last: P,
    ) -> Self {
        Self {
            constraint_accs: vec![P::ZEROS; alphas.len()],
            alphas,
            z_last,
            lagrange_basis_first,
            lagrange_basis_last,
        }
    }

    pub fn accumulators(self) -> Vec<P> {
        self.constraint_accs
    }

    /// Add one constraint valid on all rows except the last.
    pub fn constraint_transition(&mut self, constraint: P) {
        self.constraint(constraint * self.z_last);
    }

    /// Add one constraint on all rows.
    pub fn constraint(&mut self, constraint: P) {
        for (&alpha, acc) in self.alphas.iter().zip(&mut self.constraint_accs) {
            *acc *= alpha;
            *acc += constraint;
        }
    }

    /// Add one constraint, but first multiply it by a filter such that it will
    /// only apply to the first row of the trace.
    pub fn constraint_first_row(&mut self, constraint: P) {
        self.constraint(constraint * self.lagrange_basis_first);
    }

    /// Add one constraint, but first multiply it by a filter such that it will
    /// only apply to the last row of the trace.
    pub fn constraint_last_row(&mut self, constraint: P) {
        self.constraint(constraint * self.lagrange_basis_last);
    }
}

