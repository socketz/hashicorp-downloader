use clap::{Args as ClapArgs, Parser};
use lazy_static::lazy_static;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::task;
use tokio::process::Command as TokioCommand;
use std::fs::File as StdFile;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::PathBuf;
use std::io::{self, Write};

const RELEASES_URL: &str = "https://api.releases.hashicorp.com/v1/";

// --- Product List Logic ---
async fn get_all_products(client: &reqwest::Client, license_class: &str) -> Result<Vec<String>, MyError> {
    let url = format!("{}products?license_class={}", RELEASES_URL, license_class);
    println!("Fetching product list from API: {}", url);

    let products: Vec<String> = client
        .get(&url)
        .header("Accept", "application/vnd+hashicorp.releases-api.v1+json")
        .send()
        .await?
        .json::<Vec<String>>()
        .await?;
    
    Ok(products)
}


// --- Data Models (Structs) ---

#[derive(Deserialize, Debug, Clone)]
struct Status {
    state: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Build {
    arch: String,
    os: String,
    url: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Release {
    version: String,
    status: Status,
    builds: Vec<Build>,
    is_prerelease: bool,
}

// --- Platform Mappings ---

lazy_static! {
    static ref ARCH_MAPPING: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("x86_64", "amd64");
        m.insert("aarch64", "arm64");
        m.insert("arm", "arm");
        m.insert("i686", "386");
        m
    };
    static ref OS_MAPPING: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("linux", "linux");
        m.insert("macos", "darwin");
        m.insert("windows", "windows");
        m.insert("freebsd", "freebsd");
        m.insert("openbsd", "openbsd");
        m
    };
}

// --- Custom Error Handling ---

#[derive(Error, Debug)]
pub enum MyError {
    #[error("Network request error")]
    Request(#[from] reqwest::Error),
    #[error("JSON processing error")]
    Json(#[from] serde_json::Error),
    #[error("Logic error: {0}")]
    LogicError(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

// --- Command-Line Arguments ---

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(flatten)]
    download_args: DownloadArgs,
}

#[derive(ClapArgs, Debug)]
struct DownloadArgs {
     /// Name of the product to download, or "all" to download all available products from the API.
    product: Option<String>,

    /// Product version (e.g., "1.9.3", defaults to "latest").
    #[arg(short = 'v', long, default_value_t = String::from("latest"))]
    product_version: String,

    /// Allow downloading prerelease versions.
    #[arg(long)]
    prerelease: bool,

    /// Target architecture (e.g., amd64, arm64, i386). Auto-detected by default.
    #[arg(short, long, default_value_t = String::from("auto"))]
    arch: String,

    /// Target operating system (e.g., linux, windows). Auto-detected by default.
    #[arg(short, long, default_value_t = String::from("auto"))]
    os: String,

    /// License class of the product to download. Possible values: enterprise, hcp, oss
    #[arg(short = 'l', long, default_value_t = String::from("oss"))]
    license_class: String,

    /// Path to save the downloaded file(s).
    #[arg(short = 'f', long, default_value_t = String::from("./downloads"))]
    filepath: String,

    /// After download, extract the ZIP (keeping only executable files) into the same directory and remove the ZIP file.
    #[arg(long)]
    extract: bool,

    /// Force overwrite of already existing downloaded files and extracted executables.
    #[arg(long)]
    force: bool,

    /// List all available products from releases.hashicorp.com
    #[arg(long)]
    list: bool,
}


// --- Download Logic ---

async fn download_file(client: &reqwest::Client, url: &str, target_dir: &str, force: bool) -> Result<PathBuf, MyError> {
    // 1. Ensure the target directory exists
    tokio::fs::create_dir_all(target_dir).await?;

    // 2. Extract the filename from the URL
    let filename = url.split('/').last().ok_or_else(|| {
        MyError::LogicError("Could not extract filename from URL.".to_string())
    })?;
    let dest_path = Path::new(target_dir).join(filename);

    // If file exists and not forcing, skip re-download
    if dest_path.exists() && !force {
        println!("\nFile already exists, skipping download: {}", dest_path.display());
        return Ok(dest_path);
    }

    println!("\nDownloading {} to {}...", filename, dest_path.display());

    // 3. Perform the request and get the response bytes
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(MyError::LogicError(format!(
            "Failed to download file. Status: {}",
            response.status()
        )));
    }

    // 4. Create the destination file and write the content
    let mut dest_file = File::create(&dest_path).await?;

    let bytes = response.bytes().await.map_err(MyError::Request)?;
    dest_file.write_all(&bytes).await?;

    println!("Download completed successfully.");
    Ok(dest_path)
}

// Helper: check for .zip extension
fn has_zip_ext(p: &Path) -> bool {
    p.extension().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case("zip")).unwrap_or(false)
}

// Helper: check for .msi extension
fn has_msi_ext(p: &Path) -> bool {
    p.extension().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case("msi")).unwrap_or(false)
}

// Helper: prompt user for yes/no question
fn prompt_yes_no(question: &str) -> io::Result<bool> {
    loop {
        print!("{} (y/N): ", question);
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" | "" => return Ok(false),
            _ => println!("Please answer 'y' for yes or 'n' for no."),
        }
    }
}

// Helper: run MSI installation silently
async fn install_msi_silent(msi_path: &Path) -> Result<(), MyError> {
    println!("Starting silent installation of {}...", msi_path.display());
    
    let status = TokioCommand::new("msiexec")
        .args([
            "/i",
            &msi_path.to_string_lossy(),
            "/quiet",
            "/norestart"
        ])
        .status()
        .await
        .map_err(|e| MyError::LogicError(format!("Failed to execute msiexec: {}", e)))?;

    if status.success() {
        println!("✅ Installation completed successfully.");
    } else {
        return Err(MyError::LogicError(format!(
            "Installation failed with exit code: {:?}",
            status.code()
        )));
    }
    
    Ok(())
}

// Helper: recursively move executable files from src to dest root (flatten), returns count
fn move_exes_recursively(src: &Path, dest_root: &Path, force: bool) -> std::io::Result<usize> {
    fn move_file(from: &Path, to: &Path, force: bool) -> std::io::Result<()> {
        if force && to.exists() {
            // Remove destination first to allow rename on Windows
            let _ = std::fs::remove_file(to);
        }
        match std::fs::rename(from, to) {
            Ok(()) => Ok(()),
            Err(_) => {
                // If rename fails (e.g., across filesystems), copy and remove
                if to.exists() && force {
                    let _ = std::fs::remove_file(to);
                }
                std::fs::copy(from, to)?;
                std::fs::remove_file(from)
            }
        }
    }

    let mut count = 0usize;
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("exe")).unwrap_or(false) {
                let file_name = path.file_name().unwrap();
                let mut dest_path = dest_root.join(file_name);
                // Avoid overwriting by adding a numeric suffix if needed (when not forced)
                if dest_path.exists() && !force {
                    let stem = dest_path.file_stem().and_then(|s| s.to_str()).unwrap_or("program");
                    let ext = dest_path.extension().and_then(|s| s.to_str()).unwrap_or("exe");
                    let mut idx = 1u32;
                    loop {
                        let candidate = dest_root.join(format!("{}-{}.{}", stem, idx, ext));
                        if !candidate.exists() { dest_path = candidate; break; }
                        idx += 1;
                    }
                }
                move_file(&path, &dest_path, force)?;
                count += 1;
            }
        }
    }
    Ok(count)
}

// Extract only executable files using OS facilities on Windows (PowerShell Expand-Archive),
// falling back to zip crate on other platforms. Returns number of executable files extracted.
async fn extract_exe_from_zip(zip_path: &Path, dest_dir: &Path, force: bool) -> Result<usize, MyError> {
    #[cfg(windows)]
    {
        // Create a temporary extraction directory under dest_dir
        let millis = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let tmp_dir = dest_dir.join(format!(".hcd_extract_{}", millis));
        tokio::fs::create_dir_all(&tmp_dir).await?;

        // Use PowerShell's Expand-Archive to extract contents
    let status = TokioCommand::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "Expand-Archive -LiteralPath {} -DestinationPath {} -Force",
                    format!("\"{}\"", zip_path.display()),
                    format!("\"{}\"", tmp_dir.display())
                ),
            ])
            .status()
            .await
            .map_err(|e| MyError::LogicError(format!("Failed to invoke PowerShell Expand-Archive: {}", e)))?;

        if !status.success() {
            // Cleanup tmp dir and fall back to internal extractor
            let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
        } else {
            // Move only executable files from tmp_dir to dest_dir
            let count = task::spawn_blocking({
                let tmp_dir = tmp_dir.clone();
                let dest_dir = dest_dir.to_path_buf();
                move || move_exes_recursively(&tmp_dir, &dest_dir, force)
            })
            .await
            .map_err(|e| MyError::LogicError(format!("Task join error: {}", e)))
            .and_then(|r| r.map_err(|e| MyError::Io(e)))?;

            // Remove temp dir
            tokio::fs::remove_dir_all(&tmp_dir).await.ok();
            return Ok(count);
        }
    }

    // On Unix/macOS: try system tools first, then fallback to internal
    #[cfg(all(unix, not(windows)))]
    {
        use std::ffi::OsStr;
        // Create a temporary extraction directory under dest_dir
        let millis = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let tmp_dir = dest_dir.join(format!(".hcd_extract_{}", millis));
        tokio::fs::create_dir_all(&tmp_dir).await?;

        // Helper to run a command and return whether it succeeded
        async fn run_status(mut cmd: TokioCommand) -> bool {
            match cmd.status().await { Ok(s) if s.success() => true, _ => false }
        }

        // 1) Try unzip (widely available on macOS and many Linux distros)
        let unzip_ok = run_status({
            let mut c = TokioCommand::new("unzip");
            c.arg("-o").arg(zip_path).arg("-d").arg(&tmp_dir);
            c
        }).await;

        // 2) macOS specific alternative: ditto
        #[cfg(target_os = "macos")]
        let ditto_ok = if !unzip_ok {
            run_status({
                let mut c = TokioCommand::new("ditto");
                c.args(["-x", "-k"]).arg(zip_path).arg(&tmp_dir);
                c
            }).await
        } else { false };
        #[cfg(not(target_os = "macos"))]
        let ditto_ok = false;

        // 3) Try bsdtar as another common option
        let bsdtar_ok = if !unzip_ok && !ditto_ok {
            run_status({
                let mut c = TokioCommand::new("bsdtar");
                c.args(["-xf"]).arg(zip_path).args(["-C"]).arg(&tmp_dir);
                c
            }).await
        } else { false };

    if unzip_ok || ditto_ok || bsdtar_ok {
            // Move only executable files from tmp_dir to dest_dir
            let count = task::spawn_blocking({
                let tmp_dir = tmp_dir.clone();
                let dest_dir = dest_dir.to_path_buf();
        move || move_exes_recursively(&tmp_dir, &dest_dir, force)
            })
            .await
            .map_err(|e| MyError::LogicError(format!("Task join error: {}", e)))
            .and_then(|r| r.map_err(|e| MyError::Io(e)))?;

            // Remove temp dir
            tokio::fs::remove_dir_all(&tmp_dir).await.ok();
            return Ok(count);
        } else {
            // Cleanup and fallback to internal
            let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
        }
    }

    // Fallback: internal ZIP parsing (keeps only executable entries) for all platforms
    let zip_path_buf = zip_path.to_path_buf();
    let dest_dir_buf = dest_dir.to_path_buf();
    let count = task::spawn_blocking(move || -> Result<usize, MyError> {
        let file = StdFile::open(&zip_path_buf)?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| MyError::LogicError(format!("Invalid ZIP file: {}", e)))?;
        let mut exe_count = 0usize;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| MyError::LogicError(format!("ZIP read error: {}", e)))?;
            let enclosed = match file.enclosed_name() { Some(p) => p.to_owned(), None => continue };
            if enclosed.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("exe")).unwrap_or(false) {
                // Resolve destination path with force-aware overwrite or suffixing
                let filename = enclosed.file_name().unwrap();
                let mut outpath = dest_dir_buf.join(filename);
                if outpath.exists() && !force {
                    let stem = outpath.file_stem().and_then(|s| s.to_str()).unwrap_or("program");
                    let ext = outpath.extension().and_then(|s| s.to_str()).unwrap_or("exe");
                    let mut idx = 1u32;
                    loop {
                        let candidate = dest_dir_buf.join(format!("{}-{}.{}", stem, idx, ext));
                        if !candidate.exists() { outpath = candidate; break; }
                        idx += 1;
                    }
                }
                if force && outpath.exists() { let _ = std::fs::remove_file(&outpath); }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
                exe_count += 1;
            }
        }
        Ok(exe_count)
    })
    .await
    .map_err(|e| MyError::LogicError(format!("Task join error: {}", e)))??;
    Ok(count)
}

// --- Main Logic ---

async fn get_download_url(
    client: &reqwest::Client,
    product: &str,
    version_req: &str,
    allow_prerelease: bool,
    target_arch: &str,
    target_os: &str,
    license_class: &str,
) -> Result<String, MyError> {
    // 1. Build URL and fetch all releases for the product
    let url = format!(
        "{}releases/{}?license_class={}",
        RELEASES_URL, product, license_class
    );
    println!("Fetching releases from: {}", url);

    let all_releases: Vec<Release> = client.get(&url).send().await?.json::<Vec<Release>>().await?;

    if all_releases.is_empty() {
        return Err(MyError::LogicError(format!(
            "Product '{}' with license class '{}' not found or has no releases.",
            product, license_class
        )));
    }

    // 2. Filter releases to find the one we want to download
    let target_release: Release = {
        // First, filter for only supported releases
        let supported_releases: Vec<Release> = all_releases
            .into_iter()
            .filter(|r| r.status.state == "supported")
            .collect();

        if supported_releases.is_empty() {
            return Err(MyError::LogicError(format!("No supported versions found for '{}'.", product)));
        }

        if version_req != "latest" {
            // If a specific version is requested
            supported_releases
                .into_iter()
                .find(|r| r.version == version_req)
                .ok_or_else(|| MyError::LogicError(format!("Version '{}' not found or is not supported.", version_req)))?
        } else {
            // If the latest version is requested
            let mut release_iterator = supported_releases.into_iter();
            
            if allow_prerelease {
                // The first in the list (most recent, with or without prerelease)
                release_iterator.next()
            } else {
                // The first that is not a prerelease
                release_iterator.find(|r| !r.is_prerelease)
            }
            .ok_or_else(|| MyError::LogicError("No suitable version found. Try with --prerelease for preliminary versions.".to_string()))?
        }
    };

    println!("Selected version: {} (Prerelease: {})", target_release.version, target_release.is_prerelease);

    // 3. Find the build for the correct architecture and OS
    let build = target_release.builds.iter()
        .find(|b| b.os == target_os && b.arch == target_arch)
        .ok_or_else(|| {
            let available_platforms = target_release.builds.iter()
                .map(|b| format!("{}/{}", b.os, b.arch))
                .collect::<Vec<_>>()
                .join(", ");
            MyError::LogicError(format!(
                "No compatible build found for platform '{}/{}'.\nAvailable platforms for v{}: {}",
                target_os, target_arch, target_release.version, available_platforms
            ))
        })?;

    Ok(build.url.clone())
}

#[tokio::main]
async fn main() -> Result<(), MyError> {
    let cli = Cli::parse();

    let args = cli.download_args;

    // Handle list command first
    if args.list {
        let client = reqwest::Client::new();
        println!("Fetching available products from releases.hashicorp.com...\n");
        
        match get_all_products(&client, &args.license_class).await {
            Ok(products) => {
                println!("Available products (license class: {}):", args.license_class);
                println!("{}", "=".repeat(50));
                for (i, product) in products.iter().enumerate() {
                    println!("{:3}. {}", i + 1, product);
                }
                println!("\nTotal: {} products", products.len());
                println!("\nUsage: hcd <product_name> [options]");
                println!("Example: hcd terraform --extract");
                return Ok(());
            },
            Err(e) => {
                eprintln!("Error fetching product list: {}", e);
                return Err(e);
            }
        }
    }

    let product_arg = args.product.ok_or_else(|| MyError::LogicError("Product name is required for downloading. Use --list to see available products or specify --product <name>.".to_string()))?;

    let client = reqwest::Client::new();

    // Resolve OS and Arch if set to "auto"
    let os = if args.os == "auto" {
        OS_MAPPING.get(std::env::consts::OS).map(|s| s.to_string())
            .ok_or_else(|| MyError::LogicError(format!("Unsupported operating system: {}", std::env::consts::OS)))?
    } else {
        args.os
    };

    let arch = if args.arch == "auto" {
        ARCH_MAPPING.get(std::env::consts::ARCH).map(|s| s.to_string())
            .ok_or_else(|| MyError::LogicError(format!("Unsupported architecture: {}", std::env::consts::ARCH)))?
    } else {
        args.arch
    };

    let products_to_download: Vec<String> = if product_arg.to_lowercase() == "all" {
        get_all_products(&client, &args.license_class).await?
    } else {
        vec![product_arg]
    };

    for product in &products_to_download {
        println!("\n----------------------------------------");
        println!("Product: {}", product);
        println!("Requested Version: {}", args.product_version);
        println!("License Class: {}", args.license_class);
        println!("Target Platform: {}/{}", os, arch);
        println!("Allow Prerelease: {}", args.prerelease);

        // Get the download URL
        match get_download_url(
            &client,
            product,
            &args.product_version,
            args.prerelease,
            &arch,
            &os,
            &args.license_class,
        )
        .await
        {
            Ok(download_url) => {
                println!("\nDownload URL found:\n{}", download_url);
                
                // Start the file download
                if let Err(e) = async {
                    let saved_path = download_file(&client, &download_url, &args.filepath, args.force).await?;

                    if args.extract {
                        // Only attempt to extract if it looks like a ZIP
                        if has_zip_ext(&saved_path) {
                            println!("Extracting (only executable) from {} ...", saved_path.display());
                            let count = extract_exe_from_zip(&saved_path, Path::new(&args.filepath), args.force).await?;
                            println!("Extracted {} executable file(s).", count);
                            // Remove the ZIP after extraction
                            tokio::fs::remove_file(&saved_path).await?;
                            println!("Extraction complete and ZIP removed.");
                        } else {
                            println!("--extract specified, but downloaded file is not a .zip: {}", saved_path.display());
                        }
                    } else if has_zip_ext(&saved_path) {
                        // Ask if user wants to extract when --extract not specified
                        let question = format!("Do you want to extract executables from {}?", saved_path.file_name().unwrap().to_string_lossy());
                        match prompt_yes_no(&question) {
                            Ok(true) => {
                                println!("Extracting (only executable) from {} ...", saved_path.display());
                                let count = extract_exe_from_zip(&saved_path, Path::new(&args.filepath), args.force).await?;
                                println!("Extracted {} executable file(s).", count);
                                // Remove the ZIP after extraction
                                tokio::fs::remove_file(&saved_path).await?;
                                println!("Extraction complete and ZIP removed.");
                            },
                            Ok(false) => {
                                println!("ZIP file downloaded but not extracted: {}", saved_path.display());
                                println!("To extract later, run the same command with --extract flag.");
                            },
                            Err(prompt_err) => {
                                eprintln!("⚠️  Input error: {}", prompt_err);
                                println!("ZIP file available at: {}", saved_path.display());
                            }
                        }
                    } else if has_msi_ext(&saved_path) {
                        // Handle MSI files - offer installation
                        #[cfg(windows)]
                        {
                            let question = format!("Do you want to install {} silently?", saved_path.file_name().unwrap().to_string_lossy());
                            match prompt_yes_no(&question) {
                                Ok(true) => {
                                    if let Err(install_err) = install_msi_silent(&saved_path).await {
                                        eprintln!("⚠️  Installation error: {}", install_err);
                                        println!("You can manually install the MSI file: {}", saved_path.display());
                                    }
                                },
                                Ok(false) => {
                                    println!("MSI file downloaded but not installed: {}", saved_path.display());
                                    println!("To install later, run: msiexec /i \"{}\" /quiet /norestart", saved_path.display());
                                },
                                Err(prompt_err) => {
                                    eprintln!("⚠️  Input error: {}", prompt_err);
                                    println!("MSI file available at: {}", saved_path.display());
                                }
                            }
                        }
                        #[cfg(not(windows))]
                        {
                            println!("MSI file downloaded: {}", saved_path.display());
                            println!("Note: MSI files are Windows installers and cannot be used on this platform.");
                        }
                    }

                    Ok::<(), MyError>(())
                }.await {
                    eprintln!("\nError during download for {}: {}", product, e);
                    // Continue to the next product instead of exiting
                }
            },
            Err(e) => {
                eprintln!("\nError processing product {}: {}", product, e);
                // Continue to the next product
            }
        }
    }
    println!("----------------------------------------");

    Ok(())
}