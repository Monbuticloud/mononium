#!/usr/bin/env awk -f
# Convert macOS `sample` output to folded stack format for inferno-flamegraph.
#
# macOS `sample` output:
#     Call graph:
#        3052 Thread_...  (running)
#        3052 start (in mononium-cli) + 123
#        3052   main (in mononium-cli) + 45
#        3052     my_func (in mononium-cli) + 10
#
# Folded output:
#     mononium-cli;main;my_func 3052
#
# Usage: sample-to-folded.awk < profile.txt | inferno-flamegraph > profile.svg

BEGIN {
    in_callgraph = 0
}

/^Call graph:/ {
    in_callgraph = 1
    next
}

in_callgraph && /^[[:space:]]*[0-9]+/ {
    # Parse: <spaces><count> <rest>
    match($0, /^[[:space:]]*([0-9]+)[[:space:]]+(.*)$/, m)
    count = m[1]
    rest = m[2]

    # Strip " (in mononium-cli) + NNN" or " (in ...) + NNN" suffix
    gsub(/ \(in [^)]*\)(\+[0-9]+)?/, "", rest)

    # Skip thread/queue lines
    if (rest ~ /^Thread_/ || rest ~ /^start$/ || rest ~ /^main$/) {
        # Keep start and main as root
    }

    # Build stack: track depth by indentation
    # The sample format uses deeper indentation for deeper calls
    # Count leading spaces to determine depth
    match($0, /^[[:space:]]*/)
    depth = length(m[0]) / 4  # roughly 4 spaces per level

    # Store frames by depth
    frames[depth] = rest

    # When we drop back to depth 0, emit the full stack
    if (depth == 0 && count > 0) {
        stack = frames[0]
        for (i = 1; i <= 10; i++) {
            if (frames[i] == "") break
            stack = stack ";" frames[i]
        }
        print stack " " count
        delete frames
    }
}

in_callgraph && /^[[:space:]]*$/ {
    # Blank line between sections — clear frames
    delete frames
}
