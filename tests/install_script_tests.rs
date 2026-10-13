//! Installer integrity: `install.sh --staged` against a local tarball. The
//! installer must refuse any binary it cannot verify against its checksum.
#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Staged {
    root: tempfile::TempDir,
    staged: PathBuf,
    install: PathBuf,
    tarball: PathBuf,
}

/// Packs a stub `aegis` binary the way the release workflow does.
fn stage() -> Staged {
    let root = tempfile::tempdir().unwrap();
    let dist = root.path().join("dist");
    let staged = root.path().join("staged");
    fs::create_dir_all(&dist).unwrap();
    fs::create_dir_all(&staged).unwrap();

    let bin = dist.join("aegis");
    fs::write(&bin, "#!/bin/sh\necho 'aegis test stub'\n").unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();

    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    };
    let tarball = staged.join(format!("aegis-linux-{arch}.tar.gz"));
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&tarball)
        .arg("-C")
        .arg(&dist)
        .arg("aegis")
        .status()
        .unwrap();
    assert!(status.success());

    Staged {
        install: root.path().join("bin"),
        root,
        staged,
        tarball,
    }
}

fn write_checksum(s: &Staged, contents: &str) {
    let mut name = s.tarball.clone().into_os_string();
    name.push(".sha256");
    fs::write(name, contents).unwrap();
}

fn good_checksum(tarball: &Path) -> String {
    let out = Command::new("sha256sum").arg(tarball).output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap()
}

fn install(s: &Staged, path: Option<&Path>) -> Output {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh"))
        .arg("--staged")
        .arg(&s.staged)
        .arg("--install-dir")
        .arg(&s.install);
    if let Some(path) = path {
        cmd.env("PATH", path);
    }
    cmd.output().unwrap()
}

fn refused(s: &Staged, out: &Output, message: &str) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "install succeeded: {stderr}");
    assert!(stderr.contains(message), "unexpected stderr: {stderr}");
    assert!(!s.install.join("aegis").exists());
}

#[test]
fn verified_tarball_installs() {
    let s = stage();
    write_checksum(&s, &good_checksum(&s.tarball));
    let out = install(&s, None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(s.install.join("aegis").exists());
}

#[test]
fn missing_checksum_is_refused() {
    let s = stage();
    let out = install(&s, None);
    refused(&s, &out, "No checksum file");
}

#[test]
fn empty_checksum_is_refused() {
    let s = stage();
    write_checksum(&s, "");
    let out = install(&s, None);
    refused(&s, &out, "No checksum file");

    write_checksum(&s, "\n");
    let out = install(&s, None);
    refused(&s, &out, "Checksum verification failed");
}

#[test]
fn mismatched_checksum_is_refused() {
    let s = stage();
    write_checksum(&s, &format!("{}  aegis.tar.gz\n", "0".repeat(64)));
    let out = install(&s, None);
    refused(&s, &out, "Checksum verification failed");
}

#[test]
fn missing_hash_tool_is_refused() {
    let s = stage();
    write_checksum(&s, &good_checksum(&s.tarball));

    // A PATH with every tool the installer uses except sha256sum and shasum.
    let tools = s.root.path().join("tools");
    fs::create_dir_all(&tools).unwrap();
    for tool in [
        "uname", "mktemp", "rm", "cp", "find", "head", "awk", "tar", "gzip", "mkdir", "chmod",
    ] {
        let found = ["/usr/bin", "/bin", "/usr/local/bin"]
            .iter()
            .map(|dir| Path::new(dir).join(tool))
            .find(|p| p.exists())
            .unwrap_or_else(|| panic!("{tool} not found"));
        symlink(found, tools.join(tool)).unwrap();
    }

    let out = install(&s, Some(&tools));
    refused(&s, &out, "Neither sha256sum nor shasum found");
}
