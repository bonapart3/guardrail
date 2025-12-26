use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;

fn main() {
    let password = b"admin123";
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password, &salt).unwrap();
    let hash_str = hash.to_string();
    println!("Hash: {}", hash_str);

    // Verify it works
    let parsed = PasswordHash::new(&hash_str).unwrap();
    match argon2.verify_password(password, &parsed) {
        Ok(_) => println!("Verification: OK"),
        Err(e) => println!("Verification FAILED: {:?}", e),
    }

    // Test stored hash
    let stored = "$argon2id$v=19$m=19456,t=2,p=1$wUHoa7fU9JgRafz9XnPU/A$MFw+KpQf0PdTSzF2NDZohwZAdynGru61cYWzuXqcnQQ";
    let stored_parsed = PasswordHash::new(stored).unwrap();
    match argon2.verify_password(password, &stored_parsed) {
        Ok(_) => println!("Stored hash verification: OK"),
        Err(e) => println!("Stored hash verification FAILED: {:?}", e),
    }
}
