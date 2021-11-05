//! The workflow and queue consumer for sys validation

use super::*;

use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;
use holochain_sqlite::db::DbKind;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for Publish workflow
#[instrument(skip(env, conductor_handle, stop, dna_network))]
pub fn spawn_publish_dht_ops_consumer(
    env: EnvWrite,
    conductor_handle: ConductorHandle,
    mut stop: sync::broadcast::Receiver<()>,
    dna_network: Box<dyn HolochainP2pDnaT + Send + Sync>,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    // Create a trigger with an exponential back off starting at 1 minute
    // and maxing out at 5 minutes.
    // The back off is reset any time the trigger is called (when new data is committed)

    // Temporary workaround until we remove the need for an
    // cell id in the next PR.
    let cell_id = match env.kind() {
        DbKind::Cell(id) => id.clone(),
        _ => unreachable!(),
    };
    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60 * 5), true);
    let trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        let dna_network = dna_network;
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping publish_dht_ops_workflow queue consumer."
                );
                break;
            }

            #[cfg(any(test, feature = "test_utils"))]
            {
                if !conductor_handle.dev_settings().publish {
                    continue;
                }
            }

            // Run the workflow
            match publish_dht_ops_workflow(
                env.clone(),
                dna_network.as_ref(),
                &trigger_self,
                cell_id.agent_pubkey().clone(),
            )
            .await
            {
                Ok(WorkComplete::Incomplete) => trigger_self.trigger(),
                Err(err) => {
                    handle_workflow_error(
                        conductor_handle.clone(),
                        cell_id.clone(),
                        err,
                        "publish_dht_ops failure",
                    )
                    .await?
                }
                _ => (),
            };
        }
        Ok(())
    });
    (tx, handle)
}
