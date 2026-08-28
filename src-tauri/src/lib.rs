pub mod db;
pub mod search;
pub mod case;
pub mod plugin;
pub mod collect;
pub mod entity;
pub mod config;
pub mod crypto;
pub mod cmd;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use config::Config;
use crate::db::GraphDb;
use crate::search::SearchIndex;

pub struct AppState {
    pub db_path: Arc<Mutex<Option<PathBuf>>>,
    pub case_config: Arc<Mutex<Option<case::CaseMetadata>>>,
    pub global_config: Arc<Mutex<Config>>,
    pub db: Arc<Mutex<Option<Arc<GraphDb>>>>,
    pub search: Arc<Mutex<Option<Arc<SearchIndex>>>>,
}
