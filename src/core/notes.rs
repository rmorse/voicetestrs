use anyhow::Result;
use chrono::{DateTime, Local};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub created: DateTime<Local>,
    pub duration: f32,
    pub model: String,
    pub language: String,
    pub audio_file: Option<PathBuf>,
    pub text: String,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

impl Note {
    pub fn new(text: String) -> Self {
        Self {
            created: Local::now(),
            duration: 0.0,
            model: "base".to_string(),
            language: "en".to_string(),
            audio_file: None,
            text,
            segments: Vec::new(),
        }
    }
    
    pub fn to_markdown(&self) -> String {
        let mut content = String::new();
        
        // Frontmatter
        content.push_str("---\n");
        content.push_str(&format!("created: {}\n", self.created.to_rfc3339()));
        content.push_str(&format!("duration: {:.1}s\n", self.duration));
        content.push_str(&format!("model: {}\n", self.model));
        content.push_str(&format!("language: {}\n", self.language));
        if let Some(audio) = &self.audio_file {
            content.push_str(&format!("audio_file: {}\n", audio.display()));
        }
        content.push_str("---\n\n");
        
        // Title
        content.push_str(&format!("# Voice Note - {}\n\n", 
            self.created.format("%I:%M %p")));
        
        // Main text
        content.push_str(&self.text);
        content.push_str("\n\n");
        
        // Segments with timestamps
        if !self.segments.is_empty() {
            content.push_str("## Timestamps\n\n");
            for segment in &self.segments {
                let start = format_time(segment.start);
                let end = format_time(segment.end);
                content.push_str(&format!("**[{} - {}]** {}\n\n", 
                    start, end, segment.text));
            }
        }
        
        content
    }
    
    pub fn save(&self, base_path: &Path) -> Result<PathBuf> {
        let date_dir = base_path
            .join(self.created.format("%Y").to_string())
            .join(self.created.format("%Y-%m-%d").to_string());
        
        std::fs::create_dir_all(&date_dir)?;
        
        let filename = format!("{}-{}.md", 
            self.created.format("%H%M%S"),
            self.generate_slug());
        
        let filepath = date_dir.join(filename);
        std::fs::write(&filepath, self.to_markdown())?;
        
        Ok(filepath)
    }
    
    fn generate_slug(&self) -> String {
        self.text
            .split_whitespace()
            .take(5)
            .collect::<Vec<_>>()
            .join("-")
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect()
    }
}

fn format_time(seconds: f32) -> String {
    let mins = (seconds / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    format!("{}:{:02}", mins, secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_note_markdown() {
        let mut note = Note::new("This is a test note".to_string());
        note.segments.push(Segment {
            start: 0.0,
            end: 2.5,
            text: "This is a test".to_string(),
        });
        
        let markdown = note.to_markdown();
        assert!(markdown.contains("# Voice Note"));
        assert!(markdown.contains("This is a test note"));
        assert!(markdown.contains("## Timestamps"));
    }
}