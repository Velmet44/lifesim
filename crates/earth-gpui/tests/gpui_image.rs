#![cfg(feature = "gpui-ui")]
//! Path-B → gpui interop proof: CPU RGBA8 frame becomes a gpui ImageSource.
//! Run with: cargo test -p earth-gpui --features gpui-ui --test gpui_image
use earth_core::{GridPreset, World};
use earth_gpui::{to_gpui_image_source, PathBRenderer};

#[test]
fn gpui_image_source_roundtrip() {
    let world = World::fresh(GridPreset::L0, 42, "formation", -4_540_000_000.0);
    let r = PathBRenderer::new(64, 32);
    let px = r.render_cpu(&world);
    let src = to_gpui_image_source(&px, 64, 32);
    match src {
        gpui::ImageSource::Render(data) => {
            assert_eq!(data.frame_count(), 1);
        }
        _ => panic!("expected ImageSource::Render"),
    }
}
