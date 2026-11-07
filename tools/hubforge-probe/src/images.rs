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
    fn jpeg_is_valid() {
        let j = to_jpeg(&key_test_image(14), 80);
        assert!(j.len() > 100);
        assert_eq!(&j[..2], &[0xFF, 0xD8]);
    }
}
