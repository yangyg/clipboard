//! Cheap image fingerprints and pre-channel downscaling for captured bitmaps.
use arboard::ImageData;
use tracing::debug;

/// H-2: Quick dedup fingerprint for clipboard images. Uses FNV-1a (non-crypto)
/// over dimensions + sampled head/tail bytes. This only guards the poll-loop
/// dedup check (last_image_hash); the authoritative content hash is computed
/// later by the image worker (SHA-256 over full RGBA).
/// Collision risk: two different images with identical size and matching edge
/// samples (e.g. large near-solid screenshots) may be treated as unchanged.
pub fn image_quick_fingerprint(img: &ImageData<'_>) -> String {
    let bytes = img.bytes.as_ref();
    // FNV-1a 64-bit: fast, no heap alloc, sufficient collision resistance for dedup.
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    let mut h = FNV_OFFSET;
    let mut feed = |data: &[u8]| {
        for &b in data {
            h ^= b as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
    };
    feed(&img.width.to_le_bytes());
    feed(&img.height.to_le_bytes());
    feed(&(bytes.len() as u64).to_le_bytes());
    let n = bytes.len().min(2048);
    feed(&bytes[..n]);
    if bytes.len() > 4096 {
        feed(&bytes[bytes.len() - 2048..]);
    }
    format!("{:016x}", h)
}

/// Downscale an RGBA clipboard bitmap to at most `media::MAX_EDGE` on its
/// longest side before it is moved into the bounded capture channel.
///
/// Without this, an 8K screenshot carries ~660MB of raw RGBA that sits in the
/// channel (capacity) plus the worker until PNG encoding completes — a real OOM
/// risk on memory-constrained machines. `arboard` guarantees RGBA byte order,
/// which matches `image::RgbaImage`. See `media::downscale_rgba` (shared with
/// the on-disk store path).
pub fn downscale_captured_rgba_if_large(
    rgba: Vec<u8>,
    width: u32,
    height: u32,
) -> (Vec<u8>, u32, u32) {
    let (out, nw, nh) = crate::media::downscale_rgba(rgba, width, height, crate::media::MAX_EDGE);
    if nw != width || nh != height {
        debug!(
            "Downscaled captured image {}x{} -> {}x{}",
            width, height, nw, nh
        );
    }
    (out, nw, nh)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- image_quick_fingerprint ---

    #[test]
    fn quick_fp_is_deterministic() {
        let pixels = vec![42u8; 64 * 64 * 4];
        let img = ImageData {
            width: 64,
            height: 64,
            bytes: std::borrow::Cow::Owned(pixels),
        };
        assert_eq!(image_quick_fingerprint(&img), image_quick_fingerprint(&img));
    }

    #[test]
    fn quick_fp_differs_for_different_images() {
        let a = ImageData {
            width: 64,
            height: 64,
            bytes: std::borrow::Cow::Owned(vec![0u8; 64 * 64 * 4]),
        };
        let b = ImageData {
            width: 64,
            height: 64,
            bytes: std::borrow::Cow::Owned(vec![255u8; 64 * 64 * 4]),
        };
        assert_ne!(image_quick_fingerprint(&a), image_quick_fingerprint(&b));
    }

    #[test]
    fn quick_fp_differs_for_different_sizes() {
        let a = ImageData {
            width: 64,
            height: 64,
            bytes: std::borrow::Cow::Owned(vec![0u8; 64 * 64 * 4]),
        };
        let b = ImageData {
            width: 32,
            height: 32,
            bytes: std::borrow::Cow::Owned(vec![0u8; 32 * 32 * 4]),
        };
        assert_ne!(image_quick_fingerprint(&a), image_quick_fingerprint(&b));
    }

    #[test]
    fn quick_fp_ignores_middle_only_changes() {
        // Cheap-fp contract: only the head (first 2048 bytes) and tail (last
        // 2048 bytes) are sampled; a middle-only change must NOT alter the
        // fingerprint (the worker still hashes the full bytes via SHA-256).
        let len = 64 * 64 * 4;
        let a = vec![1u8; len];
        let mut b = a.clone();
        b[len / 2] = 99; // outside both sampled windows
        let img_a = ImageData {
            width: 64,
            height: 64,
            bytes: std::borrow::Cow::Owned(a),
        };
        let img_b = ImageData {
            width: 64,
            height: 64,
            bytes: std::borrow::Cow::Owned(b),
        };
        assert_eq!(
            image_quick_fingerprint(&img_a),
            image_quick_fingerprint(&img_b)
        );
    }

    #[test]
    fn quick_fp_detects_tail_changes() {
        // The last 2048 bytes ARE sampled — a tail-only change must differ.
        let len = 64 * 64 * 4;
        let a = vec![1u8; len];
        let mut b = a.clone();
        *b.last_mut().unwrap() = 2;
        let img_a = ImageData {
            width: 64,
            height: 64,
            bytes: std::borrow::Cow::Owned(a),
        };
        let img_b = ImageData {
            width: 64,
            height: 64,
            bytes: std::borrow::Cow::Owned(b),
        };
        assert_ne!(
            image_quick_fingerprint(&img_a),
            image_quick_fingerprint(&img_b)
        );
    }

    // --- downscale_captured_rgba_if_large ---

    #[test]
    fn small_image_passes_through_unchanged() {
        let w = 100u32;
        let h = 50u32;
        let rgba = vec![0u8; (w * h * 4) as usize];
        let (out_rgba, out_w, out_h) = downscale_captured_rgba_if_large(rgba.clone(), w, h);
        assert_eq!(out_w, w);
        assert_eq!(out_h, h);
        assert_eq!(out_rgba, rgba);
    }

    #[test]
    fn zero_size_passes_through() {
        let (out, ow, oh) = downscale_captured_rgba_if_large(vec![], 0, 0);
        assert_eq!(ow, 0);
        assert_eq!(oh, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn large_image_is_downscaled() {
        // Big enough to exceed MAX_EDGE (2560) without an 80MB buffer.
        let w = 3000u32;
        let h = 1000u32;
        let rgba = vec![128u8; (w * h * 4) as usize];
        let (out, ow, oh) = downscale_captured_rgba_if_large(rgba, w, h);
        // Both edges must be <= MAX_EDGE
        assert!(ow <= crate::media::MAX_EDGE);
        assert!(oh <= crate::media::MAX_EDGE);
        // Output buffer matches dimensions
        assert_eq!(out.len(), (ow * oh * 4) as usize);
        assert!(!out.is_empty());
    }

    #[test]
    fn large_image_with_stride_padding_is_normalized() {
        // arboard buffers can carry trailing stride padding (len > w*h*4);
        // the function must truncate to the expected size before resize.
        let w = 3000u32;
        let h = 100u32;
        let expected = (w * h * 4) as usize;
        let rgba = vec![128u8; expected + 64];
        let (out, ow, oh) = downscale_captured_rgba_if_large(rgba, w, h);
        assert!(ow <= crate::media::MAX_EDGE);
        assert!(oh <= crate::media::MAX_EDGE);
        assert_eq!(out.len(), (ow * oh * 4) as usize);
    }

    #[test]
    fn large_image_with_short_buffer_is_padded() {
        // Some sources return a short buffer (len < w*h*4); the function must
        // pad to the expected size before handing it to the image crate.
        let w = 3000u32;
        let h = 100u32;
        let expected = (w * h * 4) as usize;
        let rgba = vec![128u8; expected - 64];
        let (out, ow, oh) = downscale_captured_rgba_if_large(rgba, w, h);
        assert!(ow <= crate::media::MAX_EDGE);
        assert!(oh <= crate::media::MAX_EDGE);
        assert_eq!(out.len(), (ow * oh * 4) as usize);
    }
}
