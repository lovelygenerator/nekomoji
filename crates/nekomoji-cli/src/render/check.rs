//! `check` の出力。人が読む形と、機械で読む JSON(`--json`)。

use nekomoji::ParseError;

use super::format_errors;

/// 検査した1つのファイル: パス・中身・見つけたエラー(無ければ空)。
pub type Checked<'a> = (&'a str, &'a str, &'a [ParseError]);

/// 人が読む形。ファイルが1つなら `OK` かエラーだけ、2つ以上ならファイルごとに `== パス` の見出しを付ける。
pub fn check_text(files: &[Checked<'_>]) -> String {
    let one = |src: &str, errors: &[ParseError]| {
        if errors.is_empty() {
            "OK".to_string()
        } else {
            format_errors(src, errors)
        }
    };
    match files {
        [(_, src, errors)] => one(src, errors),
        _ => files
            .iter()
            .map(|(path, src, errors)| format!("== {path}\n{}", one(src, errors)))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// 機械で読む形(1つの JSON。末尾に改行は付けない)。
///
/// ```json
/// {"ok":false,"error_count":1,"files":[{"path":"a.neko","ok":false,"errors":[
///   {"line":1,"col":3,"code":"unknown_tag","message":"知らないタグ [foo]","line_text":"あい[foo]"}]}]}
/// ```
///
/// - `line`・`col` は 1 始まり。`col` は文字(Unicode のスカラー値)で数える
/// - `code` は種類の変わらない名前(`ErrorKind::code`)、`message` は日本語のメッセージ(位置なし)
/// - `line_text` はエラーのある行の元の文章
/// - エラーは各ファイルの中で位置の順
pub fn check_json(files: &[Checked<'_>]) -> String {
    let count: usize = files.iter().map(|f| f.2.len()).sum();
    let files: Vec<String> = files
        .iter()
        .map(|(path, src, errors)| {
            let errors: Vec<String> = errors
                .iter()
                .map(|e| {
                    let line_text = src.lines().nth(e.line.saturating_sub(1)).unwrap_or("");
                    format!(
                        r#"{{"line":{},"col":{},"code":{},"message":{},"line_text":{}}}"#,
                        e.line,
                        e.col,
                        json_string(e.kind.code()),
                        json_string(&e.kind.to_string()),
                        json_string(line_text),
                    )
                })
                .collect();
            format!(
                r#"{{"path":{},"ok":{},"errors":[{}]}}"#,
                json_string(path),
                errors.is_empty(),
                errors.join(",")
            )
        })
        .collect();
    format!(
        r#"{{"ok":{},"error_count":{count},"files":[{}]}}"#,
        count == 0,
        files.join(",")
    )
}

/// JSON の文字列にする(`"` `\` と制御文字をエスケープする)。
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
