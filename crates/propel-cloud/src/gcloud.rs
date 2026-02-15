#[derive(Debug, thiserror::Error)]
pub enum GcloudError {
    #[error("gcloud CLI not found — install: https://cloud.google.com/sdk/docs/install")]
    NotFound { source: std::io::Error },

    #[error("gcloud command failed: {args:?}\n{stderr}")]
    CommandFailed { args: Vec<String>, stderr: String },

    #[error("gcloud output was not valid UTF-8")]
    InvalidUtf8 { source: std::string::FromUtf8Error },

    #[error("failed to write to gcloud stdin")]
    StdinWrite { source: std::io::Error },
}
