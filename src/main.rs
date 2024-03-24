use std::fs;
use std::path::{Path, PathBuf};

use nlprule::{Rules, Tokenizer, tokenizer_filename, rules_filename, types::Suggestion, rules::apply_suggestions};
use rayon::prelude::*;
use regex::Regex;


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

fn list_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            //if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("png") {
            if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn clean_ocr_text(text: &str) -> String {
    let mut cleaned_text = text.to_string();

    // List of cleaning functions to apply
    let cleaning_functions: Vec<&dyn Fn(&str) -> String> = vec![
        &remove_scan_artifacts,
        &remove_page_numbers,
        &combine_split_words,
        //&remove_headers,
        &remove_headers_and_footers,
        &normalize_newlines_preserve_paragraphs,
        &trim_lines,
        //&remove_non_alphabetic,
        &remove_repetitive_patterns_preserving_paragraphs,
    ];

    // Apply each cleaning function in sequence
    for func in cleaning_functions {
        cleaned_text = func(&cleaned_text);
    }

    cleaned_text
}

// fn remove_headers(text: &str) -> String {
//     let header_patterns = vec![
//         r"(?i)\b(?:[ivxlcdm]+|\d+)\s+", // Matches Roman numerals or digits at the beginning of a line
//         r"\s+(?:[ivxlcdm]+|\d+)\b", // Matches Roman numerals or digits at the end of a line
//     ];
//
//     let mut cleaned_text = text.to_string();
//
//     for pattern in header_patterns {
//         let re = Regex::new(&format!(r"{}{}", r"(?m)^.*?", pattern)).unwrap(); // Use multiline mode, match start of line, and lazy quantifier
//         cleaned_text = re.replace_all(&cleaned_text, "").to_string();
//     }
//
//     // Further cleanup to remove any orphaned page numbers and excessive newlines created by header removal
//     let re_orphaned_page_numbers = Regex::new(r"(?m)^\s*(?:[ivxlcdm]+|\d+)\s*$").unwrap();
//     cleaned_text = re_orphaned_page_numbers.replace_all(&cleaned_text, "").to_string();
//
//     let re_excessive_newlines = Regex::new(r"\n{2,}").unwrap(); // Collapse multiple newlines to just double newlines, preserving paragraphs
//     cleaned_text = re_excessive_newlines.replace_all(&cleaned_text, "\n\n").to_string();
//
//     cleaned_text
// }

fn combine_split_words(text: &str) -> String {
    let re = Regex::new(r"-\n").unwrap();
    re.replace_all(text, "").to_string()
}


fn remove_scan_artifacts(text: &str) -> String {
    // Example: Removing random characters or patterns identified as scan artifacts
    let re = Regex::new(r"\[?\{\}!\]|\.\.\.|::").unwrap();
    re.replace_all(text, "").to_string()
}

fn remove_page_numbers(text: &str) -> String {
    // Example: Removing page numbers (assuming they are at the start or end of a line)
    let re = Regex::new(r"^\d+\s*|\s*\d+$").unwrap();
    re.replace_all(text, "").to_string()
}

fn remove_headers_and_footers(text: &str) -> String {
    // Example: Removing common headers/footers (this is highly dependent on the document's structure)
    //let re = Regex::new(r"(CHAPTER \w+)|(Page \d+)").unwrap();
    let re = Regex::new(r"(Page \d+)").unwrap();
    re.replace_all(text, "").to_string()
}

fn remove_non_alphabetic(text: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9\s.,?!]").unwrap();
    re.replace_all(text, "").to_string()
}

fn normalize_newlines_preserve_paragraphs(text: &str) -> String {
    let mut normalized_text = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Check for a single newline character that isn't part of a paragraph break
        if chars[i] == '\n' && (i == 0 || chars[i - 1] != '\n') && (i == chars.len() - 1 || chars[i + 1] != '\n') {
            normalized_text.push(' '); // Replace it with a space
        } else {
            normalized_text.push(chars[i]);
        }
        i += 1;
    }

    normalized_text
}

fn remove_repetitive_patterns_preserving_paragraphs(text: &str) -> String {
    let mut cleaned_text = String::new();
    let mut prev_char = '\0';
    let mut char_count = 1;

    for c in text.chars() {
        if c == prev_char && c != '\n' { // Ignore newlines for repetition checks
            char_count += 1;
        } else {
            if char_count < 4 || prev_char == '\n' { // Always include newlines
                cleaned_text.extend(std::iter::repeat(prev_char).take(char_count));
            }
            prev_char = c;
            char_count = 1;
        }
    }
    // Handle the last sequence
    if char_count < 4 || prev_char == '\n' {
        cleaned_text.extend(std::iter::repeat(prev_char).take(char_count));
    }

    cleaned_text
}

fn trim_lines(text: &str) -> String {
    text.lines()
        .map(|line| line.trim())
        .collect::<Vec<&str>>()
        .join("\n")
}

fn count_edges(image: &image::GrayImage) -> u32 {
    image.pixels().filter(|&p| p[0] > 0).count() as u32
}

fn has_text(image_path: &str) -> Option<image::GrayImage> {
    // Load the image
    let image = image::open(image_path).unwrap().to_luma8();

    // Perform Canny edge detection
    let edges = imageproc::edges::canny(&image, 50.0, 100.0);

    // Count the number of distinct edges
    let edge_count = count_edges(&edges);

    // Threshold for determining if the image contains text
    let threshold = 50_000; // Adjust this based on your needs

    println!("{}: {}", image_path, edge_count);
    if edge_count > threshold {
        Some(edges)
    } else {
        None
    }
}

fn crop_image(path_in: &str, path_out: &str, edges: &image::GrayImage) {
    // Load the image
    let image = image::open(path_in).unwrap().to_luma8();

    // Convert edge points to coordinates
    let edge_points: Vec<_> = edges.enumerate_pixels()
        .filter(|&(_, _, p)| p[0] > 0)
        .map(|(x, y, _)| vec![x as f64, y as f64])
        .collect();

    // Cluster the edge points using DBSCAN
    let classifications = dbscan::Model::new(10.0, 5).run(&edge_points);

    // Iterate through the clusters and find bounding boxes
    let mut clusters: Vec<Vec<_>> = Vec::new();
    for (i, label) in classifications.iter().enumerate() {
        match label {
            dbscan::Classification::Noise => continue,
            dbscan::Classification::Core(cluster_idx) | dbscan::Classification::Edge(cluster_idx) => {
                if cluster_idx < &clusters.len() {
                    clusters[*cluster_idx].push(edge_points[i].clone());
                } else {
                    clusters.push(vec![edge_points[i].clone()]);
                }
            },
        }
    }

    for cluster in clusters.iter() {
        let (min_x, max_x) = find_min_max(cluster.iter().map(|p| p[0]).collect()).unwrap();
        let (min_y, max_y) = find_min_max(cluster.iter().map(|p| p[1]).collect()).unwrap();

        println!("Cluster; Bounding box: x: {} - {}, y: {} - {}", min_x, max_x, min_y, max_y);

        // Crop the image to the bounding box (adjust as necessary for your use case)
        //let cropped = imageproc::crop::crop_imm(&image, min_x as u32, min_y as u32, (max_x - min_x) as u32, (max_y - min_y) as u32);
        let cropped = image::imageops::crop_imm(&image, min_x as u32, min_y as u32, (max_x - min_x) as u32, (max_y - min_y) as u32).to_image();
        cropped.save(path_out).unwrap();
    }
}

fn find_min_max(values: Vec<f64>) -> Option<(f64, f64)> {
    values.iter().fold(None, |minmax, &val| match minmax {
        None => Some((val, val)),
        Some(current_minmax) => Some((f64::min(val, current_minmax.0), (f64::max(val, current_minmax.1)))),
    })
}

fn ocr(filename: &str, language: &str) -> Result<String, tesseract::TesseractError> {
    let mut t = tesseract::Tesseract::new(None, Some(language))?;
    t.set_page_seg_mode(tesseract::PageSegMode::PsmAuto);
    Ok(t
        .set_image(filename)?
        .recognize()?
        .get_text()?)
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
    println!("Done converting PDF to pngs in {:?}.", start.elapsed());

    let files = list_files(&processing_dir);

    // ~68s not parallelized
    // ~20s parallelized
    let start = std::time::Instant::now();
    println!("OCRing pages...");
    let mut texts = files
    //let text = files
    //    .iter()
    //    .fold(String::new(), |mut acc, file| {
        .par_iter()
        .enumerate()
        .filter_map(|(i, file)| {
            let path = file.to_string_lossy().to_string();
            match has_text(&path) {
                None => None,
                Some(_edges) => {
                    Some((
                        i,
                        //tesseract::ocr(&path, "eng")
                        ocr(&path, "eng").unwrap_or_default(),
                    ))
                    //let crop_path = format!("crop-{}", path);
                    //crop_image(&path, &crop_path, &edges);
                    //Some((
                    //    i,
                    //    //tesseract::ocr(&path, "eng")
                    //    ocr(&crop_path, "eng").unwrap_or_default(),
                    //))
                },
            }
        })
        .collect::<Vec<(usize, String)>>();
    texts.sort_by(|a, b| a.0.cmp(&b.0));
    let text = texts
        .iter()
        .fold(String::new(), |mut acc, ocr| {
            acc.push_str(&ocr.1);
            acc
        });
    //        let path = file.to_string_lossy().to_string();
    //        if has_text(&path) {
    //            acc.push_str(
    //                &tesseract::ocr(&path, "eng")
    //                    .unwrap_or_default()
    //            );
    //        }
    //        acc
    //    });
    println!("Done OCRing pages in {:?}.", start.elapsed());
    let mut out = txt_name.to_string();
    out.push_str("ocr");
    std::fs::write(out, &text).unwrap();

    //let text = std::fs::read_to_string(txt_name).unwrap();
    println!("Cleaning text...");
    let text = clean_ocr_text(&text);
    println!("Done cleaning text.");
    let mut out = txt_name.to_string();
    out.push_str("ocr-clean");
    std::fs::write(out, &text).unwrap();

    let start = std::time::Instant::now();
    println!("Applying NLP rules...");
    let mut tokenizer_bytes: &'static [u8] = include_bytes!(concat!(
        env!("OUT_DIR"),
        "/",
        tokenizer_filename!("en")
    ));
    let mut rules_bytes: &'static [u8] = include_bytes!(concat!(
        env!("OUT_DIR"),
        "/",
        rules_filename!("en")
    ));

    let tokenizer = Tokenizer::from_reader(&mut tokenizer_bytes).expect("tokenizer binary is valid");
    let rules = Rules::from_reader(&mut rules_bytes).expect("rules binary is valid");

    let suggestions = rules.suggest(&text, &tokenizer);

    let text = apply_suggestions(&text, &suggestions);
    println!("Done applying NLP rules in {:?}.", start.elapsed());

    std::fs::write(txt_name, &text).unwrap();
}
