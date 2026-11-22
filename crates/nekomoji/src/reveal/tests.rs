use super::effect::{GLOW_RISE, HOP_SECS};
use super::options::STEADY_GLOW;
use super::*;
use crate::{parse, CellMetrics, Layout, LayoutOptions, Outline, Shadow, TextEvent, Vars};

fn reveal(src: &str) -> (Layout, Reveal) {
    let script = parse(src).unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let opts = RevealOptions {
        chars_per_sec: 10.0,
        comma_pause: 0.0,
        period_pause: 0.0,
        motion: Motion::Full,
        blip: None,
        auto: AutoWait::default(),
    };
    let r = Reveal::new(&layout, opts);
    (layout, r)
}

/// 文字送りの音の時間割: (時刻, 音の ID)
fn blips(src: &str, blip: Option<Blip>) -> Vec<(f32, String)> {
    let script = parse(src).unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let opts = RevealOptions::default()
        .chars_per_sec(10.0)
        .comma_pause(0.0)
        .period_pause(0.0)
        .blip(blip);
    let r = Reveal::new(&layout, opts);
    r.timeline(0)
        .iter()
        .filter_map(|s| match &s.kind {
            StepKind::Emit(TextEvent::Blip(id)) => Some(((s.at * 10.0).round() / 10.0, id.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn 文字送りの音は既定では鳴らさない() {
    assert!(blips("あいうえお", None).is_empty());
}

#[test]
fn 文字送りの音は字の何文字かごとに鳴り_空白と記号では鳴らさない() {
    let ids = |v: Vec<(f32, String)>| v.into_iter().map(|(t, _)| t).collect::<Vec<_>>();
    // 2文字ごと: あ・う・お
    assert_eq!(
        ids(blips("あいうえお", Some(Blip::default()))),
        [0.0, 0.2, 0.4]
    );
    // 記号・空白は数えない(あ・い・う → あ と う)
    assert_eq!(
        ids(blips("あ、　い！う", Some(Blip::default().every(2)))),
        [0.0, 0.5]
    );
    assert_eq!(
        blips("ab", Some(Blip::default().every(1).id("pi"))),
        [(0.0, "pi".to_string()), (0.1, "pi".to_string())]
    );
}

#[test]
fn 範囲タグで音を変え_止められ_設定が無くても範囲では鳴る() {
    let v = blips("[blip mike]あいう[/blip][blip]えお[/blip]か", None);
    assert_eq!(
        v,
        [
            (0.0, "mike".to_string()),
            (0.2, "mike".to_string()),
            (0.3, "blip".to_string())
        ],
        "音が変わったら数え直す。範囲の外は鳴らさない"
    );
    let v = blips(
        "あい[blip off]うえ[/blip]お",
        Some(Blip::default().every(1)),
    );
    let ts: Vec<f32> = v.iter().map(|(t, _)| *t).collect();
    assert_eq!(ts, [0.0, 0.1, 0.4]);
}

#[test]
fn 音と音の間が短すぎれば鳴らさない() {
    let v = blips(
        "あいうえ",
        Some(Blip::default().every(1).min_interval(0.15)),
    );
    let ts: Vec<f32> = v.iter().map(|(t, _)| *t).collect();
    assert_eq!(ts, [0.0, 0.2]);
}

#[test]
fn 一度に進めても音は1つだけで_skip_では鳴らさない() {
    let script = parse("[blip]あいうえおかきくけこ[/blip][voice v1]さ").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let opts = RevealOptions::default().chars_per_sec(10.0);
    let mut r = Reveal::new(&layout, opts.clone().blip(Some(Blip::default().every(1))));
    let ev = r.advance(0.55);
    let n = ev
        .iter()
        .filter(|e| matches!(e, TextEvent::Blip(_)))
        .count();
    assert_eq!(n, 1, "{ev:?}");
    let ev = r.skip();
    assert!(
        !ev.iter().any(|e| matches!(e, TextEvent::Blip(_))),
        "{ev:?}"
    );
    assert!(
        ev.contains(&TextEvent::Voice("v1".into())),
        "ほかの出来事は返す"
    );
}

fn auto_reveal(src: &str, auto: AutoWait) -> Reveal {
    let script = parse(src).unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    Reveal::new(
        &layout,
        RevealOptions::default()
            .chars_per_sec(10.0)
            .comma_pause(0.0)
            .period_pause(0.0)
            .auto(auto),
    )
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn オートの待ち時間は基本の秒数に文字数の分を足し_上限で止める() {
    let r = auto_reveal("あいう　え[p]あ", AutoWait::default());
    let none = |_: &str| None;
    // 空白を除いて 4 文字
    assert!(near(r.auto_wait(0, none), 1.0 + 0.05 * 4.0));
    assert!(near(r.auto_wait(1, none), 1.05));
    let long = "あ".repeat(500);
    let r = auto_reveal(&long, AutoWait::default());
    assert!(near(r.auto_wait(0, none), 8.0));
}

#[test]
fn ボイスの長さが分かれば鳴り終わってから待つ() {
    // ボイスは 0 秒に鳴り、ページの最後は 0.3 秒
    let r = auto_reveal("[voice v1]あいう", AutoWait::default());
    let len = |id: &str| (id == "v1").then_some(3.0);
    // 3.0 - 0.3 + 0.5 = 3.2(読む時間 1.15 より長い)
    assert!(near(r.auto_wait(0, len), 3.2), "{}", r.auto_wait(0, len));
    // 短いボイスなら読む時間
    assert!(near(r.auto_wait(0, |_: &str| Some(0.1)), 1.15));
    // 長さの分からないボイスは数えない
    assert!(near(r.auto_wait(0, |_: &str| None), 1.15));
}

#[test]
fn オートで進めてよいのは入力待ちから待ち時間がたってから() {
    let mut r = auto_reveal("あいう[p]え", AutoWait::default().base(0.5).per_char(0.0));
    let none = |_: &str| None;
    r.advance(0.2);
    assert!(!r.auto_due(none), "文字送り中");
    r.advance(0.2);
    assert!(r.is_waiting());
    assert!(!r.auto_due(none), "入力待ちになったばかり");
    r.advance(0.5);
    assert!(r.auto_due(none));
    assert!(r.proceed());
    r.skip();
    r.advance(10.0);
    assert!(!r.auto_due(none), "最後のページ(終了)では進めない");
}

#[test]
fn 時間に応じて文字が増える() {
    let (_, mut r) = reveal("あいうえお");
    r.advance(0.0);
    assert_eq!(r.visible_count(), 1);
    r.advance(0.25);
    assert_eq!(r.visible_count(), 3);
}

#[test]
fn 待機は時間割を後ろへずらす() {
    let (_, r) = reveal("あ[wait 1]い");
    let at: Vec<f32> = r
        .timeline(0)
        .iter()
        .filter(|s| matches!(s.kind, StepKind::Show(_)))
        .map(|s| s.at)
        .collect();
    assert_eq!(at, vec![0.0, 1.1]);
}

#[test]
fn 出来事は位置に来たときに一度だけ返る() {
    let (_, mut r) = reveal("あい[voice v1]う");
    assert!(r.advance(0.15).is_empty());
    assert_eq!(r.advance(0.1), vec![TextEvent::Voice("v1".into())]);
    assert!(r.advance(0.0).is_empty());
}

#[test]
fn 改ページで入力を待ち_proceed_で次へ() {
    let (_, mut r) = reveal("あ[p]い");
    let ev = r.advance(1.0);
    assert_eq!(ev, vec![TextEvent::WaitInput]);
    assert!(r.is_waiting());
    assert!(r.advance(1.0).is_empty());
    assert!(r.proceed());
    assert_eq!((r.page(), r.visible_count()), (1, 0));
    assert_eq!(r.advance(1.0), vec![TextEvent::Finished]);
    assert!(r.is_finished());
}

#[test]
fn skip_はページの残りを出し切る() {
    let (_, mut r) = reveal("あいう[se s1]えお[p]か");
    let ev = r.skip();
    assert_eq!(ev, vec![TextEvent::Se("s1".into()), TextEvent::WaitInput]);
    assert_eq!(r.visible_count(), 5);
}

#[test]
fn 句読点の後に間が入る() {
    let script = parse("あ、い").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let r = Reveal::new(&layout, RevealOptions::default());
    let shows: Vec<f32> = r
        .timeline(0)
        .iter()
        .filter(|s| matches!(s.kind, StepKind::Show(_)))
        .map(|s| s.at)
        .collect();
    assert!((shows[2] - (0.05 + 0.05 + 0.1)).abs() < 1e-5);
}

#[test]
fn エフェクトは同じ時刻なら同じ結果() {
    let (layout, mut r) = reveal("[shake 2]あい[/shake]");
    r.advance(1.0);
    let a = r.glyphs(&layout);
    let b = r.clone().glyphs(&layout);
    assert_eq!(a, b);
    assert_ne!(a[0].offset, (0.0, 0.0));
}

#[test]
fn 秒数を付けた揺れと波はだんだん弱まり_その秒数で止まる() {
    let (layout, mut r) = reveal("[shake 4 1]あ[/shake][wave 4 1]い[/wave][shake 4]う[/shake]");
    r.skip();
    let at = |r: &Reveal, t: f32| {
        let mut r = r.clone();
        r.advance(t);
        r.glyphs(&layout)
    };
    // 出た直後は揺れ、半分の時間で振れ幅は 1/4 以下、秒数を過ぎると止まる
    let mut max_early = 0.0f32;
    let mut max_mid = 0.0f32;
    for k in 0..30 {
        let g = at(&r, 0.01 + k as f32 * 0.004);
        max_early = max_early.max(g[0].offset.0.abs()).max(g[0].offset.1.abs());
        let g = at(&r, 0.5 + k as f32 * 0.004);
        max_mid = max_mid.max(g[0].offset.0.abs()).max(g[0].offset.1.abs());
    }
    assert!(max_early > 0.0);
    assert!(max_mid <= 4.0 * 0.05 * 0.25 + 1e-5);
    let late = at(&r, 1.2);
    assert_eq!(late[0].offset, (0.0, 0.0));
    assert_eq!(late[1].offset, (0.0, 0.0));
    // 秒数がなければ揺れ続ける
    let far: Vec<_> = (0..10)
        .map(|k| at(&r, 5.0 + k as f32 * 0.04)[2].offset)
        .collect();
    assert!(far.iter().any(|o| *o != (0.0, 0.0)));
}

#[test]
fn 縁取りと影は描画命令に載り_動きなしでも消えない() {
    let (layout, mut r) = reveal(
        "[outline white 0.1][shadow]あ[/shadow]い[/outline]う[fade 1][shadow]え[/shadow][/fade]",
    );
    r.set_motion(Motion::Off);
    r.advance(0.35);
    let g = r.glyphs(&layout);
    assert_eq!(
        g[0].outline,
        Some(Outline::default().color("white").width(0.1))
    );
    assert_eq!(g[0].shadow, Some(Shadow::default()));
    assert_eq!(g[1].outline, g[0].outline);
    assert_eq!(g[1].shadow, None);
    assert_eq!((g[2].outline.clone(), g[2].shadow.clone()), (None, None));
    // 不透明度は文字に従う(描画側が alpha を掛ける)
    assert!(g[3].shadow.is_some() && g[3].alpha < 0.1);
}

#[test]
fn 発光_in_は出たときにふわっと光って元へ戻る() {
    // 「あ」は 0 秒、「い」は 0.1 秒に出る
    let (layout, r) = reveal("[glow in 1 1]あ[/glow]い");
    let at = |r: &Reveal, t: f32| {
        let mut r = r.clone();
        r.advance(t);
        r.glyphs(&layout)
    };
    assert!(at(&r, 0.0)[0].glow < 0.05);
    assert!(at(&r, GLOW_RISE)[0].glow > 0.99);
    let mid = at(&r, GLOW_RISE + 0.5)[0].glow;
    assert!(mid > 0.1 && mid < 0.5, "{mid}");
    assert_eq!(at(&r, GLOW_RISE + 1.1)[0].glow, 0.0);
    // 囲んでいない文字は光らない
    assert_eq!(at(&r, GLOW_RISE)[1].glow, 0.0);
}

#[test]
fn 発光_breathe_は明滅を繰り返し_flicker_は範囲ごとに揃ってときどき瞬く() {
    let (layout, r) = reveal("[glow breathe 2 1]あ[/glow][glow flicker 1 0.5]いう[/glow]");
    let mut b = Vec::new();
    let mut dim = 0;
    for k in 0..200 {
        let mut r = r.clone();
        r.advance(k as f32 * 0.02);
        let g = r.glyphs(&layout);
        b.push(g[0].glow);
        if k <= 20 {
            continue; // 「う」が出て光り始めるまで(0.2 秒 + 0.15 秒)
        }
        // 同じ範囲の文字は同じ明るさで瞬く
        assert_eq!((g[1].glow, g[1].alpha), (g[2].glow, g[2].alpha));
        if g[1].alpha < 1.0 {
            dim += 1;
        }
    }
    let (lo, hi) = (
        b.iter().cloned().fold(f32::INFINITY, f32::min),
        b.iter().cloned().fold(0.0, f32::max),
    );
    assert!(lo < 0.6 && hi > 1.8, "{lo} {hi}");
    assert!(dim > 0 && dim < 60, "{dim}");
}

/// `src` を `motion` で文字送りし、0.02 秒ごとのグリフを返す。
fn frames(src: &str, motion: Motion, secs: f32) -> Vec<Vec<GlyphInstance>> {
    let (layout, mut r) = reveal(src);
    r.set_motion(motion);
    (0..(secs / 0.02) as usize)
        .map(|_| {
            r.advance(0.02);
            r.glyphs(&layout)
        })
        .collect()
}

#[test]
fn 動きなしでは揺れ_波_猫の跳ね方が動かず_発光は一定の明るさになる() {
    let src = "[shake 3]あ[/shake][wave 3]い[/wave][neko hop 2]う[/neko][glow flicker 1 0.3]え[/glow][glow breathe]お[/glow]";
    let fs = frames(src, Motion::Off, 4.0);
    for g in fs.iter().flatten() {
        assert_eq!((g.offset, g.rotation), ((0.0, 0.0), 0.0), "{g:?}");
        assert!(g.alpha == 1.0 || g.ch == 'う', "瞬きで暗くしない: {g:?}");
    }
    // 光り始めた後は一定
    let late: Vec<(f32, f32)> = fs[100..].iter().map(|g| (g[3].glow, g[4].glow)).collect();
    assert!(
        late.iter().all(|&v| v == (STEADY_GLOW, STEADY_GLOW)),
        "{:?}",
        &late[..3]
    );
}

#[test]
fn 控えめでは揺れが弱く_1秒以内に止まり_跳ね方は回さない() {
    let max_off = |fs: &[Vec<GlyphInstance>], k: usize, from: usize, to: usize| {
        fs[from..to]
            .iter()
            .map(|g| g[k].offset.0.abs().max(g[k].offset.1.abs()))
            .fold(0.0f32, f32::max)
    };
    let src = "[shake 3]あ[/shake][neko hop 2]い[/neko]";
    let full = frames(src, Motion::Full, 3.0);
    let reduced = frames(src, Motion::Reduced, 3.0);
    assert!(max_off(&reduced, 0, 0, 40) < max_off(&full, 0, 0, 40) * 0.5);
    // 文字が出てから 1 秒を過ぎたら止まっている
    assert_eq!(max_off(&reduced, 0, 60, 150), 0.0);
    assert!(max_off(&full, 0, 60, 150) > 0.0, "標準では揺れ続ける");
    assert!(reduced
        .iter()
        .all(|g| g.get(1).map_or(true, |g| g.rotation == 0.0)));
    assert!(full
        .iter()
        .any(|g| g.get(1).is_some_and(|g| g.rotation != 0.0)));
}

#[test]
fn 発光_flicker_は標準でも1秒に3度を超えて瞬かず_控えめでは1度までで文字を暗くしない() {
    // 平均 0.05 秒に1度という無理な指定でも上限で抑える
    let src = "[glow flicker 1 0.05]あ[/glow]";
    let count = |fs: &[Vec<GlyphInstance>], dim: &dyn Fn(&GlyphInstance) -> bool| {
        // どの 1 秒(50 コマ)を取っても、瞬きの始まりを数えた最大
        let flags: Vec<bool> = fs.iter().map(|g| dim(&g[0])).collect();
        let starts: Vec<bool> = (0..flags.len())
            .map(|i| flags[i] && (i == 0 || !flags[i - 1]))
            .collect();
        starts
            .windows(50)
            .map(|w| w.iter().filter(|&&b| b).count())
            .max()
            .unwrap_or(0)
    };
    let full = frames(src, Motion::Full, 10.0);
    let n = count(&full, &|g| g.alpha < 1.0);
    assert!((1..=3).contains(&n), "{n}");
    let reduced = frames(src, Motion::Reduced, 10.0);
    assert!(reduced.iter().all(|g| g[0].alpha == 1.0));
    let n = count(&reduced[10..], &|g| g.glow < 0.6);
    assert!((1..=1).contains(&n), "{n}");
}

#[test]
fn 名前付きの範囲は発光_flicker_の揃いを崩さない() {
    let (layout, r) = reveal("[glow flicker 1 0.5]い[mark a]う[/mark]え[/glow]");
    for k in 21..200 {
        let mut r = r.clone();
        r.advance(k as f32 * 0.02);
        let g = r.glyphs(&layout);
        assert_eq!((g[0].glow, g[0].alpha), (g[1].glow, g[1].alpha));
        assert_eq!((g[1].glow, g[1].alpha), (g[2].glow, g[2].alpha));
    }
}

#[test]
fn 入力待ちではカーソルの位置を最後の文字の右端に出す() {
    let (layout, mut r) = reveal("あい[p]う");
    r.advance(0.05);
    assert!(r.cursor(&layout).is_none());
    r.advance(1.0);
    let c = r.cursor(&layout).unwrap();
    assert_eq!((c.x, c.line), (2.0, 0));
    assert!(c.since > 0.5);
}

#[test]
fn 入力待ちの間も時刻は進むが文字と出来事は増えない() {
    let (_, mut r) = reveal("あ[p]い");
    r.advance(1.0);
    assert!(r.is_waiting());
    let t = r.time();
    assert!(r.advance(0.5).is_empty());
    assert!((r.time() - (t + 0.5)).abs() < 1e-5);
    assert_eq!(r.visible_count(), 1);
}

#[test]
fn 猫の跳ね方は現れた直後に上へずれ_着地すると元に戻る() {
    let (layout, mut r) = reveal("[neko hop]あ[/neko]");
    r.advance(HOP_SECS / 2.0);
    let g = &r.glyphs(&layout)[0];
    assert!(g.offset.1 < -0.5, "{g:?}");
    assert!(g.rotation < 0.0);
    r.advance(1.0);
    let g = &r.glyphs(&layout)[0];
    assert_eq!((g.offset, g.rotation, g.alpha), ((0.0, 0.0), 0.0, 1.0));
}

#[test]
fn skip_したことを覚え_次のページで戻る() {
    let (_, mut r) = reveal("あいう[p]え");
    r.skip();
    assert!(r.skipped());
    r.proceed();
    assert!(!r.skipped());
}

#[test]
fn 線は表示済みの文字の分だけ伸びる() {
    let (layout, mut r) = reveal("[s]あいう[/s]");
    r.advance(0.15); // 10 文字/秒なので 2文字
    let d = r.decorations(&layout);
    assert_eq!(d.len(), 1);
    assert_eq!((d[0].x0, d[0].x1), (0.0, 2.0));
    r.skip();
    assert_eq!(r.decorations(&layout)[0].x1, 3.0);
}

#[test]
fn 保存した状態に戻すと表示数も入力待ちもエフェクトも同じになる() {
    let (layout, mut r) = reveal("[wave]あいう[/wave][p]え[voice v]お");
    r.advance(0.15);
    let saved = r.state();
    let glyphs = r.glyphs(&layout);
    let (_, mut r2) = reveal("[wave]あいう[/wave][p]え[voice v]お");
    assert!(r2.restore(saved));
    assert_eq!(r2.visible_count(), 2);
    assert_eq!(r2.glyphs(&layout), glyphs);
    // 入力待ちの状態
    r.advance(1.0);
    assert!(r.is_waiting());
    assert!(r2.restore(r.state()));
    assert!(r2.is_waiting());
    // 次のページの途中。戻すまでの出来事は返さず、その後は続きから返る
    r.proceed();
    r.advance(0.05);
    let (_, mut r3) = reveal("[wave]あいう[/wave][p]え[voice v]お");
    assert!(r3.restore(r.state()));
    assert_eq!((r3.page(), r3.visible_count()), (1, 1));
    assert_eq!(r3.advance(0.1), vec![TextEvent::Voice("v".into())]);
}

#[test]
fn skip_した状態も戻せる() {
    let (_, mut r) = reveal("あいうえお[p]か");
    r.skip();
    let (_, mut r2) = reveal("あいうえお[p]か");
    assert!(r2.restore(r.state()));
    assert!(r2.skipped() && r2.is_waiting());
    assert_eq!(r2.visible_count(), 5);
}

#[test]
fn 無いページや負の時刻には戻さない() {
    let (_, mut r) = reveal("あ");
    assert!(!r.restore(RevealState::new(5, 0.0, false)));
    assert!(!r.restore(RevealState::new(0, -1.0, false)));
    assert!(!r.restore(RevealState::new(0, 1.0, false).clicks(vec![f32::NAN])));
}

#[test]
fn l_で同じページのまま入力を待ち_進むと続きが待った分だけ遅れて出る() {
    let (layout, mut r) = reveal("あい[l]うえ");
    assert_eq!(layout.pages.len(), 1);
    let ev = r.advance(0.25);
    assert_eq!(ev, vec![TextEvent::WaitClick]);
    assert_eq!(r.visible_count(), 2);
    assert_eq!(r.wait_kind(), Some(WaitKind::Click));
    let c = r.cursor(&layout).unwrap();
    assert_eq!((c.x, c.line, c.kind), (2.0, 0, WaitKind::Click));
    // 待っている間は続きが出ず、続きの表示時刻とページの終わりも後ろへずれる
    assert!(r.advance(3.0).is_empty());
    assert_eq!(r.visible_count(), 2);
    assert!(r.page_end(0) > r.time());
    assert!(r.appear_times(0)[2] > r.time());
    assert!(r.proceed());
    assert_eq!(r.page(), 0);
    assert_eq!(r.wait_kind(), None);
    r.advance(0.0);
    assert_eq!(r.visible_count(), 3);
    assert!((r.appear_times(0)[2] - 3.25).abs() < 1e-4);
    assert_eq!(r.advance(0.11), vec![]);
    assert_eq!(r.advance(0.11), vec![TextEvent::Finished]);
    assert!((r.page_end(0) - 3.45).abs() < 1e-4);
    assert_eq!(r.wait_kind(), Some(WaitKind::End));
}

#[test]
fn skip_は次の_l_の手前まで出す() {
    let (_, mut r) = reveal("あい[l]うえ[l]お[p]か");
    assert_eq!(r.skip(), vec![TextEvent::WaitClick]);
    assert_eq!(r.visible_count(), 2);
    assert!(r.skip().is_empty());
    assert!(r.proceed());
    assert_eq!(r.skip(), vec![TextEvent::WaitClick]);
    assert_eq!(r.visible_count(), 4);
    assert!(r.proceed());
    assert_eq!(r.skip(), vec![TextEvent::WaitInput]);
    assert_eq!(r.wait_kind(), Some(WaitKind::Page));
    assert!(r.proceed());
    assert_eq!(r.page(), 1);
}

#[test]
fn ページの終わりの_l_は待たない() {
    let (_, r) = reveal("あ[l][p]い[l]");
    assert!(r
        .timeline(0)
        .iter()
        .all(|s| s.kind != StepKind::Emit(TextEvent::WaitClick)));
    assert!(r
        .timeline(1)
        .iter()
        .all(|s| s.kind != StepKind::Emit(TextEvent::WaitClick)));
}

#[test]
fn l_から進んだ時刻を保存して戻せる() {
    let (_, mut r) = reveal("あい[l]うえ[l]お");
    r.advance(0.2);
    r.advance(1.0);
    assert!(r.proceed());
    r.advance(0.5);
    let st = r.state();
    assert_eq!(st.clicks.len(), 1);
    assert_eq!(r.wait_kind(), Some(WaitKind::Click));
    let (_, mut r2) = reveal("あい[l]うえ[l]お");
    assert!(r2.restore(st.clone()));
    assert_eq!(r2.visible_count(), 4);
    assert_eq!(r2.wait_kind(), Some(WaitKind::Click));
    assert_eq!(r2.appear_times(0), r.appear_times(0));
    // 文字列でも同じ
    let (_, mut r3) = reveal("あい[l]うえ[l]お");
    assert!(r3.restore(st.to_string().parse().unwrap()));
    assert_eq!(r3.visible_count(), 4);
}

#[test]
fn オートモードは_l_でも区切りの文字数で待ち時間を決める() {
    let (_, mut r) = reveal("あいうえ[l]お[voice v]か");
    let none = |_: &str| None;
    assert!((r.auto_wait_click(0, 0, none).unwrap() - 1.2).abs() < 1e-4);
    assert!((r.auto_wait(0, none) - 1.1).abs() < 1e-4);
    assert_eq!(r.auto_wait_click(0, 1, none), None);
    // ボイスは後ろの区切りに数える
    let two = |_: &str| Some(2.0);
    assert!((r.auto_wait(0, two) - 2.4).abs() < 1e-4);
    r.advance(0.4);
    assert_eq!(r.wait_kind(), Some(WaitKind::Click));
    r.advance(1.1);
    assert!(!r.auto_due(none));
    r.advance(0.2);
    assert!(r.auto_due(none));
    assert!(r.proceed());
    assert!(!r.auto_due(none));
}

#[test]
fn 状態は文字列にして読み戻せる() {
    let s = RevealState::new(3, 0.1, true);
    assert_eq!(s.to_string(), "p3 t0.1 skip");
    let c = RevealState::new(0, 2.0, true).clicks(vec![0.5]);
    assert_eq!(c.to_string(), "p0 t2 l0.5 skip");
    assert_eq!("p0 t2 l0.5 skip".parse::<RevealState>(), Ok(c));
    assert!("p0 t2 l".parse::<RevealState>().is_err());
    assert!("p0 t2 l1,-1".parse::<RevealState>().is_err());
    assert_eq!("p3 t0.1 skip".parse::<RevealState>(), Ok(s));
    assert!("p3".parse::<RevealState>().is_err());
    assert!("p3 t-1".parse::<RevealState>().is_err());
    assert!("p3 t1 x".parse::<RevealState>().is_err());
}

#[test]
fn 上付きはベースラインの基準と小さい大きさを返す() {
    let (layout, mut r) = reveal("[size 2]x[sup]2[/sup][/size]");
    r.skip();
    let g = r.glyphs(&layout);
    assert_eq!((g[0].size, g[0].baseline_size, g[0].rise), (2.0, 2.0, 0.0));
    assert!((g[1].size - 1.2).abs() < 1e-6);
    assert_eq!(g[1].baseline_size, 2.0);
    assert!((g[1].rise - 0.8).abs() < 1e-6);
}

#[test]
fn 発光_flicker_の瞬きの間のゆらぎは段差なくなめらかに変わる() {
    // 瞬かないほど長い間隔にして、ゆらぎだけを見る(1 コマは 0.02 秒)
    let src = "[glow flicker 1 100000]あ[/glow]";
    let step = |motion: Motion| {
        let fs = frames(src, motion, 10.0);
        fs[10..]
            .windows(2)
            .map(|w| (w[1][0].glow - w[0][0].glow).abs())
            .fold(0.0, f32::max)
    };
    assert!(step(Motion::Full) < 0.04, "{}", step(Motion::Full));
    assert!(step(Motion::Reduced) < 0.01, "{}", step(Motion::Reduced));
}

#[test]
fn 負やnanの経過時間では時刻を動かさない() {
    let (_, mut r) = reveal("あいう");
    r.advance(0.15);
    r.advance(f32::NAN);
    r.advance(-5.0);
    r.advance(f32::INFINITY);
    assert_eq!(r.time(), 0.15);
    assert_eq!(r.visible_count(), 2);
}

#[test]
fn 一秒あたりの文字数が0以下やnanなら待たずに表示する() {
    let script = parse("あいう").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    for cps in [0.0, -1.0, f32::NAN] {
        let mut r = Reveal::new(&layout, RevealOptions::default().chars_per_sec(cps));
        r.advance(0.001);
        assert_eq!(r.visible_count(), 3, "{cps}");
        r.skip();
        assert!(r.time().is_finite());
    }
}

#[test]
fn 数の中の読点では間を置かない() {
    let script = parse("1,000、あ").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let opts = RevealOptions::default()
        .chars_per_sec(10.0)
        .comma_pause(1.0)
        .period_pause(0.0);
    let r = Reveal::new(&layout, opts);
    let t = r.appear_times(0);
    assert!((t[2] - t[1] - 0.1).abs() < 1e-4, "1,0 の間");
    assert!((t[6] - t[5] - 1.1).abs() < 1e-4, "、の後");
}

#[test]
fn 句読点の間は続く句読点と閉じ括弧の最後の文字の後に1度だけ置く() {
    let script = parse("「あ。」い！？う、").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let opts = RevealOptions::default()
        .chars_per_sec(10.0)
        .comma_pause(1.0)
        .period_pause(2.0);
    let t = Reveal::new(&layout, opts).appear_times(0);
    let gap = |k: usize| t[k + 1] - t[k];
    assert!((gap(2) - 0.1).abs() < 1e-4, "。と」の間は空けない");
    assert!((gap(3) - 2.1).abs() < 1e-4, "」の後に句点の間");
    assert!((gap(5) - 0.1).abs() < 1e-4, "！と？の間は空けない");
    assert!((gap(6) - 2.1).abs() < 1e-4, "？の後に1度だけ");
}

#[test]
fn 三点リーダーの後には読点と同じ間を続いた最後に1度だけ置く() {
    let script = parse("そうか……い……」う……。え").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let opts = RevealOptions::default()
        .chars_per_sec(10.0)
        .comma_pause(1.0)
        .period_pause(2.0);
    let t = Reveal::new(&layout, opts).appear_times(0);
    let gap = |k: usize| t[k + 1] - t[k];
    assert!((gap(3) - 0.1).abs() < 1e-4, "…と…の間は空けない");
    assert!((gap(4) - 1.1).abs() < 1e-4, "……の後に読点の間");
    assert!((gap(7) - 0.1).abs() < 1e-4, "……と」の間は空けない");
    assert!((gap(8) - 1.1).abs() < 1e-4, "」の後に読点の間");
    assert!((gap(12) - 2.1).abs() < 1e-4, "……。は長いほうの間を1度");
}

#[test]
fn 句読点の間は合図と段落の区切りを越えて後ろへ送らない() {
    let script = parse("あ。[l]……い！\n……う").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let opts = RevealOptions::default()
        .chars_per_sec(10.0)
        .comma_pause(1.0)
        .period_pause(2.0);
    let r = Reveal::new(&layout, opts);
    let t = r.timeline(0);
    let at = |k: usize| {
        t.iter()
            .find(|s| matches!(s.kind, StepKind::Show(g) if g == k))
            .unwrap()
            .at
    };
    // 。の間はクリック待ちの前に置く(…… の後ろへ送らない)
    let click = t
        .iter()
        .find(|s| matches!(s.kind, StepKind::Emit(TextEvent::WaitClick)))
        .unwrap();
    assert!((click.at - at(1) - 2.1).abs() < 1e-4);
    // ！の間は段落の区切りの前に置く
    assert!((at(6) - at(5) - 2.1).abs() < 1e-4);
}

#[test]
fn 使い回すバッファへの描画命令は新しく作るものと同じで_前の中身を残さない() {
    let (layout, mut r) =
        reveal("[shake 2]あい[/shake][wave]うえ[/wave][glow flicker]お[/glow][p]か");
    // 前のフレームの中身(多め)が残っていても消える
    let mut buf = Vec::new();
    r.skip();
    r.glyphs_into(&layout, &mut buf);
    assert_eq!(buf.len(), 5);
    let cap = buf.capacity();
    for _ in 0..20 {
        r.advance(0.05);
        r.glyphs_into(&layout, &mut buf);
        assert_eq!(buf, r.glyphs(&layout));
    }
    // 次のページ(文字が少ない)へ進んでも、前のページの文字を残さず、領域は使い回す
    r.proceed();
    r.advance(1.0);
    r.glyphs_into(&layout, &mut buf);
    assert_eq!(buf, r.glyphs(&layout));
    assert_eq!(buf.len(), 1);
    assert_eq!(buf.capacity(), cap);
}

#[test]
fn 線の使い回すバッファは新しく作るものと同じで_不透明度は表示済みの最後の文字に合わせる() {
    let (layout, mut r) = reveal(
        "[fade 0.5][s]あい[/s][/fade][glow flicker][u]う[mark m]え[/mark][/u][/glow][p][s]か[/s]",
    );
    let mut buf = Vec::new();
    for _ in 0..30 {
        r.advance(0.05);
        r.decorations_into(&layout, &mut buf);
        assert_eq!(buf, r.decorations(&layout));
        // 各線の不透明度は、線の最後の表示済みの文字の不透明度と同じ
        let glyphs = r.glyphs(&layout);
        let page = &layout.pages[r.page()];
        for d in &buf {
            let last = page
                .glyphs
                .iter()
                .enumerate()
                .take(glyphs.len())
                .rfind(|(_, g)| g.line == d.line && g.x + g.width == d.x1)
                .map(|(i, _)| i)
                .unwrap();
            assert_eq!(d.alpha, glyphs[last].alpha);
        }
    }
    assert!(!buf.is_empty());
    let cap = buf.capacity();
    // 次のページへ進んでも、前のページの線を残さず、領域は使い回す
    r.proceed();
    r.advance(1.0);
    r.decorations_into(&layout, &mut buf);
    assert_eq!(buf, r.decorations(&layout));
    assert_eq!(buf.len(), 1);
    assert_eq!(buf.capacity(), cap);
}
