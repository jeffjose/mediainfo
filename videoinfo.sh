#!/bin/tcsh

# Check if mediainfo is installed
which mediainfo > /dev/null
if ($status != 0) then
    echo "Error: mediainfo is not installed. Please install it first."
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
    mediainfo "--Inform=file://$SCRIPT_DIR/mediainfo.tmpl" "$mediafile" >> $tmpfile
    shift
end

# Format the output as a table
column -t -s '|' $tmpfile

# Clean up
rm -f $tmpfile
