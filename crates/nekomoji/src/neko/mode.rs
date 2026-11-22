//! 猫モードの設定。

/// 猫モードの設定。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct NekoMode {
    /// 見えない猫が歩き回ってちょっかいを出す。`false` なら `[neko hop]` だけ
    pub walk: bool,
    /// 乱数のシード。同じシード・同じ入力なら同じ結果になる
    pub seed: u64,
    /// 入力待ちで最後の文字を押して落とす確率(1 ページあたり、0.0〜1.0)
    pub knock_chance: f32,
    /// 入力待ちでページの文字をまとめて爆散させる確率(1 ページあたり、0.0〜1.0)。
    /// 落とすより先に判定する
    pub burst_chance: f32,
    /// 猫が何かをしようとするたびに、そばの文字に猫パンチする確率(0.0〜1.0)
    pub punch_chance: f32,
    /// 猫が小走りするときの速さ(全角文字/秒)。ぶらぶら歩きはその 0.4 倍、駆け足は 1.8 倍。
    /// 歩き出しと止まる前はゆっくりになる
    pub walk_speed: f32,
}

impl NekoMode {
    setters! {
        /// 見えない猫のちょっかい([`NekoMode::walk`])
        walk: bool;
        /// 乱数のシード([`NekoMode::seed`])
        seed: u64;
        /// 最後の文字を押して落とす確率([`NekoMode::knock_chance`])
        knock_chance: f32;
        /// 爆散させる確率([`NekoMode::burst_chance`])
        burst_chance: f32;
        /// 猫パンチの確率([`NekoMode::punch_chance`])
        punch_chance: f32;
        /// 猫が小走りするときの速さ([`NekoMode::walk_speed`])
        walk_speed: f32;
    }
}

impl Default for NekoMode {
    fn default() -> Self {
        Self {
            walk: true,
            seed: 0,
            knock_chance: 0.15,
            burst_chance: 0.05,
            punch_chance: 0.12,
            walk_speed: 8.0,
        }
    }
}
