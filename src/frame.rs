use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

/// Straight RGBA8 with sRGB colors and linear pixel alpha, rows top to bottom with no padding.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Frame {
    pub fn write_png(&self, png: &Path) -> io::Result<()> {
        self.encode_png(BufWriter::new(File::create(png)?))
    }

    pub fn png(&self) -> io::Result<Vec<u8>> {
        let mut png = Vec::new();
        self.encode_png(&mut png)?;
        Ok(png)
    }

    pub fn encode_png(&self, out: impl Write) -> io::Result<()> {
        let mut encoder = png::Encoder::new(out, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().map_err(io::Error::other)?;
        writer
            .write_image_data(&self.rgba)
            .map_err(io::Error::other)?;
        writer.finish().map_err(io::Error::other)
    }
}
