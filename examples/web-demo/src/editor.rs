//! エディタ画面のうち、ブラウザに依存しない部分(メッセージの組み立て)。
//! ネイティブでテストできるように、`web` モジュールから分けてある。

use nekomoji::{Layout, ParseError};
use nekomoji_wgpu::FontSet;

/// 解析エラーを画面に出す1行ずつの日本語にする(行・桁は 1 始まり)。
pub fn format_errors(errors: &[ParseError]) -> Vec<String> {
    errors
        .iter()
        .map(|e| format!("{}行{}桁: {}", e.line, e.col, e.kind))
        .collect()
}

/// 値の無い変数と、フォントの字形に関する注意を日本語で返す。
/// 字形が無い文字(代わりの文字も無く、豆腐になる)と、代わりのフォントで描く文字を分ける。
/// 画面の文字として読むので、先頭は注意の見出し。
pub fn warnings(layout: &Layout, font: &FontSet) -> Vec<String> {
    let mut out = Vec::new();
    for name in &layout.missing_vars {
        out.push(format!(
            "変数 [var {name}] に値がないので、書いたまま表示します"
        ));
    }
    let mut missing: Vec<char> = Vec::new();
    let mut borrowed: Vec<char> = Vec::new();
    let chars = layout.pages.iter().flat_map(|p| {
        p.glyphs
            .iter()
            .filter(|g| g.icon.is_none())
            .map(|g| g.ch)
            .chain(p.rubies.iter().flat_map(|r| r.text.chars()))
    });
    for ch in chars {
        if ch.is_whitespace() || missing.contains(&ch) || borrowed.contains(&ch) {
            continue;
        }
        match font.glyph(ch) {
            None => missing.push(ch),
            Some(g) if g.font > 0 => borrowed.push(ch),
            Some(_) => {}
        }
    }
    if !borrowed.is_empty() {
        let list: String = borrowed.iter().collect();
        out.push(format!(
            "代わりのフォントで描く文字 {} 字: {list}",
            borrowed.len()
        ));
    }
    if !missing.is_empty() {
        let list: String = missing.iter().collect();
        out.push(format!(
            "フォントに字形が無い文字 {} 字: {list}(「フォントを開く」で字の多いフォントを読み込めます)",
            missing.len()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nekomoji::{parse_all, Layout, LayoutOptions, Vars};

    /// 既定で配るデモ用のサブセット(demo.neko の文字だけ)
    const FONT: &[u8] = include_bytes!("../font.ttf");

    fn font() -> FontSet {
        FontSet::from_bytes(FONT, 0, 36.0).expect("font.ttf を読める")
    }

    fn layout(src: &str) -> Layout {
        let script = parse_all(src).expect("解析できる");
        Layout::build(&script, &font(), &LayoutOptions::default(), &Vars::new())
    }

    #[test]
    fn 解析エラーは行と桁つきの日本語になる() {
        let errors = parse_all("あ\nい[nope]う").unwrap_err();
        let lines = format_errors(&errors);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("2行"), "{}", lines[0]);
        assert!(lines[0].contains("知らないタグ [nope]"), "{}", lines[0]);
    }

    #[test]
    fn 複数のエラーをすべて返す() {
        let errors = parse_all("[a]\n[b]").unwrap_err();
        assert_eq!(format_errors(&errors).len(), errors.len());
        assert!(errors.len() >= 2);
    }

    #[test]
    fn デモの文字だけなら注意は出ない() {
        let src = include_str!("../demo.neko").trim_end();
        assert_eq!(warnings(&layout(src), &font()), Vec::<String>::new());
    }

    #[test]
    fn サブセットに無い文字は注意に並ぶ() {
        let w = warnings(&layout("龍と鸞"), &font());
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("フォントに字形が無い文字"), "{}", w[0]);
        assert!(w[0].contains('龍') && w[0].contains('鸞'), "{}", w[0]);
    }

    #[test]
    fn ルビの読みの文字も調べる() {
        let w = warnings(&layout("[ruby ゐ]迷[/ruby]"), &font());
        assert!(
            w.iter().any(|m| m.contains('ゐ')),
            "ルビの読みの欠けを知らせる: {w:?}"
        );
    }

    #[test]
    fn 値の無い変数は注意に出る() {
        let w = warnings(&layout("[var player_name]"), &font());
        assert!(w.iter().any(|m| m.contains("[var player_name]")), "{w:?}");
    }
}
