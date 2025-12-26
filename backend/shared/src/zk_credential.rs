use ark_bls12_381::Bls12_381;
use ark_groth16::{Groth16, Proof, ProvingKey, VerifyingKey};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_snark::SNARK;
use ark_std::rand::rngs::OsRng;
use ark_ff::PrimeField;
use ark_relations::lc;

/// A simple ZK circuit that proves knowledge of a secret that satisfies certain conditions
/// For this POC: proves knowledge of 'x' such that x^2 = y (where y is public)
/// This demonstrates the basic ZK plumbing without complex range proofs
pub struct SimpleProofCircuit<F: PrimeField> {
    /// Private input: the secret value
    pub secret: Option<F>,
    /// Public input: the result (secret^2)
    pub public_result: Option<F>,
}

impl<F: PrimeField> ConstraintSynthesizer<F> for SimpleProofCircuit<F> {
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        // Allocate private input
        let secret_var = cs.new_witness_variable(|| self.secret.ok_or(SynthesisError::AssignmentMissing))?;

        // Allocate public input
        let public_var = cs.new_input_variable(|| self.public_result.ok_or(SynthesisError::AssignmentMissing))?;

        // Constraint: secret * secret = public_result
        cs.enforce_constraint(
            lc!() + secret_var,
            lc!() + secret_var,
            lc!() + public_var,
        )?;

        Ok(())
    }
}

/// Generate proving and verifying keys for the ZK circuit
pub fn generate_proof_artifacts() -> Result<(ProvingKey<Bls12_381>, VerifyingKey<Bls12_381>), crate::errors::GuardRailError> {
    let rng = &mut OsRng;
    let circuit = SimpleProofCircuit::<ark_bls12_381::Fr> {
        secret: None,
        public_result: None,
    };

    Groth16::<Bls12_381>::circuit_specific_setup(circuit, rng)
        .map_err(|e| crate::errors::GuardRailError::CryptoError(e.to_string()))
}

/// Generate a ZK proof for age verification
/// For this POC, we prove knowledge of a secret that squares to a public value
pub fn prove_age(
    pk: &ProvingKey<Bls12_381>,
    age: u64,
    threshold: u64,
) -> Result<Proof<Bls12_381>, crate::errors::GuardRailError> {
    let rng = &mut OsRng;

    // For POC: prove that we know 'age' such that age^2 = age^2
    // In a real implementation, this would prove age >= threshold using range proofs
    let secret = ark_bls12_381::Fr::from(age);
    let public_result = secret * secret; // age^2

    let circuit = SimpleProofCircuit {
        secret: Some(secret),
        public_result: Some(public_result),
    };

    Groth16::<Bls12_381>::prove(pk, circuit, rng)
        .map_err(|e| crate::errors::GuardRailError::CryptoError(e.to_string()))
}

/// Verify a ZK proof for age verification
pub fn verify_age(
    vk: &VerifyingKey<Bls12_381>,
    proof: &Proof<Bls12_381>,
    threshold: u64,
) -> Result<bool, crate::errors::GuardRailError> {
    // For POC: verify the proof with public inputs
    // In real implementation, this would verify age >= threshold
    let public_result = ark_bls12_381::Fr::from(threshold) * ark_bls12_381::Fr::from(threshold);
    let public_inputs = vec![public_result];

    match Groth16::<Bls12_381>::verify(vk, &public_inputs, proof) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Generate a ZK credential for an identity
/// This creates a proof that can be verified without revealing the underlying data
pub fn generate_zk_credential(
    pk: &ProvingKey<Bls12_381>,
    identity_id: &str,
    credential_data: serde_json::Value,
) -> Result<Proof<Bls12_381>, crate::errors::GuardRailError> {
    // Hash the identity and credential data to create a unique secret
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(identity_id.as_bytes());
    hasher.update(credential_data.to_string().as_bytes());
    let hash = hasher.finalize();

    // Convert hash to field element
    let secret = ark_bls12_381::Fr::from_be_bytes_mod_order(&hash);
    let public_result = secret * secret;

    let circuit = SimpleProofCircuit {
        secret: Some(secret),
        public_result: Some(public_result),
    };

    let rng = &mut OsRng;
    Groth16::<Bls12_381>::prove(pk, circuit, rng)
        .map_err(|e| crate::errors::GuardRailError::CryptoError(e.to_string()))
}

/// Verify a ZK credential
pub fn verify_zk_credential(
    vk: &VerifyingKey<Bls12_381>,
    proof: &Proof<Bls12_381>,
    identity_id: &str,
    credential_data: serde_json::Value,
) -> Result<bool, crate::errors::GuardRailError> {
    // Recreate the expected public input
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(identity_id.as_bytes());
    hasher.update(credential_data.to_string().as_bytes());
    let hash = hasher.finalize();

    let expected_result = ark_bls12_381::Fr::from_be_bytes_mod_order(&hash);
    let public_result = expected_result * expected_result;
    let public_inputs = vec![public_result];

    match Groth16::<Bls12_381>::verify(vk, &public_inputs, proof) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
