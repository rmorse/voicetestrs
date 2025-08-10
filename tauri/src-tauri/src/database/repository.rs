use super::{Database, models::*};
use sqlx::{query, query_as, Row};

impl Database {
    // Create
    pub async fn insert_transcription(&self, t: &Transcription) -> Result<(), sqlx::Error> {
        let metadata_str = t.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default());
        
        query(
            r#"
            INSERT INTO transcriptions (
                id, audio_path, text_path, transcription_text,
                created_at, transcribed_at, duration_seconds, file_size_bytes,
                language, model, status, source, error_message, metadata, session_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#
        )
        .bind(&t.id)
        .bind(&t.audio_path)
        .bind(&t.text_path)
        .bind(&t.transcription_text)
        .bind(t.created_at)
        .bind(t.transcribed_at)
        .bind(t.duration_seconds)
        .bind(t.file_size_bytes)
        .bind(&t.language)
        .bind(&t.model)
        .bind(&t.status)
        .bind(&t.source)
        .bind(&t.error_message)
        .bind(metadata_str)
        .bind(t.session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    
    // Read
    pub async fn get_transcription(&self, id: &str) -> Result<Option<Transcription>, sqlx::Error> {
        let result = query_as::<_, Transcription>(
            "SELECT * FROM transcriptions WHERE id = ?1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(result)
    }
    
    // Update
    pub async fn update_transcription(&self, id: &str, updates: TranscriptionUpdate) -> Result<(), sqlx::Error> {
        let mut query_str = String::from("UPDATE transcriptions SET ");
        let mut updates_vec = Vec::new();
        
        if let Some(text_path) = updates.text_path {
            updates_vec.push(format!("text_path = '{}'", text_path));
        }
        if let Some(text) = updates.transcription_text {
            updates_vec.push(format!("transcription_text = '{}'", text.replace("'", "''")));
        }
        if let Some(transcribed_at) = updates.transcribed_at {
            updates_vec.push(format!("transcribed_at = '{}'", transcribed_at.to_rfc3339()));
        }
        if let Some(status) = updates.status {
            updates_vec.push(format!("status = '{}'", status));
        }
        if let Some(error) = updates.error_message {
            updates_vec.push(format!("error_message = '{}'", error.replace("'", "''")));
        }
        if let Some(metadata) = updates.metadata {
            updates_vec.push(format!("metadata = '{}'", serde_json::to_string(&metadata).unwrap()));
        }
        
        if updates_vec.is_empty() {
            return Ok(());
        }
        
        query_str.push_str(&updates_vec.join(", "));
        query_str.push_str(&format!(" WHERE id = '{}'", id));
        
        sqlx::query(&query_str)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
    
    pub async fn update_transcription_status(
        &self, 
        id: &str, 
        status: &str,
        error: Option<String>
    ) -> Result<(), sqlx::Error> {
        query(
            "UPDATE transcriptions SET status = ?1, error_message = ?2 WHERE id = ?3"
        )
        .bind(status)
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    
    // Delete
    pub async fn delete_transcription(&self, id: &str) -> Result<(), sqlx::Error> {
        query("DELETE FROM transcriptions WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    
    // List with pagination
    pub async fn list_transcriptions(
        &self,
        limit: i32,
        offset: i32,
        status_filter: Option<String>
    ) -> Result<Vec<Transcription>, sqlx::Error> {
        let transcriptions = if let Some(status) = status_filter {
            query_as::<_, Transcription>(
                r#"
                SELECT * FROM transcriptions 
                WHERE status = ?1 
                ORDER BY created_at DESC 
                LIMIT ?2 OFFSET ?3
                "#
            )
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            query_as::<_, Transcription>(
                r#"
                SELECT * FROM transcriptions 
                ORDER BY created_at DESC 
                LIMIT ?1 OFFSET ?2
                "#
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };
        
        Ok(transcriptions)
    }
    
    // Search with FTS
    pub async fn search_transcriptions(&self, search_query: &str) -> Result<Vec<Transcription>, sqlx::Error> {
        let transcriptions = query_as::<_, Transcription>(
            r#"
            SELECT t.* FROM transcriptions t
            JOIN transcriptions_fts fts ON t.rowid = fts.rowid
            WHERE fts.transcription_text MATCH ?1
            ORDER BY rank
            LIMIT 100
            "#
        )
        .bind(search_query)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(transcriptions)
    }
    
    // Get all IDs (for sync optimization)
    pub async fn get_all_transcription_ids(&self) -> Result<Vec<String>, sqlx::Error> {
        let records = query("SELECT id FROM transcriptions")
            .fetch_all(&self.pool)
            .await?;
        
        Ok(records.into_iter().map(|r| r.get::<String, _>("id")).collect())
    }
    
    // Database stats
    pub async fn get_stats(&self) -> Result<DatabaseStats, sqlx::Error> {
        let row = query(
            r#"
            SELECT 
                COUNT(*) as total_transcriptions,
                COALESCE(SUM(file_size_bytes), 0) as total_size_bytes,
                COALESCE(SUM(duration_seconds), 0.0) as total_duration_seconds,
                COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0) as pending_count,
                COALESCE(SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END), 0) as completed_count,
                COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0) as failed_count
            FROM transcriptions
            "#
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(DatabaseStats {
            total_transcriptions: row.get("total_transcriptions"),
            total_size_bytes: row.get("total_size_bytes"),
            total_duration_seconds: row.get("total_duration_seconds"),
            pending_count: row.get("pending_count"),
            completed_count: row.get("completed_count"),
            failed_count: row.get("failed_count"),
        })
    }
    
    // Clear all transcriptions
    pub async fn clear_all_transcriptions(&self) -> Result<(), sqlx::Error> {
        query("DELETE FROM transcriptions")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    
    // Clean up duplicate entries, keeping only the ones with clean paths
    pub async fn cleanup_duplicates(&self) -> Result<usize, sqlx::Error> {
        // First, get all transcriptions
        let all_transcriptions = query_as::<_, Transcription>(
            "SELECT * FROM transcriptions ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        
        let mut seen_paths = std::collections::HashSet::new();
        let mut to_delete = Vec::new();
        
        for t in all_transcriptions {
            // Normalize the path for comparison
            let normalized = crate::database::utils::normalize_audio_path(
                std::path::Path::new(&t.audio_path)
            );
            
            // If we've seen this normalized path before, mark for deletion
            if seen_paths.contains(&normalized) {
                to_delete.push(t.id);
            } else {
                seen_paths.insert(normalized);
                // If this entry has a non-normalized path, also mark it
                if t.audio_path.contains(":\\") || t.audio_path.starts_with("\\\\") {
                    to_delete.push(t.id);
                }
            }
        }
        
        // Delete duplicates
        let mut deleted_count = 0;
        for id in to_delete {
            query("DELETE FROM transcriptions WHERE id = ?1")
                .bind(id)
                .execute(&self.pool)
                .await?;
            deleted_count += 1;
        }
        
        Ok(deleted_count)
    }
}