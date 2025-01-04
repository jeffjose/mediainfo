#!/bin/tcsh

set MEDIAINFO_BIN = "/usr/bin/mediainfo"

# ANSI color codes
set GREEN = "\033[32m"
set RESET = "\033[0m"

# Check if mediainfo is installed
if (! -x "$MEDIAINFO_BIN") then
    echo "Error: mediainfo is not installed or not found at $MEDIAINFO_BIN"
    echo "Please install mediainfo using your package manager"
    echo "For example: sudo apt-get install mediainfo"
    exit 1
endif

# Check if at least one file is provided
if ($#argv < 1) then
    echo "Usage: $0 <media_file1> [media_file2 ...]"
    exit 1
endif

# Get the directory of the script
set SCRIPT_DIR = "$0:h"

# Create temporary files
set tmpfile = `mktemp`
set formatted = `mktemp`

# Print header
printf "%-30s | %-10s | %-10s | %-6s | %-15s | %-15s | %-10s | %-10s | %-6s | %-10s\n" "Filename" "Size" "Duration" "FPS" "Bitrate" "Resolution" "Format" "Profile" "Depth" "Audio" > $tmpfile

# Process each file
while ($#argv > 0)
    set mediafile = "$1:q"
    
    if (! -f "$mediafile") then
        echo "Error: File not found - $mediafile"
        shift
        continue
    endif
    
    # Get mediainfo output
    set line = `"$MEDIAINFO_BIN" "--Inform=file://$SCRIPT_DIR/mediainfo.tmpl" "$mediafile"`
    
    # Get bitrate (5th field)
    set bitrate = `echo "$line" | cut -d'|' -f5 | sed 's/[^0-9]//g'`
    
    if ("$bitrate" != "" && "$bitrate" =~ [0-9]*) then
        if ($bitrate > 5000) then
            # Add color to bitrate field
            set colored = `echo "$line" | awk -F'|' -v green="$GREEN" -v reset="$RESET" '{OFS="|"; $5=green$5reset; print}'`
            echo "$colored" >> $tmpfile
        else
            echo "$line" >> $tmpfile
        endif
    else
        echo "$line" >> $tmpfile
    endif
    
    shift
end

# Display formatted output
cat $tmpfile | column -t -s '|' -o ' | '

# Clean up
rm -f $tmpfile $formatted
