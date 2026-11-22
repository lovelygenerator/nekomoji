//! フォントのサブセット化。シナリオに出る文字だけを残した軽いフォントを作る。
//!
//! 使う文字は、シナリオを実際に組版して集める(本文とルビの読み。タグや変数名は含まない)。
//! 変数 `[var name]` の値は `--set` で渡す。渡さなければ警告する。変数の値は名前入力などで実行時に
//! 決まり、サブセットでは字形を保証できないので、変数を使うシナリオでは、描画側で代わりのフォントを
//! 連鎖に足すよう知らせる(`nekomoji-wgpu` の `FontSet::with_fallback`)。
//!
//! 入力が ttc のときは、`--index` の番号のフォントを取り出してからサブセットにする。
//!
//! 引数・使う文字の収集・結果の表示はここで、フォントのバイト列の処理は `font`。

mod font;
#[cfg(test)]
mod tests;

pub use font::{extract_font, subset_font};

use std::collections::BTreeSet;

use nekomoji::Layout;

use crate::{format_errors, Settings};

/// `nekomoji subset` の引数。
#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub font: String,
    pub scenarios: Vec<String>,
    pub output: String,
    /// ttc の中の何番目のフォントか
    pub index: u32,
    /// シナリオ以外に残す文字
    pub extra_chars: String,
    /// 半角の英数記号と全角スペースを無条件で残すか
    pub basic_chars: bool,
    pub settings: Settings,
}

pub const USAGE: &str = "\
使い方:
  nekomoji subset <フォント> <シナリオ>... -o <出力> [オプション]

オプション:
  -o, --output <ファイル>  出力先(必須)
  --index <番号>           ttc のとき何番目のフォントか(既定 0)
  --chars <文字列>         シナリオ以外に残す文字
  --no-basic               半角の英数記号と全角スペースを自動で残さない
  --set <変数>=<値>        シナリオの変数の値(繰り返し可)";

impl Args {
    pub fn parse(args: &[String]) -> Result<Args, String> {
        let mut positional = Vec::new();
        let mut output = None;
        let mut index = 0u32;
        let mut extra_chars = String::new();
        let mut basic_chars = true;
        let mut settings = Settings::default();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "-o" | "--output" => {
                    output = Some(it.next().ok_or("-o には出力先が必要です")?.clone())
                }
                "--index" => {
                    index = it
                        .next()
                        .and_then(|v| v.parse().ok())
                        .ok_or("--index には 0 以上の整数が必要です")?
                }
                "--chars" => extra_chars.push_str(it.next().ok_or("--chars には文字列が必要です")?),
                "--no-basic" => basic_chars = false,
                "--set" => {
                    let kv = it.next().ok_or("--set には <変数>=<値> が必要です")?;
                    let (k, v) = kv
                        .split_once('=')
                        .ok_or("--set には <変数>=<値> が必要です")?;
                    settings.vars.insert(k.to_string(), v.to_string());
                }
                f if f.starts_with('-') => return Err(format!("知らないオプション: {f}")),
                f => positional.push(f.to_string()),
            }
        }
        let mut positional = positional.into_iter();
        let font = positional.next().ok_or("フォントを指定してください")?;
        let scenarios: Vec<String> = positional.collect();
        if scenarios.is_empty() && extra_chars.is_empty() {
            return Err("シナリオか --chars を指定してください".into());
        }
        let output = output.ok_or("-o で出力先を指定してください")?;
        Ok(Args {
            font,
            scenarios,
            output,
            index,
            extra_chars,
            basic_chars,
            settings,
        })
    }
}

/// 組版済みのレイアウトに出てくる文字(本文とルビの読み)。
pub fn layout_chars(layout: &Layout) -> BTreeSet<char> {
    let mut set = BTreeSet::new();
    for page in &layout.pages {
        for g in &page.glyphs {
            // 異体字セレクタなどの続きの文字も残す(cmap の異体字の表を持つフォントのため)
            set.insert(g.ch);
            set.extend(g.tail.chars());
        }
        for r in &page.rubies {
            set.extend(r.text.chars());
        }
    }
    set
}

/// 半角の英数記号(U+0020〜007E)と全角スペース。
pub fn basic_chars() -> BTreeSet<char> {
    let mut set: BTreeSet<char> = (0x20u8..=0x7e).map(char::from).collect();
    set.insert('\u{3000}');
    set
}

/// 実行結果。表示は呼び出し側。
#[derive(Debug)]
pub struct Report {
    /// 残そうとした文字の数
    pub wanted: usize,
    /// フォントに字形が無く、代わりの文字も無かった文字
    pub missing_in_font: Vec<char>,
    /// フォントに字形が無いので、代わりの文字で表示されるもの(元, 代わり)
    pub substituted: Vec<(char, char)>,
    /// 値が無かった変数(`[var name]` のまま組まれた)
    pub missing_vars: Vec<String>,
    /// シナリオに出てくる変数(値の有無によらない。名前の順)。値は実行時に変わりうるので、
    /// サブセットでは字形を保証できない
    pub vars: Vec<String>,
    pub input_bytes: usize,
    pub output_bytes: usize,
}

/// シナリオを読み、フォントをサブセットにして書き出す。
pub fn execute(args: &Args) -> Result<Report, String> {
    let mut chars: BTreeSet<char> = args.extra_chars.chars().collect();
    if args.basic_chars {
        chars.extend(basic_chars());
    }
    let mut missing_vars = BTreeSet::new();
    let mut vars = BTreeSet::new();
    for path in &args.scenarios {
        let src = std::fs::read_to_string(path)
            .map_err(|e| format!("読み込めません {path}: {e}"))?
            .trim_end()
            .to_string();
        let layout = args
            .settings
            .build(&src)
            .map_err(|e| format!("{path}:\n{}", format_errors(&src, &e)))?;
        chars.extend(layout_chars(&layout));
        missing_vars.extend(layout.missing_vars.iter().cloned());
        vars.extend(layout.vars.iter().cloned());
    }
    // 組版で入る改行などの制御文字は要らない
    chars.retain(|c| !c.is_control());

    let data =
        std::fs::read(&args.font).map_err(|e| format!("読み込めません {}: {e}", args.font))?;
    let single = extract_font(&data, args.index)?;
    // 描画側はフォントに無い記号を代わりの文字で描くので、代わりの文字も残す
    let wanted = chars.clone();
    for c in &wanted {
        chars.extend(nekomoji::chars::fallback_chars(*c));
    }
    let (out, missing) = subset_font(&single, &chars)?;
    let missing: BTreeSet<char> = missing.into_iter().collect();
    let mut missing_in_font = Vec::new();
    let mut substituted = Vec::new();
    for &c in wanted.iter().filter(|c| missing.contains(c)) {
        match nekomoji::chars::fallback_chars(c)
            .iter()
            .find(|f| !missing.contains(f))
        {
            Some(&f) => substituted.push((c, f)),
            None => missing_in_font.push(c),
        }
    }
    std::fs::write(&args.output, &out)
        .map_err(|e| format!("書き込めません {}: {e}", args.output))?;
    Ok(Report {
        wanted: wanted.len(),
        missing_in_font,
        substituted,
        missing_vars: missing_vars.into_iter().collect(),
        vars: vars.into_iter().collect(),
        input_bytes: data.len(),
        output_bytes: out.len(),
    })
}

/// 結果の報告を、人が読む文字列にする。
pub fn format_report(args: &Args, r: &Report) -> String {
    let mut s = format!(
        "{} 文字を残しました: {} → {}({} バイト → {} バイト)",
        r.wanted - r.missing_in_font.len() - r.substituted.len(),
        args.font,
        args.output,
        r.input_bytes,
        r.output_bytes
    );
    if !r.missing_in_font.is_empty() {
        let list: String = r.missing_in_font.iter().collect();
        s.push_str(&format!(
            "\n警告: フォントに字形が無い文字が {} 個あります: {list}",
            r.missing_in_font.len()
        ));
    }
    if !r.substituted.is_empty() {
        let list: Vec<String> = r
            .substituted
            .iter()
            .map(|(c, f)| format!("{c}→{f}"))
            .collect();
        s.push_str(&format!(
            "\n字形が無いので代わりの文字で表示します(代わりの文字を残しました): {}",
            list.join(" ")
        ));
    }
    if !r.missing_vars.is_empty() {
        s.push_str(&format!(
            "\n警告: 値が無い変数があります(--set で渡すと、その値の文字も残ります): {}",
            r.missing_vars.join(", ")
        ));
    }
    if !r.vars.is_empty() {
        let list: Vec<String> = r.vars.iter().map(|v| format!("[var {v}]")).collect();
        s.push_str(&format!(
            "\n注意: 変数の値(名前入力など)は実行時に変わるので、このフォントでは字形を保証できません: {}\n  \
             描画側で、字の多いフォントを代わりのフォントとして連鎖に足してください\
             (nekomoji-wgpu の FontSet::with_fallback)",
            list.join(" ")
        ));
    }
    s
}
