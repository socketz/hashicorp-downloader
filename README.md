# HashiCorp Downloader

![Build Status](https://github.com/socketz/hashicorp-downloader/actions/workflows/release.yaml/badge.svg)

A fast, cross-platform HashiCorp product downloader written in Rust, to always get the version you need.

This tool is designed to simplify the process of fetching HashiCorp tools like Terraform, Vault, Consul, and others. It automatically detects your OS and architecture and downloads the latest stable version by default, eliminating the need for complex scripts or manual browsing of the releases website.

## ✨ Features

- **🚀 Effortless Downloads**: Get the latest stable version of any HashiCorp tool with a single command.
- **🔍 Auto-Detection**: Automatically detects your system's operating system and architecture.
- **📌 Version Pinning**: Easily specify an exact product version to download.
- **🌐 Cross-Platform Support**: Download builds for different operating systems and architectures.
- **🧪 Pre-Release Support**: Option to download the latest pre-release versions.
- **📄 License Class Selection**: Download different license editions (`oss`, `enterprise`, `hcp`).
- **📦 Bulk Downloads**: Download all available products for a specific license class with a single command (`all`).
- **📋 Product Listing**: List all available products with `--list` flag.
- **📁 Smart Extraction**: Automatically extract ZIP files and keep only executables, removing unnecessary files.
- **🗂️ MSI Installation**: Interactive installation support for Windows MSI packages (like Vagrant).
- **💪 Force Overwrite**: Force overwrite existing files with `--force` flag.
- **🔒 Safe Extraction**: Uses system tools (PowerShell on Windows, unzip/ditto/bsdtar on Unix) to avoid antivirus false positives.
- **📊 Interactive Prompts**: Ask user whether to extract ZIP files or install MSI packages when flags not specified.
- **🔄 Version Automation**: GitHub Actions workflow for automated version bumping on releases.

## 🛠️ Installation

Ensure you have the Rust toolchain installed. You can get it from [rustup.rs](https://rustup.rs/).

Clone the repository and build the project:

```sh
git clone https://github.com/socketz/hashicorp-downloader.git
cd hashicorp-downloader
cargo build --release
```

The executable will be available at `./target/release/hcd` (or `hcd.exe` on Windows).

## 📖 Usage

### Quick Examples

**1. List all available products:**

```sh
hcd --list
```

**2. Download the latest stable version of Terraform:**

```sh
hcd terraform
```

**3. Download and auto-extract Terraform:**

```sh
hcd terraform --extract
```

**4. Download a specific version of Vault:**

```sh
hcd vault -v 1.15.2
```

**5. Download Vagrant (MSI) with installation prompt:**

```sh
hcd vagrant
```

**6. Download all available OSS tools to a specific directory:**

```sh
hcd all -f ./my-tools
```

**7. Force overwrite existing files:**

```sh
hcd terraform --force --extract
```

**8. Download enterprise products:**

```sh
hcd --list -l enterprise
hcd consul -l enterprise
```

### 🔧 Arguments and Options

| Flag                | Short | Description                                                              | Default      |
|---------------------|-------|--------------------------------------------------------------------------|--------------|
| `[PRODUCT]`         |       | Name of the product to download, or "all" to download all products      | (Required)*  |
| `--list`            |       | List all available products from releases.hashicorp.com                 | `false`      |
| `--product-version` | `-v`  | Product version to download (e.g., "1.9.3")                            | `latest`     |
| `--prerelease`      |       | Allow downloading pre-release versions                                   | `false`      |
| `--arch`            | `-a`  | Target architecture (e.g., amd64, arm64, 386)                          | `auto`       |
| `--os`              | `-o`  | Target operating system (e.g., linux, windows, darwin)                 | `auto`       |
| `--license-class`   | `-l`  | License class: `oss`, `enterprise`, `hcp`                              | `oss`        |
| `--filepath`        | `-f`  | Path to save the downloaded file(s)                                     | `./downloads`|
| `--extract`         |       | Extract ZIP files (keeping only executables) and remove ZIP             | `false`      |
| `--force`           |       | Force overwrite existing downloaded files and extracted executables     | `false`      |
| `--help`            | `-h`  | Print help information                                                   |              |
| `--version`         | `-V`  | Print version information                                               |              |

*Product is required unless using `--list`

### 📁 File Handling Behavior

#### ZIP Files

- **Without `--extract`**: Downloads ZIP and prompts user whether to extract
- **With `--extract`**: Automatically extracts ZIP, keeps only `.exe` files, removes ZIP
- **Extraction method**: Uses system tools (PowerShell Expand-Archive on Windows, unzip/ditto/bsdtar on Unix) with fallback to internal Rust implementation

#### MSI Files (Windows)

- **Always prompts**: Asks user whether to install silently using `msiexec`
- **Silent installation**: Uses `msiexec /i "file.msi" /quiet /norestart`
- **Cross-platform**: Shows informative message on non-Windows systems

#### Force Overwrite

- **Downloads**: Skip re-download if file exists unless `--force` is used
- **Extraction**: Add numeric suffixes (`program-1.exe`, `program-2.exe`) unless `--force` is used
- **With `--force`**: Always overwrites existing files

### 🌍 Supported Platforms

| Platform | Auto-Detection | Download Support | Extraction Support |
|----------|----------------|------------------|--------------------|
| Windows  | ✅             | ✅               | PowerShell + Rust  |
| Linux    | ✅             | ✅               | unzip/bsdtar + Rust|
| macOS    | ✅             | ✅               | unzip/ditto + Rust |
| FreeBSD  | ✅             | ✅               | unzip/bsdtar + Rust|
| OpenBSD  | ✅             | ✅               | unzip/bsdtar + Rust|

### 📊 Advanced Examples

**List products by license class:**

```sh
# List OSS products
hcd --list

# List Enterprise products  
hcd --list -l enterprise

# List HCP products
hcd --list -l hcp
```

**Download specific architecture/OS:**

```sh
# Download Linux AMD64 version on Windows
hcd terraform -o linux -a amd64

# Download ARM64 version
hcd terraform -a arm64
```

**Batch operations:**

```sh
# Download all OSS products with extraction
hcd all --extract -f ./tools

# Download all enterprise products 
hcd all -l enterprise -f ./enterprise-tools
```

**Version management:**

```sh
# Download latest stable
hcd terraform

# Download specific version
hcd terraform -v 1.6.0

# Download latest including pre-releases
hcd terraform --prerelease
```

## 🤖 GitHub Actions Integration

The project includes automated version bumping on releases. When you create a release:

**Tag naming for version bumps:**

- `v1.0.0-major` → Bumps major version
- `v1.0.0-minor` → Bumps minor version  
- `v1.0.0-patch` → Bumps patch version
- `v1.0.0` → Default to patch bump

**Release title keywords:**

- Include "major" or "minor" in release title for respective bumps
- Default is patch bump

The workflow automatically:

1. Updates `Cargo.toml` version
2. Updates `Cargo.lock` version  
3. Creates a PR with the changes
4. Maintains version consistency

## 🔧 Development

**Build:**

```sh
cargo build --release
```

**Test:**

```sh
cargo test
```

**Clean:**

```sh
cargo clean
```

**Version bump script:**

```sh
python scripts/bump_version.py [major|minor|patch]
```

## 📝 Examples Output

```sh
$ hcd --list
Fetching available products from releases.hashicorp.com...

Available products (license class: oss):
==================================================
  1. boundary
  2. consul  
  3. nomad
  4. packer
  5. terraform
  6. vagrant
  7. vault
  ...
Total: 280 products

$ hcd terraform
----------------------------------------
Product: terraform
Requested Version: latest
License Class: oss
Target Platform: windows/amd64
Allow Prerelease: false

Download URL found:
https://releases.hashicorp.com/terraform/1.13.5/terraform_1.13.5_windows_amd64.zip

Downloading terraform_1.13.5_windows_amd64.zip to ./downloads\terraform_1.13.5_windows_amd64.zip...
Download completed successfully.
Do you want to extract executables from terraform_1.13.5_windows_amd64.zip? (y/N): y
Extracting (only executable) from ./downloads\terraform_1.13.5_windows_amd64.zip ...
Extracted 1 executable file(s).
Extraction complete and ZIP removed.
```

## 🐛 Troubleshooting

**Antivirus false positives during extraction:**

- The tool uses system utilities (PowerShell, unzip) to minimize false positives
- If issues persist, try `--force` flag or whitelist the download directory

**Network issues:**

- Check internet connection
- Verify HashiCorp releases API is accessible: [https://api.releases.hashicorp.com/v1/products](https://api.releases.hashicorp.com/v1/products)

**File permissions:**

- Ensure write permissions to the target directory
- On Unix systems, extracted executables preserve original permissions

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## 📄 License

This project is licensed under the [MIT License](LICENSE).
