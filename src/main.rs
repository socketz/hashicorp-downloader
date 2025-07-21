use clap::{Args as ClapArgs, Parser, Subcommand};
use lazy_static::lazy_static;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

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
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    download_args: DownloadArgs,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Clean the destination directory
    Clean {
        /// Path of the directory to clean.
        #[arg(short = 'c', long, default_value_t = String::from("./downloads"))]
        filepath: String,
    },
}

#[derive(ClapArgs, Debug)]
struct DownloadArgs {
     /// Name of the product to download, or "all" to download all available products from the API.
    #[arg(short, long)]
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
}


// --- Download Logic ---

async fn download_file(client: &reqwest::Client, url: &str, target_dir: &str) -> Result<(), MyError> {
    // 1. Ensure the target directory exists
    tokio::fs::create_dir_all(target_dir).await?;

    // 2. Extract the filename from the URL
    let filename = url.split('/').last().ok_or_else(|| {
        MyError::LogicError("Could not extract filename from URL.".to_string())
    })?;
    let dest_path = Path::new(target_dir).join(filename);

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
    Ok(())
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

    if let Some(Command::Clean { filepath }) = cli.command {
        let dest_dir = Path::new(&filepath);
        if dest_dir.exists() {
            println!("Cleaning destination directory: {}", dest_dir.display());
            tokio::fs::remove_dir_all(dest_dir).await?;
            println!("Directory cleaned successfully.");
        } else {
            println!("Destination directory {} does not exist, nothing to clean.", dest_dir.display());
        }
        return Ok(());
    }

    let args = cli.download_args;
    let product_arg = args.product.ok_or_else(|| MyError::LogicError("Product name is required for downloading. Use --product <name> or see --help.".to_string()))?;

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
                if let Err(e) = download_file(&client, &download_url, &args.filepath).await {
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