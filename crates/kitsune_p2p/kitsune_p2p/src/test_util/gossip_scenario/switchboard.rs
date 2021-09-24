use crate::gossip::sharded_gossip::{BandwidthThrottle, GossipType, ShardedGossip};
use crate::test_util::spawn_handler;
use crate::types::gossip::*;
use crate::types::wire;
use futures::stream::StreamExt;
use kitsune_p2p_types::bin_types::*;
use kitsune_p2p_types::codec::Codec;
use kitsune_p2p_types::metrics::metric_task;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use std::sync::Arc;
use tokio::task::JoinHandle;

use super::gossip_scenario_node::GossipScenarioNode;

type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KOpHash = Arc<KitsuneOpHash>;

/// An channel-based implementation of networking for tests, where messages are
/// simply routed in-memory
pub struct SwitchboardNetwork {
    metric_tasks: Vec<JoinHandle<KitsuneResult<()>>>,
    handler_tasks: Vec<JoinHandle<ghost_actor::GhostResult<()>>>,
}

impl SwitchboardNetwork {
    pub fn new() -> Self {
        Self {
            metric_tasks: Default::default(),
            handler_tasks: Default::default(),
        }
    }

    /// Set up a channel for a new node
    pub async fn add_endpoint(
        &mut self,
        mem_config: MemConfig,
        agents: &[i8],
    ) -> Tx2EpHnd<wire::Wire> {
        let f = tx2_mem_adapter(mem_config).await.unwrap();
        let f = tx2_pool_promote(f, Default::default());
        let f = tx2_api(f, Default::default());

        let mut ep = f
            .bind("none:", KitsuneTimeout::from_millis(5000))
            .await
            .unwrap();
        let ep_hnd = ep.handle().clone();

        let tuning_params = Arc::new(Default::default());

        // TODO: randomize space
        let space = Arc::new(KitsuneSpace::new([0; 36].to_vec()));
        let evt_handler = GossipScenarioNode::new(space.clone());
        let (evt_sender, task) = spawn_handler(evt_handler).await;

        self.handler_tasks.push(task);

        // TODO: generalize
        let gossip_type = GossipType::Historical;

        let bandwidth = Arc::new(BandwidthThrottle::new(1000.0, 1000.0));

        let gossip = ShardedGossip::new(
            tuning_params,
            space,
            ep_hnd.clone(),
            evt_sender,
            gossip_type,
            bandwidth,
        );

        gossip.local_agent_join(Arc::new(KitsuneAgent::new([0; 36].to_vec())));

        // TODO also call new_integrated_data when injecting ops

        self.metric_tasks.push(metric_task(async move {
            while let Some(evt) = ep.next().await {
                match evt {
                    // what other messages do i need to handle?
                    Tx2EpEvent::IncomingNotify(Tx2EpIncomingNotify { con, url, data, .. }) => {
                        match data {
                            wire::Wire::Gossip(wire::Gossip {
                                space,
                                data,
                                module,
                            }) => {
                                let data: Vec<u8> = data.into();
                                let data: Box<[u8]> = data.into_boxed_slice();

                                gossip.incoming_gossip(con, url, data).unwrap()
                            }
                            _ => unimplemented!(),
                        }
                    }
                    _ => unimplemented!(),
                }
            }
            Ok(())
        }));

        ep_hnd
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn smoke() {
    let mut sb = SwitchboardNetwork::new();

    let ep1 = sb.add_endpoint(Default::default(), &[0]).await;
    let ep2 = sb.add_endpoint(Default::default(), &[0]).await;
    let ep3 = sb.add_endpoint(Default::default(), &[0]).await;

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
}
