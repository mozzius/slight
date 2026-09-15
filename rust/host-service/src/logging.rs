use gateway_protocol::LogEntryDto;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const LOG_FILE_NAME: &str = "host.log";

pub struct HostLogger {
    directory: PathBuf,
    max_bytes: u64,
    retained_files: usize,
    file: Mutex<FileState>,
}

struct FileState {
    file: File,
    bytes: u64,
}

impl HostLogger {
    pub fn new(directory: &Path, max_bytes: u64, retained_files: usize) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let path = directory.join(LOG_FILE_NAME);
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            directory: directory.to_path_buf(),
            max_bytes: max_bytes.max(1),
            retained_files: retained_files.max(1),
            file: Mutex::new(FileState { file, bytes }),
        })
    }

    pub fn write(&self, entry: &LogEntryDto) {
        let Ok(line) = serde_json::to_string(entry) else {
            return;
        };
        let line = format!("{line}\n");
        let Ok(mut state) = self.file.lock() else {
            return;
        };
        if state.bytes > 0 && state.bytes.saturating_add(line.len() as u64) > self.max_bytes {
            if self.rotate(&mut state).is_err() {
                return;
            }
        }
        if state.file.write_all(line.as_bytes()).is_ok() {
            let _ = state.file.flush();
            state.bytes = state.bytes.saturating_add(line.len() as u64);
        }
    }

    fn rotate(&self, state: &mut FileState) -> io::Result<()> {
        state.file.flush()?;
        if self.retained_files > 2 {
            for index in (1..=self.retained_files - 2).rev() {
                let source = self.rotated_path(index);
                let destination = self.rotated_path(index + 1);
                if source.exists() {
                    if destination.exists() {
                        fs::remove_file(&destination)?;
                    }
                    fs::rename(source, destination)?;
                }
            }
        }
        let active = self.directory.join(LOG_FILE_NAME);
        if self.retained_files > 1 && active.exists() {
            let rotated = self.rotated_path(1);
            if rotated.exists() {
                fs::remove_file(&rotated)?;
            }
            fs::rename(active, rotated)?;
        } else if active.exists() {
            fs::remove_file(active)?;
        }
        state.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.directory.join(LOG_FILE_NAME))?;
        state.bytes = 0;
        Ok(())
    }

    fn rotated_path(&self, index: usize) -> PathBuf {
        self.directory.join(format!("{LOG_FILE_NAME}.{index}"))
    }
}

#[cfg(test)]
mod tests {
    use super::HostLogger;
    use gateway_protocol::LogEntryDto;

    #[test]
    fn rotates_and_limits_log_files() {
        let directory = std::env::temp_dir().join(format!("slight-logs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let logger = HostLogger::new(&directory, 80, 3).expect("logger creates directory");
        for index in 0..12 {
            logger.write(&LogEntryDto {
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                level: "info".to_string(),
                message: format!("entry-{index}-with-padding"),
            });
        }
        assert!(directory.join("host.log").exists());
        assert!(directory.join("host.log.1").exists());
        assert!(directory.join("host.log.2").exists());
        assert!(!directory.join("host.log.3").exists());
        let files = std::fs::read_dir(&directory)
            .expect("read log directory")
            .count();
        assert_eq!(files, 3);
        std::fs::remove_dir_all(directory).expect("remove test logs");
    }
}
