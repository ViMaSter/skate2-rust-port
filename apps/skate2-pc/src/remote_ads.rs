//! Remote billboard ads (PC only, `docs/online/remote-ads.md`).
//!
//! The original fetched its in-game ads from a web server (`Sk8::Net::Massive`, Massive Ad
//! Client 3); that protocol is not recovered. This port's stand-in looks for a local ad server
//! (`skate2_net::ads`) once, when the first map generation asks, and hands the billboards its
//! ads in place of their placeholder textures. Without the `remote-ads` feature, or when no
//! server answers, [`active`] is `None` and every billboard keeps its own texture.

use crate::gpu_renderer::{GpuTexture, RenderResult};
use skate2_core::texture::NativeTexture;
use skate2_formats::material::{
    LinearTextureLevel, PIXEL_FORMAT_A8R8G8B8, TextureEndian, TextureType,
};
use skate2_net::ads::{AdListing, AdServer, bgra_mip_chain, rotation_index};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
    time::Instant,
};

static ADS: OnceLock<Option<RemoteAds>> = OnceLock::new();

#[derive(Default)]
struct Uploads {
    step: u64,
    textures: BTreeMap<usize, Option<GpuTexture>>,
}

/// The ads a server offered, decoded and ready to upload.
pub(crate) struct RemoteAds {
    host: String,
    listings: Vec<AdListing>,
    textures: Vec<NativeTexture>,
    started: Instant,
    rotation_seconds: u64,
    uploaded: Mutex<Uploads>,
}

/// The server's ads, looked for on first use; `None` for a build without the `remote-ads`
/// feature and when no server answers.
pub(crate) fn active() -> Option<&'static RemoteAds> {
    ADS.get_or_init(RemoteAds::discover).as_ref()
}

/// A stationary scene still needs a presentation when its billboard rotation advances.
pub(crate) fn redraw_due() -> RenderResult<bool> {
    let Some(ads) = ADS.get().and_then(Option::as_ref) else {
        return Ok(false);
    };
    let uploaded = ads
        .uploaded
        .lock()
        .map_err(|error| format!("remote ads: upload cache lock failed: {error}"))?;
    Ok(uploaded.step != ads.current_step())
}

/// Rotates already loaded ads without network traffic or rebuilding map bind groups.
/// Does not trigger discovery before a map first asks for an ad.
pub(crate) fn update(queue: &wgpu::Queue) -> RenderResult<()> {
    let Some(ads) = ADS.get().and_then(Option::as_ref) else {
        return Ok(());
    };
    let step = ads.current_step();
    let mut uploaded = ads
        .uploaded
        .lock()
        .map_err(|error| format!("remote ads: upload cache lock failed: {error}"))?;
    if uploaded.step == step {
        return Ok(());
    }
    let mut changed = 0;
    for (&initial, texture) in &uploaded.textures {
        let Some(texture) = texture else {
            continue;
        };
        let index = rotation_index(&ads.listings, initial, step)
            .ok_or("remote ads: uploaded ad missing from rotation")?;
        if Some(index) != rotation_index(&ads.listings, initial, uploaded.step) {
            texture.replace_bgra(queue, &ads.textures[index])?;
            changed += 1;
        }
    }
    uploaded.step = step;
    if changed > 0 && std::env::var_os("SKATE2_ADS_LOG").is_some() {
        eprintln!("remote ads: rotation {step}, updated {changed} textures");
    }
    Ok(())
}

impl RemoteAds {
    fn current_step(&self) -> u64 {
        self.started
            .elapsed()
            .as_secs()
            .checked_div(self.rotation_seconds)
            .unwrap_or(0)
    }

    fn discover() -> Option<Self> {
        if !cfg!(feature = "remote-ads") {
            return None;
        }
        let (server, offered) = AdServer::discover()?;
        let mut listings = Vec::new();
        let mut textures = Vec::new();
        for listing in offered {
            let decoded = server
                .image(&listing)
                .and_then(|bytes| decode(&listing, &bytes));
            match decoded {
                Ok(texture) => {
                    listings.push(listing);
                    textures.push(texture);
                }
                Err(error) => eprintln!("remote ads: skipped {}: {error}", listing.id),
            }
        }
        if listings.is_empty() {
            return None;
        }
        eprintln!(
            "remote ads: {} ads from {} (billboards get one of their size, else of their shape)",
            listings.len(),
            server.host()
        );
        let rotation_seconds = match std::env::var("SKATE2_ADS_ROTATE_SECONDS") {
            Ok(value) => match value.parse::<u64>() {
                Ok(seconds) => seconds,
                Err(error) => {
                    eprintln!(
                        "remote ads: invalid SKATE2_ADS_ROTATE_SECONDS {value:?}: {error}; using 10"
                    );
                    10
                }
            },
            Err(std::env::VarError::NotPresent) => 10,
            Err(error) => {
                eprintln!("remote ads: cannot read SKATE2_ADS_ROTATE_SECONDS: {error}; using 10");
                10
            }
        };
        eprintln!("remote ads: rotation every {rotation_seconds}s (0 disables rotation)");
        Some(Self {
            host: server.host().to_owned(),
            listings,
            textures,
            started: Instant::now(),
            rotation_seconds,
            uploaded: Mutex::new(Uploads::default()),
        })
    }

    /// The server the ads came from.
    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    /// The ad for a billboard whose placeholder is `width` x `height`. `slot` is the
    /// placeholder's asset number (the high half of its texture GUID): billboards of one
    /// placeholder agree, and the numbered variants of a sign (`..._v_01`, `_02`, `_03`)
    /// get different ads.
    pub(crate) fn select(&self, slot: usize, width: u32, height: u32) -> Option<usize> {
        skate2_net::ads::select(&self.listings, slot, width, height)
    }

    /// The uploaded ad, shared by every billboard that selects it. A failed upload is
    /// remembered, so the billboards that would have used it keep their own texture.
    pub(crate) fn gpu_texture(
        &self,
        index: usize,
        upload: impl FnOnce(&NativeTexture) -> RenderResult<GpuTexture>,
    ) -> Option<GpuTexture> {
        let mut uploaded = self
            .uploaded
            .lock()
            .inspect_err(|error| eprintln!("remote ads: upload cache lock failed: {error}"))
            .ok()?;
        let current = rotation_index(&self.listings, index, uploaded.step)?;
        uploaded
            .textures
            .entry(index)
            .or_insert_with(|| {
                let texture = self.textures.get(current)?;
                upload(texture)
                    .inspect_err(|error| {
                        eprintln!("remote ads: upload of {} failed: {error}", texture.name);
                    })
                    .ok()
            })
            .clone()
    }
}

/// With `SKATE2_ADS_LOG` set, names where the ad billboards are: the centre of each one's
/// vertices, which the `advertisement` effect takes as world space.
pub(crate) fn log_billboard(scope: &str, positions: &[[f32; 3]]) {
    if positions.is_empty() || active().is_none() || std::env::var_os("SKATE2_ADS_LOG").is_none() {
        return;
    }
    let mut sum = [0.0_f64; 3];
    for position in positions {
        for (total, &coordinate) in sum.iter_mut().zip(position) {
            *total += f64::from(coordinate);
        }
    }
    let count = f64::from(u32::try_from(positions.len()).unwrap_or(u32::MAX));
    eprintln!(
        "remote ads: billboard {scope} near ({:.1}, {:.1}, {:.1})",
        sum[0] / count,
        sum[1] / count,
        sum[2] / count
    );
}

/// An ad's JPEG as a full-chain A8R8G8B8 texture, the size the listing promised.
fn decode(listing: &AdListing, bytes: &[u8]) -> Result<NativeTexture, String> {
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
    let pixels = decoder.decode().map_err(|error| error.to_string())?;
    let info = decoder.info().ok_or("no image info")?;
    if info.pixel_format != jpeg_decoder::PixelFormat::RGB24 {
        return Err(format!("{:?} is not RGB", info.pixel_format));
    }
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    if (width, height) != (listing.width, listing.height) {
        return Err(format!(
            "{width}x{height}, not the listed {}x{}",
            listing.width, listing.height
        ));
    }
    let base = pixels
        .chunks_exact(3)
        .flat_map(|rgb| [rgb[2], rgb[1], rgb[0], 255])
        .collect();
    let linear_levels: Vec<LinearTextureLevel> = bgra_mip_chain(width, height, base)
        .into_iter()
        .zip(0..)
        .map(|(mip, level)| LinearTextureLevel {
            level,
            face: 0,
            width: mip.width,
            height: mip.height,
            width_in_blocks: mip.width,
            height_in_blocks: mip.height,
            bytes_per_block: 4,
            row_pitch: mip.width * 4,
            data: mip.bgra,
        })
        .collect();
    Ok(NativeTexture {
        dictionary_index: 0,
        guid: 0,
        name: format!("remote ad {}", listing.id),
        texture_type: TextureType::TwoDimensional,
        endian: TextureEndian::None,
        width,
        height,
        depth: 1,
        mip_levels: u32::try_from(linear_levels.len()).map_err(|error| error.to_string())?,
        pixel_format: PIXEL_FORMAT_A8R8G8B8,
        packed_mips: false,
        mip_data_offset: 0,
        encoded_data: Vec::new(),
        linear_levels,
    })
}
