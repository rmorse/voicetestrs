#!/bin/bash

# Whisper Binary Downloader for Linux/Mac
# Downloads and extracts whisper.cpp binaries

# Function to print colored text
print_color() {
    local color=$1
    shift
    case $color in
        red)    echo -e "\033[0;31m$@\033[0m" ;;
        green)  echo -e "\033[0;32m$@\033[0m" ;;
        yellow) echo -e "\033[0;33m$@\033[0m" ;;
        cyan)   echo -e "\033[0;36m$@\033[0m" ;;
        *)      echo "$@" ;;
    esac
}

echo "============================================"
echo "Whisper Binary Downloader for Linux/Mac"
echo "============================================"
echo

# Default to checking OS if no argument provided
VERSION="${1:-auto}"
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
WHISPER_DIR="$SCRIPT_DIR"  # We're already in the whisper directory
RELEASE_DIR="$WHISPER_DIR/Release"

# Detect OS and architecture
OS="unknown"
ARCH="unknown"

if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    OS="linux"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    OS="macos"
fi

# Get architecture
MACHINE_TYPE=$(uname -m)
if [[ "$MACHINE_TYPE" == "x86_64" ]]; then
    ARCH="x64"
elif [[ "$MACHINE_TYPE" == "aarch64" ]] || [[ "$MACHINE_TYPE" == "arm64" ]]; then
    ARCH="arm64"
fi

# Auto-detect version based on OS
if [ "$VERSION" == "auto" ]; then
    if [ "$OS" == "macos" ]; then
        if [ "$ARCH" == "arm64" ]; then
            VERSION="macos-arm64"
        else
            VERSION="macos-x64"
        fi
    elif [ "$OS" == "linux" ]; then
        VERSION="linux"
    fi
fi

# URLs for different versions (check https://github.com/ggerganov/whisper.cpp/releases for latest)
# Note: These are example URLs - actual URLs may vary by release
declare -A DOWNLOAD_URLS=(
    ["linux"]="https://github.com/ggerganov/whisper.cpp/releases/download/v1.5.4/whisper-bin-linux-x64.tar.gz"
    ["macos-x64"]="https://github.com/ggerganov/whisper.cpp/releases/download/v1.5.4/whisper-bin-macos-x64.tar.gz"
    ["macos-arm64"]="https://github.com/ggerganov/whisper.cpp/releases/download/v1.5.4/whisper-bin-macos-arm64.tar.gz"
)

# Select URL based on version
if [ -n "${DOWNLOAD_URLS[$VERSION]}" ]; then
    DOWNLOAD_URL="${DOWNLOAD_URLS[$VERSION]}"
    VERSION_NAME="$VERSION"
else
    print_color red "Error: Unknown version '$VERSION'"
    echo
    echo "Usage: ./download.sh [version]"
    echo
    echo "Available versions:"
    echo "  auto        - Auto-detect based on your OS (default)"
    echo "  linux       - Linux x64 version"
    echo "  macos-x64   - macOS Intel version"
    echo "  macos-arm64 - macOS Apple Silicon version"
    echo
    echo "Example: ./download.sh macos-arm64"
    echo
    echo "Note: For latest URLs, check:"
    echo "https://github.com/ggerganov/whisper.cpp/releases"
    echo
    echo "For source compilation instructions, visit:"
    echo "https://github.com/ggerganov/whisper.cpp#quick-start"
    exit 1
fi

print_color cyan "Detected OS: $OS ($ARCH)"
print_color cyan "Selected version: $VERSION_NAME"
echo

# Check if whisper binaries already exist
if [ -f "$RELEASE_DIR/whisper" ] || [ -f "$RELEASE_DIR/main" ]; then
    print_color yellow "Whisper binaries already exist in: $RELEASE_DIR"
    read -p "Do you want to re-download and overwrite? (y/n): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Installation cancelled."
        exit 0
    fi
fi

# Create directories if they don't exist
mkdir -p "$RELEASE_DIR"

# Download the file
TEMP_FILE="/tmp/whisper-download.tar.gz"
print_color yellow "Downloading Whisper binaries..."
echo "URL: $DOWNLOAD_URL"
echo

# Download using wget or curl
if command -v wget &> /dev/null; then
    wget -O "$TEMP_FILE" "$DOWNLOAD_URL"
    DOWNLOAD_STATUS=$?
elif command -v curl &> /dev/null; then
    curl -L -o "$TEMP_FILE" --progress-bar "$DOWNLOAD_URL"
    DOWNLOAD_STATUS=$?
else
    print_color red "Error: Neither wget nor curl is installed."
    exit 1
fi

if [ $DOWNLOAD_STATUS -ne 0 ]; then
    print_color red "Error: Failed to download Whisper binaries."
    echo "Please check the URL or download manually from:"
    echo "$DOWNLOAD_URL"
    rm -f "$TEMP_FILE"
    exit 1
fi

print_color green "Download complete. Extracting..."
echo

# Extract based on file type
if [[ "$TEMP_FILE" == *.tar.gz ]]; then
    tar -xzf "$TEMP_FILE" -C "$RELEASE_DIR" 2>/dev/null || {
        print_color red "Error: Failed to extract the tar.gz file."
        rm -f "$TEMP_FILE"
        exit 1
    }
elif [[ "$TEMP_FILE" == *.zip ]]; then
    unzip -q -o "$TEMP_FILE" -d "$RELEASE_DIR" 2>/dev/null || {
        print_color red "Error: Failed to extract the zip file."
        rm -f "$TEMP_FILE"
        exit 1
    }
fi

# Clean up temporary file
rm -f "$TEMP_FILE"

# Make binaries executable
chmod +x "$RELEASE_DIR"/* 2>/dev/null

# Verify installation
if [ -f "$RELEASE_DIR/whisper" ] || [ -f "$RELEASE_DIR/main" ]; then
    echo
    print_color green "============================================"
    print_color green "Installation successful!"
    print_color green "============================================"
    echo
    echo "Whisper binaries installed to: $RELEASE_DIR"
    echo
    echo "Next steps:"
    echo "1. Download a model: ./download-model.sh base.en"
    echo "2. Test transcription: $RELEASE_DIR/main -m models/ggml-base.en.bin audio.wav"
    echo
else
    echo
    print_color yellow "Warning: Installation may not be complete."
    echo "Could not find whisper or main binary in $RELEASE_DIR"
    echo "You may need to compile from source:"
    echo
    echo "  git clone https://github.com/ggerganov/whisper.cpp"
    echo "  cd whisper.cpp"
    echo "  make"
    echo
    echo "Then copy the 'main' binary to: $RELEASE_DIR"
fi

# Make script executable if it isn't already
if [ ! -x "$0" ]; then
    chmod +x "$0"
fi