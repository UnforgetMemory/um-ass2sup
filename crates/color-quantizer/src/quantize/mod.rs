#![allow(missing_docs)]

pub mod median_cut;
pub mod naarahara;
pub mod nearest;
pub mod palette;
pub mod temporal;

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum QuantizeMethod {
    #[default]
    MedianCut,
    Naarahara,
}
