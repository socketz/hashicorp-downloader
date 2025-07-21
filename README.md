# HashiCorp Downloader

A simple, fast, and convenient command-line tool to download any official HashiCorp product directly from their release repository.

This tool is designed to simplify the process of fetching HashiCorp tools like Terraform, Vault, Consul, and others. It automatically detects your OS and architecture and downloads the latest stable version by default, eliminating the need for complex scripts or manual browsing of the releases website.

## Features

- **Effortless Downloads**: Get the latest stable version of any HashiCorp tool with a single command.
- **Auto-Detection**: Automatically detects your system's operating system and architecture.
- **Version Pinning**: Easily specify an exact product version to download.
- **Cross-Platform Support**: Download builds for different operating systems and architectures.
- **Pre-Release Support**: Option to download the latest pre-release versions.
- **License Class Selection**: Download different license editions (`oss`, `enterprise`, `hcp`).
- **Bulk Downloads**: Download all available products for a specific license class with a single command (`--product all`).
- **Standalone Clean Command**: Safely clean the output directory without triggering a download.
- **Dynamic Product List**: Fetches the most up-to-date list of products directly from the HashiCorp API.

## Installation

> **Note**: Soon, pre-compiled binaries for major platforms (Windows, macOS, and Linux) will be automatically generated via GitHub Actions and available for download from the project's [Releases page](https://github.com/socketz/hashicorp-downloader/releases).

Ensure you have the Rust toolchain installed. You can get it from [rustup.rs](https://rustup.rs/).

Clone the repository and build the project:
```sh
git clone https://github.com/socketz/hashicorp-downloader.git
cd hashicorp-downloader
cargo build --release
```
The executable will be available at `./target/release/hcd`.

## Usage

### Examples

**1. Download the latest stable version of Terraform:**
```sh
hcd --product terraform
```

**2. Download a specific version of Vault:**
```sh
hcd --product vault --product-version 1.15.2
```

**3. Download all available OSS tools to a specific directory:**
```sh
hcd --product all --filepath ./my-tools
```

**4. Clean the default downloads folder:**
```sh
hcd clean
```

**5. Clean a specific folder:**
```sh
hcd clean --filepath ./my-tools
```

**6. See all available options:**
```sh
hcd --help
```

### Commands and Arguments

The tool now uses subcommands. The default action is to download.

**Download Arguments:**
| Flag (Long)         | Flag (Short) | Description                                                              | Default      |
| ------------------- | ------------ | ------------------------------------------------------------------------ | ------------ |
| `--product`         | `-p`         | Name of the product to download, or "all".                               | (Required)   |
| `--product-version` | `-v`         | Product version to download (e.g., "1.9.3").                             | `latest`     |
| `--license-class`   | `-l`         | License class of the product (`oss`, `enterprise`, `hcp`).               | `oss`        |
| `--prerelease`      |              | Allow downloading pre-release versions.                                  | `false`      |
| `--arch`            | `-a`         | Target architecture (e.g., `amd64`, `arm64`).                            | `auto`       |
| `--os`              | `-o`         | Target operating system (e.g., `linux`, `windows`).                      | `auto`       |
| `--filepath`        | `-f`         | Path to save the downloaded file(s).                                     | `./downloads`|

**`clean` Command:**
Cleans (removes) the specified directory.
```sh
hcd clean [OPTIONS]
```
| Flag (Long)  | Flag (Short) | Description                      | Default       |
| ------------ | ------------ | -------------------------------- | ------------- |
| `--filepath` | `-f`         | Path of the directory to clean.  | `./downloads` |


## License

This project is licensed under the [MIT License](LICENSE).