use crate::gcloud::GcloudError;

/// Abstraction over gcloud CLI execution for testability.
///
/// Production code uses [`RealExecutor`], tests use mockall-generated mocks.
#[allow(async_fn_in_trait)]
pub trait GcloudExecutor: Send + Sync {
    /// Execute a gcloud command and capture stdout.
    async fn exec(&self, args: &[String]) -> Result<String, GcloudError>;

    /// Execute a gcloud command, streaming output to the terminal.
    async fn exec_streaming(&self, args: &[String]) -> Result<(), GcloudError>;

    /// Execute a gcloud command with data piped to stdin.
    async fn exec_with_stdin(
        &self,
        args: &[String],
        stdin_data: &[u8],
    ) -> Result<String, GcloudError>;
}

/// Real gcloud CLI executor.
pub struct RealExecutor;

impl GcloudExecutor for RealExecutor {
    async fn exec(&self, args: &[String]) -> Result<String, GcloudError> {
        use std::process::Stdio;

        let output = tokio::process::Command::new("gcloud")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| GcloudError::NotFound { source: e })?;

        if output.status.success() {
            String::from_utf8(output.stdout).map_err(|e| GcloudError::InvalidUtf8 { source: e })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(GcloudError::CommandFailed {
                args: args.to_vec(),
                stderr,
            })
        }
    }

    async fn exec_streaming(&self, args: &[String]) -> Result<(), GcloudError> {
        use std::process::Stdio;

        let status = tokio::process::Command::new("gcloud")
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .map_err(|e| GcloudError::NotFound { source: e })?;

        if status.success() {
            Ok(())
        } else {
            Err(GcloudError::CommandFailed {
                args: args.to_vec(),
                stderr: format!("exit code: {status}"),
            })
        }
    }

    async fn exec_with_stdin(
        &self,
        args: &[String],
        stdin_data: &[u8],
    ) -> Result<String, GcloudError> {
        use std::process::Stdio;
        use tokio::io::AsyncWriteExt;

        let mut child = tokio::process::Command::new("gcloud")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GcloudError::NotFound { source: e })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(stdin_data)
                .await
                .map_err(|e| GcloudError::StdinWrite { source: e })?;
            stdin
                .shutdown()
                .await
                .map_err(|e| GcloudError::StdinWrite { source: e })?;
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| GcloudError::NotFound { source: e })?;

        if output.status.success() {
            String::from_utf8(output.stdout).map_err(|e| GcloudError::InvalidUtf8 { source: e })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(GcloudError::CommandFailed {
                args: args.to_vec(),
                stderr,
            })
        }
    }
}
