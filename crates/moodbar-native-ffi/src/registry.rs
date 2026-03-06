use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use moodbar_analysis::MoodbarAnalysis;
use once_cell::sync::Lazy;

use crate::errors::FfiError;
use crate::MoodbarNativeAnalysisSummary;

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
static ANALYSIS_REGISTRY: Lazy<Mutex<HashMap<u64, MoodbarAnalysis>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(crate) fn store_analysis(
    analysis: MoodbarAnalysis,
) -> Result<MoodbarNativeAnalysisSummary, FfiError> {
    let frame_count = analysis.frames.len() as u32;
    let channel_count = analysis.channel_count as u32;
    let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);

    let mut guard = ANALYSIS_REGISTRY.lock().map_err(|_| FfiError::Poisoned)?;
    guard.insert(handle, analysis);

    Ok(MoodbarNativeAnalysisSummary {
        handle,
        frame_count,
        channel_count,
    })
}

pub(crate) fn with_analysis<R>(
    handle: u64,
    f: impl FnOnce(&MoodbarAnalysis) -> Result<R, FfiError>,
) -> Result<R, FfiError> {
    let guard = ANALYSIS_REGISTRY.lock().map_err(|_| FfiError::Poisoned)?;
    let analysis = guard
        .get(&handle)
        .ok_or_else(|| FfiError::NotFound(format!("analysis handle {handle} not found")))?;
    f(analysis)
}

pub(crate) fn free_analysis(handle: u64) -> Result<(), FfiError> {
    let mut guard = ANALYSIS_REGISTRY.lock().map_err(|_| FfiError::Poisoned)?;
    if guard.remove(&handle).is_none() {
        return Err(FfiError::NotFound(format!(
            "analysis handle {handle} not found"
        )));
    }
    Ok(())
}
