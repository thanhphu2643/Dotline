use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const KEEP: usize = 500;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub printer: String,
    pub payload_hash: String,
    pub bytes: usize,
    /// sent | failed
    pub state: String,
    pub detail: Option<String>,
    pub request_id: Option<String>,
    pub created_at: u64,
}

/// Every dispatch by caller job id, newest last, persisted so a restart
/// still answers "did label X print?" and never prints it twice.
pub struct Ledger {
    file: Option<PathBuf>,
    jobs: Vec<Job>,
    /// Payload bytes by hash, so a past label previews exactly as it was sent.
    payloads: std::collections::HashMap<String, Vec<u8>>,
}

impl Ledger {
    pub fn open(file: Option<PathBuf>) -> Self {
        let jobs = file
            .as_ref()
            .and_then(|f| fs::read(f).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Ledger { file, jobs, payloads: Default::default() }
    }

    fn payload_dir(&self) -> Option<PathBuf> {
        self.file.as_ref().map(|f| f.with_file_name("payloads"))
    }

    pub fn payload(&self, hash: &str) -> Option<Vec<u8>> {
        if let Some(p) = self.payloads.get(hash) {
            return Some(p.clone());
        }
        fs::read(self.payload_dir()?.join(format!("{hash}.bin"))).ok()
    }

    pub fn record_with_payload(&mut self, job: Job, payload: &[u8]) {
        match self.payload_dir() {
            Some(dir) => {
                let _ = fs::create_dir_all(&dir);
                let _ = fs::write(dir.join(format!("{}.bin", job.payload_hash)), payload);
            }
            None => {
                self.payloads.insert(job.payload_hash.clone(), payload.to_vec());
            }
        }
        self.record(job);
    }

    pub fn get(&self, id: &str) -> Option<&Job> {
        self.jobs.iter().rev().find(|j| j.id == id)
    }

    pub fn recent(&self, n: usize) -> Vec<Job> {
        self.jobs.iter().rev().take(n).cloned().collect()
    }

    pub fn record(&mut self, job: Job) {
        self.jobs.retain(|j| j.id != job.id);
        self.jobs.push(job);
        if self.jobs.len() > KEEP {
            let dropped: Vec<Job> = self.jobs.drain(..self.jobs.len() - KEEP).collect();
            for old in dropped {
                if !self.jobs.iter().any(|j| j.payload_hash == old.payload_hash) {
                    self.payloads.remove(&old.payload_hash);
                    if let Some(dir) = self.payload_dir() {
                        let _ = fs::remove_file(dir.join(format!("{}.bin", old.payload_hash)));
                    }
                }
            }
        }
        if let Some(file) = &self.file {
            let tmp = file.with_extension("tmp");
            if fs::write(&tmp, serde_json::to_vec(&self.jobs).unwrap()).is_ok() {
                let _ = fs::rename(tmp, file);
            }
        }
    }

    pub fn clear(&mut self) {
        self.jobs.clear();
        self.payloads.clear();
        if let Some(dir) = self.payload_dir() {
            let _ = fs::remove_dir_all(dir);
        }
        if let Some(file) = &self.file {
            let _ = fs::remove_file(file);
        }
    }
}
