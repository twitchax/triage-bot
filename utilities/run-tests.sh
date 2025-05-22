#!/bin/bash
set -e
set -a

# Include env file.
source ./.hidden/.testenv

# Run the tests in the current directory.
cargo nextest run