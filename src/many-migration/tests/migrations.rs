#![feature(used_with_arg)] // Required to build the test with Bazel

use linkme::distributed_slice;
use many_migration::{
    load_enable_all_regular_migrations, load_migrations, InnerMigration, Metadata, MigrationConfig,
    MigrationSet,
};
use std::collections::{BTreeMap, HashMap};

type Storage = BTreeMap<StorageKey, u64>;

#[derive(Ord, PartialOrd, Eq, PartialEq)]
enum StorageKey {
    Init = 0,
    Counter = 1,
}

#[distributed_slice]
static SOME_MANY_RS_MIGRATIONS: [InnerMigration<Storage, String>] = [..];

fn _initialize(s: &mut Storage) -> Result<(), String> {
    s.insert(StorageKey::Init, 1);
    Ok(())
}

fn _update(s: &mut Storage) -> Result<(), String> {
    if let Some(counter) = s.get_mut(&StorageKey::Counter) {
        *counter += 1;
        return Ok(());
    }
    Err("Counter entry not found".to_string())
}

fn _hotfix(data: &[u8]) -> Option<Vec<u8>> {
    let mut new_data = [0u8; 4];
    if data.len() == 8 {
        new_data.copy_from_slice(&data[0..4]);
        return Some(new_data.to_vec());
    }
    None
}

#[distributed_slice(SOME_MANY_RS_MIGRATIONS)]
static A: InnerMigration<Storage, String> =
    InnerMigration::new_initialize(_initialize, "A", "A desc");

#[distributed_slice(SOME_MANY_RS_MIGRATIONS)]
static B: InnerMigration<Storage, String> = InnerMigration::new_update(_update, "B", "B desc");

#[distributed_slice(SOME_MANY_RS_MIGRATIONS)]
static C: InnerMigration<Storage, String> =
    InnerMigration::new_initialize_update(_initialize, _update, "C", "C desc");

#[distributed_slice(SOME_MANY_RS_MIGRATIONS)]
static D: InnerMigration<Storage, String> = InnerMigration::new_hotfix(_hotfix, "D", "D desc");

#[test]
fn initialize() {
    let migrations = load_enable_all_regular_migrations(&SOME_MANY_RS_MIGRATIONS);
    assert!(migrations.contains_key("A"));

    let mut storage = Storage::new();

    // Should not run when block height == 0
    migrations["A"].initialize(&mut storage, 0).unwrap();
    assert!(storage.is_empty());

    // Migration should run when block height == 1
    migrations["A"].initialize(&mut storage, 1).unwrap();
    assert!(!storage.is_empty());
    assert_eq!(storage.len(), 1);
    assert!(storage.contains_key(&StorageKey::Init));
    assert_eq!(storage[&StorageKey::Init], 1);

    // Should not do anything after it ran once
    migrations["A"].initialize(&mut storage, 2).unwrap();
    assert_eq!(storage.len(), 1);
}

#[test]
fn update() {
    let migrations = load_enable_all_regular_migrations(&SOME_MANY_RS_MIGRATIONS);
    assert!(migrations.contains_key("B"));

    let mut storage = Storage::new();
    storage.insert(StorageKey::Counter, 0);

    // Should not run when block height == 0
    migrations["B"].update(&mut storage, 0).unwrap();
    assert_eq!(storage[&StorageKey::Counter], 0);

    // Should not run when block height == 1
    migrations["B"].update(&mut storage, 1).unwrap();
    assert_eq!(storage[&StorageKey::Counter], 0);

    // Should run when block height is > 1
    for i in 2..10 {
        migrations["B"].update(&mut storage, 2).unwrap();
        assert_eq!(storage[&StorageKey::Counter], i - 1);
    }
}

#[test]
fn initialize_update() {
    let migrations = load_enable_all_regular_migrations(&SOME_MANY_RS_MIGRATIONS);
    assert!(migrations.contains_key("C"));

    let mut storage = Storage::from_iter([(StorageKey::Counter, 0)]);

    for i in 0..4 {
        migrations["C"].initialize(&mut storage, i).unwrap();
        match i {
            0 => assert_eq!(storage.len(), 1),
            1 => {
                assert_eq!(storage.len(), 2);
                assert!(storage.contains_key(&StorageKey::Init));
                assert_eq!(storage[&StorageKey::Init], 1);
            }
            2 => assert_eq!(storage.len(), 2),
            3 => assert_eq!(storage.len(), 2),
            _ => unimplemented!(),
        }

        migrations["C"].update(&mut storage, i).unwrap();

        match i {
            0 => {
                assert_eq!(storage.len(), 1);
                assert_eq!(storage[&StorageKey::Counter], 0);
            }
            1 => {
                assert_eq!(storage.len(), 2);
                assert_eq!(storage[&StorageKey::Counter], 0);
            }
            2 => {
                assert_eq!(storage.len(), 2);
                assert_eq!(storage[&StorageKey::Counter], 1);
            }
            3 => {
                assert_eq!(storage.len(), 2);
                assert_eq!(storage[&StorageKey::Counter], 2);
            }
            _ => unimplemented!(),
        }
    }
}

#[test]
fn hotfix() {
    let content = r#"
    [
        {
            "name": "D",
            "block_height": 2
        }
    ]
    "#;
    let migrations = load_migrations(&SOME_MANY_RS_MIGRATIONS, content).unwrap();
    assert!(migrations.contains_key("D"));

    let data = [1u8; 8];
    for i in 0..4 {
        let maybe_new_data = migrations["D"].hotfix(&data, i);

        match i {
            0..=1 | 3 => assert!(maybe_new_data.is_none()),
            2 => {
                assert!(maybe_new_data.is_some());
                assert_eq!(maybe_new_data.unwrap(), vec![1, 1, 1, 1]);
            }
            _ => unimplemented!(),
        }
    }
}

#[test]
fn name() {
    let migrations = load_enable_all_regular_migrations(&SOME_MANY_RS_MIGRATIONS);
    assert!(migrations.contains_key("A"));
    assert_eq!(migrations["A"].name(), "A");
}

#[test]
fn description() {
    let migrations = load_enable_all_regular_migrations(&SOME_MANY_RS_MIGRATIONS);
    assert!(migrations.contains_key("A"));
    assert_eq!(migrations["A"].description(), "A desc");
}

#[test]
fn load_enable_all_regular_hotfix_disabled() {
    let migrations = load_enable_all_regular_migrations(&SOME_MANY_RS_MIGRATIONS);
    for k in ["A", "B", "C", "D"] {
        assert!(migrations.contains_key(k));
        match k {
            "A" | "B" | "C" => {
                assert!(migrations[k].is_enabled())
            }
            "D" => assert!(!migrations[k].is_enabled()),
            _ => unimplemented!(),
        }
    }
}

#[test]
fn metadata() {
    let migrations = load_enable_all_regular_migrations(&SOME_MANY_RS_MIGRATIONS);
    for migration in migrations.values() {
        let metadata = migration.metadata();
        assert_eq!(metadata.block_height, 1);
        assert_eq!(metadata.issue, None);
        assert_eq!(metadata.extra, HashMap::new());
    }

    let content = r#"
    [
        {
            "name": "D",
            "block_height": 200,
            "issue": "foobar",
            "xtra": "Oh!"
        }
    ]
    "#;
    let migrations = load_migrations(&SOME_MANY_RS_MIGRATIONS, content).unwrap();
    let metadata = migrations["D"].metadata();
    assert_eq!(metadata.block_height, 200);
    assert_eq!(metadata.issue, Some("foobar".to_string()));
    assert!(!metadata.extra.is_empty());
    assert_eq!(metadata.extra.len(), 1);
    assert!(metadata.extra.contains_key("xtra"));
    assert_eq!(metadata.extra["xtra"], "Oh!");
}

#[test]
fn status() {
    let migrations = load_enable_all_regular_migrations(&SOME_MANY_RS_MIGRATIONS);
    for i in ["A", "B", "C", "D"] {
        let migration = &migrations[i];
        let status = migration.is_enabled();

        match i {
            "A" | "B" | "C" => assert_eq!(status, true),
            "D" => assert_eq!(status, false),
            _ => unimplemented!(),
        }
    }
}

#[test]
fn empty_config() {
    let migration_set =
        MigrationSet::load(&SOME_MANY_RS_MIGRATIONS, MigrationConfig::default(), 1).unwrap();
    assert_eq!(migration_set.values().count(), 0);
}

#[test]
fn basic() {
    let mut migration_set = MigrationSet::load(
        &SOME_MANY_RS_MIGRATIONS,
        [
            (&A, Metadata::enabled(1)),
            (&B, Metadata::enabled(2)),
            (&C, Metadata::disabled(1)),
        ]
        .into(),
        0,
    )
    .unwrap();
    assert_eq!(migration_set.values().count(), 3);
    assert_eq!(migration_set.values().filter(|x| x.is_enabled()).count(), 2);
    assert_eq!(migration_set.values().filter(|x| x.is_active()).count(), 0);

    let mut storage = Storage::new();
    storage.insert(StorageKey::Counter, 0);

    migration_set.update_at_height(&mut storage, 1).unwrap();
    assert_eq!(migration_set.values().count(), 3);
    assert_eq!(migration_set.values().filter(|x| x.is_enabled()).count(), 2);
    assert_eq!(migration_set.values().filter(|x| x.is_active()).count(), 1);

    migration_set.update_at_height(&mut storage, 2).unwrap();
    assert_eq!(migration_set.values().count(), 3);
    assert_eq!(migration_set.values().filter(|x| x.is_enabled()).count(), 2);
    assert_eq!(migration_set.values().filter(|x| x.is_active()).count(), 2);

    migration_set.update_at_height(&mut storage, 3).unwrap();
    assert_eq!(migration_set.values().count(), 3);
    assert_eq!(migration_set.values().filter(|x| x.is_enabled()).count(), 2);
    assert_eq!(migration_set.values().filter(|x| x.is_active()).count(), 2);
}
