//! Génération des images de test : un JPEG 72x72 par touche, fond coloré
//! et gros numéro de touche, pour vérifier l'ordre des touches.

use image::codecs::jpeg::JpegEncoder;
use image::{Rgb, RgbImage};

pub const KEY_SIZE: u32 = 72;

/// Police 5x7 pour les chiffres 0 à 9 (1 = pixel allumé).
const DIGITS: [[u8; 7]; 10] = [
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110], // 0
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 1
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111], // 2
    [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110], // 3
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010], // 4
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110], // 5
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110], // 6
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000], // 7
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110], // 8
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100], // 9
];

/// Couleur de fond distincte pour chaque touche (teinte qui tourne).
pub fn key_color(index: u8) -> (u8, u8, u8) {
    let h = (index as f32 * 24.0) % 360.0;
    let (s, v) = (0.85f32, 0.75f32);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

fn draw_digit(img: &mut RgbImage, digit: usize, x0: u32, y0: u32, scale: u32) {
    for (row, bits) in DIGITS[digit].iter().enumerate() {
        for col in 0..5u32 {
            if bits & (0b10000 >> col) != 0 {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = x0 + col * scale + dx;
                        let y = y0 + row as u32 * scale + dy;
                        if x < img.width() && y < img.height() {
                            img.put_pixel(x, y, Rgb([255, 255, 255]));
                        }
                    }
                }
            }
        }
    }
}

/// Image de test pour la touche `index` (affiche index + 1, de 1 à 15).
pub fn key_test_image(index: u8) -> RgbImage {
    let (r, g, b) = key_color(index);
    let mut img = RgbImage::from_pixel(KEY_SIZE, KEY_SIZE, Rgb([r, g, b]));
    // Liseré blanc en haut à gauche pour repérer l'orientation.
    for i in 0..14 {
        for t in 0..3 {
            img.put_pixel(i, t, Rgb([255, 255, 255]));
            img.put_pixel(t, i, Rgb([255, 255, 255]));
        }
    }
    let label = (index as usize) + 1;
    let digits: Vec<usize> = label.to_string().bytes().map(|c| (c - b'0') as usize).collect();
    let scale = if digits.len() > 1 { 5 } else { 6 };
    let w = digits.len() as u32 * 5 * scale + (digits.len() as u32 - 1) * scale;
    let h = 7 * scale;
    let mut x = (KEY_SIZE - w) / 2;
    let y = (KEY_SIZE - h) / 2;
    for d in digits {
        draw_digit(&mut img, d, x, y, scale);
        x += 6 * scale;
    }
    img
}

/// Mire de résolution de `n` × `n` pixels :
/// - liseré rouge d'1 pixel sur tout le bord (s'il manque à droite ou en bas,
///   l'image est coupée) ;
/// - un carré de couleur dans chaque coin (blanc en haut à gauche, rouge en
///   haut à droite, vert en bas à gauche, bleu en bas à droite) ;
/// - la taille au centre ;
/// - des lignes d'1 pixel noires et blanches : nettes si l'écran affiche
///   vraiment cette résolution, grises ou baveuses sinon.
pub fn resolution_test_image(n: u32) -> RgbImage {
    let mut img = RgbImage::from_pixel(n, n, Rgb([40, 44, 56]));
    let red = Rgb([255, 40, 40]);
    for i in 0..n {
        img.put_pixel(i, 0, red);
        img.put_pixel(i, n - 1, red);
        img.put_pixel(0, i, red);
        img.put_pixel(n - 1, i, red);
    }
    let c = (n / 7).max(6);
    let corners = [
        (2, 2, Rgb([255, 255, 255])),
        (n - 2 - c, 2, Rgb([255, 40, 40])),
        (2, n - 2 - c, Rgb([40, 220, 80])),
        (n - 2 - c, n - 2 - c, Rgb([40, 120, 255])),
    ];
    for (x0, y0, col) in corners {
        for y in y0..y0 + c {
            for x in x0..x0 + c {
                img.put_pixel(x, y, col);
            }
        }
    }
    // Taille au centre (en haut).
    let digits: Vec<usize> = n.to_string().bytes().map(|b| (b - b'0') as usize).collect();
    let scale = (n / 30).max(2);
    let w = digits.len() as u32 * 6 * scale - scale;
    let mut x = (n - w) / 2;
    let y = n / 5;
    for d in digits {
        draw_digit(&mut img, d, x, y, scale);
        x += 6 * scale;
    }
    // Lignes d'1 pixel : verticales à gauche, horizontales à droite.
    let (top, bottom) = (n * 55 / 100, n * 80 / 100);
    let (left, mid, right) = (n * 12 / 100, n / 2, n * 88 / 100);
    for yy in top..bottom {
        for xx in left..right {
            let on = if xx < mid { xx % 2 == 0 } else { yy % 2 == 0 };
            img.put_pixel(xx, yy, if on { Rgb([255, 255, 255]) } else { Rgb([0, 0, 0]) });
        }
    }
    img
}

pub fn to_jpeg(img: &RgbImage, quality: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(&mut out, quality);
    if enc.encode_image(img).is_err() {
        out.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_charts() {
        for n in [72, 96, 120, 144] {
            let img = resolution_test_image(n);
            assert_eq!(img.dimensions(), (n, n));
            assert_eq!(&to_jpeg(&img, 95)[..2], &[0xFF, 0xD8]);
        }
    }

    /// Planche : cargo test -p hubforge-probe resolution_sheet -- --ignored
    #[test]
    #[ignore]
    fn resolution_sheet() {
        let mut sheet = RgbImage::from_pixel(72 + 96 + 120 + 144 + 50, 154, Rgb([14, 15, 19]));
        let mut x = 10;
        for n in [72, 96, 120, 144] {
            image::imageops::overlay(&mut sheet, &resolution_test_image(n), x, 5);
            x += n as i64 + 10;
        }
        let big = image::imageops::resize(&sheet, sheet.width() * 3, sheet.height() * 3, image::imageops::FilterType::Nearest);
        big.save("/tmp/resolution-sheet.png").unwrap();
    }

    #[test]
    fn jpeg_is_valid() {
        let j = to_jpeg(&key_test_image(14), 80);
        assert!(j.len() > 100);
        assert_eq!(&j[..2], &[0xFF, 0xD8]);
    }
}
