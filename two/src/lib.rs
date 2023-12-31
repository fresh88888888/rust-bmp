#![deny(warnings)]
#![cfg_attr(test, deny(warnings))]

//! A small library for reading and writing BMP images.
//!
//! The library supports uncompressed BMP Version 3 times.
//! The different decoding and encoding schemes is shown in the table below.
//!
//! |Scheme | Decoding | Encoding | Compression |
//! |-------|----------|----------|-------------|
//! | 24 bpp| ✓        | ✓        | No          |
//! | 8 bpp | ✓        | ✗        | No          |
//! | 4 bpp | ✓        | ✗        | No          |
//! | 1 bpp | ✓        | ✗        | No          |
//!
//! # Example
//!
//! ```
//! #[macro_use]
//! extern crate two;
//! use two::{Image, Pixel};
//! use two::px;
//!
//! fn main() {
//!     let mut img = Image::new(256, 256);
//!
//!     for (x, y) in img.coordinates() {
//!         img.set_pixel(x, y, px!(x, y, 200));
//!     }
//!     let _ = img.save("img.bmp");
//! }
//!

extern crate byteorder;

use std::convert::AsRef;
use std::fmt;
use std::fs;
use std::io;
use std::io::{Cursor, Read, Write};
use std::iter::Iterator;
use std::path::Path;


// Expose decoder's public types, structs, and enums
pub use decoder::{BmpError, BmpErrorKind, BmpResult};

#[macro_export]
macro_rules! px {
    ($r:expr, $g:expr, $b:expr) => {
        Pixel {
            r: $r as u8,
            g: $g as u8,
            b: $b as u8,
        }
    };
}

macro_rules! file_size {
    ($bpp:expr, $width:expr, $height:expr) => {{
        let head_size = 2 + 12 + 40;
        let row_size = (($bpp as f32 * $width as f32 + 31.0) / 32.0).floor() as u32 * 4;
        (head_size as u32, $height as u32 * row_size)
    }};
}

pub mod consts;

mod decoder;
mod encoder;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Pixel {
    pub fn new(r: u8, g: u8, b: u8) -> Pixel {
        Pixel { r, g, b }
    }
}

impl fmt::Display for Pixel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rgb({}, {}, {})", self.r, self.g, self.b)
    }
}

impl fmt::LowerHex for Pixel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl fmt::UpperHex for Pixel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BmpVersion {
    Two,
    Three,
    ThreeNT,
    Four,
    Five,
}

impl BmpVersion {
    fn from_dib_header(dib_header: &BmpDibHeader) -> Option<BmpVersion> {
        match dib_header.header_size {
            12 => Some(BmpVersion::Two),
            40 if dib_header.compress_type == 3 => Some(BmpVersion::ThreeNT),
            40 => Some(BmpVersion::Three),
            108 => Some(BmpVersion::Four),
            124 => Some(BmpVersion::Five),
            _ => None,
        }
    }
}

impl AsRef<str> for BmpVersion {
    fn as_ref(&self) -> &str {
        match *self {
            BmpVersion::Two => "BMP Version 2",
            BmpVersion::Three => "BMP Version 3",
            BmpVersion::ThreeNT => "BMP Version 3 NT",
            BmpVersion::Four => "BMP Version 4",
            BmpVersion::Five => "BMP Version 5",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum CompressionType {
    Uncompressed,
    Rle8bit,
    Rle4bit,
    // Only for BMP version 4
    BitfieldsEncoding,
}

impl CompressionType {
    fn from_u32(val: u32) -> CompressionType {
        match val {
            1 => CompressionType::Rle8bit,
            2 => CompressionType::Rle4bit,
            3 => CompressionType::BitfieldsEncoding,
            _ => CompressionType::Uncompressed,
        }
    }
}

impl AsRef<str> for CompressionType {
    fn as_ref(&self) -> &str {
        match *self {
            CompressionType::Rle8bit => "RLE 8-bit",
            CompressionType::Rle4bit => "RLE 4-bit",
            CompressionType::BitfieldsEncoding => "Bitfields Encoding",
            CompressionType::Uncompressed => "Uncompressed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BmpHeader {
    file_size: u32,
    creator1: u16,
    creator2: u16,
    pixel_offset: u32,
}

impl BmpHeader {
    fn new(header_size: u32, data_size: u32) -> BmpHeader {
        BmpHeader {
            file_size: header_size + data_size,
            creator1: 0,
            creator2: 0,
            pixel_offset: header_size,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BmpDibHeader {
    header_size: u32,
    width: i32,
    height: i32,
    num_planes: u16,
    bits_per_pixel: u16,
    compress_type: u32,
    data_size: u32,
    hres: i32,
    vres: i32,
    num_colors: u32,
    num_imp_colors: u32,
}

impl BmpDibHeader {
    fn new(width: i32, height: i32) -> BmpDibHeader {
        let (_, pixel_array_size) = file_size!(24, width, height);
        BmpDibHeader {
            header_size: 40,
            width,
            height,
            num_planes: 1,
            bits_per_pixel: 24,
            compress_type: 0,
            data_size: pixel_array_size,
            hres: 1000,
            vres: 1000,
            num_colors: 0,
            num_imp_colors: 0,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    header: BmpHeader,
    dib_header: BmpDibHeader,
    color_palette: Option<Vec<Pixel>>,
    width: u32,
    height: u32,
    padding: u32,
    data: Vec<Pixel>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Image {
        let mut data = Vec::with_capacity((width * height) as usize);
        let (header_size, data_size) = file_size!(24, width, height);

        for _ in 0..width * height {
            data.push(px!(0, 0, 0));
        }

        Image {
            header: BmpHeader::new(header_size, data_size),
            dib_header: BmpDibHeader::new(width as i32, height as i32),
            color_palette: None,
            width,
            height,
            padding: width % 4,
            data,
        }
    }

    /// Returns the `width` of the Image.
    #[inline]
    pub fn get_width(&self) -> u32 {
        self.width
    }

    /// Returns the `height` of the Image
    #[inline]
    pub fn get_height(&self) -> u32 {
        self.height
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, val: Pixel) {
        self.data[((self.height - y - 1) * self.width + x) as usize] = val;
    }

    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> Pixel {
        self.data[((self.height - y - 1) * self.width + x) as usize]
    }

    #[inline]
    pub fn coordinates(&self) -> ImageIndex {
        ImageIndex::new(self.width, self.height)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut bmp_file = fs::File::create(path)?;
        self.to_writer(&mut bmp_file)
    }

    pub fn to_writer<W: Write>(&self, destination: &mut W) -> io::Result<()> {
        let bmp_data = encoder::encode_image(self)?;
        destination.write_all(&bmp_data)?;
        Ok(())
    }
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Image")
            .field("header", &self.header)
            .field("dib_header", &self.dib_header)
            .field("color_palette", &self.color_palette)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("padding", &self.padding)
            .field("data", &self.data)
            .finish()
    }
}

#[derive(Clone, Copy)]
pub struct ImageIndex {
    width: u32,
    height: u32,
    x: u32,
    y: u32,
}

impl ImageIndex {
    fn new(width: u32, height: u32) -> ImageIndex {
        ImageIndex {
            width,
            height,
            x: 0,
            y: 0,
        }
    }
}

impl Iterator for ImageIndex {
    type Item = (u32, u32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.x < self.width && self.y < self.height {
            let this = Some((self.x, self.y));
            self.x += 1;
            if self.x == self.width {
                self.x = 0;
                self.y += 1;
            }
            this
        } else {
            None
        }
    }
}

pub fn open<P: AsRef<Path>>(path: P) -> BmpResult<Image> {
    let mut f = fs::File::open(path)?;
    from_reader(&mut f)
}

pub fn from_reader<R: Read>(source: &mut R) -> BmpResult<Image> {
    let mut bytes = Vec::new();
    source.read_to_end(&mut bytes)?;

    let mut bmp_data = Cursor::new(bytes);
    decoder::decode_image(&mut bmp_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};
    use std::mem::size_of;

    #[test]
    fn size_of_bmp_header_is_54_bytes() {
        let bmp_header_size = size_of::<BmpHeader>();
        let bmp_bip_header_size = size_of::<BmpDibHeader>();

        assert_eq!(12, bmp_header_size);
        assert_eq!(40, bmp_bip_header_size);
    }

    fn verify_test_bmp_image(img: Image) {
        let header = img.header;
        assert_eq!(70, header.file_size);
        assert_eq!(0, header.creator1);
        assert_eq!(0, header.creator2);

        let dib_header = img.dib_header;
        assert_eq!(54, header.pixel_offset);
        assert_eq!(40, dib_header.header_size);
        assert_eq!(2, dib_header.width);
        assert_eq!(2, dib_header.height);
        assert_eq!(1, dib_header.num_planes);
        assert_eq!(24, dib_header.bits_per_pixel);
        assert_eq!(0, dib_header.compress_type);
        assert_eq!(16, dib_header.data_size);
        assert_eq!(1000, dib_header.hres);
        assert_eq!(1000, dib_header.vres);
        assert_eq!(0, dib_header.num_colors);
        assert_eq!(0, dib_header.num_imp_colors);

        assert_eq!(2, img.padding);
    }

    #[test]
    fn can_read_bmp_image_from_file_specified_by_path() {
        let bmp_img = open("test/rgbw.bmp").unwrap();
        verify_test_bmp_image(bmp_img);
    }

    #[test]
    fn can_read_bmp_image_from_reader() {
        let mut f = fs::File::open("test/rgbw.bmp").unwrap();

        let bmp_img = from_reader(&mut f).unwrap();

        verify_test_bmp_image(bmp_img);
    }

    #[test]
    fn can_read_image_data() {
        let mut f = fs::File::open("test/rgbw.bmp").unwrap();
        f.seek(SeekFrom::Start(54)).unwrap();

        let mut px = [0; 3];
        f.read_exact(&mut px).unwrap();

        assert_eq!(
            Pixel {
                r: px[2],
                g: px[1],
                b: px[0],
            },
            consts::BLUE
        );
    }

    #[test]
    fn can_read_entire_bmp_image() {
        let bmp_img = open("test/rgbw.bmp").unwrap();
        assert_eq!(bmp_img.data.len(), 4);

        assert_eq!(bmp_img.get_pixel(0, 0), consts::RED);
        assert_eq!(bmp_img.get_pixel(1, 0), consts::LIME);
        assert_eq!(bmp_img.get_pixel(0, 1), consts::BLUE);
        assert_eq!(bmp_img.get_pixel(1, 1), consts::WHITE);
    }

    #[test]
    fn read_write_1pbb_bmp_image() {
        let img = open("test/bmptestsuite-0.9/valid/1bpp-1x1.bmp").unwrap();
        assert_eq!(img.data.len(), 1);
        assert_eq!(img.get_pixel(0, 0), consts::BLACK);

        let _ = img.save("test/1bb-1x1.bmp");
        let img = open("test/1bb-1x1.bmp").unwrap();
        assert_eq!(img.data.len(), 1);
        assert_eq!(img.get_pixel(0, 0), consts::BLACK);
    }

    #[test]
    fn read_write_4pbb_bmp_image() {
        let img = open("test/bmptestsuite-0.9/valid/4bpp-1x1.bmp").unwrap();
        assert_eq!(img.data.len(), 1);
        assert_eq!(img.get_pixel(0, 0), consts::BLUE);

        let _ = img.save("test/4bb-1x1.bmp");
        let img = open("test/4bb-1x1.bmp").unwrap();
        assert_eq!(img.data.len(), 1);
        assert_eq!(img.get_pixel(0, 0), consts::BLUE);
    }

    #[test]
    fn read_write_8pbb_bmp_image() {
        let img = open("test/bmptestsuite-0.9/valid/8bpp-1x1.bmp").unwrap();
        assert_eq!(img.data.len(), 1);
        assert_eq!(img.get_pixel(0, 0), consts::BLUE);

        let _ = img.save("test/8bb-1x1.bmp");
        let img = open("test/8bb-1x1.bmp").unwrap();
        assert_eq!(img.data.len(), 1);
        assert_eq!(img.get_pixel(0, 0), consts::BLUE);
    }

    #[test]
    fn read_write_bmp_v3_image() {
        let bmp_img = open("test/bmptestsuite-0.9/valid/24bpp-320x240.bmp").unwrap();
        bmp_img.save("test/24bpp-320x240.bmp").unwrap();
    }

    #[test]
    fn read_write_bmp_v4_image() {
        let bmp_img = open("test/bmpsuite-2.5/g/pal8v4.bmp").unwrap();
        bmp_img.save("test/pal8v4-test.bmp").unwrap();
    }

    #[test]
    fn read_write_bmp_v5_image() {
        let bmp_img = open("test/bmpsuite-2.5/g/pal8v5.bmp").unwrap();
        bmp_img.save("test/pal8v5-test.bmp").unwrap();
    }

    #[test]
    fn error_when_opening_unexisting_image() {
        let result = open("test/no_img.bmp");
        match result {
            Err(BmpError {
                kind: BmpErrorKind::BmpIoError(_),
                ..
            }) => (/* Expected */),
            _ => panic!("No image expected..."),
        }
    }

    #[test]
    fn error_when_opening_image_with_wrong_bits_per_pixel() {
        let result = open("test/bmptestsuite-0.9/valid/32bpp-1x1.bmp");
        match result {
            Err(BmpError {
                kind: BmpErrorKind::UnsupportedBitsPerPixel,
                ..
            }) => (/* Expected */),
            _ => panic!("32bpp are not yet supported"),
        }
    }

    #[test]
    fn error_when_opening_image_with_wrong_magic_numbers() {
        let result = open("test/bmptestsuite-0.9/corrupt/magicnumber-bad.bmp");
        match result {
            Err(BmpError {
                kind: BmpErrorKind::WrongMagicNumbers,
                ..
            }) => (/* Expected */),
            _ => panic!("Wrong magic numbers are not supported"),
        }
    }

    #[test]
    fn can_create_bmp_file() {
        let mut bmp = Image::new(2, 2);
        bmp.set_pixel(0, 0, consts::RED);
        bmp.set_pixel(1, 0, consts::LIME);
        bmp.set_pixel(0, 1, consts::BLUE);
        bmp.set_pixel(1, 1, consts::WHITE);
        let _ = bmp.save("test/rgbw_test.bmp");

        let bmp_img = open("test/rgbw_test.bmp").unwrap();
        assert_eq!(bmp_img.get_pixel(0, 0), consts::RED);
        assert_eq!(bmp_img.get_pixel(1, 0), consts::LIME);
        assert_eq!(bmp_img.get_pixel(0, 1), consts::BLUE);
        assert_eq!(bmp_img.get_pixel(1, 1), consts::WHITE);

        verify_test_bmp_image(bmp_img);
    }

    #[test]
    fn changing_pixels_does_not_push_image_data() {
        let mut img = Image::new(2, 1);
        img.set_pixel(1, 0, consts::WHITE);
        img.set_pixel(0, 0, consts::WHITE);

        assert_eq!(img.get_pixel(0, 0), consts::WHITE);
        assert_eq!(img.get_pixel(1, 0), consts::WHITE);
    }

    #[test]
    fn coordinates_iterator_gives_x_and_y_in_row_major_order() {
        let img = Image::new(2, 3);
        let mut coords = img.coordinates();
        assert_eq!(coords.next(), Some((0, 0)));
        assert_eq!(coords.next(), Some((1, 0)));
        assert_eq!(coords.next(), Some((0, 1)));
        assert_eq!(coords.next(), Some((1, 1)));
        assert_eq!(coords.next(), Some((0, 2)));
        assert_eq!(coords.next(), Some((1, 2)));
    }
}
