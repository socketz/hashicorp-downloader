use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use lazy_static::lazy_static;
use thiserror::Error;

const RELEASES_URL: &str = "https://api.releases.hashicorp.com/v1/";

// --- Modelos de Datos (Structs) ---

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Status {
    state: String,
    #[serde(default)]
    timestamp_updated: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Build {
    arch: String,
    os: String,
    url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Release {
    version: String,
    status: Status,
    builds: Vec<Build>,
    is_prerelease: bool,
}

// --- Mapeos de Plataforma ---

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

// --- Manejo de Errores Personalizado ---

#[derive(Error, Debug)]
pub enum MyError {
    #[error("Error en la petición de red")]
    Request(#[from] reqwest::Error),
    #[error("Error al procesar JSON")]
    Json(#[from] serde_json::Error),
    #[error("Error de lógica: {0}")]
    LogicError(String),
}

// --- Argumentos de Línea de Comandos ---

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Nombre del producto a descargar (ej: terraform, vault).
    #[arg(short, long)]
    product: String,

    /// Versión del producto (ej: "1.9.3", por defecto "latest").
    #[arg(short = 'v', long, default_value_t = String::from("latest"))]
    product_version: String,

    /// Permitir la descarga de versiones preliminares (prerelease).
    #[arg(long)]
    prerelease: bool,

    /// Arquitectura destino (ej: amd64, arm64). Por defecto, auto-detecta.
    #[arg(short, long, default_value_t = String::from("auto"))]
    arch: String,

    /// Sistema operativo destino (ej: linux, windows). Por defecto, auto-detecta.
    #[arg(short, long, default_value_t = String::from("auto"))]
    os: String,

    /// Ruta para guardar el archivo descargado.
    #[arg(short = 'f', long, default_value_t = String::from("./downloads"))]
    filepath: String,
}

// --- Lógica Principal ---

async fn get_download_url(
    product: &str,
    version_req: &str,
    allow_prerelease: bool,
    target_arch: &str,
    target_os: &str,
) -> Result<String, MyError> {
    // 1. Construir URL y obtener todos los releases para el producto
    let url = format!("{}releases/{}", RELEASES_URL, product);
    println!("Obteniendo releases desde: {}", url);

    let all_releases: Vec<Release> = reqwest::get(&url).await?.json().await?;

    if all_releases.is_empty() {
        return Err(MyError::LogicError(format!("El producto '{}' no fue encontrado o no tiene releases.", product)));
    }

    // 2. Filtrar releases para encontrar el que queremos descargar
    let target_release: Release = {
        // Primero, filtramos solo los que tienen soporte
        let supported_releases: Vec<Release> = all_releases
            .into_iter()
            .filter(|r| r.status.state == "supported")
            .collect();

        if supported_releases.is_empty() {
            return Err(MyError::LogicError(format!("No se encontraron versiones con soporte para '{}'.", product)));
        }

        if version_req != "latest" {
            // Si se pide una versión específica
            supported_releases
                .into_iter()
                .find(|r| r.version == version_req)
                .ok_or_else(|| MyError::LogicError(format!("La versión '{}' no se encontró o no tiene soporte.", version_req)))?
        } else {
            // Si se pide la última versión ("latest")
            let mut release_iterator = supported_releases.into_iter();
            
            if allow_prerelease {
                // La primera de la lista (la más reciente, con o sin prerelease)
                release_iterator.next()
            } else {
                // La primera que no sea prerelease
                release_iterator.find(|r| !r.is_prerelease)
            }
            .ok_or_else(|| MyError::LogicError("No se encontró una versión adecuada. Pruebe con --prerelease si busca versiones preliminares.".to_string()))?
        }
    };

    println!("Versión seleccionada: {} (Prerelease: {})", target_release.version, target_release.is_prerelease);

    // 3. Encontrar el build para la arquitectura y SO correctos
    let build = target_release.builds.iter()
        .find(|b| b.os == target_os && b.arch == target_arch)
        .ok_or_else(|| {
            let available_platforms = target_release.builds.iter()
                .map(|b| format!("{}/{}", b.os, b.arch))
                .collect::<Vec<_>>()
                .join(", ");
            MyError::LogicError(format!(
                "No se encontró un build para la plataforma '{}/{}'.\nPlataformas disponibles para la v{}: {}",
                target_os, target_arch, target_release.version, available_platforms
            ))
        })?;

    Ok(build.url.clone())
}

#[tokio::main]
async fn main() -> Result<(), MyError> {
    let args = Args::parse();

    // Resolver OS y Arch si están en "auto"
    let os = if args.os == "auto" {
        OS_MAPPING.get(std::env::consts::OS).map(|s| s.to_string())
            .ok_or_else(|| MyError::LogicError(format!("Sistema operativo no soportado: {}", std::env::consts::OS)))?
    } else {
        args.os
    };

    let arch = if args.arch == "auto" {
        ARCH_MAPPING.get(std::env::consts::ARCH).map(|s| s.to_string())
            .ok_or_else(|| MyError::LogicError(format!("Arquitectura no soportada: {}", std::env::consts::ARCH)))?
    } else {
        args.arch
    };

    println!("Producto: {}", args.product);
    println!("Versión solicitada: {}", args.product_version);
    println!("Plataforma destino: {}/{}", os, arch);
    println!("Permitir prerelease: {}", args.prerelease);

    // Obtener la URL de descarga
    match get_download_url(&args.product, &args.product_version, args.prerelease, &arch, &os).await {
        Ok(download_url) => {
            println!("\nURL de descarga encontrada:\n{}", download_url);
            // Aquí puedes agregar la lógica para descargar el archivo
            // Ejemplo: descargar_y_guardar(&download_url, &args.filepath).await?;
        },
        Err(e) => {
            eprintln!("\nError: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}