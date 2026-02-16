//! Dockerfile generation, source bundling, and eject for propel.
//!
//! # Deploy pipeline
//!
//! ```text
//! propel deploy
//!   1. Dirty check ── git status --porcelain (skip with --allow-dirty)
//!   2. Bundle      ── git ls-files → .propel-bundle/
//!   3. Dockerfile   ── DockerfileGenerator::render()
//!   4. Cloud Build  ── gcloud builds submit .propel-bundle/
//!   5. Cloud Run    ── gcloud run deploy
//! ```
//!
//! # Bundle strategy
//!
//! The bundle mirrors the git repository state:
//! - All tracked and untracked (non-ignored) files via `git ls-files`
//! - `.gitignore`d paths are excluded automatically
//! - `.propel-bundle/`, `.propel/`, `.git/` are always excluded
//!
//! # Runtime content
//!
//! The generated Dockerfile's runtime stage varies based on `[build.include]`:
//! - **Omitted**: `COPY . .` — full bundle goes into the container
//! - **Specified**: individual `COPY` per path — selective runtime content

pub mod bundle;
pub mod dockerfile;
pub mod eject;

pub use dockerfile::DockerfileGenerator;
