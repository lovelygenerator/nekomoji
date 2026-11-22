use super::flight::{FALL_SECS, FLIGHT_TOTAL, RETURN_WAIT};
use super::roaming::walk_range;
use super::*;
use crate::{parse, CellMetrics, LayoutOptions, RevealOptions, Vars};

fn setup(src: &str, width: f32) -> (Layout, Reveal) {
    let script = parse(src).unwrap();
    let opts = LayoutOptions {
        max_width: width,
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let r = Reveal::new(
        &layout,
        RevealOptions {
            chars_per_sec: 10.0,
            comma_pause: 0.0,
            period_pause: 0.0,
            motion: Motion::Full,
            blip: None,
            auto: crate::AutoWait::default(),
        },
    );
    (layout, r)
}

/// パンチなし・爆散なしで、落とす確率だけ決めた設定。
fn mode(walk_speed: f32, knock_chance: f32) -> NekoMode {
    NekoMode::default()
        .walk_speed(walk_speed)
        .knock_chance(knock_chance)
        .burst_chance(0.0)
        .punch_chance(0.0)
}

/// 0.02 秒ごとに `until` 秒まで (時刻, 猫, 表示数, 飛んでいる文字) を集める。
fn trace(
    layout: &Layout,
    r: &Reveal,
    neko: &Neko,
    until: f32,
) -> Vec<(f32, CatState, usize, Vec<Knocked>)> {
    let mut r = r.clone();
    let mut out = Vec::new();
    let mut t = 0.0;
    while t < until {
        r.advance(0.02);
        t += 0.02;
        if let Some(c) = neko.cat(layout, &r) {
            out.push((t, c, r.visible_count(), neko.knocked(layout, &r)));
        }
    }
    out
}

const LONG: &str =
    "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわをん";

#[test]
fn ページの途中の入力待ちでは押して落とさず_猫は表示済みの文字の上にいる() {
    let (layout, mut r) = setup("あいうえお[l]かきくけこ", 20.0);
    let neko = Neko::new(&layout, mode(8.0, 1.0));
    r.advance(0.5);
    assert_eq!(r.wait_kind(), Some(crate::WaitKind::Click));
    for _ in 0..300 {
        r.advance(0.02);
        assert!(neko.knocked(&layout, &r).is_empty());
        let cat = neko.cat(&layout, &r).unwrap();
        assert!(cat.x <= 5.0 + 1e-3, "{}", cat.x);
    }
    // 進めばページの終わりで押して落とす
    assert!(r.proceed());
    for _ in 0..300 {
        r.advance(0.02);
    }
    assert_eq!(r.wait_kind(), Some(crate::WaitKind::End));
    assert!(neko.quiet_time(&layout, &r, 0) > r.page_end(0));
}

#[test]
fn 猫は表示済みの文字の上からはみ出さない() {
    for seed in 0..20 {
        let (layout, r) = setup(LONG, 12.0);
        let neko = Neko::new(&layout, mode(8.0, 0.0).seed(seed));
        let page = &layout.pages[0];
        for (t, c, shown, _) in trace(&layout, &r, &neko, 8.0) {
            if shown == 0 {
                continue;
            }
            let newest = page.glyphs[shown - 1].line;
            assert!(c.line <= newest, "seed {seed} t {t}: {c:?}");
            let (lo, hi) = walk_range(page, shown, c.line).unwrap();
            assert!(
                c.x >= lo - 1e-3 && c.x <= hi + 1e-3,
                "seed {seed} t {t}: {c:?} {lo}..{hi}"
            );
        }
    }
}

#[test]
fn 歩く速さには緩急があり_向きは歩く方向() {
    // 文字送りの途中で待つ間、猫は出た文字の上をうろつく(戻ることもある)
    let (layout, r) = setup("あいうえおかきくけこさしす[wait 6]せそたち", 20.0);
    let mut speeds = Vec::new();
    let mut left = false;
    for seed in 0..10 {
        let neko = Neko::new(&layout, mode(8.0, 0.0).seed(seed));
        let tr = trace(&layout, &r, &neko, 7.0);
        for w in tr.windows(2) {
            let (a, b) = (&w[0].1, &w[1].1);
            if a.action == CatAction::Walk && b.action == CatAction::Walk && a.line == b.line {
                let dx = b.x - a.x;
                if dx.abs() > 1e-4 {
                    speeds.push(dx.abs() / 0.02);
                    let want = if dx > 0.0 {
                        Facing::Right
                    } else {
                        Facing::Left
                    };
                    assert_eq!(b.facing, want, "{a:?} {b:?}");
                    left |= dx < 0.0;
                }
            }
        }
    }
    let max = speeds.iter().cloned().fold(0.0, f32::max);
    let min = speeds.iter().cloned().fold(f32::INFINITY, f32::min);
    assert!(max > 8.0, "速いときがある: {max}");
    assert!(min < 1.0, "歩き出しと止まる前はゆっくり: {min}");
    assert!(left, "ときどき戻る");
}

#[test]
fn ときどき伸びと毛づくろいをし_新しい行へ跳び移る() {
    let (layout, r) = setup(LONG, 8.0);
    let mut seen = Vec::new();
    let mut lines = Vec::new();
    for seed in 0..10 {
        let neko = Neko::new(&layout, mode(8.0, 0.0).seed(seed));
        for (_, c, _, _) in trace(&layout, &r, &neko, 6.0) {
            seen.push(c.action);
            lines.push(c.line);
        }
    }
    for a in [
        CatAction::Walk,
        CatAction::Idle,
        CatAction::Stretch,
        CatAction::Groom,
    ] {
        assert!(seen.contains(&a), "{a:?}");
    }
    assert!(lines.iter().any(|l| *l > 0));
}

#[test]
fn 動きはシードで変わり_同じシードなら同じ() {
    let (layout, r) = setup(LONG, 12.0);
    let xs = |seed| {
        let neko = Neko::new(&layout, NekoMode::default().seed(seed));
        trace(&layout, &r, &neko, 3.0)
            .into_iter()
            .map(|(_, c, _, k)| (c.x, c.line, c.action, k))
            .collect::<Vec<_>>()
    };
    assert_eq!(xs(1), xs(1));
    assert_ne!(xs(1), xs(2));
}

#[test]
fn 猫パンチはそばの文字を歩く向きへ飛ばし_数秒後に元へ戻す() {
    let (layout, r) = setup(LONG, 12.0);
    let mut found = false;
    for seed in 0..20 {
        let neko = Neko::new(&layout, mode(8.0, 0.0).punch_chance(0.5).seed(seed));
        let tr = trace(&layout, &r, &neko, 6.0);
        for (t, c, shown, ks) in &tr {
            for k in ks {
                let Flight::Punch(f) = k.flight else { continue };
                found = true;
                // 表示済みの文字だけが飛ぶ
                assert!(k.glyphs.end <= *shown, "t {t}: {k:?}");
                if k.since > 0.1 && k.since < 0.5 {
                    assert!(k.fall.offset.0 * f.sign() > 0.0, "向きへ飛ぶ: {k:?}");
                    assert!(k.fall.rotation != 0.0);
                }
                if k.since > FALL_SECS + 0.05 && k.since < FALL_SECS + RETURN_WAIT {
                    assert!(k.fall.gone && k.fall.alpha == 0.0, "{k:?}");
                }
                if k.since > FALL_SECS + RETURN_WAIT + 0.1 {
                    assert!(k.fall.alpha > 0.0 && k.fall.offset == (0.0, 0.0), "{k:?}");
                }
            }
            let _ = c;
        }
        // 戻り終えたものは一覧から消える
        if let Some((_, _, _, ks)) = tr.last() {
            assert!(ks.iter().all(|k| k.since < FLIGHT_TOTAL));
        }
    }
    assert!(found, "パンチが一度も出ない");
}

#[test]
fn 入力待ちで最後の文字のそばに着いてから押して落とし_数秒後に戻す() {
    let (layout, mut r) = setup("あい[p]う", 20.0);
    let neko = Neko::new(&layout, mode(100.0, 1.0));
    assert_eq!(neko.wait_plan(0, Motion::Full), Some(&WaitPlan::Drop(1..2)));
    r.advance(0.2);
    assert!(r.is_waiting());
    let settle = neko.settle_time(&layout, &r, 0);
    assert!(settle >= r.page_end(0));
    let go = |r: &mut Reveal, u: f32| {
        let now = r.time();
        r.advance(settle + u - now);
    };
    go(&mut r, 0.1);
    assert_eq!(neko.cat(&layout, &r).unwrap().action, CatAction::Knock);
    assert!(neko.knocked(&layout, &r).is_empty());
    go(&mut r, KNOCK_HIT + 0.1);
    let k = neko.knocked(&layout, &r);
    assert_eq!(k.len(), 1);
    assert_eq!((k[0].glyphs.clone(), k[0].flight), (1..2, Flight::Drop));
    // 横へ滑ってから下へ落ちる
    assert!(k[0].fall.offset.0 > 0.0);
    go(&mut r, KNOCK_HIT + 0.6);
    let g = &neko.glyphs(&layout, &r)[1];
    assert!(g.offset.1 > 0.5 && g.rotation > 0.0, "{g:?}");
    go(&mut r, KNOCK_HIT + FALL_SECS + 0.2);
    assert_eq!(neko.glyphs(&layout, &r)[1].alpha, 0.0);
    assert_eq!(neko.cat(&layout, &r).unwrap().action, CatAction::Idle);
    // 数秒後に元の位置へフェードインで戻る
    go(&mut r, KNOCK_HIT + FLIGHT_TOTAL + 0.1);
    assert!(neko.knocked(&layout, &r).is_empty());
    let g = &neko.glyphs(&layout, &r)[1];
    assert_eq!((g.alpha, g.offset, g.rotation), (1.0, (0.0, 0.0), 0.0));
    assert!((neko.quiet_time(&layout, &r, 0) - (settle + KNOCK_HIT + FLIGHT_TOTAL)).abs() < 1e-5);
    // テキストは変わらない
    assert_eq!(layout.pages[0].glyphs[1].ch, 'い');
}

#[test]
fn 動きの強さで猫のちょっかいを抑える() {
    let (layout, r) = setup("あいうえおかきくけこさしすせそ[p]た[p]ち", 20.0);
    let neko = Neko::new(
        &layout,
        NekoMode::default()
            .seed(1)
            .punch_chance(1.0)
            .burst_chance(0.0)
            .knock_chance(1.0),
    );
    let burst = Neko::new(&layout, NekoMode::default().seed(1).burst_chance(1.0));
    let with = |m: Motion| {
        let mut r = r.clone();
        r.set_motion(m);
        r
    };
    // 標準: 猫パンチあり・押して落とす・爆散
    assert!(!neko.punches(&layout, &with(Motion::Full), 0).is_empty());
    assert!(neko.wait_plan(0, Motion::Full).is_some());
    assert_eq!(burst.wait_plan(0, Motion::Full), Some(&WaitPlan::Burst));
    // 控えめ: 猫パンチと爆散はなし、押して落とすは残す
    assert!(neko.punches(&layout, &with(Motion::Reduced), 0).is_empty());
    assert!(neko.wait_plan(0, Motion::Reduced).is_some());
    assert_eq!(burst.wait_plan(0, Motion::Reduced), None);
    // なし: 何もしない
    assert!(neko.punches(&layout, &with(Motion::Off), 0).is_empty());
    assert_eq!(neko.wait_plan(0, Motion::Off), None);
    let mut off = with(Motion::Off);
    off.advance(8.0);
    assert!(neko.knocked(&layout, &off).is_empty());
}

#[test]
fn 爆散ではページの文字がまとめて外へ飛び_数秒後にすべて戻る() {
    let (layout, mut r) = setup("[ruby ねこ]猫[/ruby]がいる。[p]う", 20.0);
    let neko = Neko::new(&layout, mode(100.0, 0.0).burst_chance(1.0));
    assert_eq!(neko.wait_plan(0, Motion::Full), Some(&WaitPlan::Burst));
    r.skip();
    r.advance(KNOCK_HIT + 0.5);
    let ks = neko.knocked(&layout, &r);
    // 猫(ルビごと)・が・い・る・。
    assert_eq!(ks.len(), 5);
    assert_eq!((ks[0].glyphs.clone(), ks[0].ruby), (0..1, Some(0)));
    let gs = neko.glyphs(&layout, &r);
    assert!(gs.iter().all(|g| g.offset != (0.0, 0.0)), "{gs:?}");
    r.advance(FLIGHT_TOTAL);
    assert!(neko.knocked(&layout, &r).is_empty());
    let gs = neko.glyphs(&layout, &r);
    assert!(gs.iter().all(|g| g.alpha == 1.0 && g.offset == (0.0, 0.0)));
}

#[test]
fn 入力待ちのちょっかいはシードとページ番号だけで決まる() {
    let src = "あ[p]い[p]う[p]え[p]お[p]か[p]き[p]く[p]け[p]こ";
    let (layout, _) = setup(src, 20.0);
    let m = NekoMode::default().knock_chance(0.4).burst_chance(0.3);
    let a = Neko::new(&layout, m.clone());
    let b = Neko::new(&layout, m);
    let plans = |n: &Neko| {
        (0..10)
            .map(|p| n.wait_plan(p, Motion::Full).cloned())
            .collect::<Vec<_>>()
    };
    assert_eq!(plans(&a), plans(&b));
    assert!(plans(&a).contains(&Some(WaitPlan::Burst)));
    assert!(plans(&a)
        .iter()
        .any(|p| matches!(p, Some(WaitPlan::Drop(_)))));
    assert!(plans(&a).contains(&None));
    let none = Neko::new(&layout, mode(8.0, 0.0));
    assert!(plans(&none).iter().all(Option::is_none));
}

#[test]
fn 落とす対象は最後に見えている文字でルビ付きなら親文字ごと() {
    let (layout, _) = setup("あ[ruby めいきゅう]迷宮[/ruby] ", 20.0);
    let neko = Neko::new(&layout, mode(8.0, 1.0));
    assert_eq!(neko.wait_plan(0, Motion::Full), Some(&WaitPlan::Drop(1..3)));
    let (layout, _) = setup("ねこ。", 20.0);
    let neko = Neko::new(&layout, mode(8.0, 1.0));
    assert_eq!(neko.wait_plan(0, Motion::Full), Some(&WaitPlan::Drop(2..3)));
}

#[test]
fn 入力待ちになると最後の文字の右端へ寄って落ち着く() {
    for seed in 0..10 {
        let (layout, mut r) = setup("あいうえおかきくけこさしす[p]せ", 6.0);
        let neko = Neko::new(&layout, mode(8.0, 0.0).seed(seed));
        let settle = neko.settle_time(&layout, &r, 0);
        assert!(settle >= r.page_end(0));
        r.advance(settle + 0.01);
        let c = neko.cat(&layout, &r).unwrap();
        assert_eq!(
            (c.x, c.line, c.action),
            (1.0, 2, CatAction::Idle),
            "seed {seed}"
        );
    }
}

#[test]
fn skip_では猫が最後の位置へ即座に移り_ちょっかいの判定は同じ() {
    let (layout, mut r) = setup("あいうえおかきくけこ[p]さ", 20.0);
    let neko = Neko::new(&layout, mode(1.0, 1.0));
    r.advance(0.1);
    r.skip();
    let cat = neko.cat(&layout, &r).unwrap();
    assert!((cat.x - 10.0).abs() < 1e-4, "{cat:?}");
    r.advance(KNOCK_HIT + 0.01);
    assert_eq!(neko.knocked(&layout, &r).len(), 1);
}

#[test]
fn ちょっかいを切るとパンチも落とすも爆散もしない() {
    let (layout, mut r) = setup("あい[p]う", 20.0);
    let neko = Neko::new(
        &layout,
        NekoMode::default()
            .walk(false)
            .knock_chance(1.0)
            .punch_chance(1.0),
    );
    assert!(neko.wait_plan(0, Motion::Full).is_none());
    r.advance(5.0);
    assert!(neko.cat(&layout, &r).is_none());
    assert!(neko.knocked(&layout, &r).is_empty());
}

#[test]
fn 文字送りを保存して戻すと猫と飛んだ文字も同じになる() {
    let (layout, mut r) = setup(LONG, 8.0);
    let neko = Neko::new(&layout, NekoMode::default().punch_chance(0.6).seed(3));
    r.advance(2.3);
    let (_, mut r2) = setup(LONG, 8.0);
    assert!(r2.restore(r.state()));
    assert_eq!(neko.cat(&layout, &r2), neko.cat(&layout, &r));
    assert_eq!(neko.knocked(&layout, &r2), neko.knocked(&layout, &r));
    assert_eq!(neko.glyphs(&layout, &r2), neko.glyphs(&layout, &r));
}

#[test]
fn 猫の状態は時刻だけで決まる() {
    let (layout, mut a) = setup("あいうえおかきくけこ", 4.0);
    let neko = Neko::new(
        &layout,
        NekoMode::default().walk_speed(3.0).punch_chance(0.5),
    );
    let mut b = a.clone();
    // ページの終わり(1.0 秒)ちょうどは足し算の誤差で入力待ちかどうかが分かれるので避ける
    for _ in 0..24 {
        a.advance(1.0 / 30.0);
    }
    b.advance(0.8);
    let (ca, cb) = (
        neko.cat(&layout, &a).unwrap(),
        neko.cat(&layout, &b).unwrap(),
    );
    assert_eq!(ca.line, cb.line);
    assert!((ca.x - cb.x).abs() < 1e-3, "{ca:?} {cb:?}");
    assert_eq!(
        neko.knocked(&layout, &a).len(),
        neko.knocked(&layout, &b).len()
    );
}

#[test]
fn 猫モードの描画命令も使い回すバッファに同じものを書く() {
    let (layout, mut r) = setup("あいうえお", 20.0);
    let neko = Neko::new(&layout, mode(40.0, 1.0));
    // 前のフレームの中身(全部の文字)が残っていても消える
    let mut all = r.clone();
    all.skip();
    let mut buf = all.glyphs(&layout);
    let mut moved = false;
    for _ in 0..80 {
        r.advance(0.05);
        neko.glyphs_into(&layout, &r, &mut buf);
        assert_eq!(buf, neko.glyphs(&layout, &r));
        moved |= !neko.knocked(&layout, &r).is_empty()
            && buf.iter().any(|g| g.offset != (0.0, 0.0) || g.alpha < 1.0);
    }
    // 押して落とした文字の動きも入っている
    assert!(moved);
}

#[test]
fn とても遅い文字送りをskipしても猫の計算は止まらずに落ち着く() {
    // 時刻が大きすぎると動作を積んでも時刻が進まなくなる(以前は戻らなかった)
    let script = parse("あいう").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let mut r = Reveal::new(&layout, RevealOptions::default().chars_per_sec(1e-30));
    r.skip();
    let neko = Neko::new(&layout, NekoMode::default().seed(3));
    let cat = neko.cat(&layout, &r).expect("猫はいる");
    assert_eq!(cat.action, CatAction::Idle);
    let _ = neko.glyphs(&layout, &r);
}

#[test]
fn 長い待機を重ねたページでも猫の計算は決まった回数で終わる() {
    let src = format!("あ{}い", "[wait 3600]".repeat(200));
    let (layout, mut r) = setup(&src, 20.0);
    r.skip();
    let neko = Neko::new(&layout, NekoMode::default().seed(5));
    let cat = neko.cat(&layout, &r).expect("猫はいる");
    assert_eq!(cat.action, CatAction::Idle);
    let _ = neko.glyphs(&layout, &r);
}
