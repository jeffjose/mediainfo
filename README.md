# mediainfo

A fast and efficient command-line tool to analyze media files and display their properties in a clean, tabulated format.

## Features

- **Fast Analysis**: Uses `ffprobe` to quickly extract media information
- **Smart Caching**: Caches results in `~/.mediainfo/cache/` for instant repeat lookups
- **Recursive Scanning**: Automatically scans directories recursively for media files
- **Sortable Output**: Sort results by any column (size, duration, bitrate, etc.)
- **Beautiful Display**: Clean, formatted table output with proper alignment
- **Wide Format Support**: Handles various media formats:
  - Video: mp4, mkv, avi, mov, webm, flv, m4v, etc.
  - Audio: mp3, wav, flac, m4a, aac, ogg, etc.

## Installation

Ensure you have `ffmpeg` installed on your system:

```bash
# Ubuntu/Debian
sudo apt install ffmpeg

# macOS
brew install ffmpeg

# Arch Linux
sudo pacman -S ffmpeg
```

Then install the tool:

```bash
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

Sorting:

```bash
# Sort by file size
mediainfo --sort size Videos/

# Sort by duration
mediainfo --sort duration Movies/

# Other sort options: filename, fps, bitrate, resolution, format, profile, depth, audio
```

## Output Columns

- **Filename**: Name of the media file
- **Size**: File size in human-readable format (GB, MB, KB)
- **Duration**: Length of media in HH:MM:SS or MM:SS format
- **FPS**: Frames per second for video files
- **Bitrate**: Video bitrate in Mbps
- **Resolution**: Video dimensions (width x height)
- **Format**: Video codec (h264, hevc, etc.)
- **Profile**: Codec profile
- **Depth**: Color depth (8bit, 10bit, 12bit)
- **Audio**: Audio channels and bitrate

## Cache

The tool maintains a cache at `~/.mediainfo/cache/cache.json` to speed up repeated analyses. The cache:

- Uses file size and modification time to detect changes
- Automatically updates when files are modified
- Stores results in a single JSON file for easy management
