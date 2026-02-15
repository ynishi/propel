use mockall::mock;
use propel_cloud::client::{
    CloudBuildError, DeployError, GcloudClient, PreflightError, SecretError,
};
use propel_cloud::executor::GcloudExecutor;
use propel_cloud::gcloud::GcloudError;
use propel_core::CloudRunConfig;
use std::path::PathBuf;

mock! {
    Executor {}

    impl GcloudExecutor for Executor {
        async fn exec(&self, args: &[String]) -> Result<String, GcloudError>;
        async fn exec_streaming(&self, args: &[String]) -> Result<(), GcloudError>;
        async fn exec_with_stdin(
            &self,
            args: &[String],
            stdin_data: &[u8],
        ) -> Result<String, GcloudError>;
    }
}

// ── Preflight Tests ──

#[tokio::test]
async fn preflight_all_checks_pass() {
    let mut mock = MockExecutor::new();

    // version
    mock.expect_exec()
        .withf(|args| args.contains(&"version".to_owned()))
        .returning(|_| Ok("495.0.0\n".to_owned()));

    // auth
    mock.expect_exec()
        .withf(|args| args.contains(&"print-identity-token".to_owned()))
        .returning(|_| Ok("ya29.token\n".to_owned()));

    // project describe
    mock.expect_exec()
        .withf(|args| {
            args.contains(&"describe".to_owned()) && args.contains(&"projects".to_owned())
        })
        .returning(|_| Ok("my-project-name\n".to_owned()));

    // services list (3 API checks)
    mock.expect_exec()
        .withf(|args| args.contains(&"services".to_owned()) && args.contains(&"list".to_owned()))
        .returning(|args| {
            // Return the API name to indicate it's enabled
            let filter_arg = args.iter().find(|a| a.starts_with("config.name="));
            match filter_arg {
                Some(f) => Ok(format!(
                    "{}\n",
                    f.strip_prefix("config.name=").unwrap_or("")
                )),
                None => Ok(String::new()),
            }
        });

    let client = GcloudClient::with_executor(mock);
    let report = client.check_prerequisites("test-project").await.unwrap();

    assert_eq!(report.gcloud_version.as_deref(), Some("495.0.0"));
    assert!(report.authenticated);
    assert_eq!(report.project_name.as_deref(), Some("my-project-name"));
    assert!(report.disabled_apis.is_empty());
    assert!(!report.has_warnings());
}

#[tokio::test]
async fn preflight_gcloud_not_installed() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| args.contains(&"version".to_owned()))
        .returning(|_| {
            Err(GcloudError::NotFound {
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client.check_prerequisites("test-project").await;

    assert!(matches!(result, Err(PreflightError::GcloudNotInstalled)));
}

#[tokio::test]
async fn preflight_not_authenticated() {
    let mut mock = MockExecutor::new();

    // version OK
    mock.expect_exec()
        .withf(|args| args.contains(&"version".to_owned()))
        .returning(|_| Ok("495.0.0\n".to_owned()));

    // auth fails
    mock.expect_exec()
        .withf(|args| args.contains(&"print-identity-token".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "not logged in".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client.check_prerequisites("test-project").await;

    assert!(matches!(result, Err(PreflightError::NotAuthenticated)));
}

#[tokio::test]
async fn preflight_project_not_accessible() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| args.contains(&"version".to_owned()))
        .returning(|_| Ok("495.0.0\n".to_owned()));

    mock.expect_exec()
        .withf(|args| args.contains(&"print-identity-token".to_owned()))
        .returning(|_| Ok("ya29.token\n".to_owned()));

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"describe".to_owned()) && args.contains(&"projects".to_owned())
        })
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "not found".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client.check_prerequisites("bad-project").await;

    assert!(matches!(
        result,
        Err(PreflightError::ProjectNotAccessible(ref p)) if p == "bad-project"
    ));
}

#[tokio::test]
async fn preflight_disabled_apis_reported() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| args.contains(&"version".to_owned()))
        .returning(|_| Ok("495.0.0\n".to_owned()));

    mock.expect_exec()
        .withf(|args| args.contains(&"print-identity-token".to_owned()))
        .returning(|_| Ok("ya29.token\n".to_owned()));

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"describe".to_owned()) && args.contains(&"projects".to_owned())
        })
        .returning(|_| Ok("my-project\n".to_owned()));

    // All API checks return empty (disabled)
    mock.expect_exec()
        .withf(|args| args.contains(&"services".to_owned()) && args.contains(&"list".to_owned()))
        .returning(|_| Ok("\n".to_owned()));

    let client = GcloudClient::with_executor(mock);
    let report = client.check_prerequisites("test-project").await.unwrap();

    assert!(report.has_warnings());
    assert_eq!(report.disabled_apis.len(), 3);
    assert!(
        report
            .disabled_apis
            .contains(&"cloudbuild.googleapis.com".to_owned())
    );
    assert!(
        report
            .disabled_apis
            .contains(&"run.googleapis.com".to_owned())
    );
    assert!(
        report
            .disabled_apis
            .contains(&"secretmanager.googleapis.com".to_owned())
    );
}

// ── Cloud Build Tests ──

#[tokio::test]
async fn submit_build_success() {
    let mut mock = MockExecutor::new();

    mock.expect_exec_streaming()
        .withf(|args| {
            args.contains(&"builds".to_owned())
                && args.contains(&"submit".to_owned())
                && args.contains(&"--tag".to_owned())
        })
        .returning(|_| Ok(()));

    let client = GcloudClient::with_executor(mock);
    let result = client
        .submit_build(
            &PathBuf::from("/tmp/bundle"),
            "my-project",
            "gcr.io/my-project/my-service:latest",
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn submit_build_failure() {
    let mut mock = MockExecutor::new();

    mock.expect_exec_streaming().returning(|_| {
        Err(GcloudError::CommandFailed {
            args: vec![],
            stderr: "build failed".to_owned(),
        })
    });

    let client = GcloudClient::with_executor(mock);
    let result = client
        .submit_build(&PathBuf::from("/tmp/bundle"), "proj", "tag")
        .await;

    assert!(matches!(result, Err(CloudBuildError::Submit { .. })));
}

// ── Cloud Run Deploy Tests ──

#[tokio::test]
async fn deploy_to_cloud_run_returns_url() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| args.contains(&"run".to_owned()) && args.contains(&"deploy".to_owned()))
        .returning(|_| Ok("https://my-service-abc123-uc.a.run.app\n".to_owned()));

    let client = GcloudClient::with_executor(mock);
    let url = client
        .deploy_to_cloud_run(
            "my-service",
            "gcr.io/proj/svc:latest",
            "proj",
            "us-central1",
            &CloudRunConfig::default(),
        )
        .await
        .unwrap();

    assert_eq!(url, "https://my-service-abc123-uc.a.run.app");
}

#[tokio::test]
async fn deploy_to_cloud_run_failure() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| args.contains(&"run".to_owned()) && args.contains(&"deploy".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "permission denied".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client
        .deploy_to_cloud_run(
            "svc",
            "tag",
            "proj",
            "us-central1",
            &CloudRunConfig::default(),
        )
        .await;

    assert!(matches!(result, Err(DeployError::Deploy { .. })));
}

// ── Secret Manager Tests ──

#[tokio::test]
async fn set_secret_creates_new_secret() {
    let mut mock = MockExecutor::new();

    // describe → not found (secret doesn't exist)
    mock.expect_exec()
        .withf(|args| args.contains(&"describe".to_owned()) && args.contains(&"secrets".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "NOT_FOUND".to_owned(),
            })
        });

    // create
    mock.expect_exec()
        .withf(|args| args.contains(&"create".to_owned()) && args.contains(&"secrets".to_owned()))
        .returning(|_| Ok(String::new()));

    // versions add
    mock.expect_exec_with_stdin()
        .withf(|args, data| {
            args.contains(&"versions".to_owned())
                && args.contains(&"add".to_owned())
                && data == b"super-secret-value"
        })
        .returning(|_, _| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let result = client
        .set_secret("proj", "MY_SECRET", "super-secret-value")
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn set_secret_updates_existing() {
    let mut mock = MockExecutor::new();

    // describe → exists
    mock.expect_exec()
        .withf(|args| args.contains(&"describe".to_owned()) && args.contains(&"secrets".to_owned()))
        .returning(|_| Ok("secret exists".to_owned()));

    // No create call expected — goes straight to versions add
    mock.expect_exec_with_stdin()
        .withf(|args, _| args.contains(&"versions".to_owned()) && args.contains(&"add".to_owned()))
        .returning(|_, _| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let result = client.set_secret("proj", "EXISTING", "new-value").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn set_secret_create_fails() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| args.contains(&"describe".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "NOT_FOUND".to_owned(),
            })
        });

    mock.expect_exec()
        .withf(|args| args.contains(&"create".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "permission denied".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client.set_secret("proj", "SECRET", "val").await;

    assert!(matches!(result, Err(SecretError::Create { .. })));
}

#[tokio::test]
async fn list_secrets_returns_names() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"secrets".to_owned())
                && args.contains(&"list".to_owned())
                && args.contains(&"value(name)".to_owned())
        })
        .returning(|_| Ok("SUPABASE_URL\nSUPABASE_KEY\nJWT_SECRET\n".to_owned()));

    let client = GcloudClient::with_executor(mock);
    let secrets = client.list_secrets("proj").await.unwrap();

    assert_eq!(secrets, vec!["SUPABASE_URL", "SUPABASE_KEY", "JWT_SECRET"]);
}

#[tokio::test]
async fn list_secrets_empty() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| args.contains(&"secrets".to_owned()) && args.contains(&"list".to_owned()))
        .returning(|_| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let secrets = client.list_secrets("proj").await.unwrap();

    assert!(secrets.is_empty());
}
