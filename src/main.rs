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
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Instant, SystemTime};
use twox_hash::XxHash64;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Media files or directories to analyze
    #[arg(required_unless_present = "cached")]
    paths: Vec<PathBuf>,

    /// Sort by column (filename, size, duration, fps, bitrate, resolution, format, profile, depth, audio)
    #[arg(short, long, default_value = "bitrate", value_parser = ["filename", "size", "duration", "fps", "bitrate", "resolution", "format", "profile", "depth", "audio"])]
    sort: String,

    /// Sort direction (asc, desc)
    #[arg(short = 'd', long, default_value = "desc", value_parser = ["asc", "desc"])]
    direction: String,

    /// Filter results (format: column:operator:value, e.g., 'bitrate:>:5' for bitrate > 5 Mbps)
    #[arg(short, long)]
    filter: Vec<String>,

    /// Maximum length for filenames (default: 65)
    #[arg(short = 'l', long, default_value = "65")]
    filename_length: usize,

    /// Show only cached entries
    #[arg(long)]
    cached: bool,
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
    eprintln!("Reading cache from: {}", cache_path.display());
    if cache_path.exists() {
        let content = fs::read_to_string(&cache_path)?;
        eprintln!("Cache file size: {} bytes", content.len());
        Ok(serde_json::from_str(&content).unwrap_or_else(|e| {
            eprintln!("Error parsing cache: {}", e);
            Cache {
                entries: HashMap::new(),
            }
        }))
    } else {
        eprintln!("Cache file does not exist");
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

    // Only load cache if it hasn't been loaded yet
    let mut cache_guard = CACHE.lock().unwrap();
    if cache_guard.is_none() {
        // Load cache silently without progress indicators
        let cache_path = get_cache_file()?;
        if cache_path.exists() {
            let content = fs::read_to_string(&cache_path)?;
            *cache_guard = Some(serde_json::from_str(&content).unwrap_or_else(|_| Cache {
                entries: HashMap::new(),
            }));
        } else {
            *cache_guard = Some(Cache {
                entries: HashMap::new(),
            });
        }
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

fn format_elapsed(secs: f64) -> String {
    if secs >= 60.0 {
        let minutes = (secs / 60.0).floor();
        let seconds = secs % 60.0;
        format!("{}:{:02}", minutes as u32, seconds as u32)
    } else {
        format!("{}s", secs.round() as u32)
    }
}

fn collect_media_files(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let start_time = Instant::now();
    let mut media_files = Vec::new();
    let mut scanned = 0;
    let mut found = 0;

    // Clear line and show initial status
    eprint!(
        "\x1B[2K\rScanning: {} scanned, {} media files found ({})",
        scanned,
        found,
        format_elapsed(start_time.elapsed().as_secs_f64())
    );

    for path in paths {
        if path.is_file() {
            scanned += 1;
            if is_media_file(&path) {
                found += 1;
                eprint!(
                    "\x1B[2K\rScanning: {} scanned, {} media files found ({})",
                    scanned,
                    found,
                    format_elapsed(start_time.elapsed().as_secs_f64())
                );
                media_files.push(path);
            }
        } else if path.is_dir() {
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path().to_path_buf();
                if path.is_file() {
                    scanned += 1;
                    if is_media_file(&path) {
                        found += 1;
                        eprint!(
                            "\x1B[2K\rScanning: {} scanned, {} media files found ({})",
                            scanned,
                            found,
                            format_elapsed(start_time.elapsed().as_secs_f64())
                        );
                        media_files.push(path);
                    }
                }
            }
        }
    }
    let elapsed = start_time.elapsed().as_secs_f64();
    eprintln!("\nScanning completed in {}", format_elapsed(elapsed));
    media_files
}

fn parse_duration_to_secs(duration: &str) -> f64 {
    let parts: Vec<&str> = duration.split(':').collect();
    match parts.len() {
        2 => {
            let mins: f64 = parts[0].parse().unwrap_or(0.0);
            let secs: f64 = parts[1].parse().unwrap_or(0.0);
            mins * 60.0 + secs
        }
        3 => {
            let hours: f64 = parts[0].parse().unwrap_or(0.0);
            let mins: f64 = parts[1].parse().unwrap_or(0.0);
            let secs: f64 = parts[2].parse().unwrap_or(0.0);
            hours * 3600.0 + mins * 60.0 + secs
        }
        _ => 0.0,
    }
}

fn parse_human_duration(duration_str: &str) -> Option<f64> {
    let mut total_seconds = 0.0;
    let mut current_number = String::new();
    let mut chars = duration_str.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_digit(10) {
            current_number.push(c);
        } else {
            let number = current_number.parse::<f64>().ok()?;
            current_number.clear();

            match c {
                'h' => total_seconds += number * 3600.0,
                'm' => {
                    if chars.peek() == Some(&'i') {
                        chars.next(); // consume 'i'
                        if chars.peek() == Some(&'n') {
                            chars.next(); // consume 'n'
                            total_seconds += number * 60.0;
                        }
                    } else {
                        total_seconds += number * 60.0;
                    }
                }
                's' => total_seconds += number,
                _ => return None,
            }
        }
    }

    // Handle case where there might be a trailing number without unit (assume seconds)
    if !current_number.is_empty() {
        if let Ok(number) = current_number.parse::<f64>() {
            total_seconds += number;
        }
    }

    Some(total_seconds)
}

fn should_include_row(fields: &[String], filters: &[String]) -> bool {
    // If no filters, include all rows
    if filters.is_empty() {
        return true;
    }

    // Row must match all filters (AND logic)
    filters.iter().all(|filter| {
        let parts: Vec<&str> = filter.split(':').collect();
        if parts.len() != 2 {
            return true;
        }

        let (column, value) = (parts[0], parts[1]);

        // Special handling for filename matching with simplified syntax
        if column == "filename" {
            let filename = &fields[0].to_lowercase();
            let pattern = value.to_lowercase();
            return filename.contains(&pattern);
        }

        // For other columns, keep the existing operator-based syntax
        if parts.len() != 3 {
            return true;
        }

        let (column, op, value) = (parts[0], parts[1], parts[2]);
        let idx = match column {
            "size" => Some(1),
            "duration" => Some(2),
            "fps" => Some(3),
            "bitrate" => Some(4),
            _ => None,
        };

        if let Some(idx) = idx {
            let field_value = match column {
                "size" => parse_size(&fields[idx]) as f64,
                "duration" => parse_duration_to_secs(&fields[idx]),
                "fps" => fields[idx].parse::<f64>().unwrap_or(0.0),
                "bitrate" => parse_bitrate(&fields[idx]).unwrap_or(0.0),
                _ => return true,
            };

            let threshold = if column == "duration" {
                parse_human_duration(value).unwrap_or_else(|| value.parse::<f64>().unwrap_or(0.0))
            } else {
                value.parse::<f64>().unwrap_or(0.0)
            };

            match op {
                ">" => field_value > threshold,
                "<" => field_value < threshold,
                _ => true,
            }
        } else {
            true
        }
    })
}

fn process_file(file: &PathBuf, filename_length: usize) -> Result<FFProbeOutput> {
    // Try to get from cache first
    if let Ok(Some(probe)) = get_cached_probe(file) {
        return Ok(probe);
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

    // Save to cache immediately
    save_to_cache(file, &probe)?;

    Ok(probe)
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

fn format_probe_output(
    file: &PathBuf,
    probe: &FFProbeOutput,
    filename_length: usize,
) -> Result<Vec<String>> {
    let mut fields = Vec::new();

    // Get filename
    fields.push(truncate_middle(
        file.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown"),
        filename_length,
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

fn parse_bitrate(bitrate_str: &str) -> Option<f64> {
    bitrate_str
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
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

fn get_cached_files() -> Result<Vec<(PathBuf, FFProbeOutput)>> {
    eprintln!("Loading cache file...");
    let mut cache_guard = CACHE.lock().unwrap();
    if cache_guard.is_none() {
        eprintln!("Cache not loaded, loading from disk...");
        *cache_guard = Some(load_cache()?);
    }

    let mut files = Vec::new();
    if let Some(cache) = &*cache_guard {
        eprintln!("Found {} entries in cache", cache.entries.len());
        for (path_str, entry) in &cache.entries {
            let path = PathBuf::from(path_str);
            files.push((path, entry.probe_data.clone()));
        }
        eprintln!("Loaded {} entries", files.len());
    } else {
        eprintln!("No cache entries found");
    }
    Ok(files)
}

fn main() -> Result<()> {
    let args = Args::parse();

    let files = if args.cached {
        // Get files from cache
        let cached_files = get_cached_files()?;
        if cached_files.is_empty() {
            eprintln!("No cached entries found!");
            return Ok(());
        }
        cached_files
    } else {
        // Collect all media files
        let media_files = collect_media_files(args.paths);
        if media_files.is_empty() {
            eprintln!("No media files found!");
            return Ok(());
        }

        let process_start = Instant::now();
        let total_files = media_files.len();
        let mut processed = 0;
        let mut cached = 0;
        let mut processed_files = Vec::new();

        // Process each file
        for file in media_files {
            let is_cached = get_cached_probe(&file).ok().flatten().is_some();
            if is_cached {
                cached += 1;
            }
            match process_file(&file, args.filename_length) {
                Ok(probe) => {
                    processed += 1;
                    eprint!(
                        "\x1B[2K\rProcessing: {}/{} files ({} from cache) ({})",
                        processed,
                        total_files,
                        cached,
                        format_elapsed(process_start.elapsed().as_secs_f64())
                    );
                    processed_files.push((file, probe));
                }
                Err(e) => {
                    processed += 1;
                    eprint!(
                        "\x1B[2K\rProcessing: {}/{} files ({} from cache) ({})",
                        processed,
                        total_files,
                        cached,
                        format_elapsed(process_start.elapsed().as_secs_f64())
                    );
                    eprintln!("\nError processing {}: {}", file.display(), e);
                }
            }
        }
        eprintln!();
        processed_files
    };

    // Create rows for table
    let mut rows: Vec<(Vec<String>, Row)> = Vec::new();
    for (file, probe) in files {
        let fields = format_probe_output(&file, &probe, args.filename_length)?;

        // Apply filters if specified
        if !args.filter.is_empty() {
            if !should_include_row(&fields, &args.filter) {
                continue;
            }
        }

        let mut row_cells: Vec<Cell> = Vec::new();
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
                let a_bitrate = parse_bitrate(&a.0[sort_index]).unwrap_or(0.0);
                let b_bitrate = parse_bitrate(&b.0[sort_index]).unwrap_or(0.0);
                a_bitrate
                    .partial_cmp(&b_bitrate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
            _ => a.0[sort_index].cmp(&b.0[sort_index]),
        };
        if ascending {
            cmp
        } else {
            cmp.reverse()
        }
    });

    // Create and print table
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

    // Add sorted rows to table
    for (_, row) in rows {
        table.add_row(row);
    }

    // Print the table
    table.printstd();

    Ok(())
}
