use std::fs;
use std::path::Path;

fn main() {
    let source_ico = Path::new("frontend/vertex.ico");
    let icons_dir = Path::new("crates/app/icons");
    fs::create_dir_all(icons_dir).unwrap();

    let img = image::open(source_ico).expect("Failed to open frontend/vertex.ico");
    println!("Loaded source image: {}x{}", img.width(), img.height());

    // 32x32
    let img_32 = img.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);
    img_32.save(icons_dir.join("32x32.png")).unwrap();

    // 128x128
    let img_128 = img.resize_exact(128, 128, image::imageops::FilterType::Lanczos3);
    img_128.save(icons_dir.join("128x128.png")).unwrap();

    // 128x128@2x (256x256)
    let img_256 = img.resize_exact(256, 256, image::imageops::FilterType::Lanczos3);
    img_256.save(icons_dir.join("128x128@2x.png")).unwrap();

    // Copy original .ico
    fs::copy(source_ico, icons_dir.join("icon.ico")).unwrap();

    // Also write icon.icns using 256x256 png
    let mut buf_256 = Vec::new();
    img_256
        .write_to(
            &mut std::io::Cursor::new(&mut buf_256),
            image::ImageFormat::Png,
        )
        .unwrap();
    // ICNS container with an ic08 (256px PNG) entry, not a renamed PNG.
    let mut icns = Vec::new();
    icns.extend_from_slice(b"icns");
    icns.extend_from_slice(&((16 + buf_256.len()) as u32).to_be_bytes());
    icns.extend_from_slice(b"ic08");
    icns.extend_from_slice(&((8 + buf_256.len()) as u32).to_be_bytes());
    icns.extend_from_slice(&buf_256);
    fs::write(icons_dir.join("icon.icns"), icns).unwrap();

    // Also copy to frontend/public for browser favicon
    let public_dir = Path::new("frontend/public");
    img.save(public_dir.join("vertex.png")).unwrap();
    fs::copy(source_ico, public_dir.join("favicon.ico")).unwrap();

    println!("All icons generated and copied successfully!");
}
