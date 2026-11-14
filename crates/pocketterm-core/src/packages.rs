//! Verified Debian package installation confined to the private app prefix.
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use tar::Archive;

const PREFIX: &str = "usr";
const DB: &str = "usr/var/lib/pocketterm/status";
const MAX_DEB: usize = 512 * 1024 * 1024;
const MAX_ENTRY: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Package {
    name: String,
    version: String,
    architecture: String,
    filename: String,
    sha256: String,
    depends: Vec<String>,
    files: Vec<String>,
}

pub(crate) fn execute(files_dir: &Path, args: &[String]) -> Result<String, String> {
    let Some(action) = args.first().map(String::as_str) else {
        return Err("usage: pkg install|remove|list|verify".into());
    };
    match action {
        "install" => install(files_dir, &args[1..]),
        "remove" => remove(files_dir, &args[1..]),
        "list" if args.len() == 1 => list(files_dir),
        "verify" if args.len() == 1 => verify(files_dir),
        _ => Err("usage: pkg install package | pkg remove package | pkg list | pkg verify".into()),
    }
}

fn installed(files_dir: &Path) -> Result<BTreeMap<String, Package>, String> {
    let path = files_dir.join(DB);
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(BTreeMap::new());
    };
    let mut result = BTreeMap::new();
    for line in text.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 8 {
            return Err("installed package database is corrupt".into());
        }
        let pkg = Package {
            name: fields[0].into(),
            version: fields[1].into(),
            architecture: fields[2].into(),
            filename: fields[5].into(),
            sha256: fields[4].into(),
            depends: if fields[7].is_empty() {
                vec![]
            } else {
                fields[7].split(',').map(str::to_owned).collect()
            },
            files: if fields[6].is_empty() {
                vec![]
            } else {
                fields[6].split(',').map(str::to_owned).collect()
            },
        };
        result.insert(pkg.name.clone(), pkg);
    }
    Ok(result)
}

fn save_db(files_dir: &Path, db: &BTreeMap<String, Package>) -> Result<(), String> {
    let path = files_dir.join(DB);
    let parent = path.parent().ok_or("invalid package database path")?;
    fs::create_dir_all(parent).map_err(show)?;
    let temp = parent.join(format!("status.tmp-{}", std::process::id()));
    let mut content = String::new();
    for p in db.values() {
        content.push_str(&format!(
            "{}\t{}\t{}\tinstalled\t{}\t{}\t{}\t{}\n",
            p.name,
            p.version,
            p.architecture,
            p.sha256,
            p.filename,
            p.files.join(","),
            p.depends.join(",")
        ));
    }
    fs::write(&temp, content).map_err(show)?;
    fs::rename(temp, path).map_err(show)
}

fn read_index(files_dir: &Path) -> Result<Vec<Package>, String> {
    let lists = files_dir.join("usr/var/lib/pocketterm/lists");
    let mut paths = Vec::new();
    for entry in fs::read_dir(lists).map_err(|_| "no package index; run pkg update first")? {
        let entry = entry.map_err(show)?;
        let candidate = entry.path().join("Packages");
        if candidate.is_file() {
            paths.push(candidate);
        }
    }
    paths.sort();
    let Some(path) = paths.into_iter().next() else {
        return Err("no package index; run pkg update first".into());
    };
    let text = fs::read_to_string(path).map_err(show)?;
    parse_index(&text)
}

pub(crate) fn verify_index_package_hash(
    index: &[u8],
    filename: &str,
    expected_sha256: &str,
) -> Result<(), String> {
    let text = std::str::from_utf8(index).map_err(|_| "Packages index is not valid UTF-8")?;
    let packages = parse_index(text)?;
    if packages.iter().any(|package| {
        package.filename == filename && package.sha256.eq_ignore_ascii_case(expected_sha256)
    }) {
        Ok(())
    } else {
        Err("package filename and hash are not present in the authenticated index".into())
    }
}

fn parse_index(text: &str) -> Result<Vec<Package>, String> {
    let mut packages = Vec::new();
    for block in text.split("\n\n").filter(|s| !s.trim().is_empty()) {
        let mut fields = BTreeMap::<String, String>::new();
        let mut last = String::new();
        for line in block.lines() {
            if line.starts_with([' ', '\t']) {
                if let Some(v) = fields.get_mut(&last) {
                    v.push(' ');
                    v.push_str(line.trim());
                }
                continue;
            }
            let Some((k, v)) = line.split_once(':') else {
                return Err("malformed Packages index".into());
            };
            last = k.to_owned();
            fields.insert(last.clone(), v.trim().to_owned());
        }
        let get = |k: &str| {
            fields
                .get(k)
                .cloned()
                .ok_or_else(|| format!("Packages entry lacks {k}"))
        };
        let name = get("Package")?;
        if !valid_name(&name) {
            return Err("invalid package name in index".into());
        }
        let architecture = get("Architecture")?;
        if architecture != "aarch64" && architecture != "all" {
            continue;
        }
        let sha256 = get("SHA256")?;
        if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid package SHA-256 in index".into());
        }
        let filename = get("Filename")?;
        safe_relative(&filename)?;
        let depends = fields
            .get("Depends")
            .map(|s| {
                s.split(',')
                    .map(|group| {
                        group
                            .split('|')
                            .map(|dep| dep.split_whitespace().next().unwrap_or(""))
                            .filter(|name| !name.is_empty())
                            .collect::<Vec<_>>()
                            .join("|")
                    })
                    .filter(|group| !group.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        packages.push(Package {
            name,
            version: get("Version")?,
            architecture,
            filename,
            sha256,
            depends,
            files: vec![],
        });
    }
    Ok(packages)
}

fn install(files_dir: &Path, args: &[String]) -> Result<String, String> {
    if args.len() != 1 || !valid_name(&args[0]) {
        return Err("usage: pkg install package".into());
    }
    let index = read_index(files_dir)?;
    let available: BTreeMap<_, _> = index.into_iter().map(|p| (p.name.clone(), p)).collect();
    let mut db = installed(files_dir)?;
    let mut order = Vec::new();
    let mut visiting = BTreeSet::new();
    let mut done = BTreeSet::new();
    visit(
        &args[0],
        &available,
        &db,
        &mut visiting,
        &mut done,
        &mut order,
    )?;
    let prefix = files_dir.join(PREFIX);
    fs::create_dir_all(&prefix).map_err(show)?;
    let transaction = files_dir.join(format!("usr/tmp/pkg-txn-{}", std::process::id()));
    let stage = transaction.join("stage");
    let backup = transaction.join("backup");
    fs::create_dir_all(&stage).map_err(show)?;
    fs::create_dir_all(&backup).map_err(show)?;
    let old_db = files_dir.join(DB);
    if old_db.exists() {
        fs::copy(&old_db, transaction.join("status.backup")).map_err(show)?;
    }
    let result = (|| {
        let mut journal = fs::File::create(transaction.join("journal")).map_err(show)?;
        let mut change_index = 0usize;
        for name in &order {
            let p = available.get(name).ok_or("resolved package missing")?;
            let bytes = crate::mirrors::download_package(files_dir, &p.filename, &p.sha256)?;
            if bytes.len() > MAX_DEB {
                return Err("package exceeds size limit".into());
            }
            let payload = deb_payload(&bytes)?;
            let target_stage = stage.join(&p.name);
            fs::create_dir_all(&target_stage).map_err(show)?;
            extract_payload(payload, &target_stage)?;
            let staged_prefix = target_stage.join("data/data/com.pocketterm.android/files/usr");
            if !staged_prefix.is_dir() {
                return Err(format!(
                    "{} package does not target the PocketTerm prefix",
                    p.name
                ));
            }
            let mut entries = Vec::new();
            collect_paths(&staged_prefix, &staged_prefix, &mut entries)?;
            let mut installed_package = p.clone();
            installed_package.files.clear();
            for relative in entries {
                let from = staged_prefix.join(&relative);
                let to = prefix.join(&relative);
                let from_meta = fs::symlink_metadata(&from).map_err(show)?;
                let to_meta = fs::symlink_metadata(&to).ok();
                if from_meta.is_dir()
                    && to_meta
                        .as_ref()
                        .is_some_and(|meta| meta.is_dir() && !meta.file_type().is_symlink())
                {
                    installed_package.files.push(make_file_record(&to)?);
                    continue;
                }
                let saved_name = format!("entry-{change_index}");
                change_index += 1;
                let saved = if to_meta.is_some() {
                    copy_node(&to, &backup.join(&saved_name))?;
                    saved_name
                } else {
                    "-".to_owned()
                };
                writeln!(journal, "{}\t{}", relative.to_string_lossy(), saved).map_err(show)?;
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent).map_err(show)?;
                }
                remove_node(&to)?;
                copy_node(&from, &to)?;
                installed_package.files.push(make_file_record(&to)?);
            }
            db.insert(p.name.clone(), installed_package);
        }
        save_db(files_dir, &db)?;
        Ok(())
    })();
    match result {
        Ok(_) => {
            let _ = fs::remove_dir_all(&transaction);
            Ok(format!(
                "Installed {} package(s): {}\n",
                order.len(),
                order.join(", ")
            ))
        }
        Err(error) => {
            rollback(&transaction, &prefix);
            let _ = fs::remove_dir_all(&transaction);
            Err(format!("installation failed and was rolled back: {error}"))
        }
    }
}

fn visit(
    name: &str,
    available: &BTreeMap<String, Package>,
    db: &BTreeMap<String, Package>,
    visiting: &mut BTreeSet<String>,
    done: &mut BTreeSet<String>,
    order: &mut Vec<String>,
) -> Result<(), String> {
    if db.contains_key(name) || done.contains(name) {
        return Ok(());
    }
    if !visiting.insert(name.to_owned()) {
        return Err(format!("dependency cycle involving {name}"));
    }
    let pkg = available
        .get(name)
        .ok_or_else(|| format!("package not found: {name}"))?;
    for dependency in &pkg.depends {
        let candidates: Vec<_> = dependency.split('|').collect();
        if candidates
            .iter()
            .any(|candidate| db.contains_key(*candidate) || done.contains(*candidate))
        {
            continue;
        }
        let candidate = candidates
            .iter()
            .find(|candidate| available.contains_key(**candidate))
            .ok_or_else(|| format!("unsatisfied dependency: {dependency}"))?;
        visit(candidate, available, db, visiting, done, order)?;
    }
    visiting.remove(name);
    done.insert(name.to_owned());
    order.push(name.to_owned());
    Ok(())
}

fn deb_payload(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if !bytes.starts_with(b"!<arch>\n") {
        return Err("invalid Debian ar archive".into());
    }
    let mut pos = 8;
    while pos + 60 <= bytes.len() {
        let h = &bytes[pos..pos + 60];
        if &h[58..60] != b"`\n" {
            return Err("invalid ar member header".into());
        }
        let raw_name = std::str::from_utf8(&h[..16]).map_err(show)?.trim();
        let name = raw_name.trim_end_matches('/');
        let size_text = std::str::from_utf8(&h[48..58]).map_err(show)?.trim();
        let size: usize = size_text.parse().map_err(show)?;
        let start = pos + 60;
        let end = start.checked_add(size).ok_or("ar member size overflow")?;
        if end > bytes.len() {
            return Err("truncated ar member".into());
        }
        if name.starts_with("data.tar") {
            let data = &bytes[start..end];
            return if name.ends_with(".xz") {
                let mut out = CappedWriter {
                    bytes: Vec::new(),
                    limit: MAX_DEB,
                };
                lzma_rs::xz_decompress(&mut Cursor::new(data), &mut out).map_err(show)?;
                Ok(out.bytes)
            } else if name.ends_with(".gz") {
                let mut out = Vec::new();
                GzDecoder::new(data)
                    .take(512 * 1024 * 1024)
                    .read_to_end(&mut out)
                    .map_err(show)?;
                Ok(out)
            } else if name.ends_with(".zst") {
                Err("zstd-compressed Debian payload is unsupported".into())
            } else {
                Ok(data.to_vec())
            };
        }
        pos = end + (size % 2);
    }
    Err("Debian package has no data.tar payload".into())
}

fn extract_payload(data: Vec<u8>, destination: &Path) -> Result<(), String> {
    let mut archive = Archive::new(Cursor::new(&data));
    let entries = archive.entries().map_err(show)?;
    for entry in entries {
        let entry = entry.map_err(show)?;
        let path = entry.path().map_err(show)?.into_owned();
        validate_tar_path(&path)?;
        let package_prefix = Path::new("data/data/com.pocketterm.android/files/usr");
        let relative = path
            .strip_prefix(package_prefix)
            .map_err(|_| "package entry is outside PocketTerm prefix")?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        if entry.size() > MAX_ENTRY {
            return Err("tar entry exceeds size limit".into());
        }
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            let link = entry
                .link_name()
                .map_err(show)?
                .ok_or("tar link has no target")?;
            validate_link(relative, &link)?;
        }
    }
    let mut archive = Archive::new(Cursor::new(&data));
    archive.unpack(destination).map_err(show)?;
    Ok(())
}
fn validate_tar_path(p: &Path) -> Result<(), String> {
    if p.is_absolute()
        || p.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("unsafe path in package tar".into());
    }
    Ok(())
}
fn validate_link(path: &Path, link: &Path) -> Result<(), String> {
    if link.is_absolute() {
        return Err("absolute symlink in package tar".into());
    }
    let mut depth = path.parent().map_or(0, |p| p.components().count());
    for c in link.components() {
        match c {
            Component::ParentDir => {
                if depth == 0 {
                    return Err("package symlink escapes prefix".into());
                }
                depth -= 1
            }
            Component::Normal(_) => depth += 1,
            _ => {}
        }
    }
    Ok(())
}
fn collect_paths(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for e in fs::read_dir(dir).map_err(show)? {
        let e = e.map_err(show)?;
        let p = e.path();
        let rel = p.strip_prefix(root).map_err(show)?.to_path_buf();
        out.push(rel.clone());
        if e.file_type().map_err(show)?.is_dir() {
            collect_paths(root, &p, out)?
        }
    }
    Ok(())
}
fn copy_node(from: &Path, to: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(from).map_err(show)?;
    if meta.file_type().is_symlink() {
        let target = fs::read_link(from).map_err(show)?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, to).map_err(show)?;
        }
        #[cfg(not(unix))]
        {
            return Err("symlinks are unsupported on this platform".into());
        }
    } else if meta.is_dir() {
        fs::create_dir_all(to).map_err(show)?;
        for e in fs::read_dir(from).map_err(show)? {
            let e = e.map_err(show)?;
            copy_node(&e.path(), &to.join(e.file_name()))?
        }
    } else {
        if let Some(p) = to.parent() {
            fs::create_dir_all(p).map_err(show)?
        }
        fs::copy(from, to).map_err(show)?;
    }
    Ok(())
}
fn remove_node(p: &Path) -> Result<(), String> {
    match fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => fs::remove_dir_all(p).map_err(show),
        Ok(_) => fs::remove_file(p).map_err(show),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(show(e)),
    }
}
fn rollback(tx: &Path, prefix: &Path) {
    let backup = tx.join("backup");
    let journal = tx.join("journal");
    if let Ok(text) = fs::read_to_string(journal) {
        for line in text.lines().rev() {
            let mut f = line.split('\t');
            if let (Some(rel), Some(saved)) = (f.next(), f.next()) {
                let path = prefix.join(rel);
                let _ = remove_node(&path);
                if saved != "-" {
                    let _ = copy_node(&backup.join(saved), &path);
                }
            }
        }
    }
    let db_path = prefix.join("var/lib/pocketterm/status");
    let saved = tx.join("status.backup");
    if saved.exists() {
        let _ = fs::copy(saved, db_path);
    } else {
        let _ = fs::remove_file(db_path);
    }
}
fn remove(files_dir: &Path, args: &[String]) -> Result<String, String> {
    if args.len() != 1 || !valid_name(&args[0]) {
        return Err("usage: pkg remove package".into());
    }
    let mut db = installed(files_dir)?;
    if db.values().any(|other| {
        other
            .depends
            .iter()
            .any(|dependency| dependency.split('|').any(|name| name == args[0]))
    }) {
        return Err(format!(
            "package is required by an installed package: {}",
            args[0]
        ));
    }
    let Some(package) = db.remove(&args[0]) else {
        return Err(format!("package is not installed: {}", args[0]));
    };
    let prefix = files_dir.join(PREFIX);
    let other_owned: BTreeSet<String> = db
        .values()
        .flat_map(|p| {
            p.files
                .iter()
                .filter_map(|record| record_path(record).map(str::to_owned))
        })
        .collect();
    for record in package.files.iter().rev() {
        let file = record_path(record).ok_or("corrupt installed file record")?;
        if !other_owned.contains(file) {
            remove_node(&prefix.join(file))?;
        }
    }
    save_db(files_dir, &db)?;
    Ok(format!("Removed {}\n", package.name))
}
fn list(files_dir: &Path) -> Result<String, String> {
    let db = installed(files_dir)?;
    if db.is_empty() {
        return Ok("No packages installed.\n".into());
    }
    Ok(db
        .values()
        .map(|p| format!("{}\t{}\t{}\n", p.name, p.version, p.architecture))
        .collect())
}
fn verify(files_dir: &Path) -> Result<String, String> {
    let db = installed(files_dir)?;
    let prefix = files_dir.join(PREFIX);
    let mut bad = Vec::new();
    for p in db.values() {
        for record in &p.files {
            let Some(file) = record_path(record) else {
                bad.push(format!("{}: corrupt file record", p.name));
                continue;
            };
            let path = prefix.join(file);
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                bad.push(format!("{}: missing {file}", p.name));
                continue;
            };
            let Some((_, expected)) = record.rsplit_once('|') else {
                bad.push(format!("{}: corrupt file hash", p.name));
                continue;
            };
            if expected != "-" {
                match hash_path(&path, &metadata) {
                    Ok(actual) if actual == expected => {}
                    _ => bad.push(format!("{}: modified {file}", p.name)),
                }
            }
        }
    }
    if bad.is_empty() {
        Ok(format!(
            "Verified {} installed package file records.\n",
            db.values().map(|p| p.files.len()).sum::<usize>()
        ))
    } else {
        Err(bad.join("\n"))
    }
}
fn make_file_record(path: &Path) -> Result<String, String> {
    let name = path.to_string_lossy();
    if name.contains('|') || name.contains(',') {
        return Err("package path uses a reserved character".into());
    }
    let metadata = fs::symlink_metadata(path).map_err(show)?;
    let hash = if metadata.is_dir() && !metadata.file_type().is_symlink() {
        "-".to_owned()
    } else {
        hash_path(path, &metadata)?
    };
    Ok(format!("{name}|{hash}"))
}
fn record_path(record: &str) -> Option<&str> {
    record.rsplit_once('|').map(|(path, _)| path)
}
fn hash_path(path: &Path, metadata: &fs::Metadata) -> Result<String, String> {
    let bytes = if metadata.file_type().is_symlink() {
        fs::read_link(path)
            .map_err(show)?
            .to_string_lossy()
            .as_bytes()
            .to_vec()
    } else {
        fs::read(path).map_err(show)?
    };
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn safe_relative(s: &str) -> Result<(), String> {
    let p = Path::new(s);
    if p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err("unsafe package filename".into());
    }
    Ok(())
}
fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"+.-".contains(&b))
}
struct CappedWriter {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for CappedWriter {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(input.len()) > self.limit {
            return Err(std::io::Error::other(
                "decompressed package exceeds size limit",
            ));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn show(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn _sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_index_and_dependencies() {
        let p=parse_index("Package: python\nVersion: 3.13\nArchitecture: aarch64\nFilename: pool/main/python.deb\nSHA256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nDepends: libfoo (>= 1), libc\n\n").expect("parse");
        assert_eq!(p[0].depends, vec!["libfoo", "libc"]);
    }
    #[test]
    fn package_hash_must_appear_in_the_index() {
        let index=b"Package: python\nVersion: 3.13\nArchitecture: aarch64\nFilename: pool/python.deb\nSHA256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n\n";
        assert!(verify_index_package_hash(index, "pool/python.deb", &"a".repeat(64)).is_ok());
        assert!(verify_index_package_hash(index, "pool/python.deb", &"b".repeat(64)).is_err());
    }
    #[test]
    fn rejects_bad_index_paths() {
        assert!(parse_index("Package: x\nVersion: 1\nArchitecture: aarch64\nFilename: ../x.deb\nSHA256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n\n").is_err());
    }
    #[test]
    fn rejects_tar_traversal_and_external_link() {
        assert!(validate_tar_path(Path::new("../escape")).is_err());
        assert!(validate_tar_path(Path::new("/absolute")).is_err());
        assert!(validate_link(Path::new("bin/tool"), Path::new("../../outside")).is_err());
    }
    #[test]
    fn package_file_verification_detects_modified_content() {
        let path = std::env::temp_dir().join(format!("pt-pkg-hash-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        fs::write(&path, b"original").expect("create file");
        let record = make_file_record(&path).expect("record");
        let expected = record.rsplit_once('|').expect("hash").1.to_owned();
        assert_eq!(
            hash_path(&path, &fs::symlink_metadata(&path).expect("metadata")).expect("hash"),
            expected
        );
        fs::write(&path, b"modified").expect("modify file");
        assert_ne!(
            hash_path(&path, &fs::symlink_metadata(&path).expect("metadata")).expect("hash"),
            expected
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rollback_restores_overwritten_files_and_removes_new_files() {
        let root = std::env::temp_dir().join(format!("pt-pkg-rollback-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let prefix = root.join("usr");
        let tx = root.join("usr/tmp/transaction");
        fs::create_dir_all(prefix.join("bin")).expect("prefix");
        fs::create_dir_all(tx.join("backup")).expect("backup");
        fs::write(prefix.join("bin/old"), b"new").expect("new file");
        fs::write(prefix.join("bin/add"), b"added").expect("added file");
        fs::write(tx.join("backup/old"), b"old").expect("saved file");
        fs::write(tx.join("journal"), "bin/old\told\nbin/add\t-\n").expect("journal");
        rollback(&tx, &prefix);
        assert_eq!(fs::read(prefix.join("bin/old")).expect("restored"), b"old");
        assert!(!prefix.join("bin/add").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dependency_cycle_is_detected() {
        let a = Package {
            name: "a".into(),
            version: "1".into(),
            architecture: "aarch64".into(),
            filename: "x".into(),
            sha256: "0".repeat(64),
            depends: vec!["b".into()],
            files: vec![],
        };
        let b = Package {
            depends: vec!["a".into()],
            name: "b".into(),
            ..a.clone()
        };
        let map = BTreeMap::from([("a".into(), a), ("b".into(), b)]);
        assert!(
            visit(
                "a",
                &map,
                &BTreeMap::new(),
                &mut BTreeSet::new(),
                &mut BTreeSet::new(),
                &mut Vec::new()
            )
            .is_err()
        );
    }
}
