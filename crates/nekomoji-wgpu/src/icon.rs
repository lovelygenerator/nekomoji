//! 文中のアイコン(`[icon id]`)の絵の置き場。RGBA の絵を ID で登録し、1枚のアトラスへ棚詰めで置く。
//!
//! GPU に触れない純粋な処理。テクスチャへの転送は [`IconSet::take_uploads`] で取り出して行う
//! ([`Renderer::prepare`](crate::Renderer::prepare) が呼ぶ)。
//!
//! 文字のアトラス([`Atlas`](crate::atlas::Atlas))と違い、登録した絵は利用者のものなので、
//! 満杯になっても捨てずに、アトラスの一辺を2倍にして置き直す。絵の画素は手元にも持つ。

use std::collections::HashMap;
use std::fmt;

/// 隣の絵ににじまないための余白(px)。絵の上下左右に、端の画素を写して置く(端が薄くならない)。
const PAD: u32 = 1;

/// 最初に確保するアトラスの一辺(px)。最初の絵を登録するまでは確保しない(一辺 0)。
const FIRST_SIZE: u32 = 256;

/// アトラスの一辺の既定の上限(px)。[`IconSet::with_max_size`] で変えられる。
pub const DEFAULT_MAX_SIZE: u32 = 4096;

/// アトラス内の1枚の絵の位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct IconEntry {
    /// アトラス内の左端(px)
    pub x: u32,
    /// アトラス内の上端(px)
    pub y: u32,
    /// 幅(px)
    pub w: u32,
    /// 高さ(px)
    pub h: u32,
}

/// GPU へ転送する矩形(RGBA、1 画素 4 バイト)。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct IconUpload {
    /// 転送先の左端(px)
    pub x: u32,
    /// 転送先の上端(px)
    pub y: u32,
    /// 幅(px)
    pub w: u32,
    /// 高さ(px)
    pub h: u32,
    /// RGBA(行は詰める)
    pub data: Vec<u8>,
}

/// 絵を登録できなかった理由。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IconError {
    /// 幅か高さが 0
    Empty,
    /// 画素の長さが `幅 × 高さ × 4` でない
    BadLength {
        /// 渡された長さ
        got: usize,
        /// 必要な長さ
        expected: usize,
    },
    /// 上限の大きさのアトラスにも入らない(絵が大きすぎるか、登録した絵が多すぎる)
    TooLarge,
}

impl fmt::Display for IconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IconError::Empty => write!(f, "絵の幅と高さは 1 以上にしてください"),
            IconError::BadLength { got, expected } => write!(
                f,
                "画素の長さが合いません(幅 × 高さ × 4 = {expected} バイトのところ {got} バイト)"
            ),
            IconError::TooLarge => write!(f, "アトラスの上限の大きさに入りません"),
        }
    }
}

impl std::error::Error for IconError {}

/// 登録した1枚の絵。
struct Image {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
    entry: IconEntry,
}

/// 文中のアイコンの絵の置き場(RGBA のアトラス)。
///
/// [`Atlas::icons_mut`](crate::atlas::Atlas::icons_mut) で取り出して絵を登録する。
/// 絵は straight(事前乗算なし)の RGBA で渡す。透明な画素の色が黒だと、拡大・縮小で縁が黒ずむので、
/// 透明な画素にも隣の色を入れておくとよい。
pub struct IconSet {
    size: u32,
    max_size: u32,
    images: Vec<Image>,
    index: HashMap<String, usize>,
    pending: Vec<IconUpload>,
    // 棚詰め
    shelf_y: u32,
    shelf_h: u32,
    cursor_x: u32,
}

impl Default for IconSet {
    fn default() -> Self {
        Self::new()
    }
}

impl IconSet {
    /// 絵を1枚も持たない置き場。最初の絵を登録するまでテクスチャの場所は要らない([`IconSet::size`] は 0)。
    pub fn new() -> Self {
        Self::with_max_size(DEFAULT_MAX_SIZE)
    }

    /// アトラスの一辺の上限を決める([`Renderer::max_atlas_size`](crate::Renderer::max_atlas_size) を渡すとよい)。
    pub fn with_max_size(max_size: u32) -> Self {
        Self {
            size: 0,
            max_size,
            images: Vec::new(),
            index: HashMap::new(),
            pending: Vec::new(),
            shelf_y: 0,
            shelf_h: 0,
            cursor_x: 0,
        }
    }

    /// アトラスの一辺(px)。絵が1枚も無ければ 0。
    pub fn size(&self) -> u32 {
        self.size
    }

    /// 登録した絵の数。
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// 絵を1枚も登録していないか。
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    /// `id` の絵を登録しているか。
    pub fn contains(&self, id: &str) -> bool {
        self.index.contains_key(id)
    }

    /// `id` の絵の、アトラス内の位置。
    pub fn get(&self, id: &str) -> Option<IconEntry> {
        self.index.get(id).map(|&i| self.images[i].entry)
    }

    /// `id` の絵の、元の大きさ(幅, 高さ)(px)。
    pub fn dimensions(&self, id: &str) -> Option<(u32, u32)> {
        self.index.get(id).map(|&i| {
            let im = &self.images[i];
            (im.w, im.h)
        })
    }

    /// `id` の絵を登録する。同じ `id` が既にあれば置き換える。
    ///
    /// `rgba` は `幅 × 高さ × 4` バイトの straight の RGBA(左上から行ごと)。
    /// 満杯のときはアトラスの一辺を2倍にして(上限まで)全部置き直す。置き直したら、次の
    /// [`Renderer::prepare`](crate::Renderer::prepare) が全部転送する。
    pub fn insert(&mut self, id: &str, w: u32, h: u32, rgba: &[u8]) -> Result<(), IconError> {
        if w == 0 || h == 0 {
            return Err(IconError::Empty);
        }
        let expected = (w as usize)
            .checked_mul(h as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(IconError::TooLarge)?;
        if rgba.len() != expected {
            return Err(IconError::BadLength {
                got: rgba.len(),
                expected,
            });
        }
        if w.saturating_add(2 * PAD) > self.max_size || h.saturating_add(2 * PAD) > self.max_size {
            return Err(IconError::TooLarge);
        }
        let image = Image {
            w,
            h,
            rgba: rgba.to_vec(),
            entry: IconEntry { x: 0, y: 0, w, h },
        };
        match self.index.get(id).copied() {
            Some(i) => {
                // 置き換え。大きさが変わりうるので、全部置き直す
                let old = std::mem::replace(&mut self.images[i], image);
                if self.repack().is_err() {
                    // 置き直しに失敗しても状態は変わらない。元の絵に戻す
                    self.images[i] = old;
                    return Err(IconError::TooLarge);
                }
            }
            None => {
                self.images.push(image);
                let i = self.images.len() - 1;
                self.index.insert(id.to_string(), i);
                if self.place_last().is_err() && self.repack().is_err() {
                    self.images.pop();
                    self.index.remove(id);
                    return Err(IconError::TooLarge);
                }
            }
        }
        Ok(())
    }

    /// `id` の絵を外す。登録していなければ `false`。残りは置き直す。
    pub fn remove(&mut self, id: &str) -> bool {
        let Some(i) = self.index.remove(id) else {
            return false;
        };
        self.images.remove(i);
        for v in self.index.values_mut() {
            if *v > i {
                *v -= 1;
            }
        }
        let _ = self.repack();
        true
    }

    /// 全部の絵をもう一度転送する対象にする。デバイスを作り直した(テクスチャを失った)ときに呼ぶ。
    pub fn reupload(&mut self) {
        self.pending.clear();
        let uploads: Vec<IconUpload> = self.images.iter().map(upload_of).collect();
        self.pending = uploads;
    }

    /// 溜まった転送分を取り出す。
    pub fn take_uploads(&mut self) -> Vec<IconUpload> {
        std::mem::take(&mut self.pending)
    }

    /// 末尾の絵を今の棚の続きに置く。置けなければ `Err`(アトラスはそのまま)。
    fn place_last(&mut self) -> Result<(), ()> {
        let i = self.images.len() - 1;
        let (w, h) = (self.images[i].w + 2 * PAD, self.images[i].h + 2 * PAD);
        let (x, y) = self.allocate(w, h).ok_or(())?;
        self.images[i].entry = IconEntry {
            x: x + PAD,
            y: y + PAD,
            w: self.images[i].w,
            h: self.images[i].h,
        };
        self.pending.push(upload_of(&self.images[i]));
        Ok(())
    }

    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w > self.size || h > self.size {
            return None;
        }
        let (mut cx, mut sy, mut sh) = (self.cursor_x, self.shelf_y, self.shelf_h);
        if cx + w > self.size {
            sy += sh;
            sh = 0;
            cx = 0;
        }
        if sy + h > self.size {
            return None;
        }
        let pos = (cx, sy);
        self.cursor_x = cx + w;
        self.shelf_y = sy;
        self.shelf_h = sh.max(h);
        Some(pos)
    }

    /// 全部の絵を、必要なら一辺を大きくして置き直す。置けなければ `Err`(状態は変えない)。
    fn repack(&mut self) -> Result<(), ()> {
        let mut size = self.size.max(FIRST_SIZE.min(self.max_size));
        loop {
            if let Some(entries) = pack_all(&self.images, size) {
                for (im, e) in self.images.iter_mut().zip(entries) {
                    im.entry = e;
                }
                self.size = size;
                self.pending = self.images.iter().map(upload_of).collect();
                // 棚の続きは、置き直した最後の位置から
                self.resume_shelf();
                return Ok(());
            }
            match size.checked_mul(2).filter(|s| *s <= self.max_size) {
                Some(s) => size = s,
                None => return Err(()),
            }
        }
    }

    /// 置いた絵の並びから、棚詰めの続きの位置を求める。
    fn resume_shelf(&mut self) {
        self.shelf_y = 0;
        self.shelf_h = 0;
        self.cursor_x = 0;
        let sizes: Vec<(u32, u32)> = self
            .images
            .iter()
            .map(|im| (im.w + 2 * PAD, im.h + 2 * PAD))
            .collect();
        let mut tmp = Shelf::new(self.size);
        for (w, h) in sizes {
            tmp.allocate(w, h);
        }
        self.shelf_y = tmp.y;
        self.shelf_h = tmp.h;
        self.cursor_x = tmp.x;
    }
}

/// 棚詰めの計算だけ(絵の画素は持たない)。
struct Shelf {
    size: u32,
    x: u32,
    y: u32,
    h: u32,
}

impl Shelf {
    fn new(size: u32) -> Self {
        Self {
            size,
            x: 0,
            y: 0,
            h: 0,
        }
    }

    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w > self.size || h > self.size {
            return None;
        }
        if self.x + w > self.size {
            self.y += self.h;
            self.h = 0;
            self.x = 0;
        }
        if self.y + h > self.size {
            return None;
        }
        let pos = (self.x, self.y);
        self.x += w;
        self.h = self.h.max(h);
        Some(pos)
    }
}

/// 一辺 `size` に全部の絵が入るなら、その位置(余白の内側)を返す。
fn pack_all(images: &[Image], size: u32) -> Option<Vec<IconEntry>> {
    let mut shelf = Shelf::new(size);
    images
        .iter()
        .map(|im| {
            let (x, y) = shelf.allocate(im.w + 2 * PAD, im.h + 2 * PAD)?;
            Some(IconEntry {
                x: x + PAD,
                y: y + PAD,
                w: im.w,
                h: im.h,
            })
        })
        .collect()
}

/// 絵の周りに端の画素を写した余白を付けて、転送する矩形にする。
fn upload_of(im: &Image) -> IconUpload {
    let (w, h, pad) = (im.w as usize, im.h as usize, PAD as usize);
    let bw = w + 2 * pad;
    let bh = h + 2 * pad;
    let mut data = vec![0u8; bw * bh * 4];
    for row in 0..bh {
        let sy = row.saturating_sub(pad).min(h - 1);
        for col in 0..bw {
            let sx = col.saturating_sub(pad).min(w - 1);
            let src = (sy * w + sx) * 4;
            let dst = (row * bw + col) * 4;
            data[dst..dst + 4].copy_from_slice(&im.rgba[src..src + 4]);
        }
    }
    IconUpload {
        x: im.entry.x - PAD,
        y: im.entry.y - PAD,
        w: bw as u32,
        h: bh as u32,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(w: u32, h: u32, v: u8) -> Vec<u8> {
        vec![v; (w * h * 4) as usize]
    }

    #[test]
    fn 絵を登録するまでは場所を取らない() {
        let s = IconSet::new();
        assert_eq!((s.size(), s.len()), (0, 0));
        assert!(s.is_empty() && s.get("a").is_none());
    }

    #[test]
    fn 登録した絵の位置と元の大きさを引ける() {
        let mut s = IconSet::new();
        s.insert("a", 10, 6, &px(10, 6, 255)).unwrap();
        assert_eq!(s.size(), 256);
        assert_eq!(
            s.get("a"),
            Some(IconEntry {
                x: 1,
                y: 1,
                w: 10,
                h: 6
            })
        );
        assert_eq!(s.dimensions("a"), Some((10, 6)));
        assert!(s.contains("a") && !s.contains("b"));
    }

    #[test]
    fn 絵の周りに端の画素を写した余白を付けて転送する() {
        let mut s = IconSet::new();
        // 2x1: 左が赤、右が青
        let rgba = vec![255, 0, 0, 255, 0, 0, 255, 255];
        s.insert("a", 2, 1, &rgba).unwrap();
        let u = &s.take_uploads()[0];
        assert_eq!((u.x, u.y, u.w, u.h), (0, 0, 4, 3));
        let at = |c: usize, r: usize| &u.data[(r * 4 + c) * 4..(r * 4 + c) * 4 + 4];
        // 絵の外側(余白)は端の画素と同じ
        assert_eq!(at(0, 0), &[255, 0, 0, 255]);
        assert_eq!(at(1, 1), &[255, 0, 0, 255]);
        assert_eq!(at(2, 1), &[0, 0, 255, 255]);
        assert_eq!(at(3, 2), &[0, 0, 255, 255]);
        assert!(s.take_uploads().is_empty());
    }

    #[test]
    fn 行が埋まると次の棚へ進み_転送は追加した分だけ() {
        let mut s = IconSet::with_max_size(256);
        s.insert("a", 100, 10, &px(100, 10, 1)).unwrap();
        s.insert("b", 100, 10, &px(100, 10, 2)).unwrap();
        s.insert("c", 100, 10, &px(100, 10, 3)).unwrap();
        assert_eq!(s.get("a").unwrap().y, 1);
        assert_eq!((s.get("b").unwrap().x, s.get("b").unwrap().y), (103, 1));
        assert_eq!((s.get("c").unwrap().x, s.get("c").unwrap().y), (1, 13));
        assert_eq!(s.take_uploads().len(), 3);
        s.insert("d", 5, 5, &px(5, 5, 4)).unwrap();
        assert_eq!(s.take_uploads().len(), 1);
    }

    #[test]
    fn 満杯なら一辺を2倍にして全部置き直し_全部転送する() {
        let mut s = IconSet::with_max_size(1024);
        s.insert("a", 200, 200, &px(200, 200, 1)).unwrap();
        s.insert("b", 200, 200, &px(200, 200, 2)).unwrap();
        assert_eq!(s.size(), 256 * 2);
        assert_eq!(s.take_uploads().len(), 2);
        // 置き直した後も、続きの絵は重ならない
        s.insert("c", 200, 200, &px(200, 200, 3)).unwrap();
        let (a, b, c) = (
            s.get("a").unwrap(),
            s.get("b").unwrap(),
            s.get("c").unwrap(),
        );
        for (p, q) in [(a, b), (a, c), (b, c)] {
            let apart = p.x + p.w + 2 <= q.x
                || q.x + q.w + 2 <= p.x
                || p.y + p.h + 2 <= q.y
                || q.y + q.h + 2 <= p.y;
            assert!(apart, "{p:?} と {q:?} が重なっている");
        }
    }

    #[test]
    fn 上限に入らなければエラーで_登録済みの絵は変わらない() {
        let mut s = IconSet::with_max_size(256);
        s.insert("a", 200, 200, &px(200, 200, 1)).unwrap();
        let before = s.get("a");
        assert_eq!(
            s.insert("b", 200, 200, &px(200, 200, 2)),
            Err(IconError::TooLarge)
        );
        assert_eq!(
            s.insert("c", 300, 4, &px(300, 4, 2)),
            Err(IconError::TooLarge)
        );
        assert_eq!((s.get("a"), s.len(), s.contains("b")), (before, 1, false));
    }

    #[test]
    fn 画素の長さと大きさが合わなければエラー() {
        let mut s = IconSet::new();
        assert_eq!(s.insert("a", 0, 4, &[]), Err(IconError::Empty));
        assert_eq!(
            s.insert("a", 2, 2, &[0; 8]),
            Err(IconError::BadLength {
                got: 8,
                expected: 16
            })
        );
        assert!(s.is_empty());
    }

    #[test]
    fn 同じ_id_を登録し直すと置き換える() {
        let mut s = IconSet::new();
        s.insert("a", 4, 4, &px(4, 4, 1)).unwrap();
        s.insert("b", 4, 4, &px(4, 4, 2)).unwrap();
        s.take_uploads();
        s.insert("a", 8, 2, &px(8, 2, 9)).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s.dimensions("a"), Some((8, 2)));
        assert_eq!(s.take_uploads().len(), 2);
    }

    #[test]
    fn 外すと残りを置き直し_添字がずれない() {
        let mut s = IconSet::new();
        s.insert("a", 4, 4, &px(4, 4, 1)).unwrap();
        s.insert("b", 6, 4, &px(6, 4, 2)).unwrap();
        s.insert("c", 8, 4, &px(8, 4, 3)).unwrap();
        assert!(s.remove("a"));
        assert!(!s.remove("a"));
        assert_eq!(
            (s.len(), s.dimensions("b"), s.dimensions("c")),
            (2, Some((6, 4)), Some((8, 4)))
        );
        assert_eq!(s.get("b").unwrap().x, 1);
    }

    #[test]
    fn デバイスを作り直したら全部をもう一度転送できる() {
        let mut s = IconSet::new();
        s.insert("a", 4, 4, &px(4, 4, 1)).unwrap();
        s.insert("b", 4, 4, &px(4, 4, 2)).unwrap();
        s.take_uploads();
        s.reupload();
        assert_eq!(s.take_uploads().len(), 2);
    }
}
