#![allow(missing_docs)]

pub mod space;
pub mod tonemap;
pub mod transfer;

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum ColorSpace {
    #[default]
    Srgb,
    Bt709,
    Bt2020,
    Linear,
}
