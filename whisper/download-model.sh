#!/bin/bash

# Whisper Model Downloader for Linux/Mac
# Downloads GGML format models from Hugging Face

MODEL="${1:-base.en}"
BASE_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main"
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
MODELS_DIR="$SCRIPT_DIR/models"

# Create models directory if it doesn't exist
if [ ! -d "$MODELS_DIR" ]; then
    mkdir -p "$MODELS_DIR"
    echo "Created models directory: $MODELS_DIR"
fi

# Define available models
declare -A MODEL_FILES=(
    ["tiny"]="ggml-tiny.bin"
    ["tiny.en"]="ggml-tiny.en.bin"
    ["base"]="ggml-base.bin"
    ["base.en"]="ggml-base.en.bin"
    ["small"]="ggml-small.bin"
    ["small.en"]="ggml-small.en.bin"
    ["medium"]="ggml-medium.bin"
    ["medium.en"]="ggml-medium.en.bin"
    ["large-v1"]="ggml-large-v1.bin"
    ["large-v2"]="ggml-large-v2.bin"
    ["large-v3"]="ggml-large-v3.bin"
    ["large"]="ggml-large-v3.bin"
)

# Quantized models
declare -A QUANTIZED_MODELS=(
    ["tiny-q5_0"]="ggml-tiny-q5_0.bin"
    ["base-q5_0"]="ggml-base-q5_0.bin"
    ["small-q5_0"]="ggml-small-q5_0.bin"
    ["medium-q5_0"]="ggml-medium-q5_0.bin"
    ["large-v3-q5_0"]="ggml-large-v3-q5_0.bin"
)

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

# Function to show available models
show_models() {
    print_color yellow "Available models:"
    echo
    print_color cyan "Standard models:"
    for model in "${!MODEL_FILES[@]}"; do
        echo "  $model"
    done | sort
    echo
    print_color cyan "Quantized models (smaller size, slightly lower quality):"
    for model in "${!QUANTIZED_MODELS[@]}"; do
        echo "  $model"
    done | sort
}

# Check if model exists
if [ -z "${MODEL_FILES[$MODEL]}" ] && [ -z "${QUANTIZED_MODELS[$MODEL]}" ]; then
    print_color red "Error: Unknown model '$MODEL'"
    echo
    show_models
    exit 1
fi

# Get the filename for the model
if [ -n "${MODEL_FILES[$MODEL]}" ]; then
    FILENAME="${MODEL_FILES[$MODEL]}"
else
    FILENAME="${QUANTIZED_MODELS[$MODEL]}"
fi

DOWNLOAD_URL="$BASE_URL/$FILENAME"
OUTPUT_PATH="$MODELS_DIR/$FILENAME"

# Check if model already exists
if [ -f "$OUTPUT_PATH" ]; then
    print_color green "Model '$MODEL' already exists at: $OUTPUT_PATH"
    read -p "Do you want to re-download it? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 0
    fi
fi

print_color yellow "Downloading model: $MODEL"
echo "From: $DOWNLOAD_URL"
echo "To: $OUTPUT_PATH"
echo

# Download the model with progress bar
if command -v wget &> /dev/null; then
    # Use wget if available (shows nice progress bar)
    wget -O "$OUTPUT_PATH" "$DOWNLOAD_URL"
    DOWNLOAD_STATUS=$?
elif command -v curl &> /dev/null; then
    # Use curl as fallback
    curl -L -o "$OUTPUT_PATH" --progress-bar "$DOWNLOAD_URL"
    DOWNLOAD_STATUS=$?
else
    print_color red "Error: Neither wget nor curl is installed. Please install one of them."
    exit 1
fi

# Check download status
if [ $DOWNLOAD_STATUS -eq 0 ] && [ -f "$OUTPUT_PATH" ]; then
    # Get file size in MB
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        FILE_SIZE=$(stat -f%z "$OUTPUT_PATH" | awk '{printf "%.2f", $1/1024/1024}')
    else
        # Linux
        FILE_SIZE=$(stat -c%s "$OUTPUT_PATH" | awk '{printf "%.2f", $1/1024/1024}')
    fi
    
    print_color green "Successfully downloaded model '$MODEL'"
    echo "File size: ${FILE_SIZE} MB"
    echo "Location: $OUTPUT_PATH"
else
    print_color red "Error: Download failed"
    [ -f "$OUTPUT_PATH" ] && rm "$OUTPUT_PATH"  # Clean up partial download
    exit 1
fi

echo
print_color green "Model ready to use!"
echo "Use with: whisper-cli -m \"$OUTPUT_PATH\" <audio_file>"

# Make script executable if it isn't already
if [ ! -x "$0" ]; then
    chmod +x "$0"
fi