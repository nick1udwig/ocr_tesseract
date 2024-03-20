use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

fn convert_pdf_to_png(pdf_path: &str, output_path: &str) -> std::io::Result<()> {
    // Construct the command
    // For ImageMagick version 7 and newer, use `magick convert` instead of `convert`
    //let output = format!("{}/{}", output_path, "page-%04d.png");
    let output = format!("{}/{}", output_path, "page");

    // fast, but depending on pdf, multiple images per page
    let status = std::process::Command::new("pdfimages")
        .arg(pdf_path)
        .arg(output)
        .status()?;

    // takes a long time but gives one png per page
    //let status = std::process::Command::new("convert")
    //    .arg("-density")
    //    .arg("150")
    //    .arg(pdf_path)
    //    .arg("-quality")
    //    .arg("90")
    //    .arg(output)
    //    .status()?;

    if status.success() {
        println!("PDF conversion successful.");
    } else {
        eprintln!("PDF conversion failed.");
    }

    Ok(())
}

fn list_png_files(dir: &Path) -> Vec<PathBuf> {
    let mut png_files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            //if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("png") {
            if path.is_file() {
                png_files.push(path);
            }
        }
    }

    png_files
}

fn main() {
    let pdf_name = "octaviusofminuci00minuiala.pdf";
    let processing_dir_name = "/tmp/octavius";
    let processing_dir = Path::new(processing_dir_name);
    let txt_name = "octavius.txt";
    std::fs::create_dir_all(processing_dir_name).unwrap();

    let start = std::time::Instant::now();
    println!("Converting PDF to pngs...");
    convert_pdf_to_png(pdf_name, processing_dir_name).unwrap();
    println!("Done converting PDF to pngs in {:?}", start.elapsed());

    let png_files = list_png_files(&processing_dir);

    // ~68s not parallelized
    // ~20s parallelized
    let start = std::time::Instant::now();
    println!("OCRing pages...");
    let text = png_files
        //.iter()
        //.fold(String::new(), |mut acc, png_file| {
        .par_iter()
        .fold(|| String::new(), |mut acc, png_file| {
            acc.push_str(
                &tesseract::ocr(&png_file.to_string_lossy().to_string(), "eng")
                    .unwrap_or_default()
            );
            acc
        //});
        })
        .collect::<String>();
    println!("Done OCRing pages in {:?}", start.elapsed());

    std::fs::write(txt_name, text).unwrap();
}
