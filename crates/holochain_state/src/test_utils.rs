//! Helpers for unit tests

use either::Either;
use holochain_keystore::MetaLairClient;
use holochain_sqlite::prelude::*;
use holochain_sqlite::rusqlite::Statement;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::prelude::*;
use holochain_zome_types::test_utils::fake_cell_id;
use kitsune_p2p::KitsuneSpace;
use shrinkwraprs::Shrinkwrap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tempdir::TempDir;

use crate::prelude::Store;
use crate::prelude::Txn;

pub mod mutations_helpers;

/// Create a [TestEnv] of [`DbKindAuthored`], backed by a temp directory.
pub fn test_authored_env() -> TestEnv<DbKindAuthored> {
    test_authored_env_with_id(1)
}

pub fn test_authored_env_with_id(id: u8) -> TestEnv<DbKindAuthored> {
    test_env(DbKindAuthored(Arc::new(fake_dna_hash(id))))
}

pub fn test_authored_env_with_dna_hash(hash: DnaHash) -> TestEnv<DbKindAuthored> {
    test_env(DbKindAuthored(Arc::new(hash)))
}

/// Create a [TestEnv] of [`DbKindDht`], backed by a temp directory.
pub fn test_dht_env() -> TestEnv<DbKindDht> {
    test_dht_env_with_id(1)
}

pub fn test_dht_env_with_id(id: u8) -> TestEnv<DbKindDht> {
    test_env(DbKindDht(Arc::new(fake_dna_hash(id))))
}

pub fn test_dht_env_with_dna_hash(hash: DnaHash) -> TestEnv<DbKindDht> {
    test_env(DbKindDht(Arc::new(hash)))
}

/// Create a [TestEnv] of [`DbKindCache`], backed by a temp directory.
pub fn test_cache_env() -> TestEnv<DbKindCache> {
    test_cache_env_with_id(1)
}

pub fn test_cache_env_with_id(id: u8) -> TestEnv<DbKindCache> {
    test_env(DbKindCache(Arc::new(fake_cell_id(id).dna_hash().clone())))
}

pub fn test_cache_env_with_dna_hash(hash: DnaHash) -> TestEnv<DbKindCache> {
    test_env(DbKindCache(Arc::new(hash)))
}

/// Create a [TestEnv] of [DbKindConductor], backed by a temp directory.
pub fn test_conductor_env() -> TestEnv<DbKindConductor> {
    test_env(DbKindConductor)
}

/// Create a [TestEnv] of [DbKindWasm], backed by a temp directory.
pub fn test_wasm_env() -> TestEnv<DbKindWasm> {
    test_env(DbKindWasm)
}

/// Create a [TestEnv] of [DbKindP2pAgentStore], backed by a temp directory.
pub fn test_p2p_agent_store_env() -> TestEnv<DbKindP2pAgentStore> {
    test_env(DbKindP2pAgentStore(Arc::new(KitsuneSpace(vec![0; 36]))))
}

/// Create a [TestEnv] of [DbKindP2pMetrics], backed by a temp directory.
pub fn test_p2p_metrics_env() -> TestEnv<DbKindP2pMetrics> {
    test_env(DbKindP2pMetrics(Arc::new(KitsuneSpace(vec![0; 36]))))
}

fn test_env<Kind: DbKindT>(kind: Kind) -> TestEnv<Kind> {
    let tmpdir = TempDir::new("holochain-test-environments").unwrap();
    TestEnv {
        env: DbWrite::test(&tmpdir, kind).expect("Couldn't create test database"),
        tmpdir,
    }
}

/// Create a fresh set of test environments with a new TempDir and custom MetaLairClient
pub fn test_envs_with_keystore(keystore: MetaLairClient) -> TestEnvs {
    let tempdir = TempDir::new("holochain-test-environments").unwrap();
    TestEnvs::with_keystore(tempdir, keystore)
}

/// Create a fresh set of test environments with a new TempDir
pub fn test_environments() -> TestEnvs {
    let tempdir = TempDir::new("holochain-test-environments").unwrap();
    TestEnvs::new(tempdir)
}

/// Generate a test keystore pre-populated with a couple test keypairs.
pub fn test_keystore() -> holochain_keystore::MetaLairClient {
    tokio_helper::block_on(
        async move {
            let keystore = holochain_keystore::test_keystore::spawn_test_keystore()
                .await
                .unwrap();

            keystore
        },
        std::time::Duration::from_secs(1),
    )
    .expect("timeout elapsed")
}

/// A test database in a temp directory
#[derive(Shrinkwrap)]
pub struct TestEnv<Kind: DbKindT> {
    #[shrinkwrap(main_field)]
    /// sqlite database
    env: DbWrite<Kind>,
    /// temp directory for this environment
    tmpdir: TempDir,
}

impl<Kind: DbKindT> TestEnv<Kind> {
    /// Accessor
    pub fn env(&self) -> DbWrite<Kind> {
        self.env.clone()
    }

    /// Accessor
    pub fn into_tempdir(self) -> TempDir {
        self.tmpdir
    }

    /// Dump db to a location.
    pub fn dump(&self, out: &Path) -> std::io::Result<()> {
        std::fs::create_dir(&out).ok();
        for entry in std::fs::read_dir(self.tmpdir.path())? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let mut out = out.to_owned();
                out.push(format!(
                    "backup.{}",
                    path.extension().unwrap().to_string_lossy()
                ));
                std::fs::copy(path, out)?;
            }
        }
        Ok(())
    }

    /// Dump db into `/tmp/test_dbs`.
    pub fn dump_tmp(&self) {
        dump_tmp(&self.env);
    }

    pub fn dna_hash(&self) -> Option<Arc<DnaHash>> {
        match self.env.kind().kind() {
            DbKind::Authored(hash) | DbKind::Cache(hash) | DbKind::Dht(hash) => Some(hash),
            _ => None,
        }
    }
}
// /// Dump db into `/tmp/test_dbs`.
// pub fn dump_tmp(env: &DbWrite) {
//     let mut tmp = std::env::temp_dir();
//     tmp.push("test_dbs");
//     std::fs::create_dir(&tmp).ok();
//     tmp.push("backup.sqlite");
//     std::fs::write(&tmp, b"").unwrap();
//     env.conn()
//         .unwrap()
//         .transaction_with_behavior(holochain_sqlite::rusqlite::TransactionBehavior::Exclusive)
//         .unwrap()
//         .backup(DatabaseName::Main, tmp, None)
//         .unwrap();
// }
/// Dump db into `/tmp/test_dbs`.
pub fn dump_tmp<Kind: DbKindT>(env: &DbWrite<Kind>) {
    let mut tmp = std::env::temp_dir();
    tmp.push("test_dbs");
    std::fs::create_dir(&tmp).ok();
    tmp.push("backup.sqlite");
    println!("dumping db to {}", tmp.display());
    std::fs::write(&tmp, b"").unwrap();
    env.conn()
        .unwrap()
        .execute("VACUUM main into ?", [tmp.to_string_lossy()])
        // .backup(DatabaseName::Main, tmp, None)
        .unwrap();
}

/// A container for all three non-cell environments
pub struct TestEnvs {
    /// A test conductor environment
    conductor: DbWrite<DbKindConductor>,
    /// A test wasm environment
    wasm: DbWrite<DbKindWasm>,
    /// A test p2p environment
    p2p: Arc<parking_lot::Mutex<HashMap<Arc<KitsuneSpace>, DbWrite<DbKindP2pAgentStore>>>>,
    /// A test p2p environment
    p2p_metrics: Arc<parking_lot::Mutex<HashMap<Arc<KitsuneSpace>, DbWrite<DbKindP2pMetrics>>>>,
    /// The shared root temp dir for these environments
    dir: Either<TempDir, PathBuf>,
    /// The keystore sender for these environments
    keystore: MetaLairClient,
}

#[allow(missing_docs)]
impl TestEnvs {
    /// Create all three non-cell environments at once with a custom keystore
    pub fn with_keystore(tempdir: TempDir, keystore: MetaLairClient) -> Self {
        let conductor = DbWrite::test(&tempdir, DbKindConductor).unwrap();
        let wasm = DbWrite::test(&tempdir, DbKindWasm).unwrap();
        let p2p = Arc::new(parking_lot::Mutex::new(HashMap::new()));
        let p2p_metrics = Arc::new(parking_lot::Mutex::new(HashMap::new()));
        Self {
            conductor,
            wasm,
            p2p,
            p2p_metrics,
            dir: Either::Left(tempdir),
            keystore,
        }
    }

    /// Create all three non-cell environments at once with a test keystore
    pub fn new(tempdir: TempDir) -> Self {
        Self::with_keystore(tempdir, test_keystore())
    }

    pub fn conductor(&self) -> DbWrite<DbKindConductor> {
        self.conductor.clone()
    }

    pub fn wasm(&self) -> DbWrite<DbKindWasm> {
        self.wasm.clone()
    }

    pub fn p2p(
        &self,
    ) -> Arc<parking_lot::Mutex<HashMap<Arc<KitsuneSpace>, DbWrite<DbKindP2pAgentStore>>>> {
        self.p2p.clone()
    }

    pub fn p2p_metrics(
        &self,
    ) -> Arc<parking_lot::Mutex<HashMap<Arc<KitsuneSpace>, DbWrite<DbKindP2pMetrics>>>> {
        self.p2p_metrics.clone()
    }

    /// Consume the TempDir so that it will not be cleaned up after the test is over.
    #[deprecated = "solidified() should only be used during debugging"]
    pub fn solidified(self) -> Self {
        let Self {
            conductor,
            wasm,
            p2p,
            p2p_metrics,
            dir,
            keystore,
        } = self;
        let dir = dir.left_and_then(|tempdir| {
            let pathbuf = tempdir.into_path();
            println!("Solidified TestEnvs at {:?}", pathbuf);
            Either::Right(pathbuf)
        });
        Self {
            conductor,
            wasm,
            p2p,
            p2p_metrics,
            dir,
            keystore,
        }
    }

    pub fn into_tempdir(self) -> TempDir {
        self.dir
            .expect_left("can only use into_tempdir if not already solidified")
    }

    /// Get the root path for these environments
    pub fn path(&self) -> &Path {
        match &self.dir {
            Either::Left(tempdir) => tempdir.path(),
            Either::Right(path) => path,
        }
    }

    pub fn keystore(&self) -> &MetaLairClient {
        &self.keystore
    }
}

/// Produce file and line number info at compile-time
#[macro_export]
macro_rules! here {
    ($test: expr) => {
        concat!($test, " !!!_LOOK HERE:---> ", file!(), ":", line!())
    };
}

/// Helper to get a [`Store`] from an [`DbRead`].
pub fn fresh_store_test<F, R, K>(env: &DbRead<K>, f: F) -> R
where
    F: FnOnce(&dyn Store) -> R,
    K: DbKindT,
{
    fresh_reader_test!(env, |txn| {
        let store = Txn::from(&txn);
        f(&store)
    })
}

/// Function to help avoid needing to specify types.
pub fn fresh_reader_test<E, F, R, K>(env: E, f: F) -> R
where
    E: Into<DbRead<K>>,
    F: FnOnce(Transaction) -> R,
    K: DbKindT,
{
    fresh_reader_test!(&env.into(), f)
}

/// Function to help avoid needing to specify types.
pub fn print_stmts_test<E, F, R, K>(env: E, f: F) -> R
where
    E: Into<DbRead<K>>,
    F: FnOnce(Transaction) -> R,
    K: DbKindT,
{
    holochain_sqlite::print_stmts_test!(&env.into(), f)
}

#[tracing::instrument(skip(txn))]
pub fn dump_db(txn: &Transaction) {
    let dump = |mut stmt: Statement| {
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for column in row.as_ref().column_names() {
                let row = row.get_ref_unwrap(column);
                match row {
                    holochain_sqlite::rusqlite::types::ValueRef::Null
                    | holochain_sqlite::rusqlite::types::ValueRef::Integer(_)
                    | holochain_sqlite::rusqlite::types::ValueRef::Real(_) => {
                        tracing::debug!(?column, ?row);
                    }
                    holochain_sqlite::rusqlite::types::ValueRef::Text(text) => {
                        tracing::debug!(?column, row = ?String::from_utf8_lossy(text));
                    }
                    holochain_sqlite::rusqlite::types::ValueRef::Blob(blob) => {
                        let blob = base64::encode_config(blob, base64::URL_SAFE_NO_PAD);
                        tracing::debug!("column: {:?} row:{}", column, blob);
                    }
                }
            }
        }
    };
    tracing::debug!("Headers:");
    let stmt = txn.prepare("SELECT * FROM Header").unwrap();
    dump(stmt);

    tracing::debug!("Entries:");
    let stmt = txn.prepare("SELECT * FROM Entry").unwrap();
    dump(stmt);

    tracing::debug!("DhtOps:");
    let stmt = txn.prepare("SELECT * FROM DhtOp").unwrap();
    dump(stmt);
}
