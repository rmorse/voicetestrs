# Whisper Setup Instructions

## Download Whisper Binary

### Automated Download (Recommended)

**Windows:**
```bash
# From project root
cd whisper
download.bat        # Downloads CPU version (default)

# Or download GPU version for NVIDIA cards
download.bat cuda12  # For CUDA 12
download.bat cuda11  # For CUDA 11
```

**Linux/Mac:**
```bash
# From project root
cd whisper
chmod +x download.sh  # Make executable (first time only)
./download.sh         # Auto-detects OS and downloads appropriate version
```

### Manual Download (Fallback)

If the automated scripts don't work, you can manually download:

**Windows:**
1. Go to: https://github.com/Purfview/whisper-standalone-win/releases
2. Download the latest release:
   - `Whisper-Faster-XXL-main.zip` for CPU-only
   - `Whisper-Faster-XXL-cuda12.zip` for NVIDIA GPU with CUDA 12
   - `Whisper-Faster-XXL-cuda11.zip` for NVIDIA GPU with CUDA 11
3. Extract the contents to the `whisper/Release/` folder

**Linux/Mac:**
1. Go to: https://github.com/ggerganov/whisper.cpp/releases
2. Download the appropriate binary for your system, or compile from source:
   ```bash
   git clone https://github.com/ggerganov/whisper.cpp
   cd whisper.cpp
   make
   ```
3. Copy binaries to `whisper/Release/` folder

Your folder structure should look like:
```
voicetextrs/
├── whisper/
│   ├── Release/
│   │   ├── whisper-cli.exe  # Main whisper executable (Windows)
│   │   ├── main             # Or 'main' on Linux/Mac
│   │   └── ... (other files)
│   ├── models/              # Model files will be downloaded here
│   ├── download.bat/sh      # Binary download scripts
│   └── download-model.bat/sh # Model download scripts
├── src/
└── Cargo.toml
```

## Download Models

Use the provided download script to get Whisper models:

### Quick Download (Recommended)

**Windows:**
```bash
# From project root
cd whisper
download-model.bat base.en  # Downloads base English model

# Or download other models
download-model.bat small    # Small multilingual model
download-model.bat medium.en # Medium English-only model
download-model.bat large-v3 # Latest large model
```

**Linux/Mac:**
```bash
# From project root
cd whisper
chmod +x download-model.sh  # Make script executable (first time only)
./download-model.sh base.en  # Downloads base English model

# Or download other models
./download-model.sh small    # Small multilingual model
./download-model.sh medium.en # Medium English-only model
./download-model.sh large-v3 # Latest large model
```

### Available Models

**English-only models** (smaller, faster for English):
- `tiny.en` (39 MB) - Fastest, lower quality
- `base.en` (74 MB) - Good balance (default)
- `small.en` (244 MB) - Better quality
- `medium.en` (769 MB) - High quality

**Multilingual models** (support 99+ languages):
- `tiny` (39 MB)
- `base` (74 MB)
- `small` (244 MB)
- `medium` (769 MB)
- `large-v1` (1550 MB)
- `large-v2` (1550 MB)
- `large-v3` (1550 MB) - Latest and best

**Quantized models** (smaller file size, slightly lower quality):
- `tiny-q5_0`, `base-q5_0`, `small-q5_0`, `medium-q5_0`, `large-v3-q5_0`

### Manual Download

Models can also be downloaded directly from:
https://huggingface.co/ggerganov/whisper.cpp/tree/main

Save them to the `whisper/models/` directory with the correct filename (e.g., `ggml-base.en.bin`)

## Test the Setup

After downloading the whisper binary:

```bash
# Test recording (3 seconds)
cargo run -- --test 3

# Record and transcribe (5 seconds)
cargo run -- --record 5

# Transcribe existing audio file
cargo run -- --transcribe path/to/audio.wav
```

## Troubleshooting

- If you get "Whisper binary not found", make sure `main.exe` is in the `whisper/` folder
- If transcription fails, check that the model file exists in `whisper/models/`
- For GPU acceleration, use the CUDA or cuBLAS version of whisper