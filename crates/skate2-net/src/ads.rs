//! The PC port's remote billboard ads: a client of the local ad server that stands in for the web
//! server the original fetched its in-game ads from (`Sk8::Net::Massive`, Massive Ad Client 3).
//! The original's protocol is not recovered, so this one is the port's own and a PC-only
//! addition (`docs/online/remote-ads.md`).
//!
//! The client finds a server without being told where it is: [`AdServer::discover`] asks the
//! loopback address on [`DEFAULT_PORT`], or the server named by [`URL_VARIABLE`]. When nothing
//! answers, it returns `None` and the game keeps the world's own billboards.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// The port the stock ad server listens on, and the one discovery tries on this machine.
pub const DEFAULT_PORT: u16 = 8745;
/// Names a server (`host:port` or `http://host:port`) to use instead of looking on this machine.
pub const URL_VARIABLE: &str = "SKATE2_ADS_URL";

const API: &str = "/skate2-ads/v1";
const MAGIC: &str = "skate2-ads 1";
/// A server on this machine answers or refuses at once; this only bounds a named remote one.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const IO_TIMEOUT: Duration = Duration::from_secs(2);
const MANIFEST_LIMIT: usize = 64 * 1024;
const IMAGE_LIMIT: usize = 4 * 1024 * 1024;
const MAX_ADS: usize = 64;
const MAX_SIDE: u32 = 2048;

/// One ad the server offers: its texture size and where its image is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdListing {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub path: String,
}

/// A server that answered discovery.
#[derive(Clone, Debug)]
pub struct AdServer {
    host: String,
}

impl AdServer {
    /// The first candidate that answers `ping` and lists at least one ad.
    #[must_use]
    pub fn discover() -> Option<(Self, Vec<AdListing>)> {
        let named = std::env::var(URL_VARIABLE).ok();
        candidate_hosts(named.as_deref())
            .into_iter()
            .find_map(|host| Self::connect(&host))
    }

    /// `host` as `host:port`: the server if it speaks this protocol, with its ads.
    #[must_use]
    pub fn connect(host: &str) -> Option<(Self, Vec<AdListing>)> {
        let server = Self {
            host: host.to_owned(),
        };
        let ping = server.get(&format!("{API}/ping"), 256).ok()?;
        if !String::from_utf8_lossy(&ping).trim().eq(MAGIC) {
            return None;
        }
        let manifest = server
            .get(&format!("{API}/manifest.txt"), MANIFEST_LIMIT)
            .ok()?;
        let listings = parse_manifest(&String::from_utf8_lossy(&manifest));
        (!listings.is_empty()).then_some((server, listings))
    }

    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The ad's image file (JPEG).
    ///
    /// # Errors
    /// The server cannot be reached, refuses, or the image is too large.
    pub fn image(&self, listing: &AdListing) -> Result<Vec<u8>, String> {
        self.get(&listing.path, IMAGE_LIMIT)
    }

    /// HTTP/1.0 `GET`: the server closes the connection after its answer, so a body needs no
    /// chunk or keep-alive handling.
    fn get(&self, path: &str, limit: usize) -> Result<Vec<u8>, String> {
        let address = self
            .host
            .to_socket_addrs()
            .map_err(|error| format!("{}: {error}", self.host))?
            .next()
            .ok_or_else(|| format!("{}: no address", self.host))?;
        let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)
            .map_err(|error| format!("{}: {error}", self.host))?;
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(IO_TIMEOUT))
            .map_err(|error| error.to_string())?;
        let request = format!(
            "GET {path} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\nAccept: */*\r\n\r\n",
            self.host
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|error| error.to_string())?;
        // The head is read along with the body, so its allowance is added to the limit.
        let mut answer = Vec::new();
        stream
            .take(u64::try_from(limit + 8 * 1024).unwrap_or(u64::MAX))
            .read_to_end(&mut answer)
            .map_err(|error| error.to_string())?;
        parse_answer(&answer, limit)
    }
}

/// The servers discovery tries, in order: the named one alone, else this machine's loopback
/// addresses on [`DEFAULT_PORT`].
#[must_use]
pub fn candidate_hosts(named: Option<&str>) -> Vec<String> {
    if let Some(text) = named {
        let text = text.trim();
        let host = text.strip_prefix("http://").unwrap_or(text);
        let host = host.split('/').next().unwrap_or_default();
        if host.is_empty() {
            return Vec::new();
        }
        let has_port = host
            .rsplit_once(':')
            .is_some_and(|(_, port)| port.parse::<u16>().is_ok());
        return vec![if has_port {
            host.to_owned()
        } else {
            format!("{host}:{DEFAULT_PORT}")
        }];
    }
    vec![
        format!("127.0.0.1:{DEFAULT_PORT}"),
        format!("[::1]:{DEFAULT_PORT}"),
    ]
}

fn parse_answer(answer: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    let split = answer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or("incomplete HTTP answer")?;
    let head = String::from_utf8_lossy(&answer[..split]);
    let body = &answer[split + 4..];
    let mut lines = head.lines();
    let status = lines.next().unwrap_or_default();
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(format!("HTTP status {status:?}"));
    }
    let declared = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    });
    if declared.is_some_and(|length| length != body.len()) {
        return Err("HTTP body differs from its Content-Length".to_owned());
    }
    if body.len() > limit {
        return Err(format!("HTTP body over {limit} bytes"));
    }
    Ok(body.to_vec())
}

/// The manifest's ads: a first line `skate2-ads 1`, then `ad <id> <width> <height> <path>` lines.
/// A line that is not well formed is passed over, so one bad entry costs only its own ad.
#[must_use]
pub fn parse_manifest(text: &str) -> Vec<AdListing> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some(MAGIC) {
        return Vec::new();
    }
    lines
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            if fields.next()? != "ad" {
                return None;
            }
            let id = fields.next()?;
            let width: u32 = fields.next()?.parse().ok()?;
            let height: u32 = fields.next()?.parse().ok()?;
            let path = fields.next()?;
            let id_ok = id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
            // The path goes into a request line, so it must not carry anything that ends it.
            let path_ok = path.starts_with('/') && path.bytes().all(|byte| byte.is_ascii_graphic());
            let size_ok = (1..=MAX_SIDE).contains(&width) && (1..=MAX_SIDE).contains(&height);
            (fields.next().is_none() && id_ok && path_ok && size_ok).then(|| AdListing {
                id: id.to_owned(),
                width,
                height,
                path: path.to_owned(),
            })
        })
        .take(MAX_ADS)
        .collect()
}

/// The ad for a billboard whose placeholder texture is `width` x `height`: among the ads of that
/// size (else of that shape, so a larger or smaller copy of the placeholder), the one `slot`
/// lands on. `slot` is the placeholder's own number, so the billboards of one placeholder agree
/// and different placeholders of a size tend to differ.
#[must_use]
pub fn select(listings: &[AdListing], slot: usize, width: u32, height: u32) -> Option<usize> {
    let of_size: Vec<usize> = (0..listings.len())
        .filter(|&index| listings[index].width == width && listings[index].height == height)
        .collect();
    let pool = if of_size.is_empty() {
        let shape =
            |w: u32, h: u32| u64::from(w) * u64::from(height) == u64::from(h) * u64::from(width);
        (0..listings.len())
            .filter(|&index| shape(listings[index].width, listings[index].height))
            .collect()
    } else {
        of_size
    };
    (!pool.is_empty()).then(|| pool[slot % pool.len()])
}

/// Advances from an initially selected ad through entries of that exact size, in manifest
/// order, wrapping at the end. Keeping dimensions fixed lets the renderer reuse its bindings.
/// An out-of-range initial index has no rotation.
#[must_use]
pub fn rotation_index(listings: &[AdListing], initial: usize, step: u64) -> Option<usize> {
    let first = listings.get(initial)?;
    let pool: Vec<usize> = (0..listings.len())
        .filter(|&index| {
            listings[index].width == first.width && listings[index].height == first.height
        })
        .collect();
    let offset = pool.iter().position(|&index| index == initial)?;
    let advance = usize::try_from(step % u64::try_from(pool.len()).ok()?).ok()?;
    Some(pool[(offset + advance) % pool.len()])
}

/// One level of a [`bgra_mip_chain`]: row-major, tightly packed, 4 bytes (B, G, R, A) a pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MipLevel {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// `base` and its box-filtered half-size levels down to 1x1: the full chain a GPU texture of
/// that size takes, which is what a billboard seen from afar samples. Empty when `base` is not
/// `width` x `height` pixels of 4 bytes.
#[must_use]
pub fn bgra_mip_chain(width: u32, height: u32, base: Vec<u8>) -> Vec<MipLevel> {
    let mut levels = Vec::new();
    if width == 0 || height == 0 || base.len() != width as usize * height as usize * 4 {
        return levels;
    }
    let mut current = MipLevel {
        width,
        height,
        bgra: base,
    };
    while current.width > 1 || current.height > 1 {
        let next = halved(&current);
        levels.push(std::mem::replace(&mut current, next));
    }
    levels.push(current);
    levels
}

/// Each pixel is the rounded mean of the 2x2 source pixels (an odd edge repeats its last one).
fn halved(level: &MipLevel) -> MipLevel {
    let (width, height) = ((level.width / 2).max(1), (level.height / 2).max(1));
    let (source_width, source_height) = (level.width as usize, level.height as usize);
    let mut bgra = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height as usize {
        let rows = [2 * y, (2 * y + 1).min(source_height - 1)];
        for x in 0..width as usize {
            let columns = [2 * x, (2 * x + 1).min(source_width - 1)];
            for channel in 0..4 {
                let sum: u32 = rows
                    .iter()
                    .flat_map(|&row| columns.iter().map(move |&column| (row, column)))
                    .map(|(row, column)| {
                        u32::from(level.bgra[(row * source_width + column) * 4 + channel])
                    })
                    .sum();
                bgra.push(u8::try_from((sum + 2) / 4).unwrap_or(u8::MAX));
            }
        }
    }
    MipLevel {
        width,
        height,
        bgra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn listing(id: &str, width: u32, height: u32) -> AdListing {
        AdListing {
            id: id.to_owned(),
            width,
            height,
            path: format!("/image/{id}.jpg"),
        }
    }

    #[test]
    fn manifest_lists_well_formed_ads_and_skips_the_rest() {
        let text = "skate2-ads 1\n\
                    ad a 256 128 /skate2-ads/v1/image/a.jpg\n\
                    ad b 0 128 /image/b.jpg\n\
                    ad c 128 128 image/c.jpg\n\
                    ad d 64 64 /image/d.jpg extra\n\
                    ad e/x 64 64 /image/e.jpg\n\
                    note nothing\n\
                    ad f 64 32 /image/f.jpg\n";
        let ads = parse_manifest(text);
        assert_eq!(
            ads,
            vec![
                AdListing {
                    id: "a".into(),
                    width: 256,
                    height: 128,
                    path: "/skate2-ads/v1/image/a.jpg".into()
                },
                AdListing {
                    id: "f".into(),
                    width: 64,
                    height: 32,
                    path: "/image/f.jpg".into()
                },
            ]
        );
    }

    #[test]
    fn manifest_without_its_first_line_is_empty() {
        assert_eq!(parse_manifest("ad a 64 64 /a.jpg\n"), Vec::new());
        assert_eq!(
            parse_manifest("skate2-ads 2\nad a 64 64 /a.jpg\n"),
            Vec::new()
        );
    }

    #[test]
    fn manifest_stops_at_the_ad_limit() {
        let text: String = std::iter::once("skate2-ads 1\n".to_owned())
            .chain((0..MAX_ADS + 10).map(|number| format!("ad n{number} 64 64 /n{number}.jpg\n")))
            .collect();
        assert_eq!(parse_manifest(&text).len(), MAX_ADS);
    }

    #[test]
    fn selection_prefers_the_exact_size_then_the_shape() {
        let ads = [
            listing("a", 256, 128),
            listing("b", 128, 256),
            listing("c", 256, 128),
            listing("d", 64, 32),
        ];
        assert_eq!(select(&ads, 0, 256, 128), Some(0));
        assert_eq!(select(&ads, 1, 256, 128), Some(2));
        assert_eq!(select(&ads, 2, 256, 128), Some(0));
        assert_eq!(select(&ads, 5, 128, 256), Some(1));
        // 512x256 has no ad of its own size: the 2:1 ads serve it.
        assert_eq!(select(&ads, 0, 512, 256), Some(0));
        assert_eq!(select(&ads, 1, 512, 256), Some(2));
        assert_eq!(select(&ads, 2, 512, 256), Some(3));
        assert_eq!(select(&ads, 0, 100, 100), None);
        assert_eq!(select(&[], 0, 64, 64), None);
    }

    #[test]
    fn rotation_preserves_size_order_and_initial_slot_offset() {
        let ads = [
            listing("a", 256, 128),
            listing("portrait", 128, 256),
            listing("b", 256, 128),
            listing("larger", 512, 256),
            listing("c", 256, 128),
        ];
        let cycle: Vec<_> = (0..5).map(|step| rotation_index(&ads, 2, step)).collect();
        assert_eq!(cycle, [Some(2), Some(4), Some(0), Some(2), Some(4)]);
        assert_eq!(rotation_index(&ads, 0, u64::MAX), Some(0));
        assert_eq!(rotation_index(&ads, 1, 200), Some(1));
        assert_eq!(rotation_index(&ads, 3, 200), Some(3));
        assert_eq!(rotation_index(&ads, 5, 1), None);
        assert_eq!(rotation_index(&[], 0, 1), None);
    }

    #[test]
    fn a_mip_chain_averages_down_to_one_pixel() {
        // 2x2: blue 0, 100, 200, 255 average to 139 (138.75 rounded); alpha stays 255.
        let base = vec![
            0, 0, 0, 255, //
            100, 0, 0, 255, //
            200, 0, 0, 255, //
            255, 0, 0, 255,
        ];
        let chain = bgra_mip_chain(2, 2, base.clone());
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].bgra, base);
        assert_eq!(
            (chain[1].width, chain[1].height, chain[1].bgra.clone()),
            (1, 1, vec![139, 0, 0, 255])
        );
    }

    #[test]
    fn a_mip_chain_has_the_levels_a_texture_of_that_size_allows() {
        for (width, height) in [
            (512_u32, 256_u32),
            (128, 256),
            (64, 32),
            (3, 1),
            (5, 7),
            (1, 1),
        ] {
            let base = vec![200; width as usize * height as usize * 4];
            let chain = bgra_mip_chain(width, height, base);
            let allowed = 32 - width.max(height).leading_zeros();
            assert_eq!(chain.len(), allowed as usize, "{width}x{height}");
            for (level, mip) in chain.iter().enumerate() {
                assert_eq!(mip.width, (width >> level).max(1));
                assert_eq!(mip.height, (height >> level).max(1));
                assert_eq!(mip.bgra.len(), mip.width as usize * mip.height as usize * 4);
                assert!(mip.bgra.iter().all(|&byte| byte == 200));
            }
            let last = chain.last().expect("a level");
            assert_eq!((last.width, last.height), (1, 1));
        }
    }

    #[test]
    fn a_mip_chain_needs_the_right_amount_of_pixels() {
        assert_eq!(bgra_mip_chain(2, 2, vec![0; 15]), Vec::new());
        assert_eq!(bgra_mip_chain(0, 2, Vec::new()), Vec::new());
    }

    #[test]
    fn named_servers_get_a_port_and_none_means_loopback() {
        assert_eq!(
            candidate_hosts(Some("http://ads.example:9000/x")),
            ["ads.example:9000"]
        );
        assert_eq!(
            candidate_hosts(Some(" 10.0.0.2 ")),
            [format!("10.0.0.2:{DEFAULT_PORT}")]
        );
        assert_eq!(candidate_hosts(Some("[::1]:1234")), ["[::1]:1234"]);
        assert_eq!(candidate_hosts(Some("  ")), Vec::<String>::new());
        assert_eq!(
            candidate_hosts(None),
            [
                format!("127.0.0.1:{DEFAULT_PORT}"),
                format!("[::1]:{DEFAULT_PORT}")
            ]
        );
    }

    #[test]
    fn answers_must_be_complete_ok_replies() {
        assert_eq!(
            parse_answer(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc", 10),
            Ok(b"abc".to_vec())
        );
        assert_eq!(
            parse_answer(b"HTTP/1.0 200 OK\r\n\r\nabc", 10),
            Ok(b"abc".to_vec())
        );
        assert!(parse_answer(b"HTTP/1.1 404 Not Found\r\n\r\nno", 10).is_err());
        assert!(parse_answer(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nabc", 10).is_err());
        assert!(parse_answer(b"HTTP/1.1 200 OK\r\n\r\nabcdef", 3).is_err());
        assert!(parse_answer(b"HTTP/1.1 200 OK", 10).is_err());
    }

    /// Answers `requests` connections with `reply(path)`.
    fn serve(requests: usize, reply: fn(&str) -> Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let host = listener.local_addr().expect("address").to_string();
        std::thread::spawn(move || {
            for _ in 0..requests {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut request = [0_u8; 1024];
                let length = stream.read(&mut request).unwrap_or(0);
                let text = String::from_utf8_lossy(&request[..length]).into_owned();
                let path = text
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                let _ = stream.write_all(&reply(&path));
            }
        });
        host
    }

    fn fake_server(path: &str) -> Vec<u8> {
        let (status, body): (&str, &[u8]) = match path {
            "/skate2-ads/v1/ping" => ("200 OK", b"skate2-ads 1\n"),
            "/skate2-ads/v1/manifest.txt" => (
                "200 OK",
                b"skate2-ads 1\nad a 64 32 /skate2-ads/v1/image/a.jpg\n",
            ),
            "/skate2-ads/v1/image/a.jpg" => ("200 OK", b"jpeg bytes"),
            _ => ("404 Not Found", b""),
        };
        format!(
            "HTTP/1.0 {status}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes()
        .into_iter()
        .chain(body.iter().copied())
        .collect()
    }

    #[test]
    fn a_server_that_speaks_the_protocol_is_discovered() {
        let host = serve(3, fake_server);
        let (server, ads) = AdServer::connect(&host).expect("server answers");
        assert_eq!(server.host(), host);
        assert_eq!(
            ads,
            vec![listing_at("a", 64, 32, "/skate2-ads/v1/image/a.jpg")]
        );
        assert_eq!(server.image(&ads[0]).as_deref(), Ok(&b"jpeg bytes"[..]));
    }

    fn listing_at(id: &str, width: u32, height: u32, path: &str) -> AdListing {
        AdListing {
            id: id.to_owned(),
            width,
            height,
            path: path.to_owned(),
        }
    }

    #[test]
    fn a_server_that_is_not_an_ad_server_is_ignored() {
        let host = serve(1, |_| b"HTTP/1.0 200 OK\r\n\r\nhello".to_vec());
        assert!(AdServer::connect(&host).is_none());
    }

    #[test]
    fn nothing_listening_finds_nothing() {
        let host = {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            listener.local_addr().expect("address").to_string()
        };
        assert!(AdServer::connect(&host).is_none());
    }
}
