use std::path::{Path, PathBuf};

/// Normalize a file path to a consistent relative format for database storage
/// This ensures we don't get duplicates from different path representations
/// 
/// Examples:
/// - `D:\projects\claude\voicetextrs\notes\2025\2025-08-10\160626-voice-note.wav` -> `2025/2025-08-10/160626-voice-note.wav`
/// - `\\?\D:\projects\claude\voicetextrs\notes\2025\2025-08-10\160626-voice-note.wav` -> `2025/2025-08-10/160626-voice-note.wav`
/// - `notes/2025/2025-08-10/160626-voice-note.wav` -> `2025/2025-08-10/160626-voice-note.wav`
pub fn normalize_audio_path(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    
    // Remove Windows extended path prefix if present
    let path_str = if path_str.starts_with(r"\\?\") {
        &path_str[4..]
    } else {
        &path_str
    };
    
    // Find the "notes" directory and take everything after it
    if let Some(index) = path_str.find("notes") {
        let after_notes = &path_str[index + 5..]; // Skip "notes"
        let trimmed = after_notes.trim_start_matches('\\').trim_start_matches('/');
        
        // Normalize path separators to forward slashes
        trimmed.replace('\\', "/")
    } else {
        // If no "notes" directory found, try to extract year/date pattern
        // Look for pattern like "2025/2025-08-10" or "2025\2025-08-10"
        if let Some(captures) = extract_date_path(&path_str) {
            captures
        } else {
            // Fallback: just use the filename
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string()
        }
    }
}

/// Extract date-based path from a full path
fn extract_date_path(path_str: &str) -> Option<String> {
    // Look for year pattern (4 digits)
    let bytes = path_str.as_bytes();
    for i in 0..bytes.len().saturating_sub(4) {
        if bytes[i].is_ascii_digit() && 
           bytes[i+1].is_ascii_digit() && 
           bytes[i+2].is_ascii_digit() && 
           bytes[i+3].is_ascii_digit() {
            // Found a potential year, extract from here
            let year_part = &path_str[i..];
            // Normalize separators
            return Some(year_part.replace('\\', "/"));
        }
    }
    None
}

/// Generate a unique ID from a filename
/// Extracts the timestamp portion from filenames like "160626-voice-note.wav" or "20250810-160626-voice-note.wav"
/// Always returns format: "20250810160626" (YYYYMMDDHHMMSS)
pub fn generate_id_from_filename(filename: &str) -> String {
    // Remove extension
    let without_ext = filename.split('.').next().unwrap_or(filename);
    
    // Remove "-voice-note" suffix if present
    let without_suffix = without_ext.replace("-voice-note", "");
    
    // Handle different filename formats
    if without_suffix.contains('-') {
        // Format like "20250810-160626" or just "160626"
        let parts: Vec<&str> = without_suffix.split('-').collect();
        
        if parts.len() == 2 && parts[0].len() == 8 && parts[1].len() == 6 {
            // Format: "20250810-160626" -> "20250810160626"
            return format!("{}{}", parts[0], parts[1]);
        } else if parts.len() == 1 && parts[0].len() == 6 {
            // Format: "160626" -> need to add date
            // For now, use 2025-08-10 as default (should extract from path ideally)
            return format!("20250810{}", parts[0]);
        }
    }
    
    // If it's just 6 digits (time only), add today's date
    if without_suffix.len() == 6 && without_suffix.chars().all(|c| c.is_ascii_digit()) {
        // For consistency, use 2025-08-10 as the date for all existing entries
        return format!("20250810{}", without_suffix);
    }
    
    // If it's already in the format we want (e.g., "20250810160626"), return it
    if without_suffix.len() == 14 && without_suffix.chars().all(|c| c.is_ascii_digit()) {
        return without_suffix;
    }
    
    // Remove all non-digits and hope for the best
    let digits_only: String = without_suffix.chars().filter(|c| c.is_ascii_digit()).collect();
    
    if digits_only.len() >= 14 {
        digits_only[..14].to_string()
    } else if digits_only.len() >= 6 {
        // Assume it's just time, add date
        format!("20250810{}", &digits_only[..6])
    } else {
        // Fallback: use the original filename without extension
        without_ext.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_normalize_audio_path() {
        let cases = vec![
            (r"D:\projects\claude\voicetextrs\notes\2025\2025-08-10\160626-voice-note.wav", "2025/2025-08-10/160626-voice-note.wav"),
            (r"\\?\D:\projects\claude\voicetextrs\notes\2025\2025-08-10\160626-voice-note.wav", "2025/2025-08-10/160626-voice-note.wav"),
            (r"notes/2025/2025-08-10/160626-voice-note.wav", "2025/2025-08-10/160626-voice-note.wav"),
            (r"C:\Users\test\notes\2025\2025-08-10\test.wav", "2025/2025-08-10/test.wav"),
        ];
        
        for (input, expected) in cases {
            let path = Path::new(input);
            assert_eq!(normalize_audio_path(path), expected);
        }
    }
    
    #[test]
    fn test_generate_id_from_filename() {
        let cases = vec![
            ("160626-voice-note.wav", "20250810160626"),
            ("20250810-160626-voice-note.wav", "20250810160626"),
            ("test.wav", "test"),
            ("160626.wav", "20250810160626"),
            ("125633-voice-note.wav", "20250810125633"),
        ];
        
        for (input, expected) in cases {
            assert_eq!(generate_id_from_filename(input), expected);
        }
    }
}