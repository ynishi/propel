use mockall::mock;
use propel_cloud::client::{
    CloudBuildError, DeployError, GcloudClient, PreflightError, SecretError, WifError,
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
        .withf(|args| args.contains(&"print-access-token".to_owned()))
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
        .withf(|args| args.contains(&"print-access-token".to_owned()))
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
        .withf(|args| args.contains(&"print-access-token".to_owned()))
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
        .withf(|args| args.contains(&"print-access-token".to_owned()))
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
            &[],
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
            &[],
        )
        .await;

    assert!(matches!(result, Err(DeployError::Deploy { .. })));
}

#[tokio::test]
async fn deploy_to_cloud_run_with_secrets() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"--update-secrets".to_owned())
                && args
                    .contains(&"SUPABASE_URL=SUPABASE_URL:latest,API_KEY=API_KEY:latest".to_owned())
        })
        .returning(|_| Ok("https://svc-abc123-uc.a.run.app\n".to_owned()));

    let client = GcloudClient::with_executor(mock);
    let secrets = vec!["SUPABASE_URL".to_owned(), "API_KEY".to_owned()];
    let url = client
        .deploy_to_cloud_run(
            "svc",
            "gcr.io/proj/svc:latest",
            "proj",
            "us-central1",
            &CloudRunConfig::default(),
            &secrets,
        )
        .await
        .unwrap();

    assert_eq!(url, "https://svc-abc123-uc.a.run.app");
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

#[tokio::test]
async fn get_project_number_returns_number() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"projects".to_owned())
                && args.contains(&"describe".to_owned())
                && args.contains(&"value(projectNumber)".to_owned())
        })
        .returning(|_| Ok("123456789\n".to_owned()));

    let client = GcloudClient::with_executor(mock);
    let number = client.get_project_number("my-project").await.unwrap();

    assert_eq!(number, "123456789");
}

#[tokio::test]
async fn grant_secret_access_calls_add_iam_policy_binding() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"add-iam-policy-binding".to_owned())
                && args.contains(&"MY_SECRET".to_owned())
                && args.contains(
                    &"serviceAccount:123-compute@developer.gserviceaccount.com".to_owned(),
                )
                && args.contains(&"roles/secretmanager.secretAccessor".to_owned())
        })
        .returning(|_| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let result = client
        .grant_secret_access(
            "proj",
            "MY_SECRET",
            "123-compute@developer.gserviceaccount.com",
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn grant_secret_access_failure() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| args.contains(&"add-iam-policy-binding".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "permission denied".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client
        .grant_secret_access("proj", "SECRET", "sa@example.com")
        .await;

    assert!(matches!(result, Err(SecretError::GrantAccess { .. })));
}

#[tokio::test]
async fn revoke_secret_access_calls_remove_iam_policy_binding() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"remove-iam-policy-binding".to_owned())
                && args.contains(&"MY_SECRET".to_owned())
                && args.contains(
                    &"serviceAccount:123-compute@developer.gserviceaccount.com".to_owned(),
                )
                && args.contains(&"roles/secretmanager.secretAccessor".to_owned())
        })
        .returning(|_| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let result = client
        .revoke_secret_access(
            "proj",
            "MY_SECRET",
            "123-compute@developer.gserviceaccount.com",
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn revoke_secret_access_failure() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| args.contains(&"remove-iam-policy-binding".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "not found".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client
        .revoke_secret_access("proj", "SECRET", "sa@example.com")
        .await;

    assert!(matches!(result, Err(SecretError::RevokeAccess { .. })));
}

#[tokio::test]
async fn delete_secret_success() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"secrets".to_owned())
                && args.contains(&"delete".to_owned())
                && args.contains(&"MY_SECRET".to_owned())
                && args.contains(&"--quiet".to_owned())
        })
        .returning(|_| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let result = client.delete_secret("proj", "MY_SECRET").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn delete_secret_failure() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| args.contains(&"delete".to_owned()) && args.contains(&"secrets".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "NOT_FOUND".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client.delete_secret("proj", "GONE").await;

    assert!(matches!(result, Err(SecretError::Delete { .. })));
}

// ── WIF Pool Tests ──

#[tokio::test]
async fn ensure_wif_pool_creates_new() {
    let mut mock = MockExecutor::new();

    // create succeeds
    mock.expect_exec()
        .withf(|args| {
            args.contains(&"workload-identity-pools".to_owned())
                && args.contains(&"create".to_owned())
                && args.contains(&"propel-github".to_owned())
                && args.contains(&"global".to_owned())
        })
        .returning(|_| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let created = client
        .ensure_wif_pool("proj", "propel-github")
        .await
        .unwrap();

    assert!(created);
}

#[tokio::test]
async fn ensure_wif_pool_already_exists() {
    let mut mock = MockExecutor::new();

    // create fails with ALREADY_EXISTS
    mock.expect_exec()
        .withf(|args| {
            args.contains(&"workload-identity-pools".to_owned())
                && args.contains(&"create".to_owned())
        })
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "ALREADY_EXISTS: resource already exists".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let created = client
        .ensure_wif_pool("proj", "propel-github")
        .await
        .unwrap();

    assert!(!created);
}

#[tokio::test]
async fn ensure_wif_pool_create_fails() {
    let mut mock = MockExecutor::new();

    // create fails with non-ALREADY_EXISTS error
    mock.expect_exec()
        .withf(|args| args.contains(&"create".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "permission denied".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client.ensure_wif_pool("proj", "propel-github").await;

    assert!(matches!(result, Err(WifError::CreatePool { .. })));
}

// ── OIDC Provider Tests ──

#[tokio::test]
async fn ensure_oidc_provider_creates_new() {
    let mut mock = MockExecutor::new();

    // create-oidc succeeds
    mock.expect_exec()
        .withf(|args| {
            args.contains(&"create-oidc".to_owned())
                && args.contains(&"github".to_owned())
                && args
                    .iter()
                    .any(|a| a.contains("token.actions.githubusercontent.com"))
                && args
                    .iter()
                    .any(|a| a.contains("assertion.repository == 'owner/repo'"))
        })
        .returning(|_| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let created = client
        .ensure_oidc_provider("proj", "propel-github", "github", "owner/repo")
        .await
        .unwrap();

    assert!(created);
}

#[tokio::test]
async fn ensure_oidc_provider_already_exists() {
    let mut mock = MockExecutor::new();

    // create-oidc fails with ALREADY_EXISTS
    mock.expect_exec()
        .withf(|args| args.contains(&"create-oidc".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "ALREADY_EXISTS: provider already exists".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let created = client
        .ensure_oidc_provider("proj", "propel-github", "github", "owner/repo")
        .await
        .unwrap();

    assert!(!created);
}

// ── Service Account Tests ──

#[tokio::test]
async fn ensure_service_account_creates_new() {
    let mut mock = MockExecutor::new();

    // create succeeds
    mock.expect_exec()
        .withf(|args| {
            args.contains(&"service-accounts".to_owned())
                && args.contains(&"create".to_owned())
                && args.contains(&"propel-deploy".to_owned())
        })
        .returning(|_| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let created = client
        .ensure_service_account("proj", "propel-deploy", "Propel CI Deploy")
        .await
        .unwrap();

    assert!(created);
}

#[tokio::test]
async fn ensure_service_account_already_exists() {
    let mut mock = MockExecutor::new();

    // create fails with already exists
    mock.expect_exec()
        .withf(|args| {
            args.contains(&"service-accounts".to_owned()) && args.contains(&"create".to_owned())
        })
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "Service account already exists".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let created = client
        .ensure_service_account("proj", "propel-deploy", "Propel CI Deploy")
        .await
        .unwrap();

    assert!(!created);
}

// ── IAM Role Binding Tests ──

#[tokio::test]
async fn bind_iam_roles_success() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"add-iam-policy-binding".to_owned())
                && args.contains(&"projects".to_owned())
        })
        .times(2)
        .returning(|_| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let result = client
        .bind_iam_roles(
            "proj",
            "sa@proj.iam.gserviceaccount.com",
            &["roles/run.admin", "roles/cloudbuild.builds.editor"],
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn bind_iam_roles_partial_failure() {
    let mut mock = MockExecutor::new();

    // First role succeeds
    mock.expect_exec()
        .withf(|args| args.contains(&"roles/run.admin".to_owned()))
        .returning(|_| Ok(String::new()));

    // Second role fails
    mock.expect_exec()
        .withf(|args| args.contains(&"roles/cloudbuild.builds.editor".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "permission denied".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client
        .bind_iam_roles(
            "proj",
            "sa@proj.iam.gserviceaccount.com",
            &["roles/run.admin", "roles/cloudbuild.builds.editor"],
        )
        .await;

    assert!(matches!(
        result,
        Err(WifError::BindRole { ref role, .. }) if role == "roles/cloudbuild.builds.editor"
    ));
}

// ── WIF → SA Binding Tests ──

#[tokio::test]
async fn bind_wif_to_sa_success() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"add-iam-policy-binding".to_owned())
                && args.contains(&"roles/iam.workloadIdentityUser".to_owned())
                && args
                    .iter()
                    .any(|a| a.contains("attribute.repository/owner/repo"))
        })
        .returning(|_| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let result = client
        .bind_wif_to_sa(
            "proj",
            "123456",
            "propel-github",
            "sa@proj.iam.gserviceaccount.com",
            "owner/repo",
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn bind_wif_to_sa_failure() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| args.contains(&"add-iam-policy-binding".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "failed".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client
        .bind_wif_to_sa("proj", "123456", "pool", "sa@example.com", "owner/repo")
        .await;

    assert!(matches!(result, Err(WifError::BindWif { .. })));
}

// ── Delete WIF Pool Tests ──

#[tokio::test]
async fn delete_wif_pool_success() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"workload-identity-pools".to_owned())
                && args.contains(&"delete".to_owned())
                && args.contains(&"propel-github".to_owned())
                && args.contains(&"--quiet".to_owned())
        })
        .returning(|_| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let result = client.delete_wif_pool("proj", "propel-github").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn delete_wif_pool_failure() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"workload-identity-pools".to_owned())
                && args.contains(&"delete".to_owned())
        })
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "NOT_FOUND".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client.delete_wif_pool("proj", "propel-github").await;

    assert!(matches!(result, Err(WifError::DeletePool { .. })));
}

// ── Delete Service Account Tests ──

#[tokio::test]
async fn delete_service_account_success() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"service-accounts".to_owned())
                && args.contains(&"delete".to_owned())
                && args.contains(&"--quiet".to_owned())
        })
        .returning(|_| Ok(String::new()));

    let client = GcloudClient::with_executor(mock);
    let result = client
        .delete_service_account("proj", "sa@proj.iam.gserviceaccount.com")
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn delete_service_account_failure() {
    let mut mock = MockExecutor::new();

    mock.expect_exec()
        .withf(|args| {
            args.contains(&"service-accounts".to_owned()) && args.contains(&"delete".to_owned())
        })
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "NOT_FOUND".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client
        .delete_service_account("proj", "sa@proj.iam.gserviceaccount.com")
        .await;

    assert!(matches!(result, Err(WifError::DeleteServiceAccount { .. })));
}

// ── Logs Tests ──

#[tokio::test]
async fn read_logs_with_custom_limit() {
    let mut mock = MockExecutor::new();

    mock.expect_exec_streaming()
        .withf(|args| {
            args.contains(&"logs".to_owned())
                && args.contains(&"read".to_owned())
                && args.contains(&"50".to_owned())
        })
        .returning(|_| Ok(()));

    let client = GcloudClient::with_executor(mock);
    let result = client.read_logs("my-svc", "proj", "us-central1", 50).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn read_logs_failure() {
    let mut mock = MockExecutor::new();

    mock.expect_exec_streaming()
        .withf(|args| args.contains(&"read".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "not found".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client.read_logs("svc", "proj", "us-central1", 100).await;

    assert!(matches!(result, Err(DeployError::Logs { .. })));
}

#[tokio::test]
async fn tail_logs_success() {
    let mut mock = MockExecutor::new();

    mock.expect_exec_streaming()
        .withf(|args| {
            args.contains(&"logs".to_owned())
                && args.contains(&"tail".to_owned())
                && args.contains(&"my-svc".to_owned())
        })
        .returning(|_| Ok(()));

    let client = GcloudClient::with_executor(mock);
    let result = client.tail_logs("my-svc", "proj", "us-central1").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn tail_logs_failure() {
    let mut mock = MockExecutor::new();

    mock.expect_exec_streaming()
        .withf(|args| args.contains(&"tail".to_owned()))
        .returning(|_| {
            Err(GcloudError::CommandFailed {
                args: vec![],
                stderr: "not found".to_owned(),
            })
        });

    let client = GcloudClient::with_executor(mock);
    let result = client.tail_logs("svc", "proj", "us-central1").await;

    assert!(matches!(result, Err(DeployError::Logs { .. })));
}
