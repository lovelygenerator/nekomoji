use super::layers::{glow_bitmap, outline_bitmap};
use super::paw::paw_bitmap;
use super::*;

#[test]
fn 発光の光は字形をぼかして周りへ広げる() {
    // 4x4 の塗りつぶしを余白 4 でぼかす
    let (w, h, d) = glow_bitmap(4, 4, &[255; 16], 4);
    assert_eq!((w, h), (12, 12));
    let at = |x: u32, y: u32| d[(y * w + x) as usize];
    // 中央は濃く、外へ行くほど薄く、四隅はほぼ消える
    assert!(at(6, 6) > 120);
    assert!(at(6, 6) > at(2, 6) && at(2, 6) > at(0, 6));
    assert!(at(0, 0) < 20);
}

#[test]
fn 縁取りの形は字形を半径の分だけ丸く太らせ_外周は空ける() {
    // 1 画素の点を半径 3 で太らせる
    let (w, h, d) = outline_bitmap(1, 1, &[255], 3, 3);
    assert_eq!((w, h), (9, 9));
    let at = |x: u32, y: u32| d[(y * w + x) as usize];
    // 中心から距離 2 までは塗り、距離 3 で半分ほど、外周は 0
    assert_eq!(at(4, 4), 255);
    assert_eq!(at(6, 4), 255);
    assert!(at(7, 4) > 100 && at(7, 4) < 160, "{}", at(7, 4));
    assert_eq!(at(8, 4), 0);
    assert!((0..w).all(|x| at(x, 0) == 0 && at(x, h - 1) == 0));
    // 丸いので、角(距離 2.8)は端(距離 3)より濃く、斜め 3 つ(距離 4.2)は空ける
    assert!(at(6, 6) > at(7, 4));
    assert_eq!(at(7, 7), 0);
    // 横と縦で半径が違えば楕円
    let (w, h, _) = outline_bitmap(2, 2, &[255; 4], 1, 2);
    assert_eq!((w, h), (6, 8));
}

#[test]
fn 肉球は掌球と4つの指球で_縁がなめらか() {
    let (w, h, d) = paw_bitmap(20);
    assert_eq!((w, h), (20, 20));
    // 掌球の中心は塗られ、四隅は空く
    assert_eq!(d[(14 * 20 + 10) as usize], 255);
    assert_eq!(d[0], 0);
    assert_eq!(d[19 * 20 + 19], 0);
    // 指球の間(上の中央)は空く
    assert_eq!(d[(4 * 20 + 10) as usize], 0);
    // 縁は途中の値になる
    assert!(d.iter().any(|&v| v > 0 && v < 255));
}

#[test]
fn 色名と16進を読む() {
    assert_eq!(parse_color("#ff0000"), Some([1.0, 0.0, 0.0]));
    assert_eq!(parse_color("#fff"), Some([1.0, 1.0, 1.0]));
    assert_eq!(parse_color("#f00"), parse_color("#ff0000"));
    assert_eq!(parse_color("RED"), parse_color("red"));
    assert_eq!(parse_color("#ff00"), None);
    assert!(parse_color("silver").is_some());
    assert!(parse_color("red").is_some());
    assert_eq!(parse_color("なぞの色"), None);
}

#[test]
fn 四角形のバイト幅は_f32_の_14_個ぶん() {
    assert_eq!(std::mem::size_of::<Quad>(), 56);
}

/// 速くする前の縁取りの形(すべての画素を楕円で広げる)。今の実装と比べるために残す。
fn outline_bitmap_naive(w: u32, h: u32, src: &[u8], rx: u32, ry: u32) -> (u32, u32, Vec<u8>) {
    let (padx, pady) = (rx + 1, ry + 1);
    let (ow, oh) = (w + 2 * padx, h + 2 * pady);
    // 楕円の中で 1、縁の 1px で 1 → 0 へ下がる重み。円なら clamp(r + 0.5 - 距離, 0, 1)
    let (ex, ey) = (rx as f32 + 0.5, ry as f32 + 0.5);
    let m = ex.min(ey);
    let mut kernel = Vec::new();
    for dy in -(ry as i32)..=ry as i32 {
        for dx in -(rx as i32)..=rx as i32 {
            let n = ((dx as f32 / ex).powi(2) + (dy as f32 / ey).powi(2)).sqrt();
            let k = ((1.0 - n) * m).clamp(0.0, 1.0);
            if k > 0.0 {
                kernel.push((dx, dy, k));
            }
        }
    }
    let mut out = vec![0u8; (ow * oh) as usize];
    for y in 0..h {
        for x in 0..w {
            let v = src[(y * w + x) as usize];
            if v == 0 {
                continue;
            }
            let (cx, cy) = ((x + padx) as i32, (y + pady) as i32);
            for &(dx, dy, k) in &kernel {
                let i = ((cy + dy) as u32 * ow + (cx + dx) as u32) as usize;
                let a = (v as f32 * k).round() as u8;
                if a > out[i] {
                    out[i] = a;
                }
            }
        }
    }
    (ow, oh, out)
}

#[test]
fn 縁取りの形は前の作り方と同じになる() {
    // 字の中・縁(0 と 255 の間)・穴の混ざった覆いを、決まった乱数で作って比べる
    let mut seed = 12345u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for (w, h, rx, ry) in [
        (1, 1, 3, 3),
        (7, 5, 2, 4),
        (16, 12, 5, 5),
        (23, 9, 1, 6),
        (12, 30, 7, 3),
    ] {
        let src: Vec<u8> = (0..w * h)
            .map(|_| match next() % 4 {
                0 => 0,
                1 => (next() % 254 + 1) as u8,
                _ => 255,
            })
            .collect();
        assert_eq!(
            outline_bitmap(w, h, &src, rx, ry),
            outline_bitmap_naive(w, h, &src, rx, ry),
            "{w}x{h} r=({rx},{ry})"
        );
    }
}
