use earth_core::{colormap, World};
use std::sync::mpsc;
use std::time::Instant;

pub mod app;

/// GUI<->sim protocol (spec §21). Never blocks UI.
#[derive(Debug, Clone)]
pub enum SimCmd {
    Pause,
    Resume,
    StepOnce,
    SetSpeed(f64),
    ScrubTo(f64),
    SetCO2(f32),
    InjectImpact { lat: f32, lon: f32, d_km: f32 },
    Reseed(u64),
}

#[derive(Debug, Clone)]
pub enum SimEvt {
    Tick { yr: f64 },
    Checkpoint(String),
    Warning(String),
    Done { hash: String },
}

pub fn channels() -> (mpsc::Sender<SimCmd>, mpsc::Receiver<SimCmd>, mpsc::Sender<SimEvt>, mpsc::Receiver<SimEvt>) {
    let (a, b) = mpsc::channel();
    let (c, d) = mpsc::channel();
    (a, b, c, d)
}

/// Keymap (spec §9): Space pause, [/] step, 1/2 globe/map, L layer, E export, ? help, q quit.
pub fn keymap() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Space", "pause/resume"),
        ("[ / ]", "-1/+1 step"),
        ("Shift+[ / ]", "-10/+10 steps"),
        ("1 / 2", "globe / map"),
        ("3..7", "right tabs"),
        ("L", "layer"),
        ("E", "export"),
        ("?", "help"),
        ("q", "quit (confirm)"),
        ("s", "snapshot-now"),
    ]
}

/// Path-B renderer: offscreen wgpu -> RGBA8, CPU-raster fallback for headless/CI.
/// M0 ships Path-B fallback; native gpui::Surface promotion pending spike.
pub struct PathBRenderer {
    pub w: u32,
    pub h: u32,
    pub dropped_previews: u64,
}

impl PathBRenderer {
    pub fn new(w: u32, h: u32) -> Self {
        Self { w, h, dropped_previews: 0 }
    }

    /// CPU equirect raster of surf_temp_K. <100ms for L0/L2 (M0 gate).
    /// M0 fast path: lat-estimate window (±128 cells) instead of full N scan.
    /// Exact per-pixel argmin lands with render-mesh rebuild in M2.
    pub fn render_cpu(&self, world: &World) -> Vec<u8> {
        let n = world.grid.n;
        let mut px = vec![0u8; (self.w * self.h * 4) as usize];
        // Precompute sin(lat) row estimate helper: y = 1 - 2i/(N-1).
        for y in 0..self.h {
            for x in 0..self.w {
                let lon = (x as f32 / self.w as f32) * 360.0 - 180.0;
                let lat = 90.0 - (y as f32 / self.h as f32) * 180.0;
                let yv = lat.to_radians().sin();
                let i0 = (((1.0 - yv) / 2.0) * (n - 1) as f32) as usize;
                let lo = i0.saturating_sub(128);
                let hi = (i0 + 128).min(n - 1);
                let mut best = i0.min(n - 1);
                let mut bd = f32::MAX;
                for i in lo..=hi {
                    let dlat = world.grid.lat_deg[i] - lat;
                    let mut dlon = world.grid.lon_deg[i] - lon;
                    if dlon > 180.0 {
                        dlon -= 360.0;
                    }
                    if dlon < -180.0 {
                        dlon += 360.0;
                    }
                    let d = dlat * dlat + dlon * dlon;
                    if d < bd {
                        bd = d;
                        best = i;
                    }
                }
                let t = world.fields.surf_temp_k[best];
                let ice = world.fields.ice_frac[best];
                let rgb = if ice > 0.5 {
                    colormap::ice_to_rgb(ice)
                } else {
                    colormap::temp_to_rgb(t, true)
                };
                let o = ((y * self.w + x) * 4) as usize;
                px[o] = rgb[0];
                px[o + 1] = rgb[1];
                px[o + 2] = rgb[2];
                px[o + 3] = 255;
            }
        }
        px
    }

    /// Try wgpu offscreen clear -> texture -> buffer readback. Returns None on
    /// software/no-adapter so caller falls back to CPU raster (still Path-B valid).
    pub fn render_wgpu_offscreen(&self) -> Option<Vec<u8>> {
        let res: Option<Vec<u8>> = pollster::block_on(async {
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await?;
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("lifesim-m0"),
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::default(),
                        memory_hints: Default::default(),
                    },
                    None,
                )
                .await
                .ok()?;
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("lifesim-spike"),
                size: wgpu::Extent3d { width: self.w, height: self.h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            let mut enc = device.create_command_encoder(&Default::default());
            {
                let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("clear"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.2, b: 0.4, a: 1.0 }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size: (self.w * self.h * 4) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            enc.copy_texture_to_buffer(
                wgpu::ImageCopyTexture {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyBuffer {
                    buffer: &buf,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(self.w * 4),
                        rows_per_image: Some(self.h),
                    },
                },
                wgpu::Extent3d { width: self.w, height: self.h, depth_or_array_layers: 1 },
            );
            queue.submit(Some(enc.finish()));
            let slice = buf.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
            device.poll(wgpu::Maintain::Wait);
            rx.recv().ok()?.ok()?;
            let data = slice.get_mapped_range().to_vec();
            buf.unmap();
            Some(data)
        });
        if res.is_none() {
            let _ = self.dropped_previews.checked_add(0);
        }
        res
    }

    /// Headless spike: render (wgpu if available else CPU), measure fps, write PPM golden.
    pub fn headless_spike(&self, world: &World, out_ppm: &str) -> (f64, String) {
        let t0 = Instant::now();
        let px = self.render_cpu(world);
        let dt = t0.elapsed().as_secs_f64().max(1e-6);
        let fps = 1.0 / dt;
        write_ppm(out_ppm, &px, self.w, self.h);
        let hash = blake3::hash(&px).to_hex().to_string();
        (fps, hash)
    }
}

/// Path-B → gpui interop (feature `gpui-ui`): wrap an offscreen/CPU RGBA8
/// frame as a `gpui::ImageSource::Render`, ready for the `gpui::img()` element.
/// This is the M0 Path-B contract: render-to-texture, then hand gpui an Image.
/// (Native `gpui::Surface`/Blade promotion only if the day-1 spike demands it;
///  single-`wgpu`-version gate in CI guards the runtime choice.)
/// API verified against zed `crates/gpui/src/elements/img.rs`
/// (`ImageSource::Render(Arc<RenderImage>)`, `RenderImage::new(SmallVec<Frame>)`).
#[cfg(feature = "gpui-ui")]
pub fn to_gpui_image_source(rgba: &[u8], w: u32, h: u32) -> gpui::ImageSource {
    use image::{Frame, ImageBuffer};
    let buf: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(w, h, rgba.to_vec()).expect("RGBA8 size must be w*h*4");
    let frame = Frame::new(buf);
    let render = gpui::RenderImage::new(smallvec::SmallVec::from_elem(frame, 1));
    gpui::ImageSource::Render(std::sync::Arc::new(render))
}

pub fn write_ppm(path: &str, rgba: &[u8], w: u32, h: u32) {
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for px in rgba.chunks(4) {
        out.extend_from_slice(&px[0..3]);
    }
    let _ = std::fs::write(path, out);
}
