#!/bin/tcsh

set MEDIAINFO_BIN = "/usr/bin/mediainfo"

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

# Create a temporary file for storing results
set tmpfile = `mktemp`

# Print header
echo "Filename | Size | Duration | FPS | Bitrate | Resolution | Format | Profile | Depth | Audio" > $tmpfile

# Process each file
while ($#argv > 0)
    set mediafile = "$1:q"
    
    if (! -f "$mediafile") then
        echo "Error: File not found - $mediafile"
        shift
        continue
    endif
    
    # Run mediainfo with the template and append to temp file
    "$MEDIAINFO_BIN" "--Inform=file://$SCRIPT_DIR/mediainfo.tmpl" "$mediafile" >> $tmpfile
    shift
end

# Format the output as a table
column -t -s '|' $tmpfile

# Clean up
rm -f $tmpfile
