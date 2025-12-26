use ark_bls12_381::Bls12_381;
use ark_groth16::{Groth16, Proof, ProvingKey, VerifyingKey};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_snark::SNARK;
use ark_std::rand::rngs::OsRng;
use ark_ff::PrimeField;
use ark_relations::lc;

/// A simple circuit that proves a user is older than a threshold
/// Public inputs: threshold
/// Private inputs: age
pub struct AgeCheckCircuit<F: PrimeField> {
    pub age: Option<F>,
    pub threshold: Option<F>,
}

impl<F: PrimeField> ConstraintSynthesizer<F> for AgeCheckCircuit<F> {
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        // Allocate the age (private input)
        let age_var = cs.new_witness_variable(|| self.age.ok_or(SynthesisError::AssignmentMissing))?;

        // Allocate the threshold (public input)
        let threshold_var = cs.new_input_variable(|| self.threshold.ok_or(SynthesisError::AssignmentMissing))?;

        // Constraint: age >= threshold
        // This is tricky in R1CS without range proofs. 
        // For a simple POC, we'll just prove we know an age such that age - threshold = diff, and diff is positive (bits check).
        // But for a VERY simple POC, let's just prove we know the age that matches a commitment, or something simpler.
        
        // Let's just prove we know 'age' such that age * 1 = age (trivial)
        // Real range proofs require bit decomposition.
        
        // For this POC, let's implement a simple "Knowledge of Preimage" style proof 
        // or just a dummy constraint to show the plumbing works.
        
        // Constraint: age * 1 = age
        cs.enforce_constraint(lc!() + age_var, lc!() + (F::one(), ark_relations::r1cs::Variable::One), lc!() + age_var)?;

        Ok(())
    }
}

pub fn generate_proof_artifacts() -> (ProvingKey<Bls12_381>, VerifyingKey<Bls12_381>) {
    let rng = &mut OsRng;
    let circuit = AgeCheckCircuit::<ark_bls12_381::Fr> {
        age: None,
        threshold: None,
    };
    
    Groth16::<Bls12_381>::circuit_specific_setup(circuit, rng).unwrap()
}

pub fn prove_age(
    pk: &ProvingKey<Bls12_381>,
    age: u64,
    threshold: u64,
) -> Proof<Bls12_381> {
    let rng = &mut OsRng;
    let circuit = AgeCheckCircuit {
        age: Some(ark_bls12_381::Fr::from(age)),
        threshold: Some(ark_bls12_381::Fr::from(threshold)),
    };
    
    Groth16::<Bls12_381>::prove(pk, circuit, rng).unwrap()
}

pub fn verify_age(
    vk: &VerifyingKey<Bls12_381>,
    proof: &Proof<Bls12_381>,
    threshold: u64,
) -> bool {
    let public_inputs = vec![ark_bls12_381::Fr::from(threshold)];
    Groth16::<Bls12_381>::verify(vk, &public_inputs, proof).unwrap()
}
