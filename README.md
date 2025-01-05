# mediainfo

A fast and powerful media file analyzer with filtering, sorting, and caching capabilities.

## Features

- Analyze video and audio files (mp4, mkv, avi, etc.)
- Show detailed information (bitrate, resolution, duration, etc.)
- Filter files based on various criteria
- Sort results by any column
- Cache results for faster subsequent runs
- Define aliases for commonly used commands

## Installation

```bash
cargo build --release
```

## Usage

Basic usage:

```bash
mediainfo [PATH]...                  # Analyze files in given paths
mediainfo --cached                   # Show cached entries
mediainfo -a ALIAS                   # Use a predefined alias
```

### Filtering

You can filter files using these formats:

```bash
# Simple equals format
mediainfo . --filter 'filename=mp4'          # Files containing 'mp4' in name
mediainfo . --filter 'resolution=3840x2160'  # 4K files

# Less than format
mediainfo . --filter 'bitrate<3'             # Files with bitrate < 3 Mbps
mediainfo . --filter 'duration<30min'        # Files shorter than 30 minutes

# Greater than format
mediainfo . --filter 'duration>1h'           # Files longer than 1 hour
mediainfo . --filter 'fps>60'                # Files with FPS > 60
```

Multiple filters are combined with AND logic:

```bash
# Find 4K MP4 files
mediainfo . --filter 'filename=mp4' --filter 'resolution=3840x2160'

# Find long, low-bitrate videos
mediainfo . --filter 'duration>30min' --filter 'bitrate<3'
```

### Sorting

Sort results by any column:

```bash
mediainfo . --sort bitrate                   # Sort by bitrate (default)
mediainfo . --sort duration --direction asc  # Sort by duration, ascending
```

Available sort columns:

- filename
- size
- duration
- fps
- bitrate
- resolution
- format
- profile
- depth
- audio

### Config File

Create aliases in `~/.mediainfo/config.toml`:

```toml
[aliases]
# Find long, low-bitrate videos
lowquality = '--filter "duration=30min" --filter "bitrate<3" --sort duration --direction desc'

# Find 4K videos
uhd = '--filter "filename=mp4" --filter "resolution=3840x2160" --sort bitrate'

# Find short clips
clips = '--filter "duration=3min" --sort duration --direction asc'

# Find 2024 low quality videos
2024lq = '--filter "filename=2024" --filter "bitrate<3"'
```

Use aliases:

```bash
mediainfo . --alias lowquality
mediainfo . --alias uhd
mediainfo . --alias clips
```

### Caching

Results are cached in `~/.mediainfo/cache/` for faster subsequent runs. Use `--cached` to view cached entries:

```bash
mediainfo --cached
```

## Options

```
-s, --sort <COLUMN>          Sort by column [default: bitrate]
-d, --direction <DIRECTION>  Sort direction (asc, desc) [default: desc]
-f, --filter <FILTER>        Filter results (can be used multiple times)
-l, --length <LENGTH>        Maximum filename length [default: 65]
-a, --alias <ALIAS>         Use a predefined alias from config file
    --cached                Show only cached entries
    --no-cache             Skip cache and force ffprobe (but update cache with results)
```

## Output Columns

- **Filename**: Name of the media file (truncated with ... if too long)
- **Duration**: Length in HH:MM:SS or MM:SS format
- **FPS**: Frames per second for video files
- **Size**: File size in human-readable format (GB, MB, KB)
- **Bitrate**: Video bitrate in Mbps
- **Resolution**: Video dimensions with aspect ratio (e.g., "3840x2160 (16:9)")
- **Format**: Video codec (h264, hevc, etc.)
- **Profile**: Codec profile (high, main, etc.)
- **Depth**: Color depth (8bit, 10bit, 12bit)
- **Color**: Color space and range (e.g., "bt709 limited")
- **Audio**: Audio channels and bitrate (e.g., "2CH 192k")
