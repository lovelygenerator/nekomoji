//! タグ(`[名前 引数...]`)の解釈。範囲タグはスタイルを積み、単独タグは要素を足す。

use super::{err, Open, Parser, Pos};
use crate::markup::error::{ErrorKind, ParseError};
use crate::markup::script::{Item, TextEvent};
use crate::markup::style::{
    is_color, Align, Baseline, Effect, GlowKind, Outline, Shadow, EMPHASIS_MARK, OUTLINE_WIDTH,
    SHADOW_OFFSET,
};

/// `[wait 秒]` の上限(秒)。
pub(crate) const MAX_WAIT: f32 = 3600.0;
/// `[speed 倍率]` の下限。
pub(crate) const MIN_SPEED: f32 = 0.01;
/// `[speed 倍率]` の上限。
pub(crate) const MAX_SPEED: f32 = 100.0;
/// 名前・ID(`[mark]` `[icon]` `[var]` `[blip]` `[voice]` `[se]` `[event]`)の最大の文字数。
pub(crate) const MAX_ID_CHARS: usize = 64;
/// `[event 名前 引数...]` の引数の最大の数。
pub(crate) const MAX_EVENT_ARGS: usize = 16;
/// `[size 倍率]` の上限。
pub(crate) const MAX_SIZE: f32 = 10.0;
/// `[outline 色? 太さ?]` の太さの上限(字の大きさに対する比)。
pub(crate) const MAX_OUTLINE: f32 = 0.5;
/// `[icon id 幅?]` の幅の上限(全角の文字数)。
pub(crate) const MAX_ICON: f32 = 10.0;
/// `[shake]` `[wave]` `[neko hop]` の強さの上限。
pub(crate) const MAX_EFFECT_STRENGTH: f32 = 10.0;
/// `[shadow 色? x? y?]` のずれの絶対値の上限(字の大きさに対する比)。
pub(crate) const MAX_SHADOW_OFFSET: f32 = 10.0;

impl Parser {
    pub(super) fn tag(&mut self, body: &str, pos: Pos) -> Result<(), ParseError> {
        // `[` の直後(閉じタグでは `/` の直後)の空白は書けない
        let after = body.strip_prefix('/').unwrap_or(body);
        if after.starts_with(char::is_whitespace) {
            return Err(err(pos, ErrorKind::SpaceAfterBracket));
        }
        let body = body.trim();
        if let Some(name) = body.strip_prefix('/') {
            return self.close(name.trim(), pos);
        }
        let mut words = body.split_whitespace();
        let name = words.next().unwrap_or("").to_string();
        let args: Vec<String> = words.map(str::to_string).collect();
        let first = |args: &[String]| -> Result<String, ParseError> {
            args.first()
                .cloned()
                .ok_or_else(|| err(pos, ErrorKind::MissingArg(name.clone())))
        };
        let num = |arg: &str, positive: bool| -> Result<f32, ParseError> {
            match arg.parse::<f32>() {
                Ok(v) if v.is_finite() && v >= 0.0 && (!positive || v > 0.0) => Ok(v),
                _ => Err(err(
                    pos,
                    ErrorKind::BadArg {
                        tag: name.clone(),
                        arg: arg.to_string(),
                    },
                )),
            }
        };
        let bad = |arg: &str| {
            err(
                pos,
                ErrorKind::BadArg {
                    tag: name.clone(),
                    arg: arg.to_string(),
                },
            )
        };
        let opt_num = |args: &[String], default: f32| -> Result<f32, ParseError> {
            match args.first() {
                Some(a) => num(a, false),
                None => Ok(default),
            }
        };
        // 0 より大きい数(省略時は既定)
        let opt_pos = |args: &[String], default: f32| -> Result<f32, ParseError> {
            match args.first() {
                Some(a) => num(a, true),
                None => Ok(default),
            }
        };
        // 名前・ID: 1〜MAX_ID_CHARS 文字で `[` を含まない(空白は引数の区切りなので元から含まない)
        let id_ok = |id: &str| -> Result<(), ParseError> {
            if id.chars().count() > MAX_ID_CHARS || id.contains('[') {
                Err(err(
                    pos,
                    ErrorKind::BadArg {
                        tag: name.clone(),
                        arg: id.to_string(),
                    },
                ))
            } else {
                Ok(())
            }
        };
        // 色: 色名か `#rgb` `#rrggbb`
        let color_ok = |c: &str| -> Result<(), ParseError> {
            if is_color(c) {
                Ok(())
            } else {
                Err(err(
                    pos,
                    ErrorKind::BadArg {
                        tag: name.clone(),
                        arg: c.to_string(),
                    },
                ))
            }
        };
        let mut style = self.current_style();
        let no_args = |args: &[String]| -> Result<(), ParseError> {
            match args.first() {
                Some(a) => Err(err(
                    pos,
                    ErrorKind::BadArg {
                        tag: name.clone(),
                        arg: a.clone(),
                    },
                )),
                None => Ok(()),
            }
        };
        // 引数の数が決まっているタグは、余った引数をエラーにする(黙って捨てない)
        let max_args = match name.as_str() {
            "b" | "p" | "br" => Some(0),
            "color" | "size" | "speed" | "fade" | "wait" | "voice" | "se" | "var" => Some(1),
            "neko" => Some(2),
            _ => None,
        };
        if let Some(extra) = max_args.and_then(|m| args.get(m)) {
            return Err(err(
                pos,
                ErrorKind::BadArg {
                    tag: name,
                    arg: extra.clone(),
                },
            ));
        }
        match name.as_str() {
            "ruby" => {
                if style.baseline != Baseline::Normal {
                    return Err(err(pos, ErrorKind::NestedInShift));
                }
                // 読みは空白を含まない1語(空白のある読みは短い書き方 `｜親文字《よ み》` で書く)
                if let Some(extra) = args.get(1) {
                    return Err(bad(extra));
                }
                let Some(reading) = args.first().cloned() else {
                    return Err(err(pos, ErrorKind::MissingArg(name)));
                };
                return self.ruby(reading, pos);
            }
            "tcy" => {
                no_args(&args)?;
                if style.baseline != Baseline::Normal {
                    return Err(err(pos, ErrorKind::NestedInShift));
                }
                return self.raw_range("tcy", pos).map(|(text, source)| {
                    let style = self.intern(style);
                    self.push_item(Item::Tcy {
                        text,
                        style,
                        source,
                    });
                });
            }
            "color" => {
                let c = first(&args)?;
                color_ok(&c)?;
                style.color = Some(c);
            }
            "size" => {
                // 大きすぎる字は描画側で字形を作れない(メモリが足りない)ので受け付けない
                let arg = first(&args)?;
                let v = num(&arg, true)?;
                if v > MAX_SIZE {
                    return Err(bad(&arg));
                }
                style.size = v;
            }
            "b" => style.bold = true,
            "blip" => {
                // [blip 音?]: 文字送りの音。引数なしは設定の音、off は鳴らさない
                if let Some(extra) = args.get(1) {
                    return Err(err(
                        pos,
                        ErrorKind::BadArg {
                            tag: name,
                            arg: extra.clone(),
                        },
                    ));
                }
                if let Some(id) = args.first() {
                    id_ok(id)?;
                }
                style.blip = Some(args.first().cloned().unwrap_or_default());
            }
            "align" => {
                // [align 揃え]: 段落ごとの揃え
                let a = first(&args)?;
                let bad = |arg: &String| {
                    err(
                        pos,
                        ErrorKind::BadArg {
                            tag: name.clone(),
                            arg: arg.clone(),
                        },
                    )
                };
                if let Some(extra) = args.get(1) {
                    return Err(bad(extra));
                }
                // 入れ子にはできない(揃えは段落ごとに1つ)
                if style.align.is_some() {
                    return Err(bad(&a));
                }
                style.align = Some(Align::from_name(&a).ok_or_else(|| bad(&a))?);
                if !self.at_paragraph_start() {
                    return Err(err(pos, ErrorKind::AlignMidParagraph));
                }
            }
            "mark" => {
                // [mark id]: 名前付きの範囲。ID は空白を含まない1語
                if let Some(extra) = args.get(1) {
                    return Err(err(
                        pos,
                        ErrorKind::BadArg {
                            tag: name,
                            arg: extra.clone(),
                        },
                    ));
                }
                let id = first(&args)?;
                id_ok(&id)?;
                style.marks.push(id);
            }
            "sup" | "sub" => {
                no_args(&args)?;
                if style.baseline != Baseline::Normal {
                    return Err(err(pos, ErrorKind::NestedInShift));
                }
                style.baseline = if name == "sup" {
                    Baseline::Sup
                } else {
                    Baseline::Sub
                };
            }
            "s" => {
                no_args(&args)?;
                style.strike = true;
            }
            "u" => {
                no_args(&args)?;
                style.underline = true;
            }
            "em" => {
                let mark = match args.as_slice() {
                    [] => EMPHASIS_MARK,
                    [a] if a.chars().count() == 1 && !a.starts_with(char::is_control) => {
                        a.chars().next().unwrap_or(EMPHASIS_MARK)
                    }
                    // 余った引数があれば、その引数を誤りとして示す
                    [_, a, ..] | [a] => {
                        return Err(err(
                            pos,
                            ErrorKind::BadArg {
                                tag: name,
                                arg: a.clone(),
                            },
                        ))
                    }
                };
                style.emphasis = Some(mark);
            }
            "speed" => {
                // 遅すぎる倍率は時刻が桁あふれするので受け付けない
                let arg = first(&args)?;
                let v = num(&arg, true)?;
                if !(MIN_SPEED..=MAX_SPEED).contains(&v) {
                    return Err(bad(&arg));
                }
                style.speed = v;
            }
            "shake" | "wave" => {
                // [shake 強さ? 秒?]: 秒は文字が出てから弱まって止まるまで(0 より大きい)
                if let Some(extra) = args.get(2) {
                    return Err(err(
                        pos,
                        ErrorKind::BadArg {
                            tag: name,
                            arg: extra.clone(),
                        },
                    ));
                }
                let strength = opt_pos(&args, 1.0)?;
                if strength > MAX_EFFECT_STRENGTH {
                    return Err(bad(&args[0]));
                }
                let decay = args.get(1).map(|a| num(a, true)).transpose()?;
                style.effects.push(if name == "shake" {
                    Effect::Shake { strength, decay }
                } else {
                    Effect::Wave { strength, decay }
                });
            }
            "fade" => style.effects.push(Effect::Fade(opt_num(&args, 0.3)?)),
            "glow" => {
                // [glow 種類? 強さ? 秒?]。種類を省くと in
                let (kind, rest) = match args.first().map(String::as_str) {
                    Some("in") => (GlowKind::In, &args[1..]),
                    Some("breathe") => (GlowKind::Breathe, &args[1..]),
                    Some("flicker") => (GlowKind::Flicker, &args[1..]),
                    Some(a) if a.parse::<f32>().is_err() => {
                        return Err(err(
                            pos,
                            ErrorKind::BadArg {
                                tag: name,
                                arg: a.to_string(),
                            },
                        ))
                    }
                    _ => (GlowKind::In, &args[..]),
                };
                if let Some(extra) = rest.get(2) {
                    return Err(err(
                        pos,
                        ErrorKind::BadArg {
                            tag: name,
                            arg: extra.clone(),
                        },
                    ));
                }
                let strength = opt_pos(rest, 1.0)?;
                let secs = match rest.get(1) {
                    Some(a) => num(a, true)?,
                    None => kind.default_secs(),
                };
                style.effects.push(Effect::Glow {
                    kind,
                    strength,
                    secs,
                });
            }
            "outline" | "shadow" => {
                // [outline 色? 太さ?] [shadow 色? x? y?]。数として読めない最初の引数は色
                let (color, rest) = match args.first() {
                    Some(a) if a.parse::<f32>().is_err() => (a.clone(), &args[1..]),
                    _ => ("black".to_string(), &args[..]),
                };
                color_ok(&color)?;
                let max = if name == "outline" { 1 } else { 2 };
                if let Some(extra) = rest.get(max) {
                    return Err(err(
                        pos,
                        ErrorKind::BadArg {
                            tag: name,
                            arg: extra.clone(),
                        },
                    ));
                }
                if name == "outline" {
                    let width = match rest.first() {
                        Some(a) => {
                            let v = num(a, true)?;
                            if v > MAX_OUTLINE {
                                return Err(bad(a));
                            }
                            v
                        }
                        None => OUTLINE_WIDTH,
                    };
                    style.outline = Some(Outline { color, width });
                } else {
                    // ずれは負でもよい(左・上へ)
                    let signed = |a: &String| -> Result<f32, ParseError> {
                        match a.parse::<f32>() {
                            Ok(v) if v.is_finite() && v.abs() <= MAX_SHADOW_OFFSET => Ok(v),
                            _ => Err(err(
                                pos,
                                ErrorKind::BadArg {
                                    tag: name.clone(),
                                    arg: a.clone(),
                                },
                            )),
                        }
                    };
                    let x = rest.first().map(signed).transpose()?;
                    let y = rest.get(1).map(signed).transpose()?;
                    style.shadow = Some(Shadow {
                        color,
                        x: x.unwrap_or(SHADOW_OFFSET),
                        y: y.unwrap_or(SHADOW_OFFSET),
                    });
                }
            }
            "neko" => {
                let kind = first(&args)?;
                match kind.as_str() {
                    "hop" => {
                        let strength = opt_pos(&args[1..], 1.0)?;
                        if strength > MAX_EFFECT_STRENGTH {
                            return Err(bad(&args[1]));
                        }
                        style.effects.push(Effect::NekoHop(strength));
                    }
                    _ => {
                        return Err(err(
                            pos,
                            ErrorKind::BadArg {
                                tag: name,
                                arg: kind,
                            },
                        ))
                    }
                }
            }
            "wait" => {
                // 長すぎる待機は時刻が桁あふれするので受け付けない
                let arg = first(&args)?;
                let secs = num(&arg, false)?;
                if secs > MAX_WAIT {
                    return Err(bad(&arg));
                }
                self.push_item(Item::Wait(secs));
                return Ok(());
            }
            "l" => {
                no_args(&args)?;
                self.push_item(Item::Click);
                return Ok(());
            }
            "icon" => {
                // [icon id 幅?]: 文中のアイコン。幅の既定は全角1文字
                let id = first(&args)?;
                id_ok(&id)?;
                if let Some(extra) = args.get(2) {
                    return Err(err(
                        pos,
                        ErrorKind::BadArg {
                            tag: name,
                            arg: extra.clone(),
                        },
                    ));
                }
                let width = match args.get(1) {
                    Some(a) => {
                        let v = num(a, true)?;
                        if v > MAX_ICON {
                            return Err(bad(a));
                        }
                        v
                    }
                    None => 1.0,
                };
                let style = self.intern(style);
                let source = pos.byte..self.chars[self.i - 1].1.byte + 1;
                self.push_item(Item::Icon {
                    id,
                    width,
                    style,
                    source,
                });
                return Ok(());
            }
            "p" => {
                self.flush();
                // 改ページの直前の改行(`[br]` も)1つは、ページの最後の空行にしない
                // (`文。` の次の行に `[p]` を書く書き方で空行ができないように。待機・イベントは飛ばして見る)
                if let Some(k) = self
                    .script
                    .items
                    .iter()
                    .rposition(|it| !matches!(it, Item::Wait(_) | Item::Event(_) | Item::Click))
                {
                    if matches!(self.script.items[k], Item::Break) {
                        self.script.items.remove(k);
                    }
                }
                self.push_item(Item::Page);
                // 行末の [p] の後の改行は、次のページの空行にしない
                if self.peek(0) == Some('\n') {
                    self.i += 1;
                }
                return Ok(());
            }
            "br" => {
                self.push_item(Item::Break);
                return Ok(());
            }
            "voice" => {
                let id = first(&args)?;
                id_ok(&id)?;
                self.push_item(Item::Event(TextEvent::Voice(id)));
                return Ok(());
            }
            "se" => {
                let id = first(&args)?;
                id_ok(&id)?;
                self.push_item(Item::Event(TextEvent::Se(id)));
                return Ok(());
            }
            "var" => {
                let var = first(&args)?;
                id_ok(&var)?;
                let style = self.current_id();
                let source = pos.byte..self.chars[self.i - 1].1.byte + 1;
                self.push_item(Item::Var {
                    name: var,
                    style,
                    source,
                });
                return Ok(());
            }
            "event" => {
                let ev = first(&args)?;
                id_ok(&ev)?;
                if let Some(extra) = args.get(MAX_EVENT_ARGS + 1) {
                    return Err(bad(extra));
                }
                self.push_item(Item::Event(TextEvent::Custom {
                    name: ev,
                    args: args[1..].to_vec(),
                }));
                return Ok(());
            }
            _ => return Err(err(pos, ErrorKind::UnknownTag(name))),
        }
        self.stack.push(Open {
            name,
            style,
            id: None,
            pos,
        });
        Ok(())
    }
}
