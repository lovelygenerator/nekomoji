//! `--icon id=絵.png` の読み込み。PNG を RGBA にして `Atlas` に登録する(確認用の道具)。

use nekomoji_wgpu::atlas::Atlas;

/// `id=パス` を (id, パス) に分ける。
pub fn parse_arg(arg: &str) -> Option<(String, String)> {
    let (id, path) = arg.split_once('=')?;
    (!id.is_empty() && !path.is_empty()).then(|| (id.to_string(), path.to_string()))
}

/// PNG を読んで (幅, 高さ, RGBA) にする。グレー・パレット・16bit も RGBA の 8bit にそろえる。
pub fn load_png(path: &str) -> Result<(u32, u32, Vec<u8>), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| format!("{path}: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("{path}: {e}"))?;
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
        png::ColorType::Indexed => return Err(format!("{path}: パレットを展開できなかった")),
    };
    Ok((info.width, info.height, rgba))
}

/// `--icon id=パス` の並びを読んで `atlas` に登録する。失敗したら理由を返す。
pub fn register(atlas: &mut Atlas, args: &[String]) -> Result<(), String> {
    for arg in args {
        let (id, path) =
            parse_arg(arg).ok_or_else(|| format!("--icon は id=パス の形で書く: {arg}"))?;
        let (w, h, rgba) = load_png(&path)?;
        atlas
            .icons_mut()
            .insert(&id, w, h, &rgba)
            .map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(())
}
