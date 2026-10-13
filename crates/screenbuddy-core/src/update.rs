//! Auto-update: version checking, downloading, and installation.
//!
//! Compares the running version against a remote manifest, downloads the
//! installer if a newer version exists, and launches it.
//!
//! The manifest is a JSON file served over HTTPS:
//! ```json
//! {
//!   "version": "0.7.0",
//!   "url": "https://example.com/screenbuddy-0.7.0-setup.exe",
//!   "sha256": "abc123...",
//!   "notes": "Bug fixes and performance improvements",
//!   "mandatory": false
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Semantic version with comparison support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub prerelease: Option<String>,
}

impl Version {
    /// Parse a semantic version string.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim_start_matches('v');
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p.to_string())),
            None => (s, None),
        };
        let parts: Vec<&str> = core.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
            prerelease: pre,
        })
    }

    /// Returns true if this version is newer than `other`.
    pub fn is_newer_than(&self, other: &Self) -> bool {
        self.major > other.major
            || (self.major == other.major
                && (self.minor > other.minor
                    || (self.minor == other.minor && self.patch > other.patch)))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.prerelease {
            write!(f, "-{}", pre)?;
        }
        Ok(())
    }
}

/// The update manifest fetched from the remote server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub mandatory: bool,
}

/// Result of an update check.
#[derive(Debug, Clone)]
pub enum UpdateCheck {
    /// No update available — already on the latest version.
    UpToDate,
    /// A newer version is available.
    Available(UpdateManifest),
    /// The check failed (network error, parse error, etc.).
    Failed(String),
}

/// Checks for updates by fetching the manifest from `manifest_url`.
///
/// Uses a 10-second timeout. Returns `UpdateCheck::Available` if the
/// remote version is newer than `current`.
pub async fn check_for_update(manifest_url: &str, current: &Version) -> UpdateCheck {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!("ScreenBuddy/{}", current))
        .build()
    {
        Ok(c) => c,
        Err(e) => return UpdateCheck::Failed(e.to_string()),
    };

    let response = match client.get(manifest_url).send().await {
        Ok(r) => r,
        Err(e) => return UpdateCheck::Failed(e.to_string()),
    };

    if !response.status().is_success() {
        return UpdateCheck::Failed(format!("server returned {}", response.status()));
    }

    let manifest: UpdateManifest = match response.json().await {
        Ok(m) => m,
        Err(e) => return UpdateCheck::Failed(e.to_string()),
    };

    let remote = match Version::parse(&manifest.version) {
        Some(v) => v,
        None => {
            return UpdateCheck::Failed(format!(
                "invalid version in manifest: {}",
                manifest.version
            ))
        }
    };

    if remote.is_newer_than(current) {
        UpdateCheck::Available(manifest)
    } else {
        UpdateCheck::UpToDate
    }
}

/// Downloads a file from `url` to `dest`, reporting progress via `progress_fn`.
///
/// Returns the number of bytes downloaded.
pub async fn download_file<F>(url: &str, dest: &str, progress_fn: F) -> Result<u64, String>
where
    F: Fn(u64, u64),
{
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client.get(url).send().await.map_err(|e| e.to_string())?;

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut last_report = Instant::now();

    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();

    use futures::StreamExt;
    use std::io::Write;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        // Report progress at most every 250ms to avoid flooding the UI.
        if last_report.elapsed() >= Duration::from_millis(250) {
            progress_fn(downloaded, total_size);
            last_report = Instant::now();
        }
    }

    // Final progress report.
    progress_fn(downloaded, total_size);
    Ok(downloaded)
}

/// Computes the SHA-256 hash of a file, as a hex string.
pub fn sha256_file(path: &str) -> Result<String, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = sha256::Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(&hasher.finalize()))
}

/// Verifies that a file's SHA-256 matches the expected hash.
pub fn verify_checksum(path: &str, expected: &str) -> Result<bool, String> {
    let actual = sha256_file(path)?;
    Ok(actual.eq_ignore_ascii_case(expected))
}

/// Launches an installer and exits the current process.
///
/// On Windows, uses `ShellExecuteW` to run the installer with UI privileges.
/// The caller should exit after calling this.
pub fn launch_installer(installer_path: &str) -> Result<(), String> {
    unsafe {
        let path = widestring::Hstring::from(installer_path);
        let operation = widestring::Hstring::from("open");
        let result = winapi::um::shellapi::ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            winapi::um::winuser::SW_SHOWNORMAL,
        );
        // ShellExecuteW returns > 32 on success.
        if result as usize > 32 {
            Ok(())
        } else {
            Err(format!(
                "ShellExecuteW failed with error code {}",
                result as usize
            ))
        }
    }
}

/// Returns the default manifest URL.
pub fn default_manifest_url() -> &'static str {
    "https://api.screenbuddy.app/updates/manifest.json"
}

/// The directory where installers are downloaded.
pub fn download_dir() -> Option<std::path::PathBuf> {
    let local_app_data = std::env::var("LOCALAPPDATA").ok()?;
    let dir = std::path::PathBuf::from(local_app_data)
        .join("ScreenBuddy")
        .join("Updates");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

// Minimal dependencies to avoid adding crates.
mod sha256 {
    pub struct Sha256 {
        state: [u32; 8],
        buffer: [u8; 64],
        buffer_len: usize,
        total_len: u64,
    }

    impl Sha256 {
        pub fn new() -> Self {
            Self {
                state: [
                    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
                    0x1f83d9ab, 0x5be0cd19,
                ],
                buffer: [0u8; 64],
                buffer_len: 0,
                total_len: 0,
            }
        }

        pub fn update(&mut self, data: &[u8]) {
            self.total_len += data.len() as u64;
            let mut offset = 0;
            if self.buffer_len > 0 {
                let need = 64 - self.buffer_len;
                let take = need.min(data.len());
                self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(&data[..take]);
                self.buffer_len += take;
                offset = take;
                if self.buffer_len == 64 {
                    let block = self.buffer;
                    self.process_block(&block);
                    self.buffer_len = 0;
                }
            }
            while offset + 64 <= data.len() {
                let mut block = [0u8; 64];
                block.copy_from_slice(&data[offset..offset + 64]);
                self.process_block(&block);
                offset += 64;
            }
            if offset < data.len() {
                let remain = &data[offset..];
                self.buffer[..remain.len()].copy_from_slice(remain);
                self.buffer_len = remain.len();
            }
        }

        pub fn finalize(mut self) -> [u8; 32] {
            let bit_len = self.total_len * 8;
            let rem = self.buffer_len;

            // Append 0x80 padding byte directly to the buffer (not via update,
            // which would corrupt total_len).
            if rem < 64 {
                self.buffer[rem] = 0x80;
            }
            let new_len = rem + 1;

            // If we don't have room for the 8-byte length, pad to 64 and process.
            if new_len > 56 {
                for i in new_len..64 {
                    self.buffer[i] = 0;
                }
                let block = self.buffer;
                self.process_block(&block);
                // Clear buffer for the next block.
                self.buffer = [0u8; 64];
            } else {
                // Zero from new_len to 56.
                for i in new_len..56 {
                    self.buffer[i] = 0;
                }
            }

            // Write the bit length as big-endian 64-bit.
            let len_bytes = bit_len.to_be_bytes();
            self.buffer[56..64].copy_from_slice(&len_bytes);
            let block = self.buffer;
            self.process_block(&block);

            let mut result = [0u8; 32];
            for (i, &word) in self.state.iter().enumerate() {
                result[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
            }
            result
        }

        fn process_block(&mut self, block: &[u8; 64]) {
            let k: [u32; 64] = [
                0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
                0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
                0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
                0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
                0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
                0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
                0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
                0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
                0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
                0xc67178f2,
            ];

            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = u32::from_be_bytes([
                    block[i * 4],
                    block[i * 4 + 1],
                    block[i * 4 + 2],
                    block[i * 4 + 3],
                ]);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[i - 7])
                    .wrapping_add(s1);
            }

            let mut a = self.state[0];
            let mut b = self.state[1];
            let mut c = self.state[2];
            let mut d = self.state[3];
            let mut e = self.state[4];
            let mut f = self.state[5];
            let mut g = self.state[6];
            let mut h = self.state[7];

            for i in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ ((!e) & g);
                let temp1 = h
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(k[i])
                    .wrapping_add(w[i]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let temp2 = s0.wrapping_add(maj);

                h = g;
                g = f;
                f = e;
                e = d.wrapping_add(temp1);
                d = c;
                c = b;
                b = a;
                a = temp1.wrapping_add(temp2);
            }

            self.state[0] = self.state[0].wrapping_add(a);
            self.state[1] = self.state[1].wrapping_add(b);
            self.state[2] = self.state[2].wrapping_add(c);
            self.state[3] = self.state[3].wrapping_add(d);
            self.state[4] = self.state[4].wrapping_add(e);
            self.state[5] = self.state[5].wrapping_add(f);
            self.state[6] = self.state[6].wrapping_add(g);
            self.state[7] = self.state[7].wrapping_add(h);
        }
    }
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

// Minimal UTF-16 string helper.
mod widestring {
    pub struct Hstring(Vec<u16>);
    impl Hstring {
        pub fn from(s: &str) -> Self {
            Self(s.encode_utf16().chain(std::iter::once(0)).collect())
        }
        pub fn as_ptr(&self) -> *const u16 {
            self.0.as_ptr()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parses_simple_semver() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.prerelease, None);
    }

    #[test]
    fn version_parses_with_prerelease() {
        let v = Version::parse("1.2.3-beta.1").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.prerelease, Some("beta.1".to_string()));
    }

    #[test]
    fn version_parses_with_v_prefix() {
        let v = Version::parse("v1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn version_rejects_invalid_strings() {
        assert!(Version::parse("1.2").is_none());
        assert!(Version::parse("1.2.3.4").is_none());
        assert!(Version::parse("abc").is_none());
        assert!(Version::parse("").is_none());
    }

    #[test]
    fn version_comparison_works() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("2.0.0").unwrap();
        let v1_1 = Version::parse("1.1.0").unwrap();
        let v1_0_1 = Version::parse("1.0.1").unwrap();
        assert!(v2.is_newer_than(&v1));
        assert!(v1_1.is_newer_than(&v1));
        assert!(v1_0_1.is_newer_than(&v1));
        assert!(!v1.is_newer_than(&v2));
        assert!(!v1.is_newer_than(&v1));
    }

    #[test]
    fn version_display_roundtrips() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.to_string(), "1.2.3");
        let v2 = Version::parse("1.2.3-alpha").unwrap();
        assert_eq!(v2.to_string(), "1.2.3-alpha");
    }

    #[test]
    fn sha256_known_vectors() {
        // SHA-256 of empty string.
        let mut hasher = sha256::Sha256::new();
        hasher.update(b"");
        let result = hex::encode(&hasher.finalize());
        assert_eq!(
            result,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        // SHA-256 of "abc".
        let mut hasher2 = sha256::Sha256::new();
        hasher2.update(b"abc");
        let result2 = hex::encode(&hasher2.finalize());
        assert_eq!(
            result2,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn checksum_verification_works() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"hello world").unwrap();
        let path = tmp.path().to_str().unwrap();

        let mut hasher = sha256::Sha256::new();
        hasher.update(b"hello world");
        let hash = hex::encode(&hasher.finalize());

        assert!(verify_checksum(path, &hash).unwrap());
        assert!(!verify_checksum(path, "deadbeef").unwrap());
    }
}
