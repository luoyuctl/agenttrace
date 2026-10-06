//! `agenttrace update`: replace the running binary with the latest GitHub release.
//!
//! Package-manager installs (Homebrew, npm, cargo) are left to their manager; standalone
//! installs from install.sh / install.ps1 download the release asset, verify its published
//! SHA-256, and swap it in place.

use agenttrace_core::{tr, Message, ReportLanguage, VERSION};
use anyhow::{anyhow, bail, Context};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

const REPO: &str = "luoyuctl/agenttrace";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Total budget for reading a release binary, generous enough for slow links.
const BODY_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const ATTEMPTS: u32 = 3;
/// Release binaries are ~10 MB; anything far outside that is not a real asset.
const MIN_BINARY_BYTES: usize = 1_000_000;
const MAX_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, Default, PartialEq)]
struct Options {
    check: bool,
    force: bool,
    help: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Channel {
    Homebrew,
    Npm,
    Cargo,
    Standalone,
}

impl Channel {
    fn manager(self) -> Option<(&'static str, &'static str)> {
        match self {
            Channel::Homebrew => Some(("Homebrew", "brew upgrade luoyuctl/tap/agenttrace")),
            Channel::Npm => Some(("npm", "npm install -g @zack78/agenttrace@latest")),
            Channel::Cargo => Some((
                "cargo",
                "cargo install --git https://github.com/luoyuctl/agenttrace agenttrace --force",
            )),
            Channel::Standalone => None,
        }
    }
}

pub fn run(args: &[OsString], language: ReportLanguage) -> anyhow::Result<()> {
    let options = parse_options(args, language)?;
    if options.help {
        say(tr(language, "cli.update.usage"))?;
        return Ok(());
    }

    say(tr(language, "cli.update.checking"))?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(READ_TIMEOUT))
        .timeout_recv_body(Some(BODY_TIMEOUT))
        .build()
        .into();
    let latest_tag = latest_release_tag(&agent)?;
    let latest = latest_tag.trim_start_matches('v');
    let dev_build = is_dev_build(VERSION);
    if !dev_build && !is_newer(latest, VERSION) && !options.force {
        say(&msg(
            language,
            "cli.update.up_to_date",
            &[("version", VERSION)],
        ))?;
        return Ok(());
    }
    say(&msg(
        language,
        "cli.update.available",
        &[("current", VERSION), ("latest", latest)],
    ))?;
    if options.check {
        return Ok(());
    }

    let exe = std::env::current_exe()
        .and_then(fs::canonicalize)
        .context("locate the running agenttrace binary")?;
    if let Some((manager, command)) = install_channel(&exe).manager() {
        if !options.force {
            say(&msg(
                language,
                "cli.update.managed",
                &[("manager", manager), ("command", command)],
            ))?;
            return Ok(());
        }
    }
    if dev_build && !options.force {
        say(&msg(
            language,
            "cli.update.dev_build",
            &[("current", VERSION), ("latest", latest)],
        ))?;
        return Ok(());
    }

    let asset = asset_name(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| {
        anyhow!(msg(
            language,
            "cli.update.unsupported_platform",
            &[
                ("os", std::env::consts::OS),
                ("arch", std::env::consts::ARCH)
            ],
        ))
    })?;
    let base = format!("https://github.com/{REPO}/releases/download/{latest_tag}/{asset}");
    say(&msg(
        language,
        "cli.update.downloading",
        &[("asset", &asset)],
    ))?;
    let expected = parse_checksum(&String::from_utf8_lossy(&download(
        &agent,
        &format!("{base}.sha256"),
    )?))
    .ok_or_else(|| anyhow!("invalid checksum file for {asset}"))?;
    let binary = download(&agent, &base)?;
    if binary.len() as u64 > MAX_DOWNLOAD_BYTES {
        bail!(msg(
            language,
            "cli.update.oversized_download",
            &[
                ("asset", &asset),
                ("limit", &(MAX_DOWNLOAD_BYTES / 1024 / 1024).to_string())
            ]
        ));
    }
    if binary.len() < MIN_BINARY_BYTES {
        bail!(msg(
            language,
            "cli.update.incomplete_download",
            &[("asset", &asset), ("bytes", &binary.len().to_string())]
        ));
    }
    if sha256_hex(&binary) != expected {
        bail!(msg(
            language,
            "cli.update.checksum_mismatch",
            &[("asset", &asset)]
        ));
    }
    say(tr(language, "cli.update.verified"))?;

    replace_executable(&exe, &binary).map_err(|error| {
        anyhow!(msg(
            language,
            "cli.update.write_failed",
            &[
                ("path", &exe.display().to_string()),
                ("error", &format!("{error:#}"))
            ],
        ))
    })?;
    say(&msg(
        language,
        "cli.update.updated",
        &[("version", latest), ("path", &exe.display().to_string())],
    ))
}

fn parse_options(args: &[OsString], language: ReportLanguage) -> anyhow::Result<Options> {
    let mut options = Options::default();
    let mut iter = args.iter().map(|arg| arg.to_string_lossy());
    while let Some(arg) = iter.next() {
        match arg.as_ref() {
            "--check" => options.check = true,
            "--force" => options.force = true,
            "-h" | "--help" => options.help = true,
            "--lang" => {
                iter.next();
            }
            other if other.starts_with("--lang=") => {}
            other => bail!(msg(language, "cli.update.unknown_arg", &[("value", other)])),
        }
    }
    Ok(options)
}

/// Resolve the latest release tag from the `releases/latest` redirect on github.com, which
/// is not subject to the 60 requests/hour limit of the unauthenticated REST API. The API
/// (authenticated with `GITHUB_TOKEN` when set) is only a fallback.
fn latest_release_tag(agent: &ureq::Agent) -> anyhow::Result<String> {
    latest_tag_from_redirect().or_else(|redirect_error| {
        latest_tag_from_api(agent)
            .with_context(|| format!("resolve latest release (redirect: {redirect_error:#})"))
    })
}

fn latest_tag_from_redirect() -> anyhow::Result<String> {
    let url = format!("https://github.com/{REPO}/releases/latest");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(READ_TIMEOUT))
        .max_redirects(0)
        .build()
        .into();
    let response = agent
        .head(&url)
        .call()
        .map_err(|error| anyhow!("{url}: {error}"))?;
    let location = response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| anyhow!("{url}: no redirect location"))?;
    tag_from_release_url(location).ok_or_else(|| anyhow!("{url}: unexpected redirect {location}"))
}

fn latest_tag_from_api(agent: &ureq::Agent) -> anyhow::Result<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut request = agent
        .get(&url)
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = std::env::var("GITHUB_TOKEN")
        .ok()
        .filter(|token| !token.is_empty())
    {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let body = request
        .call()
        .map_err(|error| anyhow!("{url}: {error}"))?
        .body_mut()
        .read_to_string()
        .context("read latest release")?;
    let release: serde_json::Value = serde_json::from_str(&body).context("parse latest release")?;
    release["tag_name"]
        .as_str()
        .filter(|tag| parse_version(tag).is_some())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("latest release has no valid tag_name"))
}

/// `.../releases/tag/v0.9.1` -> `v0.9.1`, only for well-formed version tags.
fn tag_from_release_url(location: &str) -> Option<String> {
    let tag = location
        .trim_end_matches('/')
        .rsplit_once("/releases/tag/")?
        .1;
    parse_version(tag).is_some().then(|| tag.to_string())
}

fn download(agent: &ureq::Agent, url: &str) -> anyhow::Result<Vec<u8>> {
    let mut attempt = 1;
    loop {
        match download_once(agent, url) {
            Ok(bytes) => return Ok(bytes),
            Err(error) if attempt >= ATTEMPTS => return Err(error),
            Err(_) => {
                std::thread::sleep(Duration::from_secs(u64::from(attempt)));
                attempt += 1;
            }
        }
    }
}

fn download_once(agent: &ureq::Agent, url: &str) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    agent
        .get(url)
        .call()
        .map_err(|error| anyhow!("{error}"))?
        .into_body()
        .into_reader()
        // One byte past the cap, so an oversized body is detected instead of truncated.
        .take(MAX_DOWNLOAD_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("download {url}"))?;
    Ok(bytes)
}

/// Stage the new binary next to the old one, then rename it into place. Windows cannot
/// overwrite a running executable, but it can rename it out of the way first.
fn replace_executable(exe: &Path, binary: &[u8]) -> anyhow::Result<()> {
    let dir = exe.parent().context("binary has no parent directory")?;
    let staged = dir.join(format!(".agenttrace-update-{}", std::process::id()));
    let result = stage(&staged, binary).and_then(|()| swap(exe, &staged));
    if result.is_err() {
        let _ = fs::remove_file(&staged);
    }
    result
}

fn stage(staged: &Path, binary: &[u8]) -> anyhow::Result<()> {
    fs::write(staged, binary)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(staged, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn swap(exe: &Path, staged: &Path) -> anyhow::Result<()> {
    fs::rename(staged, exe)?;
    Ok(())
}

#[cfg(windows)]
fn swap(exe: &Path, staged: &Path) -> anyhow::Result<()> {
    remove_stale_images(exe);
    // A running image cannot be replaced, only renamed. Each update parks the old image
    // under a unique name so a copy still running elsewhere never blocks the next update.
    let old = exe.with_extension(format!("old-{}.exe", std::process::id()));
    fs::rename(exe, &old)?;
    if let Err(error) = fs::rename(staged, exe) {
        let _ = fs::rename(&old, exe);
        return Err(error.into());
    }
    // Fails while this process is still running; the next update cleans it up.
    let _ = fs::remove_file(&old);
    Ok(())
}

/// Best-effort removal of images parked by earlier updates (`agenttrace.old-<pid>.exe`).
#[cfg(windows)]
fn remove_stale_images(exe: &Path) {
    let (Some(dir), Some(stem)) = (exe.parent(), exe.file_stem().and_then(|s| s.to_str())) else {
        return;
    };
    let prefix = format!("{stem}.old");
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) && name.ends_with(".exe") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn install_channel(exe: &Path) -> Channel {
    let path = exe.to_string_lossy().replace('\\', "/");
    if path.contains("/Cellar/") {
        Channel::Homebrew
    } else if path.contains("/node_modules/") {
        Channel::Npm
    } else if path.contains("/.cargo/bin/") {
        Channel::Cargo
    } else {
        Channel::Standalone
    }
}

fn asset_name(os: &str, arch: &str) -> Option<String> {
    let os = match os {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "windows",
        _ => return None,
    };
    let arch = match arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return None,
    };
    let ext = if os == "windows" { ".exe" } else { "" };
    Some(format!("agenttrace-{os}-{arch}{ext}"))
}

/// First token of a `sha256sum`-style line (`<hex>  name` or `<hex> *name`).
fn parse_checksum(text: &str) -> Option<String> {
    let hex = text.split_whitespace().next()?.to_ascii_lowercase();
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hex)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let core = version
        .trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()?;
    let mut parts = core.split('.').map(|part| part.parse::<u64>().ok());
    let parsed = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(parsed)
}

fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

fn is_dev_build(version: &str) -> bool {
    version.contains("-dev") || parse_version(version) == Some((0, 0, 0))
}

fn msg(language: ReportLanguage, key: &'static str, args: &[(&str, &str)]) -> String {
    args.iter()
        .fold(Message::new(key), |message, (name, value)| {
            message.arg(name, value)
        })
        .render_or(language, key)
}

fn say(line: &str) -> anyhow::Result<()> {
    let mut stdout = std::io::stdout();
    writeln!(stdout, "{line}")?;
    stdout.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically_and_ignore_prefix_and_suffix() {
        assert_eq!(parse_version("v0.9.1"), Some((0, 9, 1)));
        assert_eq!(parse_version("0.10.0-rc.1"), Some((0, 10, 0)));
        assert_eq!(parse_version("0.9"), None);
        assert_eq!(parse_version("0.9.1.2"), None);
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(!is_newer("v0.9.1", "0.9.1"));
        assert!(!is_newer("garbage", "0.9.1"));
        assert!(is_dev_build("0.0.0-dev"));
        assert!(!is_dev_build("0.9.1"));
    }

    #[test]
    fn release_redirects_yield_valid_tags_only() {
        let tag = |location: &str| tag_from_release_url(location);
        assert_eq!(
            tag("https://github.com/luoyuctl/agenttrace/releases/tag/v0.9.1").as_deref(),
            Some("v0.9.1")
        );
        assert_eq!(
            tag("https://github.com/luoyuctl/agenttrace/releases/tag/v0.10.0/").as_deref(),
            Some("v0.10.0")
        );
        assert_eq!(tag("https://github.com/luoyuctl/agenttrace/releases"), None);
        assert_eq!(
            tag("https://github.com/luoyuctl/agenttrace/releases/tag/nightly"),
            None
        );
    }

    #[test]
    fn asset_names_match_release_assets() {
        assert_eq!(
            asset_name("macos", "aarch64").as_deref(),
            Some("agenttrace-darwin-arm64")
        );
        assert_eq!(
            asset_name("linux", "x86_64").as_deref(),
            Some("agenttrace-linux-amd64")
        );
        assert_eq!(
            asset_name("windows", "aarch64").as_deref(),
            Some("agenttrace-windows-arm64.exe")
        );
        assert_eq!(asset_name("freebsd", "x86_64"), None);
        assert_eq!(asset_name("linux", "riscv64"), None);
    }

    #[test]
    fn checksum_lines_parse_both_sha256sum_styles() {
        let hex = "42ec5fd9b85cdf153e80df697facd773ebe9d8066eb63ee7e1a5d4e6672bced7";
        assert_eq!(
            parse_checksum(&format!("{hex} *agenttrace-windows-amd64.exe\n")).as_deref(),
            Some(hex)
        );
        assert_eq!(
            parse_checksum(&format!("{}  agenttrace-linux-amd64", hex.to_uppercase())).as_deref(),
            Some(hex)
        );
        assert_eq!(parse_checksum("not-a-checksum file"), None);
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn package_manager_installs_are_detected_from_the_binary_path() {
        let channel = |path: &str| install_channel(Path::new(path));
        assert_eq!(
            channel("/opt/homebrew/Cellar/agenttrace/0.9.1/bin/agenttrace"),
            Channel::Homebrew
        );
        assert_eq!(
            channel("/home/linuxbrew/.linuxbrew/Cellar/agenttrace/0.9.1/bin/agenttrace"),
            Channel::Homebrew
        );
        assert_eq!(
            channel(
                r"C:\Users\me\AppData\Roaming\npm\node_modules\@zack78\agenttrace\lib\agenttrace.exe"
            ),
            Channel::Npm
        );
        assert_eq!(channel("/home/me/.cargo/bin/agenttrace"), Channel::Cargo);
        assert_eq!(
            channel("/home/me/.local/bin/agenttrace"),
            Channel::Standalone
        );
        assert_eq!(
            channel(r"C:\Users\me\AppData\Local\agenttrace\agenttrace.exe"),
            Channel::Standalone
        );
    }

    #[test]
    fn options_accept_lang_and_reject_unknown_flags() {
        let args = |values: &[&str]| values.iter().map(OsString::from).collect::<Vec<_>>();
        let options = parse_options(&args(&["--check", "--lang", "zh"]), ReportLanguage::En)
            .expect("valid options");
        assert_eq!(
            options,
            Options {
                check: true,
                ..Options::default()
            }
        );
        assert!(
            parse_options(&args(&["--force", "--lang=en"]), ReportLanguage::En)
                .expect("valid options")
                .force
        );
        assert!(parse_options(&args(&["--yes"]), ReportLanguage::En).is_err());
    }

    #[test]
    fn replace_executable_swaps_the_binary_in_place() {
        let dir =
            std::env::temp_dir().join(format!("agenttrace-update-test-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let exe = dir.join("agenttrace");
        fs::write(&exe, b"old").expect("write old binary");
        replace_executable(&exe, b"new").expect("replace binary");
        assert_eq!(fs::read(&exe).expect("read binary"), b"new");
        let leftovers = fs::read_dir(&dir).expect("list dir").count();
        assert_eq!(leftovers, 1, "staged file should be renamed into place");
        let _ = fs::remove_dir_all(&dir);
    }
}
