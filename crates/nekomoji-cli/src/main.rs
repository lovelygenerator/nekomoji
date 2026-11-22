//! nekomoji CLI。
//!
//! ```text
//! nekomoji log <ファイル> [オプション]    行分割・ルビ・時間割を表示
//! nekomoji check <ファイル>... [--json]   マークアップを検査(エラーなら終了コード 1。--json で機械で読める形)
//! nekomoji repl [ファイル] [オプション]   対話的に試す
//! nekomoji subset <フォント> <シナリオ>... -o <出力>   使う文字だけのフォントを作る
//!
//! オプション: --width <全角文字数> --lines <行数> --vertical --tsume --wao <比> --justify --indent <..> --bracket-indent <..> --align <..> --kinsoku <standard|off> --no-hanging
//!             --speed <文字/秒> --set <変数>=<値>
//!             --neko --seed <数> --knock <確率> --burst <確率> --punch <確率> --no-walk --ascii
//! ```

use std::io::{self, BufRead, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use nekomoji::{parse_all, Kinsoku, Layout, NekoMode, Reveal, TextEvent, WaitKind, Writing};
use nekomoji_cli::{
    check_json, check_text, format_errors, log, page_rows_with, Action, CatView, Repl, Settings,
    MAX_WIDTH,
};

const USAGE: &str = "\
使い方:
  nekomoji log <ファイル> [オプション]    行分割・ルビ・時間割を表示
  nekomoji check <ファイル>... [--json]   マークアップを検査(エラーをすべて出す。
                                          --json で機械で読める JSON を標準出力へ)
  nekomoji repl [ファイル] [オプション]   対話的に試す
  nekomoji subset <フォント> <シナリオ>... -o <出力>
                                          使う文字だけのフォントを作る
                                          (詳しくは nekomoji subset --help)

オプション:
  --width <全角文字数>   1行の幅(既定 20、1000 まで。縦書きでは1行の高さ)
  --lines <行数>         1ページの最大行数(超えたら自動で改ページ)
  --vertical             縦書きで組む
  --kinsoku <standard|off>
  --no-hanging           句読点をぶら下げない
  --tsume                約物の詰め(端末では半角未満を表せないので既定は切)
  --wao <比>             和欧間のアキ(全角に対する比。端末の既定は 0)
  --justify              両端揃え
  --indent <novel|dialogue|off|1行目[,2行目以降]>
                         地の文の字下げ(全角の文字数)。novel は1字下げ・会話は天付き、
                         dialogue は会話の2行目以降を括弧の後ろに揃える
  --align <start|center|end>
                         [align] の無い段落の揃え(既定 start。縦書きでは end が下)
  --bracket-indent <1行目[,2行目以降]>
                         会話(括弧で始まる段落)の字下げ。1行目 0 で天付き、0.5 で半角下げ
  --speed <文字/秒>      文字送りの速さ(既定 20)
  --auto                 ページごとにオートモードの待ち時間を出す
  --speech               ページごとに読み上げ用のテキスト(ルビは読み、段落は / で区切る)を出す
  --voice-len <ID>=<秒>   ボイスの長さ(オートモードの待ち時間に使う。繰り返し可)
  --blip <ID[,文字数]>    文字送りの音を本文全体で鳴らす(既定は [blip] の範囲だけ。文字数の既定 2)
  --motion <full|reduced|off>
                         動きの強さ(既定 full)。reduced は猫パンチ・爆散なし、off は猫のちょっかいなし
  --set <変数>=<値>      変数を設定(繰り返し可)

猫モード:
  --neko                 猫モードを有効にする(見えない猫のちょっかいと 🐾 のカーソル)
  --seed <数>            乱数のシード(既定 0。--neko を含む)
  --knock <確率>         入力待ちで最後の文字を押して落とす確率(既定 0.15。--neko を含む)
  --burst <確率>         入力待ちでページの文字を爆散させる確率(既定 0.05。--neko を含む)
  --punch <確率>         文字送り中の猫パンチの確率(既定 0.12。--neko を含む)
  --no-walk              見えない猫のちょっかいを切る(カーソルだけ)
  --ascii                カーソルを =^.^= で表示する(絵文字の幅がずれる端末向け)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let Some(cmd) = args.first() else {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    };
    #[cfg(feature = "subset")]
    if cmd == "subset" {
        return subset(&args[1..]);
    }
    if cmd == "check" {
        return check(&args[1..]);
    }
    let (settings, files) = parse_options(&args[1..])?;
    match cmd.as_str() {
        "log" => {
            let src = read(files.first())?;
            match settings.build(&src) {
                Ok(layout) => {
                    print!("{}", log(&layout, &settings));
                    Ok(ExitCode::SUCCESS)
                }
                Err(e) => {
                    eprintln!("{}", format_errors(&src, &e));
                    Ok(ExitCode::from(1))
                }
            }
        }
        "repl" => {
            repl(settings, files.first().map(String::as_str));
            Ok(ExitCode::SUCCESS)
        }
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("知らないコマンド: {other}\n\n{USAGE}")),
    }
}

#[cfg(feature = "subset")]
fn subset(args: &[String]) -> Result<ExitCode, String> {
    use nekomoji_cli::subset::{execute, format_report, Args, USAGE};
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    }
    let a = Args::parse(args).map_err(|e| format!("{e}\n\n{USAGE}"))?;
    let report = execute(&a)?;
    println!("{}", format_report(&a, &report));
    Ok(ExitCode::SUCCESS)
}

/// `check <ファイル>... [--json]`。エラーが1つでもあれば終了コード 1。
///
/// 人が読む形はエラーを標準エラーへ(エラーが無ければ標準出力へ `OK`)、JSON は常に標準出力へ出す。
fn check(args: &[String]) -> Result<ExitCode, String> {
    let json = args.iter().any(|a| a == "--json");
    let paths: Vec<&String> = args.iter().filter(|a| *a != "--json").collect();
    if let Some(bad) = paths.iter().find(|a| a.starts_with("--")) {
        return Err(format!("check のオプションは --json だけです: {bad}"));
    }
    if paths.is_empty() {
        return Err(format!("ファイルを指定してください\n\n{USAGE}"));
    }
    let mut sources = Vec::new();
    for path in &paths {
        sources.push(read(Some(path))?);
    }
    let errors: Vec<Vec<nekomoji::ParseError>> = sources
        .iter()
        .map(|src| parse_all(src).err().unwrap_or_default())
        .collect();
    let checked: Vec<nekomoji_cli::Checked> = paths
        .iter()
        .zip(&sources)
        .zip(&errors)
        .map(|((p, s), e)| (p.as_str(), s.as_str(), e.as_slice()))
        .collect();
    let failed = errors.iter().any(|e| !e.is_empty());
    if json {
        println!("{}", check_json(&checked));
    } else if failed {
        eprintln!("{}", check_text(&checked));
    } else {
        println!("{}", check_text(&checked));
    }
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn read(path: Option<&String>) -> Result<String, String> {
    let path = path.ok_or_else(|| format!("ファイルを指定してください\n\n{USAGE}"))?;
    std::fs::read_to_string(path)
        .map(|s| s.trim_end().to_string())
        .map_err(|e| format!("読み込めません {path}: {e}"))
}

fn parse_options(args: &[String]) -> Result<(Settings, Vec<String>), String> {
    let mut s = Settings::default();
    let mut files = Vec::new();
    let mut it = args.iter();
    let num = |v: Option<&String>, name: &str| -> Result<f32, String> {
        v.and_then(|v| v.parse::<f32>().ok())
            .filter(|v| *v > 0.0)
            .ok_or_else(|| format!("{name} には正の数が必要です"))
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--width" => {
                s.layout.max_width = num(it.next(), "--width")
                    .ok()
                    .filter(|v| *v <= MAX_WIDTH)
                    .ok_or(format!("--width には {MAX_WIDTH} 以下の正の数が必要です"))?
            }
            "--lines" => {
                let v = it
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|v| *v > 0)
                    .ok_or("--lines には 1 以上の整数が必要です")?;
                s.layout.max_lines = Some(v);
            }
            "--speed" => s.reveal.chars_per_sec = num(it.next(), "--speed")?,
            "--motion" => {
                s.reveal.motion = it
                    .next()
                    .and_then(|v| nekomoji_cli::parse_motion(v))
                    .ok_or("--motion には full・reduced・off のどれかが必要です")?;
            }
            "--auto" => s.auto = true,
            "--speech" => s.speech = true,
            "--voice-len" => {
                let kv = it.next().ok_or("--voice-len には <ID>=<秒> が必要です")?;
                let (k, v) = kv
                    .split_once('=')
                    .and_then(|(k, v)| {
                        Some((
                            k,
                            v.parse::<f32>()
                                .ok()
                                .filter(|v| v.is_finite() && *v >= 0.0)?,
                        ))
                    })
                    .ok_or("--voice-len には <ID>=<秒>(0 以上)が必要です")?;
                s.voice_lens.insert(k.to_string(), v);
                s.auto = true;
            }
            "--blip" => {
                s.reveal.blip = it
                    .next()
                    .and_then(|v| nekomoji_cli::parse_blip(v))
                    .ok_or("--blip には ID[,文字数](文字数は 1 以上)か off が必要です")?;
            }
            "--no-hanging" => s.layout.hanging = false,
            "--tsume" => s.layout.punct_squeeze = true,
            "--justify" => s.layout.justify = true,
            "--indent" => {
                let v = it.next().ok_or("--indent には値が必要です")?;
                if !nekomoji_cli::apply_indent(&mut s.layout.indent, v) {
                    return Err(
                        "--indent には novel・dialogue・off か 1行目[,2行目以降](0 以上の数)が必要です"
                            .into(),
                    );
                }
            }
            "--align" => {
                s.layout.align = it
                    .next()
                    .and_then(|v| nekomoji_cli::parse_align(v))
                    .ok_or("--align には start・center・end のどれかが必要です")?;
            }
            "--bracket-indent" => {
                let v = it.next().ok_or("--bracket-indent には値が必要です")?;
                if !nekomoji_cli::apply_bracket_indent(&mut s.layout.indent, v) {
                    return Err(
                        "--bracket-indent には 1行目[,2行目以降](0 以上の数)が必要です".into(),
                    );
                }
            }
            "--wao" => {
                s.layout.latin_space = it
                    .next()
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .ok_or("--wao には 0 以上の数が必要です")?;
            }
            "--vertical" => s.layout.writing = Writing::Vertical,
            "--neko" => {
                s.neko.get_or_insert_with(NekoMode::default);
            }
            "--seed" => {
                let v = it
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .ok_or("--seed には 0 以上の整数が必要です")?;
                s.neko.get_or_insert_with(NekoMode::default).seed = v;
            }
            "--knock" => {
                let v = it
                    .next()
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| (0.0..=1.0).contains(v))
                    .ok_or("--knock には 0〜1 の数が必要です")?;
                s.neko.get_or_insert_with(NekoMode::default).knock_chance = v;
            }
            "--burst" | "--punch" => {
                let v = it
                    .next()
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| (0.0..=1.0).contains(v))
                    .ok_or(format!("{a} には 0〜1 の数が必要です"))?;
                let m = s.neko.get_or_insert_with(NekoMode::default);
                if a == "--burst" {
                    m.burst_chance = v;
                } else {
                    m.punch_chance = v;
                }
            }
            "--no-walk" => s.neko.get_or_insert_with(NekoMode::default).walk = false,
            "--ascii" => s.ascii = true,
            "--kinsoku" => {
                s.layout.kinsoku = match it.next().map(String::as_str) {
                    Some("standard" | "on") => Kinsoku::Standard,
                    Some("off") => Kinsoku::Off,
                    _ => return Err("--kinsoku には standard か off が必要です".into()),
                }
            }
            "--set" => {
                let kv = it.next().ok_or("--set には <変数>=<値> が必要です")?;
                let (k, v) = kv
                    .split_once('=')
                    .ok_or("--set には <変数>=<値> が必要です")?;
                s.vars.insert(k.to_string(), v.to_string());
            }
            f if f.starts_with("--") => return Err(format!("知らないオプション: {f}")),
            f => files.push(f.to_string()),
        }
    }
    Ok((s, files))
}

fn repl(settings: Settings, file: Option<&str>) {
    let mut repl = Repl::new(settings);
    println!("nekomoji REPL(:help でコマンド一覧、:quit で終了)");
    if let Some(f) = file {
        if let Action::Print(s) = repl.handle(&format!(":load {f}")) {
            println!("{s}");
        }
    }
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    loop {
        print!("nekomoji> ");
        let _ = io::stdout().flush();
        let Some(Ok(line)) = lines.next() else {
            println!();
            break;
        };
        match repl.handle(&line) {
            Action::Print(s) => {
                if !s.is_empty() {
                    println!("{s}");
                }
            }
            Action::Quit => break,
            Action::Play => {
                if let Some(layout) = repl.layout() {
                    play(layout, &repl.settings, &mut lines);
                }
            }
        }
    }
}

/// 実時間で再生する。入力待ちでは Enter を待つ。
fn play(
    layout: &Layout,
    settings: &Settings,
    lines: &mut impl Iterator<Item = io::Result<String>>,
) {
    let mut out = io::stdout();
    let mut reveal = Reveal::new(layout, settings.reveal.clone());
    let neko = settings.neko(layout);
    let frame = Duration::from_millis(33);
    loop {
        let page = reveal.page();
        println!("[ページ {}/{}]", page + 1, layout.pages.len());
        let mut drawn = 0usize;
        let mut last = Instant::now();
        let mut notes: Vec<String> = Vec::new();
        loop {
            let now = Instant::now();
            let dt = now.duration_since(last).as_secs_f32();
            last = now;
            for e in reveal.advance(dt) {
                match e {
                    TextEvent::WaitInput | TextEvent::WaitClick | TextEvent::Finished => {}
                    other => notes.push(format!("♪ {other}")),
                }
            }
            let view = neko
                .as_ref()
                .map(|n| CatView::new(n, layout, &reveal, settings.ascii))
                .unwrap_or_default();
            let mut rows = page_rows_with(layout, page, reveal.visible_count(), &view);
            rows.push(notes.join("  "));
            if drawn > 0 {
                let _ = write!(out, "\x1b[{drawn}A");
            }
            for row in &rows {
                let _ = writeln!(out, "\x1b[2K{row}");
            }
            let _ = out.flush();
            drawn = rows.len();
            if reveal.wait_kind() == Some(WaitKind::Click) {
                break;
            }
            if reveal.is_waiting() || reveal.is_finished() {
                // 猫が最後の文字へ着いて、飛ばした文字が戻り終えるまでは描き続ける
                let settled = neko
                    .as_ref()
                    .map_or(0.0, |n| n.quiet_time(layout, &reveal, page));
                if reveal.skipped() || reveal.time() >= settled {
                    break;
                }
            }
            std::thread::sleep(frame);
        }
        if reveal.is_finished() {
            println!("(終了)");
            return;
        }
        if reveal.wait_kind() == Some(WaitKind::Click) {
            print!("▽ Enter で続き(q で中断) ");
        } else {
            print!("▼ Enter で次のページ(q で中断) ");
        }
        let _ = out.flush();
        match lines.next() {
            Some(Ok(l)) if l.trim() == "q" => return,
            Some(Ok(_)) => {
                reveal.proceed();
            }
            _ => return,
        }
    }
}
