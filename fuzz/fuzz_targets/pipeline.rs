//! 解析 → レイアウト → 文字送り → 猫モードを通す。
//!
//! 先頭の 4 バイトで設定を選び、残りをマークアップとして読む。
//! `cargo +nightly fuzz run pipeline`
#![no_main]

use libfuzzer_sys::fuzz_target;
use nekomoji::{
    parse, CellMetrics, Kinsoku, Layout, LayoutOptions, Neko, NekoMode, Reveal, RevealOptions,
    Vars, Writing,
};

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    let (head, rest) = data.split_at(4);
    let Ok(src) = std::str::from_utf8(rest) else {
        return;
    };
    let Ok(script) = parse(src) else {
        return;
    };
    let opts = LayoutOptions::default()
        .writing(if head[0] & 1 == 0 {
            Writing::Horizontal
        } else {
            Writing::Vertical
        })
        .max_width(f32::from(head[1] % 40) * 0.5 + 0.1)
        .kinsoku(if head[0] & 2 == 0 {
            Kinsoku::Standard
        } else {
            Kinsoku::Off
        })
        .hanging(head[0] & 4 == 0)
        .max_lines(match head[2] % 4 {
            0 => None,
            n => Some(usize::from(n)),
        })
        .punct_squeeze(head[0] & 8 == 0)
        .latin_space(f32::from(head[0] >> 4) * 0.1)
        .justify(head[2] & 4 == 0);
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let neko = Neko::new(
        &layout,
        NekoMode::default()
            .seed(u64::from(head[3]))
            .knock_chance(0.5),
    );
    let mut reveal = Reveal::new(&layout, RevealOptions::default());
    for i in 0..200u32 {
        match (i + u32::from(head[3])) % 7 {
            0 => {
                reveal.skip();
            }
            1 => {
                reveal.proceed();
            }
            2 => {
                let mut other = Reveal::new(&layout, RevealOptions::default());
                other.restore(reveal.state());
            }
            _ => {
                reveal.advance(0.07);
            }
        }
        let _ = reveal.glyphs(&layout);
        let _ = reveal.decorations(&layout);
        let _ = neko.cat(&layout, &reveal);
        let _ = neko.glyphs(&layout, &reveal);
        if reveal.is_finished() {
            break;
        }
    }
    for page in &layout.pages {
        let _ = page.text();
    }
});
