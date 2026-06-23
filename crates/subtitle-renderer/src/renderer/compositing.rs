//! Compositing and clip operations — re-exported from cosmic::effects.
//!
//! Provides alpha compositing, clip masks, and sub-region compositing
//! used by the subtitle renderer.

#![allow(unused_imports)]

pub use crate::cosmic::effects::clip::{apply_clip_mask, apply_drawing_clip_mask};
pub use crate::cosmic::effects::composite::{
    apply_alpha_multiplier, composite_over, composite_subregion,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpha_multiplier_full() {
        let mut data = vec![255, 255, 255, 200, 128, 128, 128, 100];
        apply_alpha_multiplier(&mut data, 1.0);
        assert_eq!(data, vec![255, 255, 255, 200, 128, 128, 128, 100]);
    }

    #[test]
    fn test_alpha_multiplier_half() {
        let mut data = vec![0, 0, 0, 200, 0, 0, 0, 100];
        apply_alpha_multiplier(&mut data, 0.5);
        assert_eq!(data[3], 100);
        assert_eq!(data[7], 50);
    }

    #[test]
    fn test_alpha_multiplier_zero() {
        let mut data = vec![0, 0, 0, 200];
        apply_alpha_multiplier(&mut data, 0.0);
        assert_eq!(data[3], 0);
    }

    #[test]
    fn test_alpha_multiplier_empty_data() {
        let mut data: Vec<u8> = vec![];
        apply_alpha_multiplier(&mut data, 0.5);
        assert!(data.is_empty());
    }

    #[test]
    fn test_alpha_multiplier_clamp_over_1() {
        let mut data = vec![0, 0, 0, 200];
        apply_alpha_multiplier(&mut data, 2.0);
        assert_eq!(data[3], 200);
    }

    #[test]
    fn test_alpha_multiplier_clamp_under_0() {
        let mut data = vec![0, 0, 0, 200];
        apply_alpha_multiplier(&mut data, -0.5);
        assert_eq!(data[3], 0);
    }

    #[test]
    fn test_alpha_multiplier_only_alpha() {
        let mut data = vec![255, 255, 255, 200, 128, 128, 128, 100, 50, 100, 150, 255];
        apply_alpha_multiplier(&mut data, 0.5);
        assert_eq!(data[3], 100);
        assert_eq!(data[7], 50);
        assert_eq!(data[11], 127);
    }

    #[test]
    fn test_clip_mask_normal_inside_preserved() {
        let mut data = vec![255u8; 64];
        let ctx = crate::context::RenderContext {
            clip_x1: 1.0,
            clip_y1: 1.0,
            clip_x2: 3.0,
            clip_y2: 3.0,
            clip_enabled: true,
            ..Default::default()
        };
        apply_clip_mask(&mut data, 4, 4, &ctx);
        // Center 2x2 should be preserved (indices 20..28)
        assert!(data[20] != 0);
        // Corner should be cleared
        assert_eq!(data[0], 0);
    }

    #[test]
    fn test_clip_mask_full_image_clip() {
        let mut data = vec![128u8; 64];
        let ctx = crate::context::RenderContext {
            clip_x1: 0.0,
            clip_y1: 0.0,
            clip_x2: 4.0,
            clip_y2: 4.0,
            clip_enabled: true,
            ..Default::default()
        };
        apply_clip_mask(&mut data, 4, 4, &ctx);
        assert!(data.iter().all(|&b| b == 128));
    }

    #[test]
    fn test_clip_mask_inverse_inside_cleared() {
        let mut data = vec![128u8; 64];
        let ctx = crate::context::RenderContext {
            clip_x1: 1.0,
            clip_y1: 1.0,
            clip_x2: 3.0,
            clip_y2: 3.0,
            clip_enabled: true,
            clip_inverse: true,
            ..Default::default()
        };
        apply_clip_mask(&mut data, 4, 4, &ctx);
        // Center 2x2 should be cleared
        assert_eq!(data[20], 0);
    }

    #[test]
    fn test_clip_mask_out_of_bounds_clip() {
        let mut data = vec![128u8; 64];
        let ctx = crate::context::RenderContext {
            clip_x1: -10.0,
            clip_y1: -10.0,
            clip_x2: 100.0,
            clip_y2: 100.0,
            clip_enabled: true,
            ..Default::default()
        };
        apply_clip_mask(&mut data, 4, 4, &ctx);
        assert!(data.iter().all(|&b| b == 128));
    }

    #[test]
    fn test_clip_mask_negative_coords_clamped() {
        let mut data = vec![128u8; 64];
        let ctx = crate::context::RenderContext {
            clip_x1: -5.0,
            clip_y1: -5.0,
            clip_x2: 2.0,
            clip_y2: 2.0,
            clip_enabled: true,
            ..Default::default()
        };
        apply_clip_mask(&mut data, 4, 4, &ctx);
        assert!(data[0] != 0);
    }

    #[test]
    fn test_composite_subregion_empty_source() {
        let mut dst = vec![0u8; 16];
        let src = vec![];
        composite_subregion(&mut dst, &src, 2, 2, 0, 0, 0, 0);
    }

    #[test]
    fn test_composite_subregion_transparent_source() {
        let mut dst = vec![255u8; 16];
        let src = vec![0, 0, 0, 0]; // fully transparent
        composite_subregion(&mut dst, &src, 2, 2, 0, 0, 1, 1);
        assert_eq!(dst[0], 255);
    }

    #[test]
    fn test_drawing_clip_normal_triangle() {
        let mut data = vec![128u8; 64];
        let ctx = crate::context::RenderContext {
            clip_drawing_commands: Some("m 0 0 l 4 0 l 0 4".into()),
            clip_drawing_scale: 1.0,
            clip_enabled: true,
            clip_drawing_inverse: false,
            ..Default::default()
        };
        apply_drawing_clip_mask(&mut data, 4, 4, &ctx, 1920.0, 1080.0);
    }

    #[test]
    fn test_drawing_clip_inverse_triangle() {
        let mut data = vec![128u8; 64];
        let ctx = crate::context::RenderContext {
            clip_drawing_commands: Some("m 0 0 l 4 0 l 0 4".into()),
            clip_drawing_scale: 1.0,
            clip_enabled: true,
            clip_drawing_inverse: true,
            ..Default::default()
        };
        apply_drawing_clip_mask(&mut data, 4, 4, &ctx, 1920.0, 1080.0);
    }

    #[test]
    fn test_drawing_clip_no_commands() {
        let mut data = vec![128u8; 64];
        let ctx = crate::context::RenderContext {
            clip_drawing_commands: None,
            clip_enabled: true,
            ..Default::default()
        };
        apply_drawing_clip_mask(&mut data, 4, 4, &ctx, 1920.0, 1080.0);
    }

    #[test]
    fn test_drawing_clip_scaled_coordinates() {
        let mut data = vec![128u8; 64];
        let ctx = crate::context::RenderContext {
            clip_drawing_commands: Some("m 0 0 l 1 0 l 0 1".into()),
            clip_drawing_scale: 4.0,
            clip_enabled: true,
            clip_drawing_inverse: false,
            ..Default::default()
        };
        apply_drawing_clip_mask(&mut data, 4, 4, &ctx, 1920.0, 1080.0);
    }

    #[test]
    fn test_drawing_clip_empty_path() {
        let mut data = vec![128u8; 64];
        let ctx = crate::context::RenderContext {
            clip_drawing_commands: Some("".into()),
            clip_drawing_scale: 1.0,
            clip_enabled: true,
            ..Default::default()
        };
        apply_drawing_clip_mask(&mut data, 4, 4, &ctx, 1920.0, 1080.0);
    }
}
