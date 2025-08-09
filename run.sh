#!/bin/bash

set -eu

COMMAND=$1

if [ "$COMMAND" = "build" ]; then
    docker build --network=host -t streaming-server .
elif [ "$COMMAND" = "test" ]; then
    cargo test
elif [ "$COMMAND" = "run" ]; then
    docker run -p 8080:8080 -v ./av:/usr/src/app/av streaming-server
elif [ "$COMMAND" = "run-bot" ]; then
    echo "Installing Python dependencies..."
    pip install -r requirements.txt
    echo "Starting viewer bot..."
    # Pass all arguments except the first one ("run-bot") to the script
    shift
    python3 viewer_bot.py "$@"
elif [ "$COMMAND" = "lint-markdown" ]; then
    npm exec -c "markdownlint-cli2 \"**/*.md\""
else
    echo "Usage: $0 {build|test|run|lint-markdown}"
    exit 1
fi
