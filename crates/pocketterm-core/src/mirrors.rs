//! HTTPS mirror diagnostics and signed index verification.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reqwest::Url;
use reqwest::blocking::{Client, Response};
use reqwest::redirect::Policy;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

const RELEASE_LIMIT: usize = 1024 * 1024;
const INDEX_LIMIT: usize = 64 * 1024 * 1024;
const MIRROR_CONFIG: &str = "usr/etc/pocketterm/mirrors.conf";
const SELECTED_CONFIG: &str = "usr/etc/pocketterm/selected-mirror";
pub const POCKETTERM_REPOSITORY: &str = "https://e15625455-sketch.github.io/PocketTerm-pkg";
pub const POCKETTERM_PUBLIC_KEY: [u8; 32] = [
    0x86, 0x99, 0xd9, 0x82, 0x6c, 0x65, 0xb6, 0xbe, 0xb9, 0x4b, 0x97, 0xab, 0x99, 0x6e, 0xc0, 0x2e,
    0x6a, 0x63, 0xe8, 0xea, 0xbe, 0xcb, 0xb5, 0x71, 0xe1, 0x76, 0xe5, 0x50, 0xa1, 0x9e, 0xbd, 0x47,
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct Mirror {
    name: String,
    base_url: String,
    public_key: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirrorStatus {
    pub name: String,
    pub base_url: String,
    pub elapsed_ms: u128,
    pub valid: bool,
    pub detail: String,
}

pub struct MirrorManager {
    files_dir: PathBuf,
    mirrors: Vec<Mirror>,
}

impl MirrorManager {
    pub fn open(files_dir: impl AsRef<Path>) -> Result<Self, String> {
        let files_dir = files_dir.as_ref().to_path_buf();
        let config = files_dir.join(MIRROR_CONFIG);
        if !config.exists() {
            let parent = config
                .parent()
                .ok_or_else(|| "invalid mirror configuration path".to_owned())?;
            fs::create_dir_all(parent).map_err(display_error)?;
            let defaults = format!(
                "# Signed PocketTerm repository; Termux endpoints are diagnostic-only.\npocketterm\t{}\t{}\ntermux-primary\thttps://packages.termux.dev/apt/termux-main\t-\ntermux-cf\thttps://packages-cf.termux.dev/apt/termux-main\t-\n",
                POCKETTERM_REPOSITORY,
                hex(&POCKETTERM_PUBLIC_KEY)
            );
            fs::write(&config, defaults).map_err(display_error)?;
        }
        let text = fs::read_to_string(&config).map_err(display_error)?;
        let mut mirrors = Vec::new();
        for (line_number, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() != 3 {
                return Err(format!(
                    "invalid mirror configuration at line {}",
                    line_number + 1
                ));
            }
            validate_name(fields[0])?;
            validate_https_url(fields[1])?;
            let public_key = if fields[2].is_empty() || fields[2] == "-" {
                None
            } else {
                Some(parse_hex::<32>(fields[2]).ok_or_else(|| {
                    format!("invalid Ed25519 public key at line {}", line_number + 1)
                })?)
            };
            mirrors.push(Mirror {
                name: fields[0].to_owned(),
                base_url: fields[1].trim_end_matches('/').to_owned(),
                public_key,
            });
        }
        let pinned_mirror = Mirror {
            name: "pocketterm".into(),
            base_url: POCKETTERM_REPOSITORY.into(),
            public_key: Some(POCKETTERM_PUBLIC_KEY),
        };
        let needs_pin_update = mirrors.first() != Some(&pinned_mirror)
            || mirrors
                .iter()
                .filter(|mirror| mirror.name == "pocketterm")
                .count()
                != 1;
        if needs_pin_update {
            mirrors.retain(|mirror| mirror.name != "pocketterm");
            mirrors.insert(0, pinned_mirror);
            let mut content = String::from(
                "# Signed PocketTerm repository; Termux endpoints are diagnostic-only.\n",
            );
            for mirror in &mirrors {
                let key = mirror
                    .public_key
                    .map_or_else(String::new, |value| hex(&value));
                content.push_str(&format!("{}\t{}\t{}\n", mirror.name, mirror.base_url, key));
            }
            atomic_write(&config, content.as_bytes())?;
        }
        Ok(Self { files_dir, mirrors })
    }

    pub fn execute(&mut self, args: &[String]) -> Result<String, String> {
        let Some(action) = args.first().map(String::as_str) else {
            return Err("usage: pkg mirrors|test|select|update".into());
        };
        match action {
            "mirrors" => self.mirrors_command(&args[1..]),
            "test" => self.test_command(&args[1..]),
            "select" => self.select_command(&args[1..]),
            "update" => self.update_command(),
            "install" | "remove" | "list" | "verify" => {
                crate::packages::execute(&self.files_dir, args)
            }
            _ => Err("usage: pkg mirrors|test|select|update".into()),
        }
    }

    fn mirrors_command(&mut self, args: &[String]) -> Result<String, String> {
        if args.first().is_some_and(|arg| arg == "add") {
            return self.add_mirror(&args[1..]);
        }
        if !args.is_empty() {
            return Err("usage: pkg mirrors [add name https_url ed25519_public_key_hex]".into());
        }
        let mut output = String::new();
        for mirror in &self.mirrors {
            let trust = if mirror.public_key.is_some() {
                "pinned Ed25519 key"
            } else {
                "no Ed25519 key; diagnostics only"
            };
            output.push_str(&format!(
                "{}\t{}\t{}\n",
                mirror.name, mirror.base_url, trust
            ));
        }
        if output.is_empty() {
            output.push_str("No mirrors configured. Use pkg mirrors add.\n");
        }
        Ok(output)
    }

    fn add_mirror(&mut self, args: &[String]) -> Result<String, String> {
        let [name, url, key] = args else {
            return Err("usage: pkg mirrors add name https_url ed25519_public_key_hex".into());
        };
        validate_name(name)?;
        validate_https_url(url)?;
        let public_key = parse_hex::<32>(key)
            .ok_or_else(|| "Ed25519 public key must be 64 hexadecimal characters".to_owned())?;
        if self.mirrors.iter().any(|mirror| mirror.name == *name) {
            return Err(format!("mirror already exists: {name}"));
        }
        self.mirrors.push(Mirror {
            name: name.clone(),
            base_url: url.trim_end_matches('/').to_owned(),
            public_key: Some(public_key),
        });
        self.save_mirrors()?;
        Ok(format!(
            "Added mirror {name}. Run pkg test before selecting it.\n"
        ))
    }

    fn test_command(&self, args: &[String]) -> Result<String, String> {
        if args.len() > 1 {
            return Err("usage: pkg test [mirror]".into());
        }
        let mirrors = if let Some(name) = args.first() {
            vec![self.find_mirror(name)?.clone()]
        } else {
            self.mirrors.clone()
        };
        if mirrors.is_empty() {
            return Err("no mirrors configured".into());
        }
        let mut output = String::new();
        let mut any_valid = false;
        for mirror in mirrors {
            let status = test_mirror(&mirror);
            any_valid |= status.valid;
            output.push_str(&format_status(&status));
        }
        if any_valid {
            output.push_str("At least one tested mirror has a valid signed index.\n");
            Ok(output)
        } else {
            output.push_str("No trust or package update is granted to these mirrors.\n");
            Err(output)
        }
    }

    fn select_command(&mut self, args: &[String]) -> Result<String, String> {
        let [name] = args else {
            return Err("usage: pkg select mirror".into());
        };
        let mirror = self.find_mirror(name)?.clone();
        let started = Instant::now();
        fetch_validated_index(&mirror)
            .map_err(|error| format!("{error}; mirror was not selected"))?;
        let elapsed = started.elapsed().as_millis();
        atomic_write(&self.files_dir.join(SELECTED_CONFIG), name.as_bytes())?;
        Ok(format!("Selected mirror {name} ({elapsed} ms).\n"))
    }

    fn update_command(&self) -> Result<String, String> {
        let mut ordered = Vec::new();
        if let Ok(selected) = fs::read_to_string(self.files_dir.join(SELECTED_CONFIG))
            && let Some(mirror) = self
                .mirrors
                .iter()
                .find(|item| item.name == selected.trim())
        {
            ordered.push(mirror.clone());
        }
        for mirror in &self.mirrors {
            if !ordered.iter().any(|selected| selected.name == mirror.name) {
                ordered.push(mirror.clone());
            }
        }
        if ordered.is_empty() {
            return Err("no mirrors configured".into());
        }
        let mut errors = Vec::new();
        for mirror in ordered {
            let started = Instant::now();
            match fetch_validated_index(&mirror) {
                Ok(index) => {
                    let destination = self
                        .files_dir
                        .join("usr/var/lib/pocketterm/lists")
                        .join(&mirror.name)
                        .join("Packages");
                    atomic_write(&destination, &index.packages)?;
                    atomic_write(&destination.with_file_name("Release"), &index.release)?;
                    atomic_write(
                        &self.files_dir.join(SELECTED_CONFIG),
                        mirror.name.as_bytes(),
                    )?;
                    let hash = hex(&Sha256::digest(&index.packages));
                    let elapsed = started.elapsed().as_millis();
                    return Ok(format!(
                        "Updated signed package index from {} ({elapsed} ms, SHA-256 {hash}).\n",
                        mirror.name
                    ));
                }
                Err(error) => errors.push(format!(
                    "{}: INVALID ({} ms): {error}\n",
                    mirror.name,
                    started.elapsed().as_millis()
                )),
            }
        }
        Err(format!(
            "no mirror passed signature and SHA-256 validation:\n{}",
            errors.join("")
        ))
    }

    fn find_mirror(&self, name: &str) -> Result<&Mirror, String> {
        self.mirrors
            .iter()
            .find(|mirror| mirror.name == name)
            .ok_or_else(|| format!("mirror not found: {name}"))
    }

    fn save_mirrors(&self) -> Result<(), String> {
        let mut content = String::from("# name<TAB>https_url<TAB>Ed25519 public key hex\n");
        for mirror in &self.mirrors {
            let key = mirror
                .public_key
                .map_or_else(String::new, |value| hex(&value));
            content.push_str(&format!("{}\t{}\t{}\n", mirror.name, mirror.base_url, key));
        }
        atomic_write(&self.files_dir.join(MIRROR_CONFIG), content.as_bytes())
    }
}

pub fn verify_release(
    release: &[u8],
    signature: &[u8],
    public_key: &[u8; 32],
) -> Result<(), String> {
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| "invalid Ed25519 public key".to_owned())?;
    let signature = signature_bytes(signature)
        .ok_or_else(|| "invalid Ed25519 signature encoding".to_owned())?;
    key.verify(release, &Signature::from_bytes(&signature))
        .map_err(|_| "Ed25519 Release signature is invalid".to_owned())
}

pub fn verify_sha256(data: &[u8], expected_hex: &str) -> bool {
    let Some(expected) = parse_hex::<32>(expected_hex) else {
        return false;
    };
    let actual = Sha256::digest(data);
    actual.as_slice() == expected
}

struct ValidatedIndex {
    release: Vec<u8>,
    packages: Vec<u8>,
}

fn test_mirror(mirror: &Mirror) -> MirrorStatus {
    let started = Instant::now();
    let result = fetch_validated_index(mirror);
    MirrorStatus {
        name: mirror.name.clone(),
        base_url: mirror.base_url.clone(),
        elapsed_ms: started.elapsed().as_millis(),
        valid: result.is_ok(),
        detail: result
            .err()
            .unwrap_or_else(|| "HTTPS, Ed25519 signature, and SHA-256 verified".to_owned()),
    }
}

fn fetch_validated_index(mirror: &Mirror) -> Result<ValidatedIndex, String> {
    let client = http_client()?;
    let release_url = endpoint(mirror, "dists/stable/Release")?;
    let release = fetch(&client, &release_url, RELEASE_LIMIT)
        .map_err(|error| format!("Release unavailable: {error}"))?;
    let signature_url = endpoint(mirror, "dists/stable/Release.ed25519")?;
    let signature = fetch(&client, &signature_url, 1024)
        .map_err(|error| format!("Release signature unavailable: {error}"))?;
    let public_key = mirror
        .public_key
        .as_ref()
        .ok_or_else(|| "Release is reachable but no Ed25519 key is pinned".to_owned())?;
    verify_release(&release, &signature, public_key)?;
    let (index_path, expected_hash) = release_index(&release)?;
    let index_url = endpoint(mirror, &format!("dists/stable/{index_path}"))?;
    let packages = fetch(&client, &index_url, INDEX_LIMIT)
        .map_err(|error| format!("Packages index unavailable: {error}"))?;
    if !verify_sha256(&packages, &expected_hash) {
        return Err("Packages SHA-256 does not match signed Release".into());
    }
    Ok(ValidatedIndex { release, packages })
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(Policy::custom(|attempt| {
            if attempt.url().scheme() != "https" || attempt.previous().len() >= 5 {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(display_error)
}

fn endpoint(mirror: &Mirror, path: &str) -> Result<Url, String> {
    let base = validate_https_url(&mirror.base_url)?;
    let mut url = base;
    let suffix = path.trim_start_matches('/');
    let joined = format!("{}/{}", url.path().trim_end_matches('/'), suffix);
    url.set_path(&joined);
    Ok(url)
}

fn validate_https_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(display_error)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.username() != ""
        || url.password().is_some()
    {
        return Err("mirror URL must be HTTPS without embedded credentials".into());
    }
    Ok(url)
}

fn fetch(client: &Client, url: &Url, maximum: usize) -> Result<Vec<u8>, String> {
    if url.scheme() != "https" {
        return Err("refusing non-HTTPS mirror URL".into());
    }
    let response = client.get(url.clone()).send().map_err(display_error)?;
    ensure_success(&response)?;
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err(format!("response exceeds {maximum} byte limit"));
    }
    let mut bytes = Vec::new();
    response
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(display_error)?;
    if bytes.len() > maximum {
        return Err(format!("response exceeds {maximum} byte limit"));
    }
    Ok(bytes)
}

fn ensure_success(response: &Response) -> Result<(), String> {
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", response.status().as_u16()))
    }
}

fn release_index(release: &[u8]) -> Result<(String, String), String> {
    let text = std::str::from_utf8(release).map_err(|_| "Release is not valid UTF-8".to_owned())?;
    let mut in_sha256 = false;
    for line in text.lines() {
        if line == "SHA256:" {
            in_sha256 = true;
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            if in_sha256 {
                break;
            }
            continue;
        }
        if !in_sha256 {
            continue;
        }
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 3
            || !fields[0].bytes().all(|byte| byte.is_ascii_hexdigit())
            || fields[0].len() != 64
        {
            continue;
        }
        let path = fields[2];
        if path.ends_with("binary-aarch64/Packages") && safe_relative_path(path) {
            return Ok((path.to_owned(), fields[0].to_ascii_lowercase()));
        }
    }
    Err("signed Release has no safe aarch64 Packages SHA-256 entry".into())
}

fn safe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn signature_bytes(value: &[u8]) -> Option<[u8; 64]> {
    if value.len() == 64 {
        return value.try_into().ok();
    }
    let text = std::str::from_utf8(value).ok()?.trim();
    parse_hex::<64>(text)
}

fn parse_hex<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut output = [0; N];
    for (index, pair) in value.as_bytes().chunks(2).enumerate() {
        output[index] = (hex_digit(pair[0])? << 4) | hex_digit(pair[1])?;
    }
    Some(output)
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(DIGITS[(byte >> 4) as usize]));
        result.push(char::from(DIGITS[(byte & 0x0f) as usize]));
    }
    result
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err(
            "mirror name may contain only ASCII letters, digits, dot, underscore, and hyphen"
                .into(),
        );
    }
    Ok(())
}

fn format_status(status: &MirrorStatus) -> String {
    let label = if status.valid { "VALID" } else { "INVALID" };
    format!(
        "{}: {} ({} ms): {}\n",
        status.name, label, status.elapsed_ms, status.detail
    )
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "invalid destination path".to_owned())?;
    fs::create_dir_all(parent).map_err(display_error)?;
    let name = path
        .file_name()
        .ok_or_else(|| "invalid destination path".to_owned())?;
    let temporary = parent.join(format!(
        "{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(display_error)?;
    use std::io::Write;
    if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(display_error(error));
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(display_error(error));
    }
    Ok(())
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

pub(crate) fn download_package(
    files_dir: &Path,
    filename: &str,
    expected_sha256: &str,
) -> Result<Vec<u8>, String> {
    if !safe_relative_path(filename) || filename.contains('\\') {
        return Err("unsafe package filename in index".into());
    }
    let manager = MirrorManager::open(files_dir)?;
    let selected = fs::read_to_string(files_dir.join(SELECTED_CONFIG)).map_err(display_error)?;
    let mirror = manager.find_mirror(selected.trim())?;
    let index = fetch_validated_index(mirror)?;
    crate::packages::verify_index_package_hash(&index.packages, filename, expected_sha256)?;
    let client = http_client()?;
    let url = endpoint(mirror, filename)?;
    let package = fetch(&client, &url, 512 * 1024 * 1024)?;
    if !verify_sha256(&package, expected_sha256) {
        return Err("package SHA-256 does not match signed index".into());
    }
    Ok(package)
}

#[cfg(test)]
mod tests {
    use super::{
        MirrorManager, parse_hex, release_index, safe_relative_path, verify_release, verify_sha256,
    };
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    use std::fs;

    #[test]
    fn parses_sha256_entry_and_rejects_unsafe_index_path() {
        let hash = "a".repeat(64);
        let release =
            format!("Origin: PocketTerm\nSHA256:\n {hash} 21 main/binary-aarch64/Packages\n");
        let (path, found_hash) = release_index(release.as_bytes()).expect("signed index metadata");
        assert_eq!(path, "main/binary-aarch64/Packages");
        assert_eq!(found_hash, hash);
        assert!(!safe_relative_path("../Packages"));
        assert!(!safe_relative_path("/tmp/Packages"));
    }

    #[test]
    fn verifies_ed25519_release_and_sha256() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let verifying_key = signing_key.verifying_key();
        let release = b"Origin: PocketTerm\n";
        let signature = signing_key.sign(release);
        verify_release(release, &signature.to_bytes(), verifying_key.as_bytes())
            .expect("signature verifies");
        assert!(
            verify_release(b"modified", &signature.to_bytes(), verifying_key.as_bytes()).is_err()
        );
        let digest = Sha256::digest(b"Packages index");
        let hash = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(verify_sha256(b"Packages index", &hash));
        assert!(!verify_sha256(b"altered", &hash));
        assert_eq!(parse_hex::<32>(&hash).expect("hex digest").len(), 32);
    }

    #[test]
    fn mirror_config_rejects_http_and_lists_diagnostic_defaults() {
        let root = std::env::temp_dir().join(format!("pocketterm-mirrors-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let manager = MirrorManager::open(&root).expect("mirror defaults");
        assert_eq!(manager.mirrors.len(), 3);
        assert_eq!(manager.mirrors[0].name, "pocketterm");
        assert_eq!(
            manager.mirrors[0].public_key,
            Some(super::POCKETTERM_PUBLIC_KEY)
        );
        assert!(super::validate_https_url("http://example.org").is_err());
        let config = root.join(super::MIRROR_CONFIG);
        fs::write(&config, "pocketterm\thttps://e15625455-sketch.github.io/PocketTerm-pkg\tcc086f7a8574b4bc230a41b2c148fe1a239d3c07ea9a6842f679862fe50a3068\ntermux-primary\thttps://packages.termux.dev/apt/termux-main\t-\ntermux-cf\thttps://packages-cf.termux.dev/apt/termux-main\t-\n").expect("write old pin");
        let migrated = MirrorManager::open(&root).expect("migrate saved pin");
        assert_eq!(
            migrated.mirrors[0].public_key,
            Some(super::POCKETTERM_PUBLIC_KEY)
        );
        let saved = fs::read_to_string(config).expect("read migrated config");
        assert!(saved.contains("8699d9826c65b6beb94b97ab996ec02e6a63e8eabecbb571e176e550a19ebd47"));
        let _ = fs::remove_dir_all(root);
    }
}
