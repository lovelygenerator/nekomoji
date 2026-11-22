//! REPL のコマンド処理。入出力は持たず、1行を受けて表示する文字列を返す。
//!
//! コマンドは種類ごとに分けてある:
//! - `settings`: 組版・文字送りの設定(`:width` `:lines` `:writing` `:speed` `:motion` など)
//! - `session`: 変数・読み込み・文字送り・保存と復元・バックログ(`:set` `:load` `:step` `:next` `:state` `:backlog` など)
//! - `neko`: 猫モード(`:neko` `:seed` `:knock` `:burst` `:punch` `:ascii`)

mod neko;
mod session;
mod settings;
#[cfg(test)]
mod tests;

use std::fmt::Write as _;

use nekomoji::{Layout, Neko, Reveal, TextEvent};

use crate::render::{format_errors, log, page_rows_with, CatView, Settings};

/// REPL がしてほしいこと。
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// 文字列を表示する
    Print(String),
    /// 今の文を実時間で再生する(端末の制御が要るので呼び出し側が行う)
    Play,
    /// 終了する
    Quit,
}

/// REPL の状態。
#[derive(Debug, Default)]
pub struct Repl {
    /// 設定
    pub settings: Settings,
    source: Option<String>,
    session: Option<(Layout, Reveal)>,
    neko: Option<Neko>,
}

/// `:help` の内容。
pub const HELP: &str = "\
マークアップを1行入力すると、行分割・ルビ・時間割を表示します。
  :width <文字数>          1行の幅(全角の文字数)
  :lines <行数|off>        1ページの最大行数(超えたら自動で改ページ)
  :kinsoku <standard|off>  禁則の切替
  :hanging <on|off>        句読点のぶら下げ
  :tsume <on|off>          約物の詰め
  :wao <比>                和欧間のアキ(全角に対する比、0 で切)
  :justify <on|off>        両端揃え
  :indent <novel|dialogue|off|1行目[,2行目以降]>  地の文の字下げ
  :align <start|center|end>  [align] の無い段落の揃え
  :bracket <1行目[,2行目以降]>  会話(括弧で始まる段落)の字下げ(0 で天付き、0.5 で半角下げ)
  :writing <yoko|tate>     横書き・縦書き(縦書きでは :width が1行の高さ)
  :speed <文字/秒>          文字送りの速さ
  :auto <on|off>           オートモードの待ち時間を出す(:show・:backlog などの表示に)
  :blip <ID[,文字数]|off>  文字送りの音を本文全体で鳴らす(off で [blip] の範囲だけ)
  :motion <full|reduced|off> 動きの強さ(控えめ・なしで揺れ・明滅・猫のちょっかいを抑える)
  :set <変数> <値>          変数を設定(:unset <変数> で削除、:vars で一覧)
  :load <ファイル>          ファイルを読み込む(改行は改行として扱う)
  :show                    今の文をもう一度表示
  :step <秒>               文字送りを指定時間だけ進める
  :next                    入力待ちなら先へ([l] では続き、[p] では次のページ)、途中なら次の入力待ちまで
  :reset                   文字送りを最初に戻す
  :state                   文字送りの状態(保存用の文字列 p<ページ> t<秒>)
  :restore <状態>           :state の文字列の状態に戻す(例: :restore p1 t0.5)
  :play                    実時間で再生(入力待ちで Enter)
  :backlog                 読んだページのテキスト(ルビは ｜親文字《よみ》)
  :speech                  読んだページの読み上げ用のテキスト(ルビは読み)
  :neko <on|off>           猫モード(見えない猫のちょっかいと 🐾 のカーソル)
  :seed <数>               猫の乱数のシード
  :knock <確率>            入力待ちで最後の文字を押して落とす確率(0〜1)
  :burst <確率>            入力待ちでページの文字を爆散させる確率(0〜1)
  :punch <確率>            文字送り中の猫パンチの確率(0〜1)
  :ascii <on|off>          カーソルを =^.^= で表示する
  :help / :quit";

impl Repl {
    /// 新しい REPL。
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            source: None,
            session: None,
            neko: None,
        }
    }

    /// 今の文のレイアウト(`:play` 用)。
    pub fn layout(&self) -> Option<&Layout> {
        self.session.as_ref().map(|(l, _)| l)
    }

    /// 1行を処理する。
    pub fn handle(&mut self, input: &str) -> Action {
        let input = input.trim_end_matches(['\n', '\r']);
        if input.trim().is_empty() {
            return Action::Print(String::new());
        }
        let Some(cmd) = input.strip_prefix(':') else {
            return Action::Print(self.set_source(input.to_string()));
        };
        let mut words = cmd.split_whitespace();
        let name = words.next().unwrap_or("");
        let args: Vec<&str> = words.collect();
        let out = match (name, args.as_slice()) {
            ("q" | "quit" | "exit", _) => return Action::Quit,
            ("help" | "h" | "?", _) => HELP.to_string(),
            ("play", []) => {
                if self.session.is_some() {
                    return Action::Play;
                }
                "先に文を入力してください".to_string()
            }
            (name, args) => self
                .setting_command(name, args)
                .or_else(|| self.session_command(name, args))
                .or_else(|| self.neko_command(name, args))
                .unwrap_or_else(|| format!("知らないコマンドか引数の誤り: :{cmd}(:help で一覧)")),
        };
        Action::Print(out)
    }

    fn set_source(&mut self, src: String) -> String {
        self.source = Some(src);
        self.rebuild()
    }

    fn reset_reveal(&mut self) {
        if let Some((layout, reveal)) = &mut self.session {
            *reveal = Reveal::new(layout, self.settings.reveal.clone());
        }
    }

    /// 設定が変わったら組み直して全体を表示する。
    fn rebuild(&mut self) -> String {
        let mut out = self.settings.summary();
        let Some(src) = &self.source else {
            return out;
        };
        match self.settings.build(src) {
            Ok(layout) => {
                let reveal = Reveal::new(&layout, self.settings.reveal.clone());
                self.neko = self.settings.neko(&layout);
                out.push('\n');
                out.push_str(log(&layout, &self.settings).trim_end());
                self.session = Some((layout, reveal));
            }
            Err(e) => {
                self.session = None;
                self.neko = None;
                out.push('\n');
                out.push_str(&format_errors(src, &e));
            }
        }
        out
    }

    fn step(&mut self, f: impl FnOnce(&mut Reveal) -> Vec<TextEvent>) -> String {
        let Some((layout, reveal)) = &mut self.session else {
            return "先に文を入力してください".to_string();
        };
        let events = f(reveal);
        let page = reveal.page();
        let total = layout.pages.get(page).map_or(0, |p| p.glyphs.len());
        let mut out = String::new();
        let view = self
            .neko
            .as_ref()
            .map(|n| CatView::new(n, layout, reveal, self.settings.ascii))
            .unwrap_or_default();
        for row in page_rows_with(layout, page, reveal.visible_count(), &view) {
            let _ = writeln!(out, "{row}");
        }
        let state = if reveal.is_finished() {
            " 終了"
        } else if reveal.wait_kind() == Some(nekomoji::WaitKind::Click) {
            " ▽クリック待ち(:next で続き)"
        } else if reveal.is_waiting() {
            " ▼入力待ち(:next)"
        } else {
            ""
        };
        let _ = write!(
            out,
            "ページ {}/{}  t={:.2}s  {}/{} 文字{}",
            page + 1,
            layout.pages.len(),
            reveal.time(),
            reveal.visible_count(),
            total,
            state
        );
        for e in events {
            let _ = write!(out, "\n  ♪ {e}");
        }
        out
    }
}
