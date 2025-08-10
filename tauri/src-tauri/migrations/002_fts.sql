-- Add full-text search capability

-- Create FTS5 virtual table for transcription search
CREATE VIRTUAL TABLE IF NOT EXISTS transcriptions_fts USING fts5(
    transcription_text,
    content='transcriptions',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Populate FTS table with existing data
INSERT INTO transcriptions_fts(rowid, transcription_text) 
SELECT rowid, transcription_text 
FROM transcriptions 
WHERE transcription_text IS NOT NULL;

-- Create triggers to keep FTS in sync with main table
CREATE TRIGGER IF NOT EXISTS transcriptions_ai 
AFTER INSERT ON transcriptions 
WHEN new.transcription_text IS NOT NULL
BEGIN
    INSERT INTO transcriptions_fts(rowid, transcription_text) 
    VALUES (new.rowid, new.transcription_text);
END;

CREATE TRIGGER IF NOT EXISTS transcriptions_ad 
AFTER DELETE ON transcriptions 
BEGIN
    DELETE FROM transcriptions_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS transcriptions_au 
AFTER UPDATE OF transcription_text ON transcriptions 
WHEN new.transcription_text IS NOT NULL
BEGIN
    UPDATE transcriptions_fts 
    SET transcription_text = new.transcription_text 
    WHERE rowid = new.rowid;
END;

-- Add trigger for NULL text updates
CREATE TRIGGER IF NOT EXISTS transcriptions_au_null 
AFTER UPDATE OF transcription_text ON transcriptions 
WHEN new.transcription_text IS NULL AND old.transcription_text IS NOT NULL
BEGIN
    DELETE FROM transcriptions_fts WHERE rowid = old.rowid;
END;