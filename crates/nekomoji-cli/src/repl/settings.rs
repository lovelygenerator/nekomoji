//! 組版・文字送りの設定を変えるコマンド(`:width` `:lines` `:writing` `:speed` `:motion` など)。
use nekomoji::{Kinsoku, Writing};

use super::Repl;
use crate::render::{motion_name, parse_motion};

impl Repl {
    /// 組版・文字送りの設定を変え、組み直して表示する。知らないコマンドか引数が合わなければ `None`。
    pub(super) fn setting_command(&mut self, name: &str, args: &[&str]) -> Option<String> {
        let out = match (name, args) {
            ("width", [n]) => match n.parse::<f32>() {
                Ok(v) if v > 0.0 && v <= crate::MAX_WIDTH => {
                    self.settings.layout.max_width = v;
                    self.rebuild()
                }
                _ => format!(
                    "幅は {} 以下の正の数で指定してください: {n}",
                    crate::MAX_WIDTH
                ),
            },
            ("lines", ["off"]) => {
                self.settings.layout.max_lines = None;
                self.rebuild()
            }
            ("lines", [n]) => match n.parse::<usize>() {
                Ok(v) if v > 0 => {
                    self.settings.layout.max_lines = Some(v);
                    self.rebuild()
                }
                _ => format!("行数は 1 以上の整数か off で指定してください: {n}"),
            },
            ("kinsoku", [v]) => match *v {
                "standard" | "on" => {
                    self.settings.layout.kinsoku = Kinsoku::Standard;
                    self.rebuild()
                }
                "off" => {
                    self.settings.layout.kinsoku = Kinsoku::Off;
                    self.rebuild()
                }
                _ => "standard か off を指定してください".to_string(),
            },
            ("hanging", [v]) => match *v {
                "on" | "off" => {
                    self.settings.layout.hanging = *v == "on";
                    self.rebuild()
                }
                _ => "on か off を指定してください".to_string(),
            },
            ("tsume", [v]) | ("justify", [v]) => match *v {
                "on" | "off" => {
                    if name == "tsume" {
                        self.settings.layout.punct_squeeze = *v == "on";
                    } else {
                        self.settings.layout.justify = *v == "on";
                    }
                    self.rebuild()
                }
                _ => "on か off を指定してください".to_string(),
            },
            ("indent", [v]) => {
                if crate::apply_indent(&mut self.settings.layout.indent, v) {
                    self.rebuild()
                } else {
                    format!("novel・dialogue・off か 1行目[,2行目以降] を指定してください: {v}")
                }
            }
            ("auto", [v]) => match *v {
                "on" | "off" => {
                    self.settings.auto = *v == "on";
                    self.rebuild()
                }
                _ => "on か off を指定してください".to_string(),
            },
            ("blip", [v]) => match crate::parse_blip(v) {
                Some(b) => {
                    self.settings.reveal.blip = b;
                    self.rebuild()
                }
                None => format!("ID[,文字数] か off を指定してください: {v}"),
            },
            ("align", [v]) => match crate::parse_align(v) {
                Some(a) => {
                    self.settings.layout.align = a;
                    self.rebuild()
                }
                None => format!("start・center・end のどれかを指定してください: {v}"),
            },
            ("bracket", [v]) => {
                if crate::apply_bracket_indent(&mut self.settings.layout.indent, v) {
                    self.rebuild()
                } else {
                    format!("1行目[,2行目以降] を 0 以上の数で指定してください: {v}")
                }
            }
            ("wao", [n]) => match n.parse::<f32>() {
                Ok(v) if v.is_finite() && v >= 0.0 => {
                    self.settings.layout.latin_space = v;
                    self.rebuild()
                }
                _ => format!("和欧間のアキは 0 以上の数で指定してください: {n}"),
            },
            ("writing", [v]) => match *v {
                "yoko" | "horizontal" => {
                    self.settings.layout.writing = Writing::Horizontal;
                    self.rebuild()
                }
                "tate" | "vertical" => {
                    self.settings.layout.writing = Writing::Vertical;
                    self.rebuild()
                }
                _ => "yoko か tate を指定してください".to_string(),
            },
            ("speed", [n]) => match n.parse::<f32>() {
                Ok(v) if v > 0.0 => {
                    self.settings.reveal.chars_per_sec = v;
                    self.rebuild()
                }
                _ => format!("速さは正の数で指定してください: {n}"),
            },
            ("motion", [v]) => match parse_motion(v) {
                Some(m) => {
                    self.settings.reveal.motion = m;
                    let mut out = format!("動き: {}\n", motion_name(m));
                    out.push_str(&self.rebuild());
                    out
                }
                None => "full・reduced・off のどれかを指定してください".to_string(),
            },
            _ => return None,
        };
        Some(out)
    }
}
