#![feature(const_mut_refs)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fmt::Formatter;
use std::ops::Index;
use strum::Display;
use tracing::trace;

pub type FnPtr<T, E> = fn(&mut T) -> Result<(), E>;
pub type FnByte = fn(&[u8]) -> Option<Vec<u8>>;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Metadata {
    pub block_height: u64,

    #[serde(default)]
    pub disabled: bool,

    pub issue: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Metadata {
    pub fn enabled(block_height: u64) -> Self {
        Self {
            block_height,
            disabled: false,
            issue: None,
            extra: Default::default(),
        }
    }

    pub fn disabled(block_height: u64) -> Self {
        Self {
            block_height,
            disabled: true,
            issue: None,
            extra: Default::default(),
        }
    }
}

#[derive(Copy, Clone, Display)]
#[non_exhaustive]
pub enum MigrationType<T, E> {
    Regular(RegularMigration<T, E>),
    Hotfix(HotfixMigration),

    #[non_exhaustive]
    _Unreachable,
}

impl<T, E> fmt::Debug for MigrationType<T, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> std::fmt::Result {
        formatter.write_str(&format!("{self}"))
    }
}

#[derive(Copy, Clone)]
pub struct RegularMigration<T, E> {
    initialize_fn: FnPtr<T, E>,
    update_fn: FnPtr<T, E>,
}

#[derive(Copy, Clone)]
pub struct HotfixMigration {
    hotfix_fn: FnByte,
}

#[derive(Copy, Clone)]
pub struct InnerMigration<T, E> {
    r#type: MigrationType<T, E>,
    name: &'static str,
    description: &'static str,
}

// The Debug derive requires that _all_ parametric types also implement Debug,
// even if the sub-types don't. So we have to implement our own version.
impl<T, E> fmt::Debug for InnerMigration<T, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("InnerMigration")
            .field("type", &self.r#type)
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

impl<T, E> fmt::Display for InnerMigration<T, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_fmt(format_args!(
            "Type: \"{}\", Name: \"{}\", Description: \"{}\"",
            self.r#type, self.name, self.description
        ))
    }
}

impl<T, E> AsRef<str> for InnerMigration<T, E> {
    fn as_ref(&self) -> &str {
        self.name()
    }
}

impl<T, E> InnerMigration<T, E> {
    pub const fn new_hotfix(
        hotfix_fn: FnByte,
        name: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            r#type: MigrationType::Hotfix(HotfixMigration { hotfix_fn }),
            name,
            description,
        }
    }

    pub const fn new_initialize_update(
        initialize_fn: FnPtr<T, E>,
        update_fn: FnPtr<T, E>,
        name: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            r#type: MigrationType::Regular(RegularMigration {
                initialize_fn,
                update_fn,
            }),
            name,
            description,
        }
    }

    pub const fn new_initialize(
        initialize_fn: FnPtr<T, E>,
        name: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            r#type: MigrationType::Regular(RegularMigration {
                initialize_fn,
                update_fn: |_| Ok(()),
            }),
            name,
            description,
        }
    }

    pub const fn new_update(
        update_fn: FnPtr<T, E>,
        name: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            r#type: MigrationType::Regular(RegularMigration {
                initialize_fn: |_| Ok(()),
                update_fn,
            }),
            name,
            description,
        }
    }

    #[inline]
    pub const fn name(&self) -> &str {
        self.name
    }

    #[inline]
    pub const fn description(&self) -> &str {
        self.description
    }

    #[inline]
    pub const fn r#type(&self) -> &'_ MigrationType<T, E> {
        &self.r#type
    }

    /// This function gets executed when the storage block height == the migration block height
    fn initialize(&self, storage: &mut T) -> Result<(), E> {
        match &self.r#type {
            MigrationType::Regular(migration) => (migration.initialize_fn)(storage),
            MigrationType::Hotfix(_) => Ok(()),
            _ => {
                trace!("Skipping migration {}", self.name());
                Ok(())
            }
        }
    }

    /// This function gets executed when the storage block height >= the migration block height
    fn update(&self, storage: &mut T) -> Result<(), E> {
        match &self.r#type {
            MigrationType::Regular(migration) => (migration.update_fn)(storage),
            MigrationType::Hotfix(_) => Ok(()),
            _ => {
                trace!("Skipping migration {}; not of type `Regular`", self.name());
                Ok(())
            }
        }
    }

    /// This function gets executed when the storage block height == the migration block height
    fn hotfix<'b>(&'b self, b: &'b [u8]) -> Option<Vec<u8>> {
        match &self.r#type {
            MigrationType::Regular(_) => None,
            MigrationType::Hotfix(migration) => (migration.hotfix_fn)(b),
            _ => {
                trace!("Skipping migration {}; not of type `Hotfix`", self.name());
                None
            }
        }
    }
}

pub struct Migration<'a, T, E> {
    migration: &'a InnerMigration<T, E>,

    /// The metadata used during creation of this migration.
    metadata: Metadata,

    /// Whether the migration is enabled (will initialize, update, etc).
    enabled: bool,

    /// Whether the block height has been reached.
    active: bool,
}

// The Debug derive requires that _all_ parametric types also implement Debug,
// even if the sub-types don't. So we have to implement our own version.
impl<'a, T, E> fmt::Debug for Migration<'a, T, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Migration")
            .field("migration", &self.migration)
            .field("metadata", &self.metadata)
            .field("enabled", &self.enabled)
            .field("active", &self.active)
            .finish()
    }
}

impl<'a, T, E> fmt::Display for Migration<'a, T, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_fmt(format_args!(
            "{}, Metadata: \"{:?}\", Status: \"{}\"",
            self.migration,
            self.metadata(),
            self.is_enabled()
        ))
    }
}

impl<'a, T, E> Migration<'a, T, E> {
    fn new(migration: &'a InnerMigration<T, E>, metadata: Metadata) -> Self {
        let enabled = !metadata.disabled;
        Self {
            migration,
            metadata,
            enabled,
            active: false,
        }
    }

    /// Check the height and call the inner migration's methods.
    pub fn maybe_initialize_update_at_height(
        &mut self,
        storage: &mut T,
        block_height: u64,
    ) -> Result<(), E> {
        if self.is_enabled() {
            if block_height == self.metadata.block_height && !self.active {
                self.active = true;
                self.migration.initialize(storage)?;
            } else if block_height > self.metadata.block_height {
                self.migration.update(storage)?;
            }
        }

        // Else ignore.
        Ok(())
    }

    #[inline]
    pub fn initialize(&self, storage: &mut T, block_height: u64) -> Result<(), E> {
        if self.is_enabled() && block_height == self.metadata.block_height {
            self.migration.initialize(storage)?;
        }
        Ok(())
    }

    #[inline]
    pub fn update(&self, storage: &mut T, block_height: u64) -> Result<(), E> {
        if self.is_enabled() && block_height > self.metadata.block_height {
            self.migration.update(storage)?;
        }
        Ok(())
    }

    #[inline]
    pub fn hotfix(&self, b: &[u8], block_height: u64) -> Option<Vec<u8>> {
        if self.is_enabled() && self.metadata.block_height == block_height {
            self.migration.hotfix(b)
        } else {
            None
        }
    }

    #[inline]
    pub fn is_regular(&self) -> bool {
        matches!(self.migration.r#type, MigrationType::Regular(_))
    }

    #[inline]
    pub fn is_hotfix(&self) -> bool {
        matches!(self.migration.r#type, MigrationType::Hotfix(_))
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.migration.name()
    }

    #[inline]
    pub fn description(&self) -> &str {
        self.migration.description()
    }

    #[inline]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    #[inline]
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    #[inline]
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.active
    }
}

#[derive(Serialize, Deserialize)]
pub struct SingleMigrationConfig {
    name: String,

    #[serde(flatten)]
    metadata: Metadata,
}

impl<T, E> From<(&InnerMigration<T, E>, Metadata)> for SingleMigrationConfig {
    fn from((migration, metadata): (&InnerMigration<T, E>, Metadata)) -> Self {
        Self {
            name: migration.name.to_string(),
            metadata,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MigrationConfig(Vec<SingleMigrationConfig>);

impl MigrationConfig {
    pub fn with_migration<T, E>(self, migration: &InnerMigration<T, E>) -> Self {
        self.with_migration_opts(migration, Metadata::default())
    }

    pub fn with_migration_opts<T, E>(
        mut self,
        migration: &InnerMigration<T, E>,
        metadata: Metadata,
    ) -> Self {
        self.0.push(SingleMigrationConfig {
            name: migration.name.to_string(),
            metadata,
        });
        self
    }
}

impl<T: IntoIterator<Item = impl Into<SingleMigrationConfig>>> From<T> for MigrationConfig {
    fn from(value: T) -> Self {
        Self(value.into_iter().map(Into::into).collect())
    }
}

pub struct MigrationSet<'a, T: 'a, E: 'a = many_error::ManyError> {
    inner: BTreeMap<String, Migration<'a, T, E>>,
    height: u64,
}

impl<'a, T, E: fmt::Debug> fmt::Debug for MigrationSet<'a, T, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MigrationSet")
            .field("inner", &self.inner)
            .field("height", &self.height)
            .finish()
    }
}

impl<'a, T, E> MigrationSet<'a, T, E> {
    pub fn empty(height: u64) -> Result<Self, String> {
        Ok(Self {
            inner: Default::default(),
            height,
        })
    }

    pub fn load(
        registry: &'a [InnerMigration<T, E>],
        config: MigrationConfig,
        height: u64,
    ) -> Result<Self, String> {
        // Build a BTreeMap from the linear registry
        let registry = registry
            .iter()
            .map(|m| (m.name, m))
            .collect::<BTreeMap<&'static str, &'a InnerMigration<T, E>>>();

        let mut inner: BTreeMap<String, Migration<'a, T, E>> = config
            .0
            .into_iter()
            .map(|config: SingleMigrationConfig| {
                let v: &'a InnerMigration<T, E> = registry
                    .get(config.name.as_str())
                    .ok_or_else(|| format!("Unsupported migration '{}'", config.name))?;

                Ok((config.name, Migration::new(v, config.metadata)))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?
            .into_iter()
            .collect();

        // Activate all already active migrations. Do not call initialize though.
        for v in inner.values_mut().filter(|m| m.is_enabled()) {
            if height >= v.metadata.block_height {
                v.active = true;
            }
        }

        Ok(Self { inner, height })
    }

    #[inline]
    pub fn update_at_height(&mut self, storage: &mut T, block_height: u64) -> Result<(), E> {
        self.height = block_height;
        for migration in self.inner.values_mut().filter(|m| m.is_regular()) {
            migration.maybe_initialize_update_at_height(storage, block_height)?;
        }
        Ok(())
    }

    #[inline]
    pub fn hotfix(&self, name: &str, b: &[u8], block_height: u64) -> Result<Option<Vec<u8>>, E> {
        for migration in self
            .inner
            .values()
            .filter(|m| m.is_hotfix() && m.name() == name)
        {
            if let Some(r) = migration.hotfix(b, block_height) {
                return Ok(Some(r));
            }
        }
        Ok(None)
    }

    #[inline]
    pub fn is_enabled(&self, name: &str) -> bool {
        self.inner
            .get(name)
            .map(|m| m.is_enabled())
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_active(&self, name: impl AsRef<str>) -> bool {
        self.inner
            .get(name.as_ref())
            .map(|m| m.is_active())
            .unwrap_or(false)
    }
}

/// Implement necessary BTreeMap<...> methods to have the same interface for
/// existing code/tests.
/// TODO: remove these and move to new Migration-specific APIs in tests.
impl<'a, T, E> MigrationSet<'a, T, E> {
    pub fn contains_key(&self, name: impl AsRef<str>) -> bool {
        self.inner.contains_key(name.as_ref())
    }

    pub fn values(&self) -> impl Iterator<Item = &Migration<'a, T, E>> {
        self.inner.values()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<'a, T, E, IDX: AsRef<str>> Index<IDX> for MigrationSet<'a, T, E> {
    type Output = Migration<'a, T, E>;

    fn index(&self, index: IDX) -> &Self::Output {
        &self.inner[index.as_ref()]
    }
}

/// Kept for backward compatibility.
#[deprecated = "Should use MigrationSet::load() instead."]
pub fn load_migrations<'a, T, E>(
    registry: &'a [InnerMigration<T, E>],
    config: &str,
) -> Result<MigrationSet<'a, T, E>, String> {
    let config: MigrationConfig = serde_json::from_str(config).map_err(|e| e.to_string())?;
    MigrationSet::load(registry, config, 0)
}

/// Enable all migrations from the registry EXCEPT the hotfix.
/// Should not be used outside of tests.
#[deprecated = "Should use MigrationSet::load() instead."]
pub fn load_enable_all_regular_migrations<T, E>(
    registry: &[InnerMigration<T, E>],
) -> MigrationSet<T, E> {
    // Keep a default of block height 1 for backward compatibility.
    let metadata = Metadata {
        block_height: 1,
        ..Metadata::default()
    };

    let inner: BTreeMap<String, Migration<T, E>> = registry
        .iter()
        .map(|m| {
            let mut migration = Migration::new(m, metadata.clone());
            match m.r#type {
                MigrationType::Regular(_) => migration.enable(),
                MigrationType::Hotfix(_) => migration.disable(),
                _ => migration.disable(),
            }

            (m.name.to_string(), migration)
        })
        .collect();

    MigrationSet { inner, height: 0 }
}
