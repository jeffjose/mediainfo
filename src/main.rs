use anyhow::{anyhow, Result};
use base64;
use clap::Parser;
use colored::Colorize;
use dirs;
use prettytable::{format, Attr, Cell, Row, Table};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use twox_hash::XxHash64;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Media files to analyze
    #[arg(required = true)]
    files: Vec<PathBuf>,

    /// Sort by column (filename, size, duration, fps, bitrate, resolution, format, profile, depth, audio)
    #[arg(short, long, value_parser = ["filename", "size", "duration", "fps", "bitrate", "resolution", "format", "profile", "depth", "audio"])]
    sort: Option<String>,
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

fn get_cached_probe(file: &PathBuf) -> Result<Option<FFProbeOutput>> {
    let cache_dir = get_cache_dir()?;
    let file_path = file.canonicalize()?;
    let path_str = file_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid file path"))?;
    let mut hasher = DefaultHasher::new();
    path_str.hash(&mut hasher);
    let hash = hasher.finish();
    let cache_file = cache_dir.join(format!("{:x}.json", hash));

    if !cache_file.exists() {
        return Ok(None);
    }

    let cache_content = fs::read_to_string(&cache_file)?;
    let cache_entry: CacheEntry = serde_json::from_str(&cache_content)?;

    // Check if the file has changed
    let current_signature = get_file_signature(file)?;
    if current_signature != cache_entry.signature {
        return Ok(None);
    }

    Ok(Some(cache_entry.probe_data))
}

fn save_to_cache(file: &PathBuf, probe_data: &FFProbeOutput) -> Result<()> {
    let cache_dir = get_cache_dir()?;
    let file_path = file.canonicalize()?;
    let path_str = file_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid file path"))?;
    let mut hasher = DefaultHasher::new();
    path_str.hash(&mut hasher);
    let hash = hasher.finish();
    let cache_file = cache_dir.join(format!("{:x}.json", hash));

    let cache_entry = CacheEntry {
        signature: get_file_signature(file)?,
        probe_data: probe_data.clone(),
    };

    let cache_content = serde_json::to_string(&cache_entry)?;
    fs::write(cache_file, cache_content)?;
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

fn format_probe_output(file: &PathBuf, probe: &FFProbeOutput) -> Result<Vec<String>> {
    let mut fields = Vec::new();

    // Get filename
    fields.push(
        file.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string(),
    );

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

fn main() -> Result<()> {
    let args = Args::parse();

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
    for file in args.files {
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

    // Sort rows if requested
    if let Some(sort_by) = args.sort {
        let sort_index = match sort_by.as_str() {
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
            _ => 0,
        };

        rows.sort_by(|a, b| {
            let a_val = &a.0[sort_index];
            let b_val = &b.0[sort_index];

            match sort_index {
                1 => {
                    // Size
                    let a_bytes = parse_size(a_val);
                    let b_bytes = parse_size(b_val);
                    a_bytes.cmp(&b_bytes)
                }
                2 => {
                    // Duration
                    let a_secs = a_val.parse::<f64>().unwrap_or(0.0);
                    let b_secs = b_val.parse::<f64>().unwrap_or(0.0);
                    a_secs
                        .partial_cmp(&b_secs)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
                3 => {
                    // FPS
                    let a_fps = a_val.parse::<f64>().unwrap_or(0.0);
                    let b_fps = b_val.parse::<f64>().unwrap_or(0.0);
                    a_fps
                        .partial_cmp(&b_fps)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
                4 => {
                    // Bitrate
                    let a_bitrate = parse_bitrate(a_val);
                    let b_bitrate = parse_bitrate(b_val);
                    a_bitrate.cmp(&b_bitrate)
                }
                _ => a_val.cmp(b_val),
            }
        });
    }

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
