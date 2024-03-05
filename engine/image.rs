use std::mem::MaybeUninit;
use std::ptr;

use anyhow::{bail, ensure, Result};
use jpegxl_sys::JxlDecoderStatus::{
    BasicInfo, Error, FullImage, NeedImageOutBuffer, NeedMoreInput, Success,
};
use jpegxl_sys::*;

pub struct Image {
    pub data: Vec<u8>,
    pub size_x: u32,
    pub size_y: u32,
}

impl Image {
    pub fn new(input: &[u8]) -> Result<Self> {
        let (data, size_x, size_y) = unsafe { decode(input)? };

        let inst = Self {
            data,
            size_x,
            size_y,
        };

        Ok(inst)
    }

    pub fn from_file(path: &'static str) -> Result<Self> {
        let buf = std::fs::read(path)?;

        Self::new(&buf)
    }
}

trait ConvJxlError {
    fn conv_err(self, action: &'static str) -> Result<()>;
}

impl ConvJxlError for JxlDecoderStatus {
    fn conv_err(self, action: &'static str) -> Result<()> {
        ensure!(self == Success, "failed to {}: status = {:#?}", action, self);
        Ok(())
    }
}

// Adapted from jpegxl-sys
unsafe fn decode(input: &[u8]) -> Result<(Vec<u8>, u32, u32)> {
    // Default memory manager
    let decoder = JxlDecoderCreate(ptr::null());
    ensure!(!decoder.is_null());

    // Stop after getting the basic info and decoding the image
    let events_wanted = BasicInfo as i32 | FullImage as i32;
    JxlDecoderSubscribeEvents(decoder, events_wanted).conv_err("subscribe to events")?;

    let signature = JxlSignatureCheck(input.as_ptr(), 2);
    ensure!(signature == JxlSignature::Codestream);

    let next_in = input.as_ptr();
    let avail_in = input.len();

    let pixel_format = JxlPixelFormat {
        num_channels: 4,
        data_type: JxlDataType::Uint8,
        endianness: JxlEndianness::Native,
        align: 0,
    };

    let mut data = vec![];
    let mut size_x = 0;
    let mut size_y = 0;

    JxlDecoderSetInput(decoder, next_in, avail_in).conv_err("set input")?;

    loop {
        let status = JxlDecoderProcessInput(decoder);

        match status {
            Error => bail!("decoder error"),

            NeedMoreInput => bail!("more input requested"),

            BasicInfo => {
                let basic_info = {
                    let mut info = MaybeUninit::uninit();
                    JxlDecoderGetBasicInfo(decoder, info.as_mut_ptr())
                        .conv_err("get basic info")?;
                    info.assume_init()
                };

                size_x = basic_info.xsize;
                size_y = basic_info.ysize;
            }

            NeedImageOutBuffer => {
                let mut size = 0;
                JxlDecoderImageOutBufferSize(decoder, &pixel_format, &mut size)
                    .conv_err("get buffer size")?;

                data.resize(size, 0);

                JxlDecoderSetImageOutBuffer(decoder, &pixel_format, data.as_mut_ptr().cast(), size)
                    .conv_err("set output buffer")?;
            }

            FullImage => continue,

            Success => {
                let size_decoded = data.len();
                let size_expected = (size_x * size_y * pixel_format.num_channels) as usize;

                ensure!(
                    size_decoded == size_expected,
                    "unexpected image size: {} != {}",
                    size_decoded,
                    size_expected
                );

                break;
            }

            _ => bail!("unexpected decoder status: {:#?}", status),
        }
    }

    JxlDecoderDestroy(decoder);

    Ok((data, size_x, size_y))
}
