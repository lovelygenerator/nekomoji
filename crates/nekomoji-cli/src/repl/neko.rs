//! 猫モードのコマンド(`:neko` `:seed` `:knock` `:burst` `:punch` `:ascii`)。
use nekomoji::NekoMode;

use super::Repl;

impl Repl {
    /// 猫モードの設定を変える。知らないコマンドか引数が合わなければ `None`。
    pub(super) fn neko_command(&mut self, name: &str, args: &[&str]) -> Option<String> {
        let out = match (name, args) {
            ("neko", [v]) => match *v {
                "on" => {
                    self.settings.neko.get_or_insert_with(NekoMode::default);
                    self.rebuild()
                }
                "off" => {
                    self.settings.neko = None;
                    self.rebuild()
                }
                _ => "on か off を指定してください".to_string(),
            },
            ("seed", [n]) => match n.parse::<u64>() {
                Ok(v) => {
                    self.settings
                        .neko
                        .get_or_insert_with(NekoMode::default)
                        .seed = v;
                    self.rebuild()
                }
                _ => format!("シードは 0 以上の整数で指定してください: {n}"),
            },
            (which @ ("knock" | "burst" | "punch"), [n]) => match n.parse::<f32>() {
                Ok(v) if (0.0..=1.0).contains(&v) => {
                    let m = self.settings.neko.get_or_insert_with(NekoMode::default);
                    match which {
                        "knock" => m.knock_chance = v,
                        "burst" => m.burst_chance = v,
                        _ => m.punch_chance = v,
                    }
                    self.rebuild()
                }
                _ => format!("確率は 0〜1 で指定してください: {n}"),
            },
            ("ascii", [v]) => match *v {
                "on" | "off" => {
                    self.settings.ascii = *v == "on";
                    self.step(|_| Vec::new())
                }
                _ => "on か off を指定してください".to_string(),
            },
            _ => return None,
        };
        Some(out)
    }
}
