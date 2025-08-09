# Whisper Setup Instructions

## Download Whisper Binary

1. Go to: https://github.com/Purfview/whisper-standalone-win/releases
2. Download the latest release (e.g., `whisper-cublas-12.2.0-bin-x64.zip` for GPU or `whisper-bin-x64.zip` for CPU-only)
3. Extract the contents to the `whisper/` folder in this project directory

Your folder structure should look like:
```
voicetextrs/
├── whisper/
│   ├── main.exe           # The whisper executable
│   ├── models/            # Model files will be downloaded here
│   └── ... (other files from the zip)
├── src/
└── Cargo.toml
```

## Download Models

The app will automatically download the `base.en` model on first use. You can also manually download models:

1. Run: `whisper/main.exe --model base.en --model-download`
2. Available models:
   - `tiny.en` (39 MB) - Fastest, lower quality
   - `base.en` (74 MB) - Good balance (default)
   - `small.en` (244 MB) - Better quality
   - `medium.en` (769 MB) - High quality
   - `large` (1550 MB) - Best quality, slowest

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