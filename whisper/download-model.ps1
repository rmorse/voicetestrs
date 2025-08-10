# Whisper Model Downloader for Windows
# Downloads GGML format models from Hugging Face

param(
    [Parameter(Mandatory=$false)]
    [string]$Model = "base.en"
)

$baseUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main"
$modelsDir = "$PSScriptRoot\models"

# Create models directory if it doesn't exist
if (!(Test-Path $modelsDir)) {
    New-Item -ItemType Directory -Path $modelsDir | Out-Null
    Write-Host "Created models directory: $modelsDir"
}

# Map model names to filenames
$modelFiles = @{
    "tiny"       = "ggml-tiny.bin"
    "tiny.en"    = "ggml-tiny.en.bin"
    "base"       = "ggml-base.bin"
    "base.en"    = "ggml-base.en.bin"
    "small"      = "ggml-small.bin"
    "small.en"   = "ggml-small.en.bin"
    "medium"     = "ggml-medium.bin"
    "medium.en"  = "ggml-medium.en.bin"
    "large-v1"   = "ggml-large-v1.bin"
    "large-v2"   = "ggml-large-v2.bin"
    "large-v3"   = "ggml-large-v3.bin"
    "large"      = "ggml-large-v3.bin"  # Default to v3
}

# Quantized models
$quantizedModels = @{
    "tiny-q5_0"    = "ggml-tiny-q5_0.bin"
    "base-q5_0"    = "ggml-base-q5_0.bin"
    "small-q5_0"   = "ggml-small-q5_0.bin"
    "medium-q5_0"  = "ggml-medium-q5_0.bin"
    "large-v3-q5_0" = "ggml-large-v3-q5_0.bin"
}

# Combine both model lists
$allModels = $modelFiles + $quantizedModels

if (!$allModels.ContainsKey($Model)) {
    Write-Host "Error: Unknown model '$Model'" -ForegroundColor Red
    Write-Host ""
    Write-Host "Available models:" -ForegroundColor Yellow
    Write-Host "Standard models:" -ForegroundColor Cyan
    $modelFiles.Keys | Sort-Object | ForEach-Object { Write-Host "  $_" }
    Write-Host ""
    Write-Host "Quantized models (smaller size, slightly lower quality):" -ForegroundColor Cyan
    $quantizedModels.Keys | Sort-Object | ForEach-Object { Write-Host "  $_" }
    exit 1
}

$fileName = $allModels[$Model]
$downloadUrl = "$baseUrl/$fileName"
$outputPath = "$modelsDir\$fileName"

# Check if model already exists
if (Test-Path $outputPath) {
    Write-Host "Model '$Model' already exists at: $outputPath" -ForegroundColor Green
    $response = Read-Host "Do you want to re-download it? (y/n)"
    if ($response -ne 'y') {
        exit 0
    }
}

Write-Host "Downloading model: $Model" -ForegroundColor Yellow
Write-Host "From: $downloadUrl"
Write-Host "To: $outputPath"
Write-Host ""

try {
    # Download with progress
    $ProgressPreference = 'Continue'
    Invoke-WebRequest -Uri $downloadUrl -OutFile $outputPath -UseBasicParsing
    
    # Verify file was downloaded
    if (Test-Path $outputPath) {
        $fileSize = (Get-Item $outputPath).Length / 1MB
        Write-Host "Successfully downloaded model '$Model'" -ForegroundColor Green
        Write-Host "File size: $([math]::Round($fileSize, 2)) MB"
        Write-Host "Location: $outputPath"
    } else {
        Write-Host "Error: Download failed" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "Error downloading model: $_" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Model ready to use!" -ForegroundColor Green
Write-Host "Use with: whisper-cli.exe -m `"$outputPath`" <audio_file>"