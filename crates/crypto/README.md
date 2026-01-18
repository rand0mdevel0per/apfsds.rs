# apfsds-crypto

Cryptographic primitives for APFSDS.

## Features

- **AES-256-GCM**: Authenticated encryption with associated data
- **Ed25519**: Digital signatures and verification
- **X25519 ECDH**: Elliptic curve Diffie-Hellman key exchange
- **HMAC-SHA256**: Token authentication and integrity verification
- **XOR Filter**: Efficient replay protection

## Usage

```rust
use apfsds_crypto::{AesGcmCipher, Ed25519Keypair, HmacAuth};

// AES-256-GCM encryption
let cipher = AesGcmCipher::new(&key);
let ciphertext = cipher.encrypt(plaintext)?;
let plaintext = cipher.decrypt(&ciphertext)?;

// Ed25519 signing
let keypair = Ed25519Keypair::generate();
let signature = keypair.sign(message);
assert!(keypair.verify(message, &signature));

// HMAC authentication
let auth = HmacAuth::new(&secret);
let token = auth.generate_token(user_id, expiry);
assert!(auth.verify_token(&token));
```

## Security Notes

- All keys are securely generated using `rand`
- AES-GCM uses random 12-byte nonces
- Ed25519 provides 128-bit security level

## License

MIT
