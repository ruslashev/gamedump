use std::ops::Range;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Wyhash64 {
    state: u64,
}

impl Wyhash64 {
    pub fn new() -> Self {
        Self::from_seed(current_frac_ns())
    }

    pub fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn gen(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x60be_e2be_e120_fc15);

        let t = u128::from(self.state).wrapping_mul(0xa3b1_9535_4a39_b70d);
        let m = (((t >> 64) ^ t) & 0xffff_ffff_ffff_ffff) as u64;
        let y = u128::from(m).wrapping_mul(0x1b03_7387_12fa_d5c9);

        (((y >> 64) ^ y) & 0xffff_ffff_ffff_ffff) as u64
    }

    pub fn gen_in_range(&mut self, range: Range<u64>) -> u64 {
        let min = range.start;
        let max = range.end;

        min + self.gen() % (max - min)
    }
}

fn current_frac_ns() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time before Unix epoch")
        .subsec_nanos();

    u64::from(nanos)
}
