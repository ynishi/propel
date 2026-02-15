# GCP Setup Guide

Propel requires a Google Cloud Platform project with specific APIs enabled.
This guide walks through the complete setup from scratch.

## Prerequisites

- [Google Cloud SDK (gcloud CLI)](https://cloud.google.com/sdk/docs/install) installed
- A Google account with billing enabled

## 1. Authenticate

```bash
gcloud auth login
```

## 2. Create a gcloud Configuration (recommended)

Isolate propel settings from your other GCP work:

```bash
gcloud config configurations create your-project-id \
  --account=your-email@example.com
```

This automatically activates the new configuration. To switch back later:

```bash
gcloud config configurations activate your-project-id  # switch to propel
gcloud config configurations activate default             # switch back
```

## 3. Create a GCP Project

```bash
gcloud projects create your-project-id --name="your-project-id"
```

> Project IDs are globally unique. Append a date suffix or random string.

Set it as default:

```bash
gcloud config set project your-project-id
```

## 4. Link a Billing Account

List available billing accounts:

```bash
gcloud billing accounts list
```

Link to your project:

```bash
gcloud billing projects link your-project-id \
  --billing-account=your-billing-account-id
```

## 5. Enable Required APIs

```bash
gcloud services enable \
  cloudbuild.googleapis.com \
  run.googleapis.com \
  secretmanager.googleapis.com \
  artifactregistry.googleapis.com \
  cloudresourcemanager.googleapis.com \
  --project your-project-id
```

| API | Purpose |
|-----|---------|
| `cloudbuild.googleapis.com` | Remote Docker builds |
| `run.googleapis.com` | Container deployment |
| `secretmanager.googleapis.com` | Secret storage (Supabase keys, etc.) |
| `artifactregistry.googleapis.com` | Container image registry |
| `cloudresourcemanager.googleapis.com` | Project metadata access |

## 6. Set Default Region

```bash
gcloud config set compute/region asia-northeast1
```

Common regions:

| Region | Location |
|--------|----------|
| `us-central1` | Iowa, US |
| `asia-northeast1` | Tokyo, Japan |
| `europe-west1` | Belgium, EU |

## 7. Configure propel.toml

In your project directory:

```toml
[project]
gcp_project_id = "your-project-id"
region = "asia-northeast1"

[build]
# extra_packages = ["libssl-dev"]

[cloud_run]
memory = "512Mi"
cpu = 1
max_instances = 10
```

## 8. Verify Setup

```bash
propel doctor
```

Expected output:

```
Propel Doctor
------------------------------
gcloud CLI        ✓  495.0.0
Authentication    ✓  your-email@example.com
GCP Project       ✓  your-project-id
Billing           ✓  Enabled
Cloud Build API   ✓  Enabled
Cloud Run API     ✓  Enabled
Secret Manager    ✓  Enabled
Artifact Registry ✓  Enabled
propel.toml       ✓  Found
------------------------------
All checks passed!
```

## 9. Set Supabase Secrets

```bash
propel secret set SUPABASE_URL=https://your-project.supabase.co
propel secret set SUPABASE_ANON_KEY=your-anon-key
propel secret set SUPABASE_JWT_SECRET=your-jwt-secret
```

## 10. Deploy

```bash
propel deploy
```

## Cleanup

To delete the test project when done:

```bash
gcloud projects delete your-project-id
gcloud config configurations delete your-project-id
```

## Troubleshooting

### "Billing account not found"

Billing must be enabled before APIs can be activated.

```bash
gcloud billing accounts list
gcloud billing projects link your-project-id --billing-account=your-billing-account-id
```

### "Permission denied"

Ensure your account has Owner or Editor role on the project:

```bash
gcloud projects get-iam-policy your-project-id \
  --flatten="bindings[].members" \
  --filter="bindings.members:your-email@example.com"
```

### "API not enabled"

```bash
gcloud services enable your-api-name --project your-project-id
```

Or run `propel doctor` to see which APIs are missing.
