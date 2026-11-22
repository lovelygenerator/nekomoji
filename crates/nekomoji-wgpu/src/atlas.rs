//! 動的グリフアトラス。出てきた文字だけを棚詰めで1枚の R8 テクスチャへ置く。
//!
//! GPU に触れない純粋な処理。テクスチャへの転送は [`Atlas::take_uploads`] で取り出して行う。

use std::collections::{HashMap, HashSet};

use crate::font::Ch;

/// アトラス内の1文字。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entry {
    /// アトラス内の左端(px)
    pub x: u32,
    /// アトラス内の上端(px)
    pub y: u32,
    /// 幅(px)
    pub w: u32,
    /// 高さ(px)
    pub h: u32,
    /// 原点(ベースライン左)からビットマップ左端までの距離(px)
    pub xmin: i32,
    /// ベースラインからビットマップ下端までの距離(px、上向き正)
    pub ymin: i32,
}

/// GPU へ転送する矩形。
#[derive(Debug, Clone)]
pub struct Upload {
    /// 転送先の左端(px)
    pub x: u32,
    /// 転送先の上端(px)
    pub y: u32,
    /// 幅(px)
    pub w: u32,
    /// 高さ(px)
    pub h: u32,
    /// カバレッジ(1 画素 1 バイト、行は詰める)
    pub data: Vec<u8>,
}

/// 文字(異体字セレクタつき)・大きさ(px を四捨五入した値)・縦書き用の字形か・加工、で引く。
pub type Key = (Ch, u32, bool, Shape);

/// 字形から作るビットマップの種類。同じ字形の素の形・発光の光・縁取りを、アトラスで別に置く。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Shape {
    /// 字形そのもの(肉球のカーソル・塗りつぶしなどの図形もこれ)
    Plain,
    /// 発光の光(層の番号)
    Glow(u8),
    /// 縁取り。字形を太らせた形(横・縦に太らせる半径 px)
    Outline(u32, u32),
}

/// 動的グリフアトラス(R8、棚詰め)。満杯になったら [`Atlas::clear`] で作り直す。
pub struct Atlas {
    size: u32,
    entries: HashMap<Key, Entry>,
    /// 一辺より大きくて置けない字形。[`Atlas::clear`] でも消さない(何度もラスタライズしない)
    too_large: HashSet<Key>,
    pending: Vec<Upload>,
    // 棚詰め
    shelf_y: u32,
    shelf_h: u32,
    cursor_x: u32,
}

/// 置けない字形の代わりの、大きさ 0 の字形(空白と同じく何も描かない)。
const EMPTY: Entry = Entry {
    x: 0,
    y: 0,
    w: 0,
    h: 0,
    xmin: 0,
    ymin: 0,
};

/// 隣のグリフににじまないための余白(px)。字形の上下左右に置き、0 を書き込む。
///
/// 余白を「書かずに空ける」だけだと、[`Atlas::clear`] の後(テクスチャは消さない)に前の字形の
/// 画素が余白に残り、線形補間で字の端ににじむ(viewer で大きさを変えた後、字の左に縦線が出た)。
/// 余白ごと転送するので、テクスチャに何が残っていても字の周りは必ず 0 になる。
const PAD: u32 = 1;

impl Atlas {
    /// 一辺 `size` px の空のアトラス。
    pub fn new(size: u32) -> Self {
        Self {
            size,
            entries: HashMap::new(),
            too_large: HashSet::new(),
            pending: Vec::new(),
            shelf_y: 0,
            shelf_h: 0,
            cursor_x: 0,
        }
    }

    /// 一辺の大きさ(px)。
    pub fn size(&self) -> u32 {
        self.size
    }

    /// 置いた字形の数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 字形を1つも置いていないか。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 置いた字形を引く。
    pub fn get(&self, key: Key) -> Option<Entry> {
        self.entries.get(&key).copied()
    }

    /// 無ければ `raster` でビットマップを作って置く。満杯のときは `None`。
    ///
    /// `raster` は (幅, 高さ, xmin, ymin, カバレッジ) を返す。字形が一辺より大きくて置けないとき
    /// (字の大きさの px だけで一辺を超えるときはラスタライズもしない)は、大きさ 0 の字形を返し、
    /// その鍵を覚えて二度とラスタライズしない。満杯とは違い、作り直しても置けないため。
    /// 数は [`Atlas::too_large_count`]。
    pub fn get_or_insert(
        &mut self,
        key: Key,
        raster: impl FnOnce() -> (u32, u32, i32, i32, Vec<u8>),
    ) -> Option<Entry> {
        if let Some(e) = self.entries.get(&key) {
            return Some(*e);
        }
        if self.too_large.contains(&key) {
            return Some(EMPTY);
        }
        // 字の大きさ(と縁取りの太さ)だけで一辺を超えるなら、ラスタライズせずに諦める
        // (とても大きな字でビットマップのメモリを確保しようとして落ちないように)
        let least = match key.3 {
            Shape::Outline(rx, ry) => key.1.saturating_add(rx.max(ry).saturating_mul(2)),
            _ => key.1,
        };
        if least > self.size {
            self.too_large.insert(key);
            return Some(EMPTY);
        }
        let (w, h, xmin, ymin, bitmap) = raster();
        if w + 2 * PAD > self.size || h + 2 * PAD > self.size {
            self.too_large.insert(key);
            return Some(EMPTY);
        }
        let entry = if w == 0 || h == 0 {
            // 空白など。場所は要らない
            Entry {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
                xmin,
                ymin,
            }
        } else {
            let (x, y) = self.allocate(w + 2 * PAD, h + 2 * PAD)?;
            self.pending.push(Upload {
                x,
                y,
                w: w + 2 * PAD,
                h: h + 2 * PAD,
                data: with_border(&bitmap, w, h),
            });
            Entry {
                x: x + PAD,
                y: y + PAD,
                w,
                h,
                xmin,
                ymin,
            }
        };
        self.entries.insert(key, entry);
        Some(entry)
    }

    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w > self.size || h > self.size {
            return None;
        }
        if self.cursor_x + w > self.size {
            // 次の棚へ
            self.shelf_y += self.shelf_h;
            self.shelf_h = 0;
            self.cursor_x = 0;
        }
        if self.shelf_y + h > self.size {
            return None;
        }
        let pos = (self.cursor_x, self.shelf_y);
        self.cursor_x += w;
        self.shelf_h = self.shelf_h.max(h);
        Some(pos)
    }

    /// 一辺に入らないと分かっている字形を、置けなかったものとして覚える(ラスタライズしない)。
    pub(crate) fn reject(&mut self, key: Key) {
        self.too_large.insert(key);
    }

    /// 一辺より大きくて置けなかった字形の数。0 でなければ、その字は描かれていない
    /// (アトラスを大きくするか、字を小さくする)。
    pub fn too_large_count(&self) -> usize {
        self.too_large.len()
    }

    /// 溜まった転送分を取り出す。
    pub fn take_uploads(&mut self) -> Vec<Upload> {
        std::mem::take(&mut self.pending)
    }

    /// 満杯のときに場所を作る。一辺を2倍にしても `max_size` 以下なら大きくし、そうでなければ
    /// 同じ大きさのまま空にする。どちらでも置いた字形はすべて捨てるので、四角形を組み立て直す。
    /// 大きくしたら `true`。
    ///
    /// 大きくしたときは、一辺に入らなかった字形([`Atlas::too_large_count`])も忘れて、もう一度試す。
    /// [`Renderer::prepare`](crate::Renderer::prepare) はアトラスの大きさに合わせてテクスチャを
    /// 作り直す。`max_size` は [`Renderer::max_atlas_size`](crate::Renderer::max_atlas_size) を渡すとよい。
    pub fn make_room(&mut self, max_size: u32) -> bool {
        let grown = self.size.checked_mul(2).filter(|s| *s <= max_size);
        self.clear();
        match grown {
            Some(size) => {
                self.size = size;
                self.too_large.clear();
                true
            }
            None => false,
        }
    }

    /// 全部捨てる(満杯になったとき)。呼んだ側はテクスチャも消すこと。
    pub fn clear(&mut self) {
        self.entries.clear();
        self.pending.clear();
        self.shelf_y = 0;
        self.shelf_h = 0;
        self.cursor_x = 0;
    }
}

/// ビットマップの上下左右に [`PAD`] px の 0 を足す。
fn with_border(bitmap: &[u8], w: u32, h: u32) -> Vec<u8> {
    let (w, h, pad) = (w as usize, h as usize, PAD as usize);
    let bw = w + 2 * pad;
    let mut out = vec![0; bw * (h + 2 * pad)];
    for (row, src) in bitmap.chunks(w).take(h).enumerate() {
        let start = (row + pad) * bw + pad;
        out[start..start + src.len()].copy_from_slice(src);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(w: u32, h: u32) -> impl FnOnce() -> (u32, u32, i32, i32, Vec<u8>) {
        move || (w, h, 0, 0, vec![255; (w * h) as usize])
    }

    #[test]
    fn 同じ文字は二度ラスタライズしない() {
        let mut a = Atlas::new(64);
        let e1 = a
            .get_or_insert(('あ'.into(), 16, false, Shape::Plain), r(10, 10))
            .unwrap();
        let e2 = a
            .get_or_insert(('あ'.into(), 16, false, Shape::Plain), || {
                panic!("再計算してはいけない")
            })
            .unwrap();
        assert_eq!(e1, e2);
        assert_eq!(a.take_uploads().len(), 1);
    }

    #[test]
    fn 行が埋まると次の棚へ進む() {
        let mut a = Atlas::new(32);
        let e1 = a
            .get_or_insert(('a'.into(), 16, false, Shape::Plain), r(14, 10))
            .unwrap();
        let e2 = a
            .get_or_insert(('b'.into(), 16, false, Shape::Plain), r(14, 10))
            .unwrap();
        assert_eq!((e1.x, e1.y), (1, 1));
        assert_eq!((e2.x, e2.y), (17, 1));
        let e3 = a
            .get_or_insert(('c'.into(), 16, false, Shape::Plain), r(14, 10))
            .unwrap();
        assert_eq!((e3.x, e3.y), (1, 13));
    }

    #[test]
    fn 満杯ならnoneを返しクリアで戻る() {
        let mut a = Atlas::new(16);
        assert!(a
            .get_or_insert(('a'.into(), 16, false, Shape::Plain), r(14, 14))
            .is_some());
        assert!(a
            .get_or_insert(('b'.into(), 16, false, Shape::Plain), r(14, 14))
            .is_none());
        a.clear();
        assert!(a
            .get_or_insert(('b'.into(), 16, false, Shape::Plain), r(14, 14))
            .is_some());
    }

    #[test]
    fn 場所を作ると上限までは一辺を2倍にし_上限なら空にする() {
        let mut a = Atlas::new(16);
        a.get_or_insert(('a'.into(), 16, false, Shape::Plain), r(14, 14))
            .unwrap();
        // 一辺に入らなかった字形も、大きくしたら試し直す
        a.get_or_insert(('b'.into(), 16, false, Shape::Plain), r(20, 4))
            .unwrap();
        assert_eq!(a.too_large_count(), 1);
        assert!(a.make_room(32));
        assert_eq!((a.size(), a.len(), a.too_large_count()), (32, 0, 0));
        let e = a
            .get_or_insert(('b'.into(), 16, false, Shape::Plain), r(20, 4))
            .unwrap();
        assert_eq!((e.w, e.h), (20, 4));
        // 上限を超えるなら大きさはそのまま空にする
        assert!(!a.make_room(32));
        assert_eq!((a.size(), a.len()), (32, 0));
        assert!(a.take_uploads().is_empty());
    }

    #[test]
    fn 同じ文字でも加工が違えば別に置く() {
        let mut a = Atlas::new(64);
        let plain = a
            .get_or_insert(('あ'.into(), 16, false, Shape::Plain), r(10, 10))
            .unwrap();
        let glow = a
            .get_or_insert(('あ'.into(), 16, false, Shape::Glow(0)), r(12, 12))
            .unwrap();
        let o1 = a
            .get_or_insert(('あ'.into(), 16, false, Shape::Outline(1, 1)), r(13, 13))
            .unwrap();
        let o2 = a
            .get_or_insert(('あ'.into(), 16, false, Shape::Outline(2, 2)), r(15, 15))
            .unwrap();
        assert_eq!(a.len(), 4);
        assert!(plain != glow && glow != o1 && o1 != o2);
    }

    #[test]
    fn 空白は場所を取らない() {
        let mut a = Atlas::new(16);
        let e = a
            .get_or_insert((' '.into(), 16, false, Shape::Plain), r(0, 0))
            .unwrap();
        assert_eq!((e.w, e.h), (0, 0));
        assert!(a.take_uploads().is_empty());
    }

    #[test]
    fn 範囲より大きい文字は大きさ0で返し_二度とラスタライズしない() {
        let mut a = Atlas::new(16);
        let e = a
            .get_or_insert(('a'.into(), 16, false, Shape::Plain), r(20, 4))
            .unwrap();
        assert_eq!((e.w, e.h), (0, 0));
        assert_eq!(a.too_large_count(), 1);
        // 満杯とは違い、クリアしても覚えている
        a.clear();
        let e = a
            .get_or_insert(('a'.into(), 16, false, Shape::Plain), || {
                panic!("再計算してはいけない")
            })
            .unwrap();
        assert_eq!((e.w, e.h), (0, 0));
        assert!(a.take_uploads().is_empty());
    }

    #[test]
    fn 字の大きさだけで範囲を超えるならラスタライズしない() {
        let mut a = Atlas::new(64);
        let e = a
            .get_or_insert(('a'.into(), u32::MAX, false, Shape::Plain), || {
                panic!("ラスタライズしてはいけない")
            })
            .unwrap();
        assert_eq!((e.w, e.h), (0, 0));
        // 縁取りは太らせる分も足して比べる
        let e = a
            .get_or_insert(('a'.into(), 40, false, Shape::Outline(20, 1)), || {
                panic!("ラスタライズしてはいけない")
            })
            .unwrap();
        assert_eq!((e.w, e.h), (0, 0));
        assert_eq!(a.too_large_count(), 2);
    }

    #[test]
    fn 字形の周りの余白も0で転送する() {
        // クリアの後はテクスチャに前の字形が残っているので、余白を書かずに空けるだけではにじむ
        let mut a = Atlas::new(64);
        let e = a
            .get_or_insert(('あ'.into(), 16, false, Shape::Plain), r(3, 2))
            .unwrap();
        let u = &a.take_uploads()[0];
        assert_eq!((u.x, u.y, u.w, u.h), (e.x - 1, e.y - 1, 5, 4));
        #[rustfmt::skip]
        assert_eq!(u.data, vec![
            0, 0, 0, 0, 0,
            0, 255, 255, 255, 0,
            0, 255, 255, 255, 0,
            0, 0, 0, 0, 0,
        ]);
        // 隣の字形の転送と重ならない(余白は互いに接するだけ)
        let f = a
            .get_or_insert(('い'.into(), 16, false, Shape::Plain), r(3, 2))
            .unwrap();
        let v = &a.take_uploads()[0];
        assert_eq!(v.x, u.x + u.w);
        assert_eq!(f.x, e.x + 5);
    }
}
