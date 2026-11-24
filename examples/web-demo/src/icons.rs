//! デモに同梱する文中のアイコン(`[icon ok]`・`[icon heart]`)の絵。ブラウザに依存しないので、
//! ネイティブでもテストできる。絵は `examples/images/icons/` の PNG を実行ファイルに埋め込む。

use nekomoji_wgpu::atlas::Atlas;

/// 同梱するアイコンの ID と PNG。
const ICONS: &[(&str, &[u8])] = &[
    ("ok", include_bytes!("../../images/icons/ok.png")),
    ("heart", include_bytes!("../../images/icons/heart.png")),
];

/// 同梱するアイコンの ID。
pub fn ids() -> impl Iterator<Item = &'static str> {
    ICONS.iter().map(|(id, _)| *id)
}

/// PNG を (幅, 高さ, RGBA の 8bit) にする。
fn decode(png_bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(png_bytes);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        png::ColorType::Indexed => return Err("パレットを展開できなかった".into()),
    };
    Ok((info.width, info.height, rgba))
}

/// 同梱のアイコンの絵を `atlas` に登録する。登録した絵は、字形のアトラスが作り直されても残る。
pub fn register(atlas: &mut Atlas) -> Result<(), String> {
    for (id, bytes) in ICONS {
        let (w, h, rgba) = decode(bytes).map_err(|e| format!("アイコン {id}: {e}"))?;
        atlas
            .icons_mut()
            .insert(id, w, h, &rgba)
            .map_err(|e| format!("アイコン {id}: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 同梱のアイコンをすべて登録できる() {
        let mut atlas = Atlas::new(64);
        register(&mut atlas).unwrap();
        for id in ids() {
            assert!(atlas.icons().contains(id), "{id} が登録されていない");
        }
        assert_eq!(atlas.icons().len(), ids().count());
    }

    #[test]
    fn 絵は透明な背景と不透明な図形を持つ() {
        for (id, bytes) in ICONS {
            let (w, h, rgba) = decode(bytes).unwrap();
            assert_eq!(rgba.len(), (w * h * 4) as usize, "{id}");
            // 四隅は透明、どこかは不透明
            assert_eq!(rgba[3], 0, "{id}: 左上が透明でない");
            assert!(
                rgba.chunks(4).any(|p| p[3] == 255),
                "{id}: 不透明な画素が無い"
            );
        }
    }
}
