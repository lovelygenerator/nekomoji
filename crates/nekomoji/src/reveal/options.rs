//! 文字送りの設定: 動きの強さ・オートモードの待ち時間・文字送りの音。

/// 動きの強さ。光過敏や動きに弱い人のために、揺れ・波・猫の跳ね方・発光の明滅・
/// 猫のちょっかいをまとめて弱める・止める。ゲームの設定画面の「演出: 標準 / 控えめ / なし」に
/// そのままつなげられる。文字送りの時間割(いつ何が出るか)は変わらない。
///
/// | | `Full` | `Reduced` | `Off` |
/// |---|---|---|---|
/// | 揺れ・波 | そのまま | 振れ幅 0.3 倍、1 秒以内に止まる | 動かない |
/// | 猫の跳ね方 | そのまま | 振れ幅 0.3 倍、回さない | 動かない(現れ方の不透明度だけ) |
/// | 発光 `in`・`breathe` | そのまま | `breathe` の明滅を浅く | 一定の明るさで光る |
/// | 発光 `flicker` | 1/3 秒に1度まで瞬く | 1 秒に1度まで、浅く・文字は暗くしない | 一定の明るさで光る |
/// | 猫パンチ・爆散 | あり | なし | なし |
/// | 押して落とす | あり | あり | なし |
/// | フェード・入力待ちのカーソル | あり | あり | あり |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Motion {
    /// 標準
    #[default]
    Full,
    /// 控えめ
    Reduced,
    /// なし(動きと明滅を止める)
    Off,
}

impl Motion {
    /// 揺れ・波・跳ね方の振れ幅に掛ける倍率。
    pub fn amplitude(self) -> f32 {
        match self {
            Motion::Full => 1.0,
            Motion::Reduced => 0.3,
            Motion::Off => 0.0,
        }
    }
}

/// 控えめ([`Motion::Reduced`])のとき、揺れ・波が止まるまでの最長の秒数
pub(crate) const REDUCED_DECAY_SECS: f32 = 1.0;
/// [`Motion::Off`] のときの発光の明るさ(強さに掛ける)
pub(crate) const STEADY_GLOW: f32 = 0.6;

/// 文字送りの設定。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RevealOptions {
    /// 1秒あたりの文字数(`[speed]` の倍率を掛ける)。0 以下・NaN・無限大なら待たずに表示する
    pub chars_per_sec: f32,
    /// 読点(、,)と三点リーダー(…‥)の後の間(秒)。数の中の `,`(`1,000`)の後には置かない。負・NaN は 0
    pub comma_pause: f32,
    /// 句点・感嘆符・疑問符(。！？)の後の間(秒)。負・NaN は 0
    pub period_pause: f32,
    /// 動きの強さ。猫モードのちょっかいにも効く
    pub motion: Motion,
    /// 文字送りの音。`Some` なら本文全体で鳴らす。`None` なら `[blip]` の範囲だけで鳴らす
    /// (間隔などは [`Blip::default`])
    pub blip: Option<Blip>,
    /// オートモードの待ち時間の決め方([`Reveal::auto_wait`](crate::Reveal::auto_wait))
    pub auto: AutoWait,
}

/// オートモードの待ち時間の決め方。入力待ちから自動で次へ進むまでの推奨の秒数を
/// [`Reveal::auto_wait`](crate::Reveal::auto_wait) が計算する。進めるのは利用者側([`Reveal::auto_due`](crate::Reveal::auto_due) で判定して
/// [`Reveal::proceed`](crate::Reveal::proceed) を呼ぶ)。
///
/// - 読む時間: `base + per_char × ページの文字数`(空白を除く)。`max` を超えない
/// - ボイス(`[voice id]`)の長さが分かれば、ボイスが鳴り終わってから `after_voice` 秒待つ。
///   読む時間より短ければ読む時間を使う。長さの分からないボイスは数えない
///
/// ゲームの設定画面の「オートの速さ」は、返った秒数に倍率を掛けて使う。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AutoWait {
    /// どのページでも待つ秒数(既定 1.0)
    pub base: f32,
    /// 1文字あたりに足す秒数(既定 0.05)
    pub per_char: f32,
    /// 読む時間の上限(秒。既定 8.0。ボイスを待つ時間には掛けない)
    pub max: f32,
    /// ボイスが鳴り終わってから待つ秒数(既定 0.5)
    pub after_voice: f32,
}

impl Default for AutoWait {
    fn default() -> Self {
        Self {
            base: 1.0,
            per_char: 0.05,
            max: 8.0,
            after_voice: 0.5,
        }
    }
}

impl AutoWait {
    setters! {
        /// どのページでも待つ秒数([`AutoWait::base`])
        base: f32;
        /// 1文字あたりに足す秒数([`AutoWait::per_char`])
        per_char: f32;
        /// 読む時間の上限([`AutoWait::max`])
        max: f32;
        /// ボイスが鳴り終わってから待つ秒数([`AutoWait::after_voice`])
        after_voice: f32;
    }
}

/// 文字送りの音(RPG の「ポポポ」)。文字が出たときに [`TextEvent::Blip`](crate::TextEvent::Blip) を出す。再生は利用者側。
///
/// 音を鳴らすのは字(`char::is_alphanumeric`。仮名・漢字・英数字)だけで、空白・句読点・括弧・記号では
/// 鳴らさない。`every` 文字ごとに1度鳴らし、数えるのはページの始めと音の ID が変わったところから。
/// 前の音から `min_interval` 秒たっていなければ鳴らさない。1回の [`Reveal::advance`](crate::Reveal::advance) で返す音は
/// 最後の1つだけで、[`Reveal::skip`](crate::Reveal::skip) では鳴らさない(まとめて鳴らないように)。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Blip {
    /// 音の ID(`[blip]` の範囲で引数を省いたときにも使う。既定 `blip`)
    pub id: String,
    /// 何文字ごとに鳴らすか(既定 2。0 は 1 とみなす)
    pub every: usize,
    /// 音と音の最短の間隔(秒。既定 0.05)
    pub min_interval: f32,
}

impl Default for Blip {
    fn default() -> Self {
        Self {
            id: "blip".to_string(),
            every: 2,
            min_interval: 0.05,
        }
    }
}

impl Blip {
    setters! {
        /// 何文字ごとに鳴らすか([`Blip::every`])
        every: usize;
        /// 音と音の最短の間隔([`Blip::min_interval`])
        min_interval: f32;
    }

    /// 音の ID([`Blip::id`])。
    #[must_use]
    pub fn id(mut self, value: impl Into<String>) -> Self {
        self.id = value.into();
        self
    }
}

impl RevealOptions {
    setters! {
        /// 1秒あたりの文字数([`RevealOptions::chars_per_sec`])
        chars_per_sec: f32;
        /// 読点・三点リーダーの後の間([`RevealOptions::comma_pause`])
        comma_pause: f32;
        /// 句点などの後の間([`RevealOptions::period_pause`])
        period_pause: f32;
        /// 動きの強さ([`RevealOptions::motion`])
        motion: Motion;
        /// 文字送りの音([`RevealOptions::blip`])
        blip: Option<Blip>;
        /// オートモードの待ち時間([`RevealOptions::auto`])
        auto: AutoWait;
    }
}

impl Default for RevealOptions {
    fn default() -> Self {
        Self {
            chars_per_sec: 20.0,
            comma_pause: 0.1,
            period_pause: 0.3,
            motion: Motion::Full,
            blip: None,
            auto: AutoWait::default(),
        }
    }
}
