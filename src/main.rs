use anyhow::{anyhow, Result};
use base64;
use clap::Parser;
use colored::Colorize;
use dirs;
use once_cell::sync::Lazy;
use prettytable::{format, Attr, Cell, Row, Table};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::SystemTime;
use twox_hash::XxHash64;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Media files or directories to analyze
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Sort by column (filename, size, duration, fps, bitrate, resolution, format, profile, depth, audio)
    #[arg(short, long, default_value = "bitrate", value_parser = ["filename", "size", "duration", "fps", "bitrate", "resolution", "format", "profile", "depth", "audio"])]
    sort: String,

    /// Sort direction (asc, desc)
    #[arg(short = 'd', long, default_value = "desc", value_parser = ["asc", "desc"])]
    direction: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct FFProbeOutput {
    streams: Vec<Stream>,
    format: Format,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Stream {
    codec_type: String,
    codec_name: Option<String>,
    profile: Option<String>,
    width: Option<i32>,
    height: Option<i32>,
    r_frame_rate: Option<String>,
    bit_rate: Option<String>,
    pix_fmt: Option<String>,
    channels: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Format {
    filename: String,
    size: String,
    duration: String,
    bit_rate: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CacheEntry {
    signature: String,
    probe_data: FFProbeOutput,
}

#[derive(Debug, Deserialize, Serialize)]
struct Cache {
    entries: HashMap<String, CacheEntry>,
}

static CACHE: Lazy<Mutex<Option<Cache>>> = Lazy::new(|| Mutex::new(None));

fn get_file_signature(path: &PathBuf) -> Result<String> {
    let metadata = fs::metadata(path)?;
    let size = metadata.len();
    let modified = metadata
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();
    Ok(format!("{}-{}", size, modified))
}

fn get_cache_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    let cache_dir = home.join(".mediainfo").join("cache");
    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir)
}

fn get_path_hash(path: &str) -> String {
    let mut hasher = XxHash64::default();
    hasher.write(path.as_bytes());
    format!("{:016x}", hasher.finish())
}

fn load_cache() -> Result<Cache> {
    let cache_path = get_cache_file()?;
    if cache_path.exists() {
        let content = fs::read_to_string(&cache_path)?;
        Ok(serde_json::from_str(&content).unwrap_or(Cache {
            entries: HashMap::new(),
        }))
    } else {
        Ok(Cache {
            entries: HashMap::new(),
        })
    }
}

fn save_cache(cache: &Cache) -> Result<()> {
    let cache_path = get_cache_file()?;
    let content = serde_json::to_string_pretty(cache)?;
    fs::write(cache_path, content)?;
    Ok(())
}

fn get_cache_file() -> Result<PathBuf> {
    let cache_dir = get_cache_dir()?;
    Ok(cache_dir.join("cache.json"))
}

fn get_cached_probe(file: &PathBuf) -> Result<Option<FFProbeOutput>> {
    let canonical_path = file.canonicalize()?;
    let path_str = canonical_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid file path"))?;

    let mut cache_guard = CACHE.lock().unwrap();
    if cache_guard.is_none() {
        *cache_guard = Some(load_cache()?);
    }

    if let Some(cache) = &*cache_guard {
        if let Some(entry) = cache.entries.get(path_str) {
            let current_signature = get_file_signature(file)?;
            if current_signature == entry.signature {
                return Ok(Some(entry.probe_data.clone()));
            }
        }
    }

    Ok(None)
}

fn save_to_cache(file: &PathBuf, probe_data: &FFProbeOutput) -> Result<()> {
    let canonical_path = file.canonicalize()?;
    let path_str = canonical_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid file path"))?;

    let mut cache_guard = CACHE.lock().unwrap();
    if cache_guard.is_none() {
        *cache_guard = Some(load_cache()?);
    }

    if let Some(cache) = &mut *cache_guard {
        cache.entries.insert(
            path_str.to_string(),
            CacheEntry {
                signature: get_file_signature(file)?,
                probe_data: probe_data.clone(),
            },
        );
        save_cache(cache)?;
    }

    Ok(())
}

fn format_duration(duration: &str) -> String {
    if let Ok(secs) = duration.parse::<f64>() {
        let hours = (secs / 3600.0).floor();
        let minutes = ((secs % 3600.0) / 60.0).floor();
        let seconds = secs % 60.0;

        if hours > 0.0 {
            format!(
                "{:02}:{:02}:{:02}",
                hours as u32, minutes as u32, seconds as u32
            )
        } else {
            format!("{:02}:{:02}", minutes as u32, seconds as u32)
        }
    } else {
        String::new()
    }
}

fn format_size(size: &str) -> String {
    if let Ok(bytes) = size.parse::<u64>() {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    } else {
        String::new()
    }
}

fn get_bit_depth(pix_fmt: Option<&str>) -> String {
    match pix_fmt {
        Some(fmt) if fmt.contains("p10") => "10bit",
        Some(fmt) if fmt.contains("p12") => "12bit",
        _ => "8bit",
    }
    .to_string()
}

fn process_file(file: &PathBuf) -> Result<Vec<String>> {
    // Try to get from cache first
    if let Ok(Some(probe)) = get_cached_probe(file) {
        return format_probe_output(file, &probe);
    }

    // If not in cache or cache is invalid, run ffprobe
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            file.to_str().ok_or_else(|| anyhow!("Invalid file path"))?,
        ])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let probe: FFProbeOutput = serde_json::from_slice(&output.stdout)?;

    // Save to cache
    if let Err(e) = save_to_cache(file, &probe) {
        eprintln!("Warning: Failed to save to cache: {}", e);
    }

    format_probe_output(file, &probe)
}

fn truncate_middle(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        return s.to_string();
    }

    let ellipsis = "...";
    let side_len = (max_len - ellipsis.len()) / 2;

    let left: String = s.chars().take(side_len).collect();
    let right: String = s
        .chars()
        .rev()
        .take(side_len)
        .collect::<String>()
        .chars()
        .rev()
        .collect();

    format!("{}{}{}", left, ellipsis, right)
}

fn format_probe_output(file: &PathBuf, probe: &FFProbeOutput) -> Result<Vec<String>> {
    let mut fields = Vec::new();

    // Get filename
    fields.push(truncate_middle(
        file.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown"),
        75,
    ));

    // Get file size
    fields.push(format_size(&probe.format.size));

    // Get duration
    fields.push(format_duration(&probe.format.duration));

    // Find video stream
    if let Some(video) = probe.streams.iter().find(|s| s.codec_type == "video") {
        // Get FPS
        let fps = video
            .r_frame_rate
            .as_deref()
            .and_then(|r| {
                let parts: Vec<&str> = r.split('/').collect();
                if parts.len() == 2 {
                    parts[0].parse::<f64>().ok().and_then(|num| {
                        parts[1].parse::<f64>().ok().map(|den| {
                            if den != 0.0 {
                                format!("{:.2}", num / den)
                            } else {
                                String::new()
                            }
                        })
                    })
                } else {
                    None
                }
            })
            .unwrap_or_default();
        fields.push(fps);

        // Get bitrate from format (more reliable than video stream bitrate)
        let bitrate = probe
            .format
            .bit_rate
            .as_deref()
            .and_then(|b| b.parse::<f64>().ok())
            .map(|b| format!("{:.2} Mbps", b / 1_000_000.0))
            .unwrap_or_default();
        fields.push(bitrate);

        // Get resolution
        let width = video.width.unwrap_or(0);
        let height = video.height.unwrap_or(0);
        fields.push(format!("{}x{}", width, height));

        // Get codec name
        fields.push(video.codec_name.clone().unwrap_or_default());

        // Get profile
        fields.push(video.profile.clone().unwrap_or_default());

        // Get bit depth
        fields.push(get_bit_depth(video.pix_fmt.as_deref()));
    } else {
        // No video stream found, add empty fields
        fields.extend(vec!["".to_string(); 6]);
    }

    // Find audio stream
    if let Some(audio) = probe.streams.iter().find(|s| s.codec_type == "audio") {
        let channels = format!("{}CH", audio.channels.unwrap_or(0));
        let bitrate = audio
            .bit_rate
            .as_deref()
            .and_then(|b| b.parse::<f64>().ok())
            .map(|b| format!(" {:.0}k", b / 1000.0))
            .unwrap_or_default();
        fields.push(format!("{}{}", channels, bitrate));
    } else {
        fields.push("".to_string());
    }

    Ok(fields)
}

fn parse_bitrate(bitrate_str: &str) -> Option<u32> {
    bitrate_str
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .parse::<f32>()
        .ok()
        .map(|b| (b * 1000.0) as u32) // Convert Mbps to Kbps
}

fn is_media_file(path: &Path) -> bool {
    let media_extensions = [
        "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "m2v", "m4v",
        "3gp", "3g2", "mxf", "ts", "mts", "m2ts", "vob", "ogv", "qt", "rm", "rmvb", "asf", "mp3",
        "wav", "flac", "m4a", "aac", "ogg", "wma", "opus",
    ];

    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| media_extensions.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn collect_media_files(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut media_files = Vec::new();

    for path in paths {
        if path.is_file() {
            if is_media_file(&path) {
                media_files.push(path);
            }
        } else if path.is_dir() {
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path().to_path_buf();
                if path.is_file() && is_media_file(&path) {
                    media_files.push(path);
                }
            }
        }
    }

    media_files
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Collect all media files
    let files = collect_media_files(args.paths);

    if files.is_empty() {
        eprintln!("No media files found!");
        return Ok(());
    }

    // Create the table
    let mut table = Table::new();
    let format = format::FormatBuilder::new()
        .column_separator('│')
        .borders('│')
        .separator(
            format::LinePosition::Top,
            format::LineSeparator::new('─', '┬', '┌', '┐'),
        )
        .separator(
            format::LinePosition::Bottom,
            format::LineSeparator::new('─', '┴', '└', '┘'),
        )
        .separator(
            format::LinePosition::Title,
            format::LineSeparator::new('─', '┼', '├', '┤'),
        )
        .padding(1, 1)
        .build();
    table.set_format(format);

    // Add header row
    table.add_row(Row::new(vec![
        Cell::new("Filename").with_style(Attr::Bold),
        Cell::new("Size").with_style(Attr::Bold).style_spec("r"),
        Cell::new("Duration").with_style(Attr::Bold).style_spec("r"),
        Cell::new("FPS").with_style(Attr::Bold).style_spec("r"),
        Cell::new("Bitrate").with_style(Attr::Bold).style_spec("r"),
        Cell::new("Resolution").with_style(Attr::Bold),
        Cell::new("Format").with_style(Attr::Bold),
        Cell::new("Profile").with_style(Attr::Bold),
        Cell::new("Depth").with_style(Attr::Bold).style_spec("c"),
        Cell::new("Audio").with_style(Attr::Bold),
    ]));

    // Process each file
    let mut rows: Vec<(Vec<String>, Row)> = Vec::new();
    for file in files {
        match process_file(&file) {
            Ok(fields) => {
                let mut row_cells: Vec<Cell> = Vec::new();

                // Add each field to the row
                for (i, field) in fields.iter().enumerate() {
                    let cell = match i {
                        1 => Cell::new(field).style_spec("r"), // Size
                        2 => Cell::new(field).style_spec("r"), // Duration
                        3 => Cell::new(field).style_spec("r"), // FPS
                        4 => Cell::new(field).style_spec("r"), // Bitrate
                        8 => Cell::new(field).style_spec("c"), // Depth
                        _ => Cell::new(field),                 // Others left-aligned
                    };
                    row_cells.push(cell);
                }

                rows.push((fields, Row::new(row_cells)));
            }
            Err(e) => eprintln!("Error processing {}: {}", file.display(), e),
        }
    }

    // Sort rows
    let sort_index = match args.sort.as_str() {
        "filename" => 0,
        "size" => 1,
        "duration" => 2,
        "fps" => 3,
        "bitrate" => 4,
        "resolution" => 5,
        "format" => 6,
        "profile" => 7,
        "depth" => 8,
        "audio" => 9,
        _ => 4, // default to bitrate
    };

    let ascending = args.direction == "asc";
    rows.sort_by(|a, b| {
        let cmp = match sort_index {
            1 => {
                // Size
                let a_bytes = parse_size(&a.0[sort_index]);
                let b_bytes = parse_size(&b.0[sort_index]);
                a_bytes.cmp(&b_bytes)
            }
            2 => {
                // Duration
                let a_secs = a.0[sort_index].parse::<f64>().unwrap_or(0.0);
                let b_secs = b.0[sort_index].parse::<f64>().unwrap_or(0.0);
                a_secs
                    .partial_cmp(&b_secs)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
            3 => {
                // FPS
                let a_fps = a.0[sort_index].parse::<f64>().unwrap_or(0.0);
                let b_fps = b.0[sort_index].parse::<f64>().unwrap_or(0.0);
                a_fps
                    .partial_cmp(&b_fps)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
            4 => {
                // Bitrate
                let a_bitrate = parse_bitrate(&a.0[sort_index]).unwrap_or(0);
                let b_bitrate = parse_bitrate(&b.0[sort_index]).unwrap_or(0);
                a_bitrate.cmp(&b_bitrate)
            }
            _ => a.0[sort_index].cmp(&b.0[sort_index]),
        };
        if ascending {
            cmp
        } else {
            cmp.reverse()
        }
    });

    // Add sorted rows to table
    for (_, row) in rows {
        table.add_row(row);
    }

    // Print the table
    table.printstd();

    Ok(())
}

fn parse_size(size_str: &str) -> u64 {
    let parts: Vec<&str> = size_str.split_whitespace().collect();
    if parts.len() != 2 {
        return 0;
    }

    let value: f64 = parts[0].parse().unwrap_or(0.0);
    match parts[1] {
        "GB" => (value * 1024.0 * 1024.0 * 1024.0) as u64,
        "MB" => (value * 1024.0 * 1024.0) as u64,
        "KB" => (value * 1024.0) as u64,
        "B" => value as u64,
        _ => 0,
    }
}
