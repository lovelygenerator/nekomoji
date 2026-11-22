//! 文字送りの時間割と出来事を、時刻順の文字列にする(`nekomoji log`)。

use std::fmt::Write as _;

use nekomoji::neko::{Facing, WaitPlan};
use nekomoji::{Glyph, Layout, Reveal, StepKind, Writing};

use super::cells::{str_cells, to_col};
use super::horizontal::page_rows;
use super::settings::Settings;

/// レイアウト全体の確認用ログ(ページ・行・ルビ・時間割・名前付きの範囲)。
///
/// 猫モードが有効なら、ページごとに猫パンチと、入力待ちで猫が着く時刻・ちょっかいを足す(本文は変えない)。
pub fn log(layout: &Layout, settings: &Settings) -> String {
    let mut out = String::new();
    let reveal = Reveal::new(layout, settings.reveal.clone());
    let neko = settings.neko(layout);
    let pages = layout.pages.len();
    for (pi, page) in layout.pages.iter().enumerate() {
        let _ = writeln!(out, "[ページ {}/{}]", pi + 1, pages);
        let rows = page_rows(layout, pi, page.glyphs.len());
        if layout.writing == Writing::Vertical {
            // 行の番号の下と、行の長さ(max_width)の位置に罫線を引く。ぶら下げはその下に出る
            let w = rows.iter().map(|r| str_cells(r)).max().unwrap_or(0);
            let ruler = format!(" {}", "-".repeat(w.saturating_sub(1)));
            let limit = (settings.layout.max_width - 1e-4).ceil().max(0.0) as usize;
            for (i, row) in rows.iter().enumerate() {
                let _ = writeln!(out, "{row}");
                if i == 0 || i == limit {
                    let _ = writeln!(out, "{ruler}");
                }
            }
            if rows.len() <= limit {
                let _ = writeln!(out, "{ruler}");
            }
        } else {
            let ruler = "-".repeat(to_col(settings.layout.max_width));
            let _ = writeln!(out, "   +{ruler}+");
            for row in rows {
                let _ = writeln!(out, "{row}");
            }
            let _ = writeln!(out, "   +{ruler}+");
        }
        let mut shown = 0;
        let mut end = 0.0;
        for step in reveal.timeline(pi) {
            match &step.kind {
                StepKind::Show(i) => shown = i + 1,
                StepKind::Emit(e) => {
                    let _ = writeln!(out, "  {:>6.2}s  {:>3}文字目の後  {}", step.at, shown, e);
                }
                _ => {}
            }
            end = step.at;
        }
        let _ = writeln!(out, "  所要 {end:.2}s / {} 文字", page.glyphs.len());
        let clicks = reveal.click_times(pi);
        if !clicks.is_empty() {
            let _ = writeln!(
                out,
                "  (時刻は [l] で待たなかったときの値。待った分だけ後ろへずれる)"
            );
        }
        if settings.auto {
            for k in 0..clicks.len() {
                let voice = |id: &str| settings.voice_lens.get(id).copied();
                if let Some(wait) = reveal.auto_wait_click(pi, k, voice) {
                    let _ = writeln!(
                        out,
                        "  オート {}つ目のクリック待ちから {wait:.2}s で続きへ",
                        k + 1
                    );
                }
            }
        }
        if settings.auto && pi + 1 < pages {
            let wait = reveal.auto_wait(pi, |id: &str| settings.voice_lens.get(id).copied());
            let _ = writeln!(
                out,
                "  オート 入力待ちから {wait:.2}s で次へ({:.2}s)",
                end + wait
            );
        }
        // 文中のアイコン(`[icon id]`): ID と場所
        for (i, g) in page.glyphs.iter().enumerate() {
            if let Some(id) = &g.icon {
                let _ = writeln!(
                    out,
                    "  アイコン {id} / {}文字目 / {}行目 {:.2}〜{:.2}",
                    i + 1,
                    g.line + 1,
                    g.x,
                    g.x + g.width
                );
            }
        }
        if settings.speech {
            let _ = writeln!(
                out,
                "  読み上げ 「{}」",
                page.speech_text().replace('\n', " / ")
            );
        }
        // 名前付きの範囲(`[mark id]`): 文字の範囲と、行ごとの論理座標の矩形
        for m in layout.marks(pi) {
            let r = m.first_glyph..m.first_glyph + m.glyph_count;
            let chars: String = page.glyphs[r.clone()].iter().map(Glyph::text).collect();
            let spans: Vec<String> = page
                .spans(m.first_glyph, m.glyph_count)
                .iter()
                .map(|s| format!("{}行目 {:.2}〜{:.2}", s.line + 1, s.x0, s.x1))
                .collect();
            let _ = writeln!(
                out,
                "  範囲   {} / {}〜{}文字目「{chars}」/ {}",
                m.id,
                r.start + 1,
                r.end,
                spans.join(", ")
            );
        }
        if let Some(neko) = &neko {
            for (at, r, facing) in neko.punches(layout, &reveal, pi) {
                let chars: String = page.glyphs[r.clone()].iter().map(Glyph::text).collect();
                let dir = if facing == Facing::Left { "左" } else { "右" };
                let _ = writeln!(
                    out,
                    "  猫     {at:.2}s 猫パンチ / {}文字目「{chars}」が{dir}へ飛ぶ",
                    r.start + 1
                );
            }
            let settle = neko.settle_time(layout, &reveal, pi);
            let plan = match neko.wait_plan(pi, settings.reveal.motion) {
                Some(WaitPlan::Drop(r)) => {
                    let chars: String = page.glyphs[r.clone()].iter().map(Glyph::text).collect();
                    format!("{}文字目「{chars}」を押して落とす", r.start + 1)
                }
                Some(WaitPlan::Burst) => "ページの文字を爆散させる".to_string(),
                _ => "何もしない".to_string(),
            };
            let _ = writeln!(out, "  猫     {settle:.2}s に最後の文字へ着く / {plan}");
        }
    }
    if !layout.missing_vars.is_empty() {
        let _ = writeln!(out, "警告: 値のない変数 {}", layout.missing_vars.join(", "));
    }
    out
}
