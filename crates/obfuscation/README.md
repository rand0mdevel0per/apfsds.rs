# apfsds-obfuscation

Traffic obfuscation layer for APFSDS.

## Features

- **XorMask**: Rolling XOR mask with session-derived key
- **Padding**: Size obfuscation with random or fixed-block padding
- **Compression**: Optional LZ4 compression for payloads

## Usage

```rust
use apfsds_obfuscation::{XorMask, Padder, PaddingStrategy};

// XOR masking
let mask = XorMask::from_session_key(&session_key);
let masked = mask.apply(&data);
let original = mask.apply(&masked); // XOR is symmetric

// Padding
let padder = Padder::new(PaddingStrategy::Random { min: 8, max: 64 });
let padded = padder.pad(&data);
let unpadded = padder.unpad(&padded)?;
```

## Obfuscation Pipeline

```
Plaintext → Compress → Pad → XOR Mask → Ciphertext
```

## License

MIT
