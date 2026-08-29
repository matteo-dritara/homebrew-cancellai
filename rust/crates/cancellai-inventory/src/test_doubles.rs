//! Test-only observer wrappers that delegate to a real `System*` observer except for
//! specific overridden paths. A purely-real filesystem cannot construct "a child `read_dir`
//! actually lists, but a sub-observation of it reports `Unreadable`/`Absent`/`Unsupported`"
//! (a listing-to-observe race, or a permission change on one specific entry) without racing
//! the OS itself; these wrappers let an adversarial test inject exactly that fact for one
//! path while every other path still goes through the real filesystem, same rationale as
//! `cancellai-platform::identity`'s own synthetic mount-boundary test. Shared here (rather
//! than duplicated per test module) since both `scan.rs` and `completeness.rs` need it for
//! the E04-S03 round-1 repair's adversarial fixtures.
#![cfg(test)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cancellai_platform::{FsObserver, IdentityObservation, IdentityObserver, Observation};

pub struct OverrideFsObserver<'a> {
    inner: &'a dyn FsObserver,
    overrides: BTreeMap<PathBuf, Observation>,
}

impl<'a> OverrideFsObserver<'a> {
    pub fn new(inner: &'a dyn FsObserver) -> Self {
        Self {
            inner,
            overrides: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, path: impl Into<PathBuf>, observation: Observation) -> &mut Self {
        self.overrides.insert(path.into(), observation);
        self
    }
}

impl FsObserver for OverrideFsObserver<'_> {
    fn observe(&self, path: &Path) -> Observation {
        self.overrides
            .get(path)
            .cloned()
            .unwrap_or_else(|| self.inner.observe(path))
    }
}

pub struct OverrideIdentityObserver<'a> {
    inner: &'a dyn IdentityObserver,
    overrides: BTreeMap<PathBuf, IdentityObservation>,
}

impl<'a> OverrideIdentityObserver<'a> {
    pub fn new(inner: &'a dyn IdentityObserver) -> Self {
        Self {
            inner,
            overrides: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, path: impl Into<PathBuf>, observation: IdentityObservation) -> &mut Self {
        self.overrides.insert(path.into(), observation);
        self
    }
}

impl IdentityObserver for OverrideIdentityObserver<'_> {
    fn observe(&self, path: &Path) -> IdentityObservation {
        self.overrides
            .get(path)
            .cloned()
            .unwrap_or_else(|| self.inner.observe(path))
    }
}
