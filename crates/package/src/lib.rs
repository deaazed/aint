//! AINT's package manifest, lockfile, and local dependency resolution
//! (milestone 23). Covers the manifest/lockfile bookkeeping layer only
//! — nothing in AINT's compiler pipeline can yet *consume* a resolved
//! dependency's source from inside a `.an` file, since the language
//! has no multi-file/module compilation model at all yet (every
//! `import` today resolves to one of six fixed stdlib names, not a
//! user path). See `docs/milestones/23-package-manager/SPEC.md` for
//! exactly what that means and why it's a separate, named
//! prerequisite rather than something rushed in here.
//!
//! Only `path` dependencies exist — there is no registry to resolve a
//! bare name or version range against, and none is built here either
//! (see SPEC.md).

mod git;
mod lockfile;
mod manifest;
mod resolve;

pub use lockfile::{GitSource, LockedPackage, Lockfile, LockfileError, LOCKFILE_FILE_NAME};
pub use manifest::{Dependency, Manifest, ManifestError, PackageMetadata, MANIFEST_FILE_NAME};
pub use resolve::{materialize_git, resolve, ResolveError};
