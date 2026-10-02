//! Bundles the paper texture: a smaller, evened-out, re-encoded copy. Never
//! touch the original in `assets/paper/`.

use std::env;
use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::{self, FilterType};
use image::{Rgb, RgbImage};

const PAPER: &str = "../../assets/paper/ivory-off-white-paper-texture.jpg";
const QUALITY: u8 = 70;
/// egui's safe GPU texture limit. The grain is soft: upscaling won't show.
const MAX_SIDE: u32 = 2048;
/// The blurred lighting copy's size. Keep it small enough to lose the grain.
const SHADING_SIDE: u32 = 16;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed={PAPER}");
    let mut paper = image::open(PAPER)?;
    if paper.width().max(paper.height()) > MAX_SIDE {
        paper = paper.resize(MAX_SIDE, MAX_SIDE, FilterType::Lanczos3);
    }
    let paper = even_out(&paper.to_rgb8());
    let out = PathBuf::from(env::var("OUT_DIR")?).join("paper.jpg");
    let writer = BufWriter::new(File::create(out)?);
    paper.write_with_encoder(JpegEncoder::new_with_quality(writer, QUALITY))?;
    Ok(())
}

/// Removes the vignetting: divides by the blurred lighting, keeping the mean
/// colour and the grain.
fn even_out(photo: &RgbImage) -> RgbImage {
    let (width, height) = photo.dimensions();
    let small = imageops::resize(photo, SHADING_SIDE, SHADING_SIDE, FilterType::Triangle);
    let shading = imageops::resize(&small, width, height, FilterType::CatmullRom);

    let mut mean = [0.0f64; 3];
    for pixel in small.pixels() {
        for (sum, &value) in mean.iter_mut().zip(&pixel.0) {
            *sum += f64::from(value);
        }
    }
    let count = f64::from(SHADING_SIDE * SHADING_SIDE);
    let mean = mean.map(|sum| sum / count);

    RgbImage::from_fn(width, height, |x, y| {
        let pixel = photo.get_pixel(x, y).0;
        let light = shading.get_pixel(x, y).0;
        Rgb(std::array::from_fn(|c| {
            let flat = f64::from(pixel[c]) * mean[c] / f64::from(light[c]).max(1.0);
            flat.round().clamp(0.0, 255.0) as u8
        }))
    })
}
