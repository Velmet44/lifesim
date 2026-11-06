//! Shared credibility colormaps (GUI + CLI use same LUT).
//! Rules: no rainbow; fixed vmin/vmax per layer+era; colorblind-safe default on.

pub fn temp_to_rgb(t_k: f32, twin: bool) -> [u8; 3] {
    // RdBu diverging anomaly: ±8K twin / ±20K deep-time around 288K.
    let span = if twin { 8.0 } else { 20.0 };
    let a = ((t_k - 288.0) / span).clamp(-1.0, 1.0);
    if a >= 0.0 {
        // white -> red
        [255, (255.0 * (1.0 - a)) as u8, (255.0 * (1.0 - a)) as u8]
    } else {
        // white -> blue
        [(255.0 * (1.0 + a)) as u8, (255.0 * (1.0 + a)) as u8, 255]
    }
}

pub fn elev_to_rgb(elev_m: f32) -> [u8; 3] {
    // Hypsometric: ocean blues, land greens/browns.
    if elev_m < 0.0 {
        let d = (-elev_m / 6000.0).clamp(0.0, 1.0);
        [0, (60.0 + 80.0 * (1.0 - d)) as u8, (140.0 + 80.0 * (1.0 - d)) as u8]
    } else {
        let h = (elev_m / 5000.0).clamp(0.0, 1.0);
        [(60.0 + 150.0 * h) as u8, (140.0 - 40.0 * h) as u8, 60]
    }
}

pub fn ice_to_rgb(frac: f32) -> [u8; 3] {
    let f = frac.clamp(0.0, 1.0);
    [(240.0 * f + 30.0 * (1.0 - f)) as u8, (245.0 * f + 30.0 * (1.0 - f)) as u8, 255]
}
