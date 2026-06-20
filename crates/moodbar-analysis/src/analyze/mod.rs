mod color;
mod fft;
pub(crate) mod frame_analyzer;
mod normalize;

pub(crate) use color::{frame_to_rgb, frame_to_svg_rgb, scale_to_u8};
