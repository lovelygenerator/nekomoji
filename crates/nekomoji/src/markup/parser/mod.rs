//! マークアップを読む。文字を位置つきで並べ、本文・タグ・変数に分ける。
//! タグの中身の解釈は [`tag`]。

mod tag;

use std::ops::Range;

use super::error::{ErrorKind, ParseError};
use super::script::{Item, Script};
use super::style::Style;

/// ルビの短い書き方 `｜親文字《よみ》` の親文字の最大の文字数(取り違えたときに範囲が広がらないように)。
const SHORT_RUBY_MAX_BASE: usize = 32;

#[derive(Clone, Copy)]
struct Pos {
    line: usize,
    col: usize,
    /// 元の文章のバイト位置
    byte: usize,
}

struct Open {
    name: String,
    style: Style,
    /// `style` を [`Script::styles`](crate::Script::styles) に入れた番号。最初に文字・要素で使うときに決める
    /// (使われないスタイルは入れない。1文字ごとにスタイルを比べ直さないため)
    id: Option<usize>,
    pos: Pos,
}

struct Parser {
    chars: Vec<(char, Pos)>,
    i: usize,
    script: Script,
    stack: Vec<Open>,
    text: String,
    text_src: Vec<Range<usize>>,
    text_style: usize,
    /// 見つけたエラー(見つけた順)
    errors: Vec<ParseError>,
    /// エラーになった開きタグの名前。対応する閉じタグは黙って読み捨てる(エラーを連鎖させない)
    failed: Vec<String>,
}

/// マークアップを解析する。
///
/// - 範囲タグ: `[ruby よみ]…[/ruby]` `[color 名前]` `[size 倍率]` `[b]` `[speed 倍率]` `[shake 強さ? 秒?]` `[wave 強さ? 秒?]` `[fade 秒?]` `[neko hop 強さ?]` `[glow 種類? 強さ? 秒?]` `[outline 色? 太さ?]` `[shadow 色? x? y?]` `[sup]` `[sub]` `[s]` `[u]` `[em 記号?]` `[tcy]` `[mark id]` `[align 揃え]` `[blip 音?]`
/// - 単独タグ: `[wait 秒]` `[l]` `[p]` `[br]` `[icon id 幅?]` `[var 名前]`(変数) `[voice id]` `[se id]` `[event 名前 引数...]`
/// - 文字の `[` `]` は二重にする(`[[` `]]`)。`{` `}` は普通の文字
/// - ソースの改行は `[br]` と同じ
///
/// 最初に見つけたエラーを返す。すべてのエラーを集めるには [`parse_all`]。
pub fn parse(src: &str) -> Result<Script, ParseError> {
    run(src).map_err(|mut errors| errors.swap_remove(0))
}

/// マークアップを解析し、エラーをすべて集める。
///
/// 書き方は [`parse`] と同じ。エラーがあれば、見つけたエラーをすべて元の文章の位置
/// (行・桁)の順に返す(空にはならない)。最初のエラーは [`parse`] が返すものと同じとは限らない
/// (位置の順に並べるため。`[b]` の閉じ忘れは `[b]` の位置になる)。
///
/// 1つのエラーから次のエラーが連鎖しないよう、エラーの後は次のように読み進める:
///
/// - `]` の無い `[` は、その行の残りを読み飛ばす
/// - エラーになった開きタグ(知らないタグ・引数の誤り)に対応する閉じタグは黙って読み捨てる
/// - 閉じる順を誤った閉じタグは、開いているタグの中から同じ名前のものを閉じる
/// - ルビの親文字・縦中横の中のエラーは、その閉じタグまで読み飛ばす
/// - 閉じられていないタグは、開いているものすべてを知らせる
///
/// ```
/// let errors = nekomoji::parse_all("[foo]あ[/foo]い[size x]う[/size][var]").unwrap_err();
/// let at: Vec<(usize, usize)> = errors.iter().map(|e| (e.line, e.col)).collect();
/// assert_eq!(at, vec![(1, 1), (1, 14), (1, 30)]);
/// ```
pub fn parse_all(src: &str) -> Result<Script, Vec<ParseError>> {
    run(src).map_err(|mut errors| {
        errors.sort_by_key(|e| (e.line, e.col));
        errors
    })
}

/// 解析する。エラーは見つけた順。
fn run(src: &str) -> Result<Script, Vec<ParseError>> {
    let mut chars = Vec::new();
    let (mut line, mut col) = (1, 1);
    for (byte, ch) in src.char_indices() {
        // 改行の CR と、先頭の BOM(メモ帳などが付ける U+FEFF)は文字にしない。
        // 読み飛ばしてもバイト位置は元の文章のまま
        if ch == '\r' || (byte == 0 && ch == '\u{FEFF}') {
            continue;
        }
        chars.push((ch, Pos { line, col, byte }));
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    let mut p = Parser {
        chars,
        i: 0,
        script: Script {
            items: Vec::new(),
            styles: vec![Style::default()],
        },
        stack: Vec::new(),
        text: String::new(),
        text_src: Vec::new(),
        text_style: 0,
        errors: Vec::new(),
        failed: Vec::new(),
    };
    p.run();
    if p.errors.is_empty() {
        Ok(p.script)
    } else {
        Err(p.errors)
    }
}

/// ルビの親文字・縦中横を読み始めた後のエラーか(閉じタグまで読み飛ばし済みなので、閉じタグを待たない)。
fn consumed_range(kind: &ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::NestedInRuby
            | ErrorKind::NestedInTcy
            | ErrorKind::EmptyRuby
            | ErrorKind::EmptyTcy
            | ErrorKind::Unclosed(_)
            | ErrorKind::UnterminatedTag
            | ErrorKind::LoneClose(_)
    )
}

fn err(pos: Pos, kind: ErrorKind) -> ParseError {
    ParseError {
        line: pos.line,
        col: pos.col,
        kind,
    }
}

impl Parser {
    fn peek(&self, off: usize) -> Option<char> {
        self.chars.get(self.i + off).map(|c| c.0)
    }

    /// `self.i` から `n` 文字(`char`)分の、元の文章のバイト範囲。
    fn span(&self, n: usize) -> Range<usize> {
        let (_, start) = self.chars[self.i];
        let (ch, last) = self.chars[self.i + n - 1];
        start.byte..last.byte + ch.len_utf8()
    }

    fn current_style(&self) -> Style {
        self.stack
            .last()
            .map(|o| o.style.clone())
            .unwrap_or_default()
    }

    /// 今のスタイルの番号。開いているタグごとに一度だけ探して覚える。
    fn current_id(&mut self) -> usize {
        let Some(open) = self.stack.last() else {
            return 0;
        };
        if let Some(id) = open.id {
            return id;
        }
        let style = open.style.clone();
        let id = self.intern(style);
        if let Some(open) = self.stack.last_mut() {
            open.id = Some(id);
        }
        id
    }

    fn intern(&mut self, style: Style) -> usize {
        if let Some(i) = self.script.styles.iter().position(|s| *s == style) {
            return i;
        }
        self.script.styles.push(style);
        self.script.styles.len() - 1
    }

    /// `ch` を本文に足し、`n` 文字分(エスケープなら 2)読み進める。
    fn push_char(&mut self, ch: char, n: usize) {
        let id = self.current_id();
        if id != self.text_style {
            self.flush();
            self.text_style = id;
        }
        self.text.push(ch);
        self.text_src.push(self.span(n));
        self.i += n;
    }

    fn flush(&mut self) {
        if !self.text.is_empty() {
            let text = std::mem::take(&mut self.text);
            let source = std::mem::take(&mut self.text_src);
            self.script.items.push(Item::Text {
                text,
                style: self.text_style,
                source,
            });
        }
    }

    fn push_item(&mut self, item: Item) {
        self.flush();
        self.script.items.push(item);
    }

    /// 段落の頭か(前の改行・改ページの後に文字が無い。待機・イベントは数えない)。
    fn at_paragraph_start(&self) -> bool {
        self.text.is_empty()
            && matches!(
                self.script
                    .items
                    .iter()
                    .rev()
                    .find(|it| !matches!(it, Item::Wait(_) | Item::Event(_) | Item::Click)),
                None | Some(Item::Break | Item::Page)
            )
    }

    fn run(&mut self) {
        while let Some(&(ch, pos)) = self.chars.get(self.i) {
            if let Err(e) = self.step(ch, pos) {
                self.errors.push(e);
            }
        }
        self.flush();
        // 内側(後に開いたもの)から知らせる(最初のエラーを今までの `parse` と同じにするため)
        let unclosed: Vec<ParseError> = self
            .stack
            .iter()
            .rev()
            .map(|open| err(open.pos, ErrorKind::Unclosed(open.name.clone())))
            .collect();
        self.errors.extend(unclosed);
    }

    /// `self.i` の1文字(タグ・変数なら全体)を読む。エラーのときは、次を読める所まで進めてから返す。
    fn step(&mut self, ch: char, pos: Pos) -> Result<(), ParseError> {
        match ch {
            '[' if self.peek(1) == Some('[') => self.push_char('[', 2),
            ']' if self.peek(1) == Some(']') => self.push_char(']', 2),
            '[' => {
                let body = self
                    .read_until(']', pos, ErrorKind::UnterminatedTag)
                    .map_err(|e| {
                        self.skip_line();
                        e
                    })?;
                self.tag(&body, pos).map_err(|e| {
                    let body = body.trim();
                    if !body.starts_with('/') && !consumed_range(&e.kind) {
                        let name = body.split_whitespace().next().unwrap_or("");
                        self.failed.push(name.to_string());
                    }
                    e
                })?;
            }
            '|' | '｜' if self.short_ruby() => {}
            '\n' => {
                self.push_item(Item::Break);
                self.i += 1;
            }
            ']' => {
                self.i += 1;
                return Err(err(pos, ErrorKind::LoneClose(ch)));
            }
            _ => self.push_char(ch, 1),
        }
        Ok(())
    }

    /// エラーの後、行の残りを読み飛ばす(改行は残し、次に読む)。
    fn skip_line(&mut self) {
        while self.chars.get(self.i).is_some_and(|c| c.0 != '\n') {
            self.i += 1;
        }
    }

    /// ルビの親文字・縦中横の中のエラーの後、`[/name]` の後ろまで読み飛ばす。
    /// 見つからなければ行の残りを読み飛ばす。
    fn skip_range(&mut self, name: &str) {
        let close: Vec<char> = format!("[/{name}]").chars().collect();
        let found = (self.i..self.chars.len()).find(|&j| {
            close
                .iter()
                .enumerate()
                .all(|(k, &c)| self.chars.get(j + k).map(|c| c.0) == Some(c))
        });
        match found {
            Some(j) => self.i = j + close.len(),
            None => self.skip_line(),
        }
    }

    /// `self.i` は開き文字。閉じ文字の次まで進め、中身を返す。
    fn read_until(&mut self, close: char, pos: Pos, kind: ErrorKind) -> Result<String, ParseError> {
        let mut body = String::new();
        let mut j = self.i + 1;
        loop {
            match self.chars.get(j) {
                None => return Err(err(pos, kind)),
                Some(&(c, _)) if c == close => break,
                Some(&('\n', _)) => return Err(err(pos, kind)),
                Some(&(c, _)) => body.push(c),
            }
            j += 1;
        }
        self.i = j + 1;
        Ok(body)
    }

    fn close(&mut self, name: &str, pos: Pos) -> Result<(), ParseError> {
        match self.stack.last() {
            Some(open) if open.name == name => {
                self.stack.pop();
                Ok(())
            }
            other => {
                // エラーになった開きタグの閉じタグは黙って読み捨てる
                if let Some(k) = self.failed.iter().rposition(|f| f == name) {
                    self.failed.remove(k);
                    return Ok(());
                }
                let e = err(
                    pos,
                    ErrorKind::UnexpectedClose {
                        expected: other.map(|o| o.name.clone()),
                        found: name.to_string(),
                    },
                );
                // 閉じる順の誤りなら、その名前のタグを閉じて続ける(後で閉じ忘れと重ねて知らせない)
                if let Some(k) = self.stack.iter().rposition(|o| o.name == name) {
                    self.stack.remove(k);
                }
                Err(e)
            }
        }
    }

    /// ルビの短い書き方 `｜親文字《よみ》`(`|` も可)を読む。`self.i` は `|` か `｜`。
    /// 形が合えばルビにして `true`、合わなければ何も進めず `false`(`|` は文字になる)。
    ///
    /// 親文字は 1〜[`SHORT_RUBY_MAX_BASE`] 文字で、空白・改行と `| ｜ 《 》 [ ] { }` を含まない。
    /// 読みは空でなく、改行と `《 》 [ ] { }` を含まない。
    fn short_ruby(&mut self) -> bool {
        let is_stop = |c: char| c.is_whitespace() || "|｜《》[]".contains(c);
        let start = self.i + 1;
        let mut j = start;
        while let Some(c) = self.chars.get(j).map(|c| c.0) {
            if c == '《' {
                break;
            }
            if is_stop(c) || j - start >= SHORT_RUBY_MAX_BASE {
                return false;
            }
            j += 1;
        }
        let base_end = j;
        if base_end == start || self.chars.get(base_end).is_none() {
            return false;
        }
        let mut reading = String::new();
        let mut k = base_end + 1;
        loop {
            match self.chars.get(k).map(|c| c.0) {
                Some('》') => break,
                Some(c) if c.is_whitespace() && c != ' ' && c != '\u{3000}' => return false,
                Some(c) if "《[]".contains(c) => return false,
                Some(c) => reading.push(c),
                None => return false,
            }
            k += 1;
        }
        if reading.trim().is_empty() {
            return false;
        }
        let base: String = self.chars[start..base_end].iter().map(|c| c.0).collect();
        let source: Vec<Range<usize>> = self.chars[start..base_end]
            .iter()
            .map(|&(c, p)| p.byte..p.byte + c.len_utf8())
            .collect();
        let style = self.current_id();
        self.push_item(Item::Ruby {
            base,
            reading: reading.trim().to_string(),
            style,
            source,
        });
        self.i = k + 1;
        true
    }

    fn ruby(&mut self, reading: String, pos: Pos) -> Result<(), ParseError> {
        let (base, source) = self.raw_range("ruby", pos)?;
        let style = self.current_id();
        self.push_item(Item::Ruby {
            base,
            reading,
            style,
            source,
        });
        Ok(())
    }

    /// タグを書けない範囲(ルビの親文字・縦中横)を `[/name]` まで読む。エスケープ(`[[` `]]`)だけは読む。
    /// 中身と、1文字ごとの元の文章のバイト範囲を返す。
    fn raw_range(
        &mut self,
        name: &str,
        pos: Pos,
    ) -> Result<(String, Vec<Range<usize>>), ParseError> {
        let (nested, empty) = if name == "ruby" {
            (ErrorKind::NestedInRuby, ErrorKind::EmptyRuby)
        } else {
            (ErrorKind::NestedInTcy, ErrorKind::EmptyTcy)
        };
        let close = format!("/{name}");
        let mut base = String::new();
        let mut source = Vec::new();
        loop {
            let Some(&(ch, at)) = self.chars.get(self.i) else {
                return Err(err(pos, ErrorKind::Unclosed(name.into())));
            };
            match ch {
                '[' | ']' if self.peek(1) == Some(ch) => {
                    base.push(ch);
                    source.push(self.span(2));
                    self.i += 2;
                }
                '[' => {
                    let start = self.i;
                    let body = self
                        .read_until(']', at, ErrorKind::UnterminatedTag)
                        .map_err(|e| {
                            self.skip_range(name);
                            e
                        })?;
                    if body.trim() != close {
                        self.i = start;
                        self.skip_range(name);
                        return Err(err(at, nested));
                    }
                    break;
                }
                '\n' => {
                    self.skip_range(name);
                    return Err(err(at, nested));
                }
                ']' => {
                    self.skip_range(name);
                    return Err(err(at, ErrorKind::LoneClose(ch)));
                }
                _ => {
                    base.push(ch);
                    source.push(self.span(1));
                    self.i += 1;
                }
            }
        }
        if base.is_empty() {
            return Err(err(pos, empty));
        }
        Ok((base, source))
    }
}
