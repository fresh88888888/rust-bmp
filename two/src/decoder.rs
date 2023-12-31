extern crate byteorder;
use byteorder::{LittleEndian, ReadBytesExt};
use std::convert::{AsRef, From};
use std::fmt::{self, Debug};
use std::io::{self, Cursor, Read, Seek, SeekFrom};

const BMP_HEADER_SIZE: u64 = 14;
const BITS: usize = 8;

use self::BmpErrorKind::*;
use super::*;

pub type BmpResult<T> = Result<T, BmpError>;

#[derive(Debug)]
pub struct BmpError {
    pub kind: BmpErrorKind,
    pub details: String,
}

impl BmpError {
    fn new<T: AsRef<str>>(kind: BmpErrorKind, details: T) -> BmpError {
        BmpError {
            kind,
            details: String::from(details.as_ref()),
        }
    }
}

impl fmt::Display for BmpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            BmpIoError(ref error) => std::fmt::Display::fmt(&error, f),
            ref e => {
                let kind_desc: &str = e.as_ref();
                write!(f, "{}:{}", kind_desc, self.details)
            }
        }
    }
}

impl From<io::Error> for BmpError {
    fn from(err: io::Error) -> BmpError {
        BmpError::new(BmpIoError(err), "Io Error")
    }
}

#[derive(Debug)]
pub enum BmpErrorKind {
    WrongMagicNumbers,
    UnsupportedBitsPerPixel,
    UnsupportedCompressionType,
    UnsupportedBmpVersion,
    UnsupportedHeader,
    BmpIoError(io::Error),
}

impl AsRef<str> for BmpErrorKind {
    fn as_ref(&self) -> &str {
        match *self {
            WrongMagicNumbers => "Wrong magic numbers",
            UnsupportedBitsPerPixel => "Unsupported bits per pixel",
            UnsupportedCompressionType => "Unsupported compression type",
            UnsupportedBmpVersion => "Unsupported bmp version",
            UnsupportedHeader => "Unsupported header",
            _ => "BMP Error",
        }
    }
}

pub fn decode_image(bmp_data: &mut Cursor<Vec<u8>>) -> BmpResult<Image> {
    read_bmp_id(bmp_data)?;
    let header = read_bmp_header(bmp_data)?;
    let dib_header = read_bmp_dib_header(bmp_data)?;
    let color_palette = read_color_palette(bmp_data, &dib_header)?;

    let width = dib_header.width.unsigned_abs();
    let height = dib_header.height.unsigned_abs();
    let padding = width % 4;

    let data = match color_palette {
        Some(ref palette) => read_indexes(
            bmp_data.get_mut(),
            palette,
            width as usize,
            height as usize,
            dib_header.bits_per_pixel,
            header.pixel_offset as usize,
        )?,
        None => read_pixels(bmp_data, width, height, header.pixel_offset, padding as i64)?,
    };

    let image = Image {
        header,
        dib_header: BmpDibHeader::new(width as i32, height as i32),
        color_palette,
        width,
        height,
        padding,
        data,
    };

    Ok(image)
}

fn read_bmp_id(bmp_data: &mut Cursor<Vec<u8>>) -> BmpResult<()> {
    let mut bm = [0, 0];
    bmp_data.read_exact(&mut bm)?;

    if bm == b"BM"[..] {
        Ok(())
    } else {
        Err(BmpError::new(
            WrongMagicNumbers,
            format!("Expacted [66, 77], but was {:?}", bm),
        ))
    }
}

fn read_bmp_header(bmp_data: &mut Cursor<Vec<u8>>) -> BmpResult<BmpHeader> {
    let header = BmpHeader {
        file_size: bmp_data.read_u32::<LittleEndian>()?,
        creator1: bmp_data.read_u16::<LittleEndian>()?,
        creator2: bmp_data.read_u16::<LittleEndian>()?,
        pixel_offset: bmp_data.read_u32::<LittleEndian>()?,
    };

    Ok(header)
}

fn read_bmp_dib_header(bmp_data: &mut Cursor<Vec<u8>>) -> BmpResult<BmpDibHeader> {
    let dib_header = BmpDibHeader {
        header_size: bmp_data.read_u32::<LittleEndian>()?,
        width: bmp_data.read_i32::<LittleEndian>()?,
        height: bmp_data.read_i32::<LittleEndian>()?,
        num_planes: bmp_data.read_u16::<LittleEndian>()?,
        bits_per_pixel: bmp_data.read_u16::<LittleEndian>()?,
        compress_type: bmp_data.read_u32::<LittleEndian>()?,
        data_size: bmp_data.read_u32::<LittleEndian>()?,
        hres: bmp_data.read_i32::<LittleEndian>()?,
        vres: bmp_data.read_i32::<LittleEndian>()?,
        num_colors: bmp_data.read_u32::<LittleEndian>()?,
        num_imp_colors: bmp_data.read_u32::<LittleEndian>()?,
    };

    match BmpVersion::from_dib_header(&dib_header) {
        Some(BmpVersion::Three) | Some(BmpVersion::Four) | Some(BmpVersion::Five) => (),
        Some(other) => return Err(BmpError::new(UnsupportedBmpVersion, other)),
        None => {
            return Err(BmpError::new(
                UnsupportedHeader,
                format!(
                    "Only simple BMP images of version 3, 4, and 5 are currently supported. \
                Connot decode the image for the following header: {:?}",
                    dib_header
                ),
            ))
        }
    }

    match dib_header.bits_per_pixel {
        1 | 4 | 8 | 24 => (),
        other => {
            return Err(BmpError::new(
                UnsupportedBitsPerPixel,
                format!(
                    "Only 1, 4, 8, and 24 bits per pixel are currently supported, was: {}",
                    other
                ),
            ))
        }
    }

    match CompressionType::from_u32(dib_header.compress_type) {
        CompressionType::Uncompressed => (),
        other => return Err(BmpError::new(UnsupportedCompressionType, other)),
    }

    Ok(dib_header)
}

fn read_color_palette(
    bmp_data: &mut Cursor<Vec<u8>>,
    dh: &BmpDibHeader,
) -> BmpResult<Option<Vec<Pixel>>> {
    let num_entries = match dh.bits_per_pixel {
        // We have a color_palette if the num_colors in the dib header is not zero {
        _ if dh.num_colors != 0 => dh.num_colors as usize,
        bpp @ 1 | bpp @ 4 | bpp @ 8 => 1 << bpp,
        _ => return Ok(None),
    };

    let num_bytes = match BmpVersion::from_dib_header(dh) {
        // Three bytes for v2. Though, this is currently not supported
        Some(BmpVersion::Two) => return Err(BmpError::new(UnsupportedBmpVersion, BmpVersion::Two)),
        _ => 4,
    };

    bmp_data.seek(SeekFrom::Start(BMP_HEADER_SIZE + dh.header_size as u64))?;
    let px = &mut [0; 4][0..num_bytes as usize];
    let mut color_palette = Vec::with_capacity(num_entries);
    for _ in 0..num_entries {
        bmp_data.read_exact(px)?;
        color_palette.push(px!(px[2], px[1], px[0]));
    }

    Ok(Some(color_palette))
}

fn read_indexes(
    bmp_data: &mut [u8],
    palette: &[Pixel],
    width: usize,
    height: usize,
    bpp: u16,
    offset: usize,
) -> BmpResult<Vec<Pixel>> {
    let mut data = Vec::with_capacity(height * width);
    // Number of bytes to read from each row, varies based on bits_per_pixel
    let bytes_per_row = (width as f64 / (8.0 / bpp as f64)).ceil() as usize;
    for y in 0..height {
        let padding = match bytes_per_row % 4 {
            0 => 0,
            other => 4 - other,
        };
        let start = offset + (bytes_per_row + padding) * y;
        let bytes = &bmp_data[start..start + bytes_per_row];

        for i in bit_index(bytes, bpp as usize, width) {
            data.push(palette[i]);
        }
    }

    Ok(data)
}

fn read_pixels(
    bmp_data: &mut Cursor<Vec<u8>>,
    width: u32,
    height: u32,
    offset: u32,
    padding: i64,
) -> BmpResult<Vec<Pixel>> {
    let mut data = Vec::with_capacity((height * width) as usize);
    // seek until data
    bmp_data.seek(SeekFrom::Start(offset as u64))?;
    // read pixels until padding
    let mut px = [0; 3];
    for _ in 0..height {
        for _ in 0..width {
            bmp_data.read_exact(&mut px)?;
            data.push(px!(px[2], px[1], px[0]));
        }
        // seek padding
        bmp_data.seek(SeekFrom::Current(padding))?;
    }

    Ok(data)
}

#[derive(Debug)]
struct BitIndex<'a> {
    size: usize,
    nbits: usize,
    bits_left: usize,
    mask: u8,
    bytes: &'a [u8],
    index: usize,
}

fn bit_index(bytes: &[u8], nbits: usize, size: usize) -> BitIndex {
    let bits_left = BITS - nbits;
    BitIndex {
        size,
        nbits,
        bits_left,
        mask: (!0 >> bits_left),
        bytes,
        index: 0,
    }
}

impl<'a> Iterator for BitIndex<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let n = self.index / BITS;
        let offset = self.bits_left - self.index % BITS;
        self.index += self.nbits;

        if self.size == 0 {
            None
        } else {
            self.size -= 1;
            self.bytes
                .get(n)
                .map(|&block| ((block & self.mask << offset) >> offset) as usize)
        }
    }
}

#[test]
fn test_calculate_bit_index() {
    let bytes = vec![0b1000_0001, 0b1111_0001];

    let mut bi = bit_index(&bytes, 1, 15);
    assert_eq!(bi.next(), Some(1));
    assert_eq!(bi.next(), Some(0));
    assert_eq!(bi.next(), Some(0));
    assert_eq!(bi.next(), Some(0));
    assert_eq!(bi.next(), Some(0));
    assert_eq!(bi.next(), Some(0));
    assert_eq!(bi.next(), Some(0));
    assert_eq!(bi.next(), Some(1));
    assert_eq!(bi.next(), Some(1));
    assert_eq!(bi.next(), Some(1));
    assert_eq!(bi.next(), Some(1));
    assert_eq!(bi.next(), Some(1));
    assert_eq!(bi.next(), Some(0));
    assert_eq!(bi.next(), Some(0));
    assert_eq!(bi.next(), Some(0));
    assert_eq!(bi.next(), None);
    assert_eq!(bi.next(), None);

    let mut bi = bit_index(&bytes, 4, 4);
    assert_eq!(bi.next(), Some(0b1000));
    assert_eq!(bi.next(), Some(0b0001));
    assert_eq!(bi.next(), Some(0b1111));
    assert_eq!(bi.next(), Some(0b0001));
    assert_eq!(bi.next(), None);

    let mut bi = bit_index(&bytes, 8, 2);
    assert_eq!(bi.next(), Some(0b1000_0001));
    assert_eq!(bi.next(), Some(0b1111_0001));
    assert_eq!(bi.next(), None);
}
