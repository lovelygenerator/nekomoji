//! examples(`viewer`・`snapshot`)で使うフォントを決める。
//!
//! 見た目を環境で変えないために、既定は**リポジトリの `assets/fonts/` に置いた Zen Maru Gothic**
//! (SIL OFL 1.1)。無ければ初回の `cargo run` で公式のリポジトリの決まったコミットから取ってくる
//! (`curl` を使う。Windows 10 以降は標準で入っている)。`assets/fonts/` は git に入れないので、
//! リポジトリにも crate にもフォントは入らない(フォントは再配布しない)。
//!
//! 決める順:
//! 1. 引数で渡したフォント
//! 2. 環境変数 `NEKOMOJI_FONT`(パス)
//! 3. `assets/fonts/ZenMaruGothic-Regular.ttf`(無ければ取ってくる。`NEKOMOJI_NO_FETCH=1` で取らない)
//! 4. OS に入っている日本語フォント(見た目が環境で変わるので警告を出す)
//!
//! 代わりのフォントは `--fallback <フォント>`(繰り返し可)と環境変数 `NEKOMOJI_FALLBACK`
//! (パスの並び。区切りは OS のパスの区切り)で、本命に字形が無い文字に使う。`system` と書くと
//! OS の日本語フォントを探して使う。ttc の番号は `パス#番号`(既定 0)。
//!
//! `assets/fonts/` の場所はコンパイル時の `CARGO_MANIFEST_DIR` から決めるので、どこから
//! `cargo run` しても同じ場所を見る。探す処理は環境ごとに違うので、ライブラリには入れない。

use std::path::{Path, PathBuf};
use std::process::Command;

/// 既定のフォントを取ってくる Zen Maru Gothic のリポジトリのコミット。固定して中身が変わらないようにする
const ZEN_COMMIT: &str = "553c872b216d1290e2902a466edcdc9682f0df6a";
const ZEN_FILE: &str = "ZenMaruGothic-Regular.ttf";

/// よくある日本語フォントの場所と、ttc の中の日本語の番号。上から順に試す。
const CANDIDATES: &[(&str, u32)] = &[
    // Windows(%WINDIR% が違うときは下で組み立て直す)
    (r"C:\Windows\Fonts\YuGothM.ttc", 0),
    (r"C:\Windows\Fonts\YuGothR.ttc", 0),
    (r"C:\Windows\Fonts\meiryo.ttc", 0),
    (r"C:\Windows\Fonts\BIZ-UDGothicR.ttc", 0),
    (r"C:\Windows\Fonts\msgothic.ttc", 0),
    // macOS
    ("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", 0),
    ("/System/Library/Fonts/ヒラギノ丸ゴ ProN W4.ttc", 0),
    ("/Library/Fonts/Osaka.ttf", 0),
    // Linux(Noto Sans CJK の ttc は 0 番が日本語)
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0),
    (
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        0,
    ),
    (
        "/usr/share/fonts/opentype/noto/NotoSansCJKjp-Regular.otf",
        0,
    ),
    ("/usr/share/fonts/truetype/fonts-japanese-gothic.ttf", 0),
    ("/usr/share/fonts/opentype/ipaexfont-gothic/ipaexg.ttf", 0),
    ("/usr/share/fonts/truetype/takao-gothic/TakaoPGothic.ttf", 0),
];

/// 決まったフォント。
pub struct Font {
    pub path: PathBuf,
    pub index: u32,
    pub data: Vec<u8>,
}

/// フォントのファイルとして扱う拡張子か。
pub fn is_font_path(p: &str) -> bool {
    let lower = p.to_ascii_lowercase();
    [".ttf", ".otf", ".ttc", ".otc"]
        .iter()
        .any(|e| lower.ends_with(e))
}

/// `NEKOMOJI_FONT_INDEX`(ttc の番号)。
pub fn font_index_env() -> Option<u32> {
    std::env::var("NEKOMOJI_FONT_INDEX").ok()?.parse().ok()
}

/// リポジトリの `assets/fonts/`。どこから実行しても同じ場所になる。
pub fn assets_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR は crates/nekomoji-wgpu。その2つ上がリポジトリの根
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
        .join("fonts");
    match dir.canonicalize() {
        Ok(p) => strip_verbatim(p),
        Err(_) => dir,
    }
}

/// Windows の `canonicalize` が付ける `\\?\` を外す(`curl` などほかのプログラムに渡せるように)。
/// ネットワーク上の `\\?\UNC\` はそのまま残す。
fn strip_verbatim(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with(r"UNC\") => PathBuf::from(rest),
        _ => p,
    }
}

/// 使うフォントを決めて読む(決める順はこのモジュールの説明)。`explicit` は引数で渡したパス、
/// `index` は `--index` か `NEKOMOJI_FONT_INDEX` の ttc の番号。どれも無ければエラー文。
pub fn resolve(explicit: Option<&str>, index: Option<u32>) -> Result<Font, String> {
    let explicit = explicit
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("NEKOMOJI_FONT").map(PathBuf::from));
    if let Some(path) = explicit {
        let data = std::fs::read(&path)
            .map_err(|e| format!("フォントを読めない {}: {e}", path.display()))?;
        return Ok(Font {
            path,
            index: index.unwrap_or(0),
            data,
        });
    }
    match bundled() {
        Ok(path) => {
            let data = std::fs::read(&path)
                .map_err(|e| format!("フォントを読めない {}: {e}", path.display()))?;
            return Ok(Font {
                path,
                index: index.unwrap_or(0),
                data,
            });
        }
        Err(e) => eprintln!("{e}"),
    }
    let found = find_system_font()?;
    eprintln!(
        "警告: OS のフォント {} を使います。見た目が環境で変わります",
        found.path.display()
    );
    Ok(Font {
        index: index.unwrap_or(found.index),
        ..found
    })
}

/// 代わりのフォントを決めて読む。`cli` は `--fallback` で渡したもの(先に探す)。
/// そのあとに `NEKOMOJI_FALLBACK` の並びを足す。`system` は OS の日本語フォント。
pub fn resolve_fallbacks(cli: &[String]) -> Result<Vec<Font>, String> {
    let mut specs: Vec<String> = cli.to_vec();
    if let Some(v) = std::env::var_os("NEKOMOJI_FALLBACK") {
        specs.extend(
            std::env::split_paths(&v)
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.to_string_lossy().into_owned()),
        );
    }
    specs.iter().map(|s| fallback(s)).collect()
}

fn fallback(spec: &str) -> Result<Font, String> {
    if spec == "system" {
        return find_system_font();
    }
    let (path, index) = match spec.rsplit_once('#') {
        Some((p, i)) if i.parse::<u32>().is_ok() => (p, i.parse().unwrap_or(0)),
        _ => (spec, 0),
    };
    let path = PathBuf::from(path);
    let data =
        std::fs::read(&path).map_err(|e| format!("フォントを読めない {}: {e}", path.display()))?;
    Ok(Font { path, index, data })
}

/// 本命と代わりのフォントの連鎖を作る。`proportional` で仮名をプロポーショナルに詰める。
pub fn font_set(
    main: &Font,
    fallbacks: &[Font],
    px: f32,
    proportional: bool,
) -> Result<nekomoji_wgpu::FontSet, String> {
    let mut set = nekomoji_wgpu::FontSet::from_bytes(&main.data, main.index, px)
        .map_err(|e| format!("{}: {e}", main.path.display()))?;
    for f in fallbacks {
        set = set
            .with_fallback(&f.data, f.index)
            .map_err(|e| format!("{}: {e}", f.path.display()))?;
    }
    if proportional {
        if !set.has_proportional() {
            eprintln!("このフォントは仮名の詰め(palt・vpal)を持たないので、詰めても変わらない");
        }
        set = set.with_proportional();
    }
    Ok(set)
}

/// 決まったフォントを端末に出す。
pub fn report(main: &Font, fallbacks: &[Font]) {
    eprintln!("フォント: {}(番号 {})", main.path.display(), main.index);
    for f in fallbacks {
        eprintln!("代わりのフォント: {}(番号 {})", f.path.display(), f.index);
    }
}

/// `assets/fonts/` の Zen Maru Gothic。無ければ取ってくる。
fn bundled() -> Result<PathBuf, String> {
    let dir = assets_dir();
    let path = dir.join(ZEN_FILE);
    if path.is_file() {
        return Ok(path);
    }
    if std::env::var("NEKOMOJI_NO_FETCH").is_ok_and(|v| v == "1") {
        return Err(format!(
            "{} が無い(NEKOMOJI_NO_FETCH=1 なので取得しない)",
            path.display()
        ));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{} を作れない: {e}", dir.display()))?;
    let base = format!("https://raw.githubusercontent.com/googlefonts/zen-marugothic/{ZEN_COMMIT}");
    let url = format!("{base}/fonts/ttf/{ZEN_FILE}");
    eprintln!(
        "既定のフォント Zen Maru Gothic(SIL OFL 1.1、約 4 MB)を取得します\n  {url}\n  → {}",
        path.display()
    );
    // 途中で切れても壊れたファイルが残らないよう、別名に落としてから確かめて名前を変える
    let part = dir.join(format!("{ZEN_FILE}.part"));
    fetch(&url, &part)?;
    let data = std::fs::read(&part).map_err(|e| format!("取得したフォントを読めない: {e}"))?;
    if !has_japanese(&data, 0) {
        let _ = std::fs::remove_file(&part);
        return Err("取得したファイルが日本語フォントとして読めない".into());
    }
    std::fs::rename(&part, &path).map_err(|e| format!("{} に置けない: {e}", path.display()))?;
    // ライセンス文も一緒に置く(失敗してもフォントは使える)
    if fetch(&format!("{base}/OFL.txt"), &dir.join("OFL.txt")).is_err() {
        eprintln!("警告: OFL.txt を取れなかった");
    }
    Ok(path)
}

/// `curl` で `url` を `to` に落とす。
fn fetch(url: &str, to: &Path) -> Result<(), String> {
    let status = Command::new("curl")
        .args(["-fsSL", "--retry", "2", "-o"])
        .arg(to)
        .arg(url)
        .status()
        .map_err(|e| format!("curl を実行できない({e})"))?;
    if status.success() {
        Ok(())
    } else {
        let _ = std::fs::remove_file(to);
        Err(format!("取得に失敗した({status}): {url}"))
    }
}

/// OS に入っている日本語フォントを探す。
fn find_system_font() -> Result<Font, String> {
    let mut list: Vec<(PathBuf, u32)> = fc_match();
    if let Ok(windir) = std::env::var("WINDIR") {
        for (p, i) in CANDIDATES {
            if let Some(name) = p.strip_prefix(r"C:\Windows\") {
                list.push((Path::new(&windir).join(name), *i));
            }
        }
    }
    list.extend(CANDIDATES.iter().map(|(p, i)| (PathBuf::from(p), *i)));
    for (path, index) in list {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        if has_japanese(&data, index) {
            return Ok(Font { path, index, data });
        }
    }
    Err(
        "日本語フォントが見つからない。フォントのパスを引数か NEKOMOJI_FONT で渡してください"
            .into(),
    )
}

/// fontconfig に日本語のゴシックを尋ねる。`fc-match` が無ければ空。
fn fc_match() -> Vec<(PathBuf, u32)> {
    let Ok(out) = Command::new("fc-match")
        .args(["-f", "%{file}\t%{index}", "sans-serif:lang=ja"])
        .output()
    else {
        return Vec::new();
    };
    let s = String::from_utf8_lossy(&out.stdout);
    let mut it = s.trim().split('\t');
    match (it.next(), it.next()) {
        (Some(f), Some(i)) if !f.is_empty() => {
            vec![(PathBuf::from(f), i.trim().parse().unwrap_or(0))]
        }
        _ => Vec::new(),
    }
}

/// 日本語の字形(仮名と漢字)を持つか。表を引くだけなので、大きな ttc でもすぐ終わる。
fn has_japanese(data: &[u8], index: u32) -> bool {
    ttf_parser::Face::parse(data, index)
        .map(|f| f.glyph_index('あ').is_some() && f.glyph_index('漢').is_some())
        .unwrap_or(false)
}
