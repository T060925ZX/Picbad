use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;
use tokio::fs;

#[derive(Clone, Debug, Parser)]
#[command(author, version, about)]
pub struct Config {
    #[arg(long, env = "PICBAD_BIND", default_value = "0.0.0.0:8080")]
    pub bind: String,

    #[arg(long, env = "PICBAD_DATA_DIR", default_value = "./data")]
    pub data_dir: PathBuf,

    #[arg(
        long,
        env = "PICBAD_DATABASE_URL",
        default_value = "sqlite://data/picbad.sqlite"
    )]
    pub database_url: String,

    #[arg(long, env = "PICBAD_CACHE_MAX_BYTES", default_value_t = 2 * 1024 * 1024 * 1024u64)]
    pub cache_max_bytes: u64,

    #[arg(long, env = "PICBAD_MAX_UPLOAD_BYTES", default_value_t = 50 * 1024 * 1024usize)]
    pub max_upload_bytes: usize,

    #[arg(long, env = "PICBAD_ADMIN_USER", default_value = "admin")]
    pub admin_user: String,

    #[arg(long, env = "PICBAD_ADMIN_PASSWORD", default_value = "PicbadAdmin123!")]
    pub admin_password: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self::parse())
    }

    pub fn originals_dir(&self) -> PathBuf {
        self.data_dir.join("originals")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join("cache")
    }

    pub async fn ensure_dirs(&self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.data_dir)
            .await
            .context("create data dir")?;
        fs::create_dir_all(self.originals_dir())
            .await
            .context("create originals dir")?;
        fs::create_dir_all(self.cache_dir())
            .await
            .context("create cache dir")?;
        Ok(())
    }
}
