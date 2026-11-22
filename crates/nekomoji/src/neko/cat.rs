//! 見えない猫の向き・動作・状態。

/// 猫の向き。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Facing {
    /// 右向き(文字送りの方向。縦書きでは下へ)
    Right,
    /// 左向き(縦書きでは上へ)
    Left,
}

impl Facing {
    pub(super) fn sign(self) -> f32 {
        match self {
            Facing::Left => -1.0,
            _ => 1.0,
        }
    }
}

/// 見えない猫の動作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CatAction {
    /// 歩く
    Walk,
    /// 止まっている(入力待ちで落ち着いた後も)
    Idle,
    /// 伸びをする
    Stretch,
    /// 毛づくろいする
    Groom,
    /// 猫パンチ
    Punch,
    /// 入力待ちで最後の文字を押す・爆散させる
    Knock,
}

/// ある時刻の見えない猫の状態。猫は描かないが、利用者が自分で何かを描くときに使える。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CatState {
    /// 猫の足元の x(行頭からの位置)
    pub x: f32,
    /// 猫がいる行(ページ内)
    pub line: usize,
    /// その行にルビがあるか
    pub over_ruby: bool,
    /// 向き(歩く方向。止まっている間は最後に歩いた方向)
    pub facing: Facing,
    /// 動作
    pub action: CatAction,
    /// 今の動作を始めてからの秒数
    pub action_time: f32,
}
