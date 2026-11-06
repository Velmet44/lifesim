//! Deterministic per-subsystem RNG: blake3(seed||tag||tick) -> Xoshiro256++.
//! No wall-clock / thread-id entropy. tick = global phase counter u64 LE.

pub const RNG_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct Xoshiro256PlusPlus {
    s: [u64; 4],
}

impl Xoshiro256PlusPlus {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let mut s = [0u64; 4];
        for i in 0..4 {
            s[i] = u64::from_le_bytes(seed[i * 8..(i + 1) * 8].try_into().unwrap());
        }
        // Avoid all-zero state (blake3 essentially never gives it, but guard anyway).
        if s == [0; 4] {
            s[0] = 0x9E3779B97F4A7C15;
        }
        Self { s }
    }
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        // 24-bit mantissa uniform [0,1)
        ((self.next_u64() >> 40) as f32) / 16777216.0
    }
    pub fn next_range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }
}

#[derive(Debug, Clone)]
pub struct RngStream {
    pub tag: String,
    pub seed: u64,
    pub tick: u64,
    inner: Xoshiro256PlusPlus,
}

impl RngStream {
    pub fn new(seed: u64, tag: &str, tick: u64) -> Self {
        let mut h = blake3::Hasher::new();
        h.update(&seed.to_le_bytes());
        h.update(b"||");
        h.update(tag.as_bytes());
        h.update(b"||");
        h.update(&tick.to_le_bytes());
        h.update(b"||rngv1");
        let hash = h.finalize();
        let inner = Xoshiro256PlusPlus::from_seed(*hash.as_bytes());
        Self { tag: tag.to_string(), seed, tick, inner }
    }
    pub fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }
    pub fn next_f32(&mut self) -> f32 {
        self.inner.next_f32()
    }
    pub fn reseed_tick(&mut self, tick: u64) {
        *self = Self::new(self.seed, &self.tag.clone(), tick);
    }
}
