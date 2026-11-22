//! ゴールデンテスト: tests/golden/*.neko の出力を *.expected.txt と比べる。
//!
//! - ファイル名の `.w<幅>` で1行の幅を指定できる(例: `kinsoku.w5.neko`)。なければ既定の 20
//! - `.l<行数>` で1ページの最大行数を指定できる(例: `auto_page.w4.l2.neko`)
//! - `.justify` で両端揃え、`.tsume` で約物の詰めを有効にする
//! - `.auto` でページごとのオートモードの待ち時間を出す(ボイス `v1` の長さは 2 秒とする)
//! - `.blip` で文字送りの音を本文全体で鳴らす(`Blip::default()`)
//! - `.speech` でページごとの読み上げ用のテキストを出す
//! - `.novel`・`.dialogue` で字下げ(`Indent::novel`・`Indent::dialogue`)
//! - `.reduced` で動きを控えめに、`.still` で動きなしにする
//! - `.tate` があれば縦書きで組む(例: `vertical.tate.w6.neko`)
//! - `.seed<数>`・`.knock<確率>`・`.burst<確率>`・`.punch<確率>` があれば猫モードを有効にする(例: `neko.seed3.knock1.neko`)
//! - 解析エラーになる入力は、エラーの表示を比べる
//! - 意図した変更のときだけ `BLESS=1 cargo test -p nekomoji-cli --test golden` で期待値を更新する

use std::fs;
use std::path::Path;

use nekomoji::NekoMode;
use nekomoji_cli::{format_errors, log, Settings};

fn apply_name(stem: &str, settings: &mut Settings) {
    for part in stem.split('.').skip(1) {
        if part == "tate" {
            settings.layout.writing = nekomoji::Writing::Vertical;
        } else if part == "justify" {
            settings.layout.justify = true;
        } else if part == "auto" {
            settings.auto = true;
            settings.voice_lens.insert("v1".into(), 2.0);
        } else if part == "speech" {
            settings.speech = true;
        } else if part == "blip" {
            settings.reveal.blip = Some(nekomoji::Blip::default());
        } else if part == "novel" {
            settings.layout.indent = nekomoji::Indent::novel();
        } else if part == "dialogue" {
            settings.layout.indent = nekomoji::Indent::dialogue();
        } else if part == "tsume" {
            settings.layout.punct_squeeze = true;
        } else if part == "reduced" {
            settings.reveal.motion = nekomoji::Motion::Reduced;
        } else if part == "still" {
            settings.reveal.motion = nekomoji::Motion::Off;
        } else if let Some(w) = part.strip_prefix('w').and_then(|v| v.parse().ok()) {
            settings.layout.max_width = w;
        } else if let Some(v) = part.strip_prefix('l').and_then(|v| v.parse().ok()) {
            settings.layout.max_lines = Some(v);
        } else if let Some(v) = part.strip_prefix("seed").and_then(|v| v.parse().ok()) {
            settings.neko.get_or_insert_with(NekoMode::default).seed = v;
        } else if let Some(v) = part.strip_prefix("knock").and_then(|v| v.parse().ok()) {
            settings
                .neko
                .get_or_insert_with(NekoMode::default)
                .knock_chance = v;
        } else if let Some(v) = part.strip_prefix("burst").and_then(|v| v.parse().ok()) {
            settings
                .neko
                .get_or_insert_with(NekoMode::default)
                .burst_chance = v;
        } else if let Some(v) = part.strip_prefix("punch").and_then(|v| v.parse().ok()) {
            settings
                .neko
                .get_or_insert_with(NekoMode::default)
                .punch_chance = v;
        }
    }
}

fn render(path: &Path) -> String {
    let src = fs::read_to_string(path).unwrap();
    let src = src.trim_end();
    let stem = path.file_stem().unwrap().to_str().unwrap();
    let mut settings = Settings::default();
    apply_name(stem, &mut settings);
    let mut out = format!("# {}\n", settings.summary());
    match settings.build(src) {
        Ok(layout) => out.push_str(&log(&layout, &settings)),
        Err(e) => {
            out.push_str(&format_errors(src, &e));
            out.push('\n');
        }
    }
    out
}

#[test]
fn golden() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let bless = std::env::var_os("BLESS").is_some();
    let mut inputs: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "neko"))
        .collect();
    inputs.sort();
    assert!(!inputs.is_empty());
    let mut failed = Vec::new();
    for input in &inputs {
        let actual = render(input);
        let expected_path = input.with_extension("expected.txt");
        if bless {
            fs::write(&expected_path, &actual).unwrap();
            continue;
        }
        match fs::read_to_string(&expected_path) {
            Ok(expected) if expected == actual => {}
            Ok(expected) => failed.push(format!(
                "{}\n--- expected\n{expected}--- actual\n{actual}",
                input.display()
            )),
            Err(_) => failed.push(format!(
                "{}: 期待値がない(BLESS=1 で作る)\n{actual}",
                input.display()
            )),
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
