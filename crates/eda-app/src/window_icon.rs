use std::io::Cursor;

pub fn load() -> eframe::egui::viewport::IconData {
    let decoder = png::Decoder::new(Cursor::new(include_bytes!(
        "../../../assets/protocraft-icon.png"
    )));
    let mut reader = decoder
        .read_info()
        .expect("the embedded ProtoCraft window icon must be a valid PNG");
    let mut rgba = vec![
        0;
        reader
            .output_buffer_size()
            .expect("the embedded ProtoCraft icon must fit in memory")
    ];
    let info = reader
        .next_frame(&mut rgba)
        .expect("the embedded ProtoCraft icon must decode");
    assert_eq!(
        info.color_type,
        png::ColorType::Rgba,
        "the embedded ProtoCraft icon must use RGBA pixels"
    );
    rgba.truncate(info.buffer_size());
    eframe::egui::viewport::IconData {
        rgba,
        width: info.width,
        height: info.height,
    }
}
