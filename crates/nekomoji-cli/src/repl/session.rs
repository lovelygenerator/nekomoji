//! 今の文と文字送りを扱うコマンド(変数・読み込み・文字送り・保存と復元・バックログ)。
use std::fmt::Write as _;

use nekomoji::RevealState;

use super::Repl;

impl Repl {
    /// 変数・読み込み・文字送り・保存と復元・バックログを扱う。知らないコマンドか引数が合わなければ `None`。
    pub(super) fn session_command(&mut self, name: &str, args: &[&str]) -> Option<String> {
        let out = match (name, args) {
            ("set", [name, value @ ..]) if !value.is_empty() => {
                self.settings.vars.insert(name.to_string(), value.join(" "));
                self.rebuild()
            }
            ("unset", [name]) => {
                self.settings.vars.remove(*name);
                self.rebuild()
            }
            ("vars", []) => {
                if self.settings.vars.is_empty() {
                    "(変数なし)".to_string()
                } else {
                    self.settings
                        .vars
                        .iter()
                        .map(|(k, v)| format!("{k} = {v}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            }
            ("load", [path]) => match std::fs::read_to_string(path) {
                Ok(src) => self.set_source(src.trim_end().to_string()),
                Err(e) => format!("読み込めません {path}: {e}"),
            },
            ("show", []) => self.rebuild(),
            ("step", [n]) => match n.parse::<f32>() {
                Ok(v) if v >= 0.0 => self.step(|r| r.advance(v)),
                _ => format!("秒は 0 以上の数で指定してください: {n}"),
            },
            ("next", []) => self.step(|r| {
                if r.proceed() {
                    r.advance(0.0)
                } else {
                    r.skip()
                }
            }),
            ("state", []) => match &self.session {
                Some((_, reveal)) => reveal.state().to_string(),
                None => "先に文を入力してください".to_string(),
            },
            ("restore", words) if !words.is_empty() => {
                match words.join(" ").parse::<RevealState>() {
                    Ok(st) => {
                        let ok = self
                            .session
                            .as_mut()
                            .map(|(_, reveal)| reveal.restore(st.clone()));
                        match ok {
                            None => "先に文を入力してください".to_string(),
                            Some(false) => format!("その状態には戻せません(ページがない): {st}"),
                            Some(true) => self.step(|_| Vec::new()),
                        }
                    }
                    Err(e) => e.to_string(),
                }
            }
            ("reset", []) => {
                self.reset_reveal();
                self.step(|r| r.advance(0.0))
            }
            ("backlog", []) => match &self.session {
                Some((layout, reveal)) => {
                    let mut out = String::new();
                    for (i, p) in layout.pages.iter().enumerate().take(reveal.page() + 1) {
                        if i > 0 {
                            out.push('\n');
                        }
                        let _ = write!(out, "[ページ {}]\n{}", i + 1, p.text());
                    }
                    out
                }
                None => "先に文を入力してください".to_string(),
            },
            ("speech", []) => match &self.session {
                Some((layout, reveal)) => {
                    let mut out = String::new();
                    for (i, p) in layout.pages.iter().enumerate().take(reveal.page() + 1) {
                        if i > 0 {
                            out.push('\n');
                        }
                        let _ = write!(out, "[ページ {}]\n{}", i + 1, p.speech_text());
                    }
                    out
                }
                None => "先に文を入力してください".to_string(),
            },
            _ => return None,
        };
        Some(out)
    }
}
