//! シード付きの決定的な乱数。時刻の取得や外部の乱数は使わない。

/// (シード, ページ番号) から決まる 0.0〜1.0 未満の値。
pub(super) fn page_random(seed: u64, page: usize) -> f32 {
    let mut rng = XorShift::new(seed ^ (page as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    rng.unit()
}

/// xorshift64*。シードは splitmix64 で散らしてから使う。
pub(super) struct XorShift(u64);

impl XorShift {
    pub(super) fn new(seed: u64) -> Self {
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        XorShift(if z == 0 { 0x2545_f491_4f6c_dd1d } else { z })
    }

    /// 0.0〜1.0 未満。
    pub(super) fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// lo〜hi。
    pub(super) fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    pub(super) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}
