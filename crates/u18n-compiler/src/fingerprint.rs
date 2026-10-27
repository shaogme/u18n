use crate::{codegen::GenerationConfig, semantic::LocaleInput};

pub(super) struct Fingerprint;

pub(super) struct FingerprintInput<'a> {
    pub(super) schema: &'a str,
    pub(super) locales: &'a [LocaleInput],
    pub(super) registry_digest: &'a str,
    pub(super) cardinal_digest: &'a str,
    pub(super) ordinal_digest: &'a str,
    pub(super) aliases_digest: &'a str,
    pub(super) supplemental_digest: &'a str,
    pub(super) generation: GenerationConfig<'a>,
}

struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    bit_len: u64,
}

impl Fingerprint {
    pub(super) fn calculate(input: FingerprintInput<'_>) -> String {
        let FingerprintInput {
            schema,
            locales,
            registry_digest,
            cardinal_digest,
            ordinal_digest,
            aliases_digest,
            supplemental_digest,
            generation,
        } = input;
        let mut hash = Sha256::new();
        hash.update(b"u18n-catalog-input-v1\0");
        Self::field(&mut hash, "schema", schema.as_bytes());
        let mut ordered = locales
            .iter()
            .map(|input| {
                (
                    input.ast.locale.source_tag.as_str(),
                    input.source.as_bytes(),
                )
            })
            .collect::<Vec<_>>();
        ordered.sort_by(|left, right| left.0.cmp(right.0));
        for (tag, source) in ordered {
            Self::field(&mut hash, "locale-tag", tag.as_bytes());
            Self::field(&mut hash, "locale-source", source);
        }
        Self::field(&mut hash, "output-format-version", b"1");
        Self::field(&mut hash, "cldr-version", b"49");
        Self::field(
            &mut hash,
            "iana-language-subtag-registry-sha256",
            registry_digest.as_bytes(),
        );
        Self::field(
            &mut hash,
            "cldr-cardinal-rules-sha256",
            cardinal_digest.as_bytes(),
        );
        Self::field(
            &mut hash,
            "cldr-ordinal-rules-sha256",
            ordinal_digest.as_bytes(),
        );
        Self::field(&mut hash, "cldr-aliases-sha256", aliases_digest.as_bytes());
        Self::field(
            &mut hash,
            "cldr-supplemental-data-sha256",
            supplemental_digest.as_bytes(),
        );
        Self::field(&mut hash, "locale-extension-policy", b"reject-all");
        Self::field(
            &mut hash,
            "atomic-locale-selection",
            if generation.atomic_locale {
                b"true"
            } else {
                b"false"
            },
        );
        Self::field(
            &mut hash,
            "final-fallback-locale",
            generation
                .final_fallback_locale
                .unwrap_or_default()
                .as_bytes(),
        );
        Self::field(
            &mut hash,
            "pool-limit",
            generation.pool_limit.to_string().as_bytes(),
        );
        Self::hex(&hash.finish())
    }

    pub(super) fn digest(bytes: &[u8]) -> String {
        let mut hash = Sha256::new();
        hash.update(bytes);
        Self::hex(&hash.finish())
    }

    fn field(hash: &mut Sha256, label: &str, value: &[u8]) {
        hash.update(label.as_bytes());
        hash.update(&[0]);
        hash.update(&(value.len() as u64).to_be_bytes());
        hash.update(value);
    }

    fn hex(bytes: &[u8]) -> String {
        let digits = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len().saturating_mul(2));
        for byte in bytes {
            let high = usize::from(byte >> 4);
            let low = usize::from(byte & 0x0f);
            output.push(char::from(*digits.get(high).unwrap_or(&b'0')));
            output.push(char::from(*digits.get(low).unwrap_or(&b'0')));
        }
        output
    }
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffer_len: 0,
            bit_len: 0,
        }
    }

    fn update(&mut self, mut bytes: &[u8]) {
        self.bit_len = self
            .bit_len
            .wrapping_add((bytes.len() as u64).wrapping_mul(8));
        while !bytes.is_empty() {
            let space = 64usize.saturating_sub(self.buffer_len);
            let count = space.min(bytes.len());
            let destination_end = self.buffer_len.saturating_add(count);
            if let (Some(destination), Some(source)) = (
                self.buffer.get_mut(self.buffer_len..destination_end),
                bytes.get(..count),
            ) {
                destination.copy_from_slice(source);
            }
            self.buffer_len = destination_end;
            bytes = bytes.get(count..).unwrap_or(&[]);
            if self.buffer_len == 64 {
                self.compress();
                self.buffer_len = 0;
            }
        }
    }

    fn finish(&mut self) -> [u8; 32] {
        let bit_len = self.bit_len;
        if let Some(byte) = self.buffer.get_mut(self.buffer_len) {
            *byte = 0x80;
        }
        self.buffer_len += 1;
        if self.buffer_len > 56 {
            if let Some(padding) = self.buffer.get_mut(self.buffer_len..) {
                padding.fill(0);
            }
            self.compress();
            self.buffer = [0; 64];
            self.buffer_len = 0;
        }
        if let Some(padding) = self.buffer.get_mut(self.buffer_len..56) {
            padding.fill(0);
        }
        if let Some(length) = self.buffer.get_mut(56..64) {
            length.copy_from_slice(&bit_len.to_be_bytes());
        }
        self.compress();
        let mut output = [0u8; 32];
        for (index, value) in self.state.iter().enumerate() {
            let start = index.saturating_mul(4);
            if let Some(target) = output.get_mut(start..start + 4) {
                target.copy_from_slice(&value.to_be_bytes());
            }
        }
        output
    }

    fn compress(&mut self) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut words = [0u32; 64];
        for (index, chunk) in self.buffer.as_chunks::<4>().0.iter().enumerate() {
            let value = u32::from_be_bytes([
                chunk.first().copied().unwrap_or_default(),
                chunk.get(1).copied().unwrap_or_default(),
                chunk.get(2).copied().unwrap_or_default(),
                chunk.get(3).copied().unwrap_or_default(),
            ]);
            if let Some(word) = words.get_mut(index) {
                *word = value;
            }
        }
        for index in 16..64 {
            let x = words.get(index - 15).copied().unwrap_or_default();
            let y = words.get(index - 2).copied().unwrap_or_default();
            let sigma0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            let sigma1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
            let value = words
                .get(index - 16)
                .copied()
                .unwrap_or_default()
                .wrapping_add(sigma0)
                .wrapping_add(words.get(index - 7).copied().unwrap_or_default())
                .wrapping_add(sigma1);
            if let Some(word) = words.get_mut(index) {
                *word = value;
            }
        }
        let mut a = self.state.first().copied().unwrap_or_default();
        let mut b = self.state.get(1).copied().unwrap_or_default();
        let mut c = self.state.get(2).copied().unwrap_or_default();
        let mut d = self.state.get(3).copied().unwrap_or_default();
        let mut e = self.state.get(4).copied().unwrap_or_default();
        let mut f = self.state.get(5).copied().unwrap_or_default();
        let mut g = self.state.get(6).copied().unwrap_or_default();
        let mut h = self.state.get(7).copied().unwrap_or_default();
        for index in 0..64 {
            let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let first = h
                .wrapping_add(sigma1)
                .wrapping_add(choice)
                .wrapping_add(K.get(index).copied().unwrap_or_default())
                .wrapping_add(words.get(index).copied().unwrap_or_default());
            let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let second = sigma0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(first);
            d = c;
            c = b;
            b = a;
            a = first.wrapping_add(second);
        }
        for (index, value) in [a, b, c, d, e, f, g, h].iter().enumerate() {
            if let Some(state) = self.state.get_mut(index) {
                *state = state.wrapping_add(*value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Fingerprint, FingerprintInput};
    use crate::codegen::GenerationConfig;

    #[test]
    fn matches_standard_sha256_vectors() {
        assert_eq!(
            Fingerprint::digest(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            Fingerprint::digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn locale_data_digests_change_the_fingerprint() {
        let calculate = |cardinal_digest, aliases_digest, supplemental_digest| {
            Fingerprint::calculate(FingerprintInput {
                schema: "",
                locales: &[],
                registry_digest: "registry",
                cardinal_digest,
                ordinal_digest: "ordinal",
                aliases_digest,
                supplemental_digest,
                generation: GenerationConfig {
                    pool_limit: 256,
                    atomic_locale: false,
                    final_fallback_locale: None,
                },
            })
        };

        let original = calculate("cardinal", "aliases", "supplemental");

        assert_ne!(
            original,
            calculate("different-cardinal", "aliases", "supplemental")
        );
        assert_ne!(
            original,
            calculate("cardinal", "different-aliases", "supplemental")
        );
        assert_ne!(
            original,
            calculate("cardinal", "aliases", "different-supplemental")
        );
    }
}
