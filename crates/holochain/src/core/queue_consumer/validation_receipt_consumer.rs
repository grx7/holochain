//! The workflow and queue consumer for validation receipt

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::validation_receipt_workflow::validation_receipt_workflow;
use holochain_sqlite::db::DbKind;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for validation receipt workflow
#[instrument(skip(env, conductor_handle, stop, dna_network))]
pub fn spawn_validation_receipt_consumer(
    env: EnvWrite,
    conductor_handle: ConductorHandle,
    mut stop: sync::broadcast::Receiver<()>,
    dna_network: HolochainP2pDna,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    // Temporary workaround until we remove the need for an
    // cell id in the next PR.
    let cell_id = match env.kind() {
        DbKind::Cell(id) => id.clone(),
        _ => unreachable!(),
    };
    let handle = tokio::spawn(async move {
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping validation_receipt_workflow queue consumer."
                );
                break;
            }

            // Run the workflow
            match validation_receipt_workflow(env.clone(), &dna_network).await {
                Ok(WorkComplete::Incomplete) => trigger_self.trigger(),
                Err(err) => {
                    handle_workflow_error(
                        conductor_handle.clone(),
                        cell_id.clone(),
                        err,
                        "validation_receipt_workflow failure",
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
