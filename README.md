# mediainfo

A fast and efficient command-line tool to analyze media files and display their properties in a clean, tabulated format.

## Features

- **Fast Analysis**: Uses `ffprobe` to quickly extract media information
- **Smart Caching**: Caches results in `~/.mediainfo/cache/` for instant repeat lookups
- **Recursive Scanning**: Automatically scans directories and subdirectories for media files
- **Sortable Output**: Sort results by any column with ascending/descending support
- **Flexible Filtering**: Filter files by size, duration, bitrate, or fps
- **Beautiful Display**: Clean, formatted table output with proper alignment
- **Wide Format Support**: Handles various media formats:
  - Video: mp4, mkv, avi, mov, webm, flv, m4v, mpg, mpeg, ts, vob, etc.
  - Audio: mp3, wav, flac, m4a, aac, ogg, wma, opus, etc.
- **Smart Formatting**:
  - Human-readable file sizes (B, KB, MB, GB)
  - Formatted durations (HH:MM:SS)
  - Long filenames handled with ellipsis in the middle
  - Right-aligned numeric columns for better readability

## Installation

Requirements:

- Rust and Cargo
- ffmpeg/ffprobe (for media analysis)

Install ffmpeg:

```bash
# Ubuntu/Debian
sudo apt install ffmpeg

# macOS
brew install ffmpeg

# Arch Linux
sudo pacman -S ffmpeg
```

Install mediainfo:

```bash
# Clone and install
git clone https://github.com/yourusername/mediainfo.git
cd mediainfo
cargo install --path .
```

## Usage

Basic usage:

```bash
# Analyze a single file
mediainfo video.mp4

# Analyze all media files in a directory (recursive)
mediainfo Videos/

# Analyze multiple files and directories
mediainfo video.mp4 Movies/ Shows/
```

Sorting (default: bitrate descending):

```bash
# Default sort (bitrate, highest first)
mediainfo Videos/

# Sort by size (largest first)
mediainfo --sort size Videos/

# Sort by duration (shortest first)
mediainfo --sort duration --direction asc Movies/

# Available sort columns:
#   filename, size, duration, fps, bitrate, resolution,
#   format, profile, depth, audio

# Sort directions:
#   desc (default, highest first)
#   asc (lowest first)
```

Filtering:

```bash
# High quality videos (>5 Mbps)
mediainfo --filter "bitrate:>:5" Videos/

# Short clips (<5 minutes)
mediainfo --filter "duration:<:300" Movies/

# Large files (>1GB)
mediainfo --filter "size:>:1073741824" Videos/

# High framerate videos (>30 fps)
mediainfo --filter "fps:>:30" Videos/

# Filterable columns:
#   size (bytes)
#   duration (seconds)
#   fps (frames per second)
#   bitrate (Mbps)

# Filter operators:
#   > (greater than)
#   < (less than)
```

## Output Columns

- **Filename**: Name of the media file (truncated with ... if too long)
- **Size**: File size in human-readable format (GB, MB, KB)
- **Duration**: Length in HH:MM:SS or MM:SS format
- **FPS**: Frames per second for video files
- **Bitrate**: Video bitrate in Mbps
- **Resolution**: Video dimensions (width x height)
- **Format**: Video codec (h264, hevc, etc.)
- **Profile**: Codec profile (high, main, etc.)
- **Depth**: Color depth (8bit, 10bit, 12bit)
- **Audio**: Audio channels and bitrate (e.g., "2CH 192k")

## Cache

The tool maintains a cache at `~/.mediainfo/cache/cache.json` to speed up repeated analyses:

- Uses file size + modification time as signature
- Automatically invalidates when files change
- Stores results in a single JSON file
- Instant results for previously analyzed files

## Examples

Typical workflows:

```bash
# Find all high bitrate videos
mediainfo --filter "bitrate:>:8" Videos/

# List longest videos first
mediainfo --sort duration Movies/

# Find large files that need optimization
mediainfo --filter "size:>:5368709120" --sort bitrate Videos/

# Find high quality, long videos
mediainfo --filter "bitrate:>:5" --filter "duration:>:3600" Movies/
```
