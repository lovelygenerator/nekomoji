//! フォントのバイト列の処理: ttc から1つ取り出す・必要な文字だけ残す(TrueType は fontcull、CFF は allsorts)。

use std::collections::BTreeSet;

/// 文字を残してサブセットにする。戻り値の2つ目は、元のフォントに字形が無かった文字。
pub fn subset_font(data: &[u8], chars: &BTreeSet<char>) -> Result<(Vec<u8>, Vec<char>), String> {
    // CFF 形式(Noto Sans CJK など .otf の多く)は fontcull が字形データを絞れないので、
    // allsorts で絞る。
    if data.get(0..4) == Some(b"OTTO") {
        return subset_cff(data, chars);
    }
    let font = fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
        .map_err(|e| format!("フォントを解釈できません: {e}"))?;
    let (present, missing): (Vec<char>, Vec<char>) = chars
        .iter()
        .partition(|&&c| font.lookup_glyph_index(c) != 0);
    let set: std::collections::HashSet<char> = present.into_iter().collect();
    let out = fontcull::subset_font_data(data, &set, &[])
        .map_err(|e| format!("サブセット化に失敗しました: {e}"))?;
    Ok((out, missing))
}

/// CFF 形式のフォントをサブセットにする(allsorts)。
///
/// 残る表は最小限(CFF・cmap・hmtx など)。GSUB/GPOS(`vert` などの置き換え)は落ちる。
fn subset_cff(data: &[u8], chars: &BTreeSet<char>) -> Result<(Vec<u8>, Vec<char>), String> {
    use allsorts::binary::read::ReadScope;
    use allsorts::font::{Font, MatchingPresentation};
    use allsorts::font_data::FontData;
    use allsorts::subset::{subset, CmapTarget, SubsetProfile};

    let err = |e: &dyn std::fmt::Display| format!("CFF フォントを処理できません: {e}");
    let scope = ReadScope::new(data);
    let font_data = scope.read::<FontData<'_>>().map_err(|e| err(&e))?;
    let provider = font_data.table_provider(0).map_err(|e| err(&e))?;
    let mut font = Font::new(provider).map_err(|e| err(&e))?;
    let mut gids: Vec<u16> = vec![0]; // .notdef は必須
    let mut missing = Vec::new();
    for &c in chars {
        let (gid, _) = font.lookup_glyph_index(c, MatchingPresentation::NotRequired, None);
        if gid == 0 {
            missing.push(c);
        } else {
            gids.push(gid);
        }
    }
    gids.sort_unstable();
    gids.dedup();
    let out = subset(
        &font.font_table_provider,
        &gids,
        &SubsetProfile::Minimal,
        CmapTarget::Unicode,
    )
    .map_err(|e| format!("サブセット化に失敗しました: {e}"))?;
    Ok((out, missing))
}

fn be16(data: &[u8], pos: usize) -> Result<u16, String> {
    data.get(pos..pos + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or_else(|| "フォントファイルが壊れています".to_string())
}

fn be32(data: &[u8], pos: usize) -> Result<u32, String> {
    data.get(pos..pos + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| "フォントファイルが壊れています".to_string())
}

/// ttc から `index` 番目のフォントを、単独の ttf/otf として取り出す。ttc でなければそのまま返す。
pub fn extract_font(data: &[u8], index: u32) -> Result<Vec<u8>, String> {
    if data.get(0..4) != Some(b"ttcf") {
        return if index == 0 {
            Ok(data.to_vec())
        } else {
            Err("--index は ttc のときだけ指定できます".into())
        };
    }
    let count = be32(data, 8)?;
    if index >= count {
        return Err(format!(
            "--index {index} は範囲外です(この ttc には {count} 個のフォントがあります)"
        ));
    }
    let off = be32(data, 12 + 4 * index as usize)? as usize;
    let num_tables = be16(data, off + 4)? as usize;
    let dir_len = 12 + 16 * num_tables;
    let mut out = data
        .get(off..off + 12)
        .ok_or("フォントファイルが壊れています")?
        .to_vec();
    out.resize(dir_len, 0);
    for i in 0..num_tables {
        let rec = off + 12 + 16 * i;
        let tag = data
            .get(rec..rec + 4)
            .ok_or("フォントファイルが壊れています")?;
        let checksum = be32(data, rec + 4)?;
        let table_off = be32(data, rec + 8)? as usize;
        let len = be32(data, rec + 12)? as usize;
        let table = data
            .get(table_off..table_off + len)
            .ok_or("フォントファイルが壊れています")?;
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let new_off = out.len() as u32;
        out.extend_from_slice(table);
        let o = 12 + 16 * i;
        out[o..o + 4].copy_from_slice(tag);
        out[o + 4..o + 8].copy_from_slice(&checksum.to_be_bytes());
        out[o + 8..o + 12].copy_from_slice(&new_off.to_be_bytes());
        out[o + 12..o + 16].copy_from_slice(&(len as u32).to_be_bytes());
    }
    Ok(out)
}
