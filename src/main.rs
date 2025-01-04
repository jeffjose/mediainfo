use anyhow::{anyhow, Result};
use clap::Parser;
use colored::Colorize;
use prettytable::{format, Cell, Row, Table};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Media files to analyze
    #[arg(required = true)]
    files: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct FFProbeOutput {
    streams: Vec<Stream>,
    format: Format,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
struct Format {
    filename: String,
    size: String,
    duration: String,
    bit_rate: Option<String>,
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

        // Get bitrate
        let bitrate = video
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
        fields.push(format!("{}CH", audio.channels.unwrap_or(0)));
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
        Cell::new("Filename"),
        Cell::new("Size"),
        Cell::new("Duration"),
        Cell::new("FPS"),
        Cell::new("Bitrate"),
        Cell::new("Resolution"),
        Cell::new("Format"),
        Cell::new("Profile"),
        Cell::new("Depth"),
        Cell::new("Audio"),
    ]));

    // Process each file
    for file in args.files {
        match process_file(&file) {
            Ok(fields) => {
                let mut row_cells: Vec<Cell> = Vec::new();

                // Add each field to the row
                for (i, field) in fields.iter().enumerate() {
                    if i == 4 {
                        // Bitrate field
                        if let Some(bitrate) = parse_bitrate(field) {
                            if bitrate > 5000 {
                                row_cells.push(Cell::new(&field.green().to_string()));
                                continue;
                            }
                        }
                    }
                    row_cells.push(Cell::new(field));
                }

                // Pad with empty cells if needed
                while row_cells.len() < 10 {
                    row_cells.push(Cell::new(""));
                }

                table.add_row(Row::new(row_cells));
            }
            Err(e) => eprintln!("Error processing {}: {}", file.display(), e),
        }
    }

    // Print the table
    table.printstd();

    Ok(())
}
