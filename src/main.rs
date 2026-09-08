use num_bigint::BigUint;
use num_traits::{ToPrimitive, One};
use rayon::prelude::*;
use std::time::Instant;
use futures_util::StreamExt;
use alloy::{
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    network::{EthereumWallet, Ethereum},
    primitives::{address, Address, U256},
    transports::http::Http,
    sol,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Peak,
    Bottom,
    Stable,
}

struct SystemState {
    value: f64,
    timestamp: Instant,
}

pub struct MachineMetric {
    last_state: Option<SystemState>,
}

impl MachineMetric {
    pub fn new() -> Self {
        MachineMetric { last_state: None }
    }

    pub fn update_and_predict(&mut self, current_value: f64) -> (Direction, f64) {
        let now = Instant::now();

        if let Some(ref prev) = self.last_state {
            let delta_t = now.duration_since(prev.timestamp).as_secs_f64();

            if delta_t > 0.0 {
                let v_inst = (current_value - prev.value) / delta_t;

                let direction = if v_inst > 1e-9 {
                    Direction::Peak
                } else if v_inst < -1e-9 {
                    Direction::Bottom
                } else {
                    Direction::Stable
                };

                self.last_state = Some(SystemState { value: current_value, timestamp: now });
                return (direction, v_inst);
            }
        }

        self.last_state = Some(SystemState { value: current_value, timestamp: now });
        (Direction::Stable, 0.0)
    }
}

#[derive(Clone, Debug)]
pub struct QuantumNode {
    pub id: usize,
    pub energy_scale: BigUint,
    pub frequency: f64,
}

pub struct CausalCollapseSystem {
    pub nodes: Vec<QuantumNode>,
    pub threshold_limit: f64,
    pub buffer_capacity: usize,
}

impl CausalCollapseSystem {
    pub fn new(nodes: Vec<QuantumNode>) -> Self {
        Self {
            nodes,
            threshold_limit: 0.02,
            buffer_capacity: 16,
        }
    }

    fn project_to_inverse_dimensional_symmetry(&self, raw_value: f64, index: usize) -> f64 {
        let dimension_factor = (index as f64 + 1.0).ln();
        let high_dimensional_shadow = (raw_value * dimension_factor).sin();
        high_dimensional_shadow.abs()
    }

    pub fn execute_collapse(&self) -> (Vec<Address>, Vec<Vec<u8>>) {
        if self.nodes.is_empty() { return (vec![], vec![]); }

        let mut ordered_nodes = self.nodes.clone();
        ordered_nodes.sort_by(|a, b| b.energy_scale.cmp(&a.energy_scale));

        let active_nodes: Vec<QuantumNode> = ordered_nodes.par_iter().map(|node| {
            let mut triggered = node.clone();
            if triggered.frequency == 0.0 {
                triggered.frequency = 0.01;
            }
            triggered
        }).collect();

        let mut final_path = Vec::new();
        let mut skipped_buffer: Vec<&QuantumNode> = Vec::with_capacity(self.buffer_capacity);

        final_path.push(active_nodes[0].id);

        let mut cumulative_frequency = active_nodes[0].frequency;
        let mut active_count = 1.0;

        for i in 1..active_nodes.len() {
            let next = &active_nodes[i];
            let current_avg_freq = cumulative_frequency / active_count;
            let pure_dev = (current_avg_freq - next.frequency).abs();

            if pure_dev > self.threshold_limit {
                if pure_dev > self.threshold_limit * 3.0 {
                    if skipped_buffer.len() < self.buffer_capacity {
                        skipped_buffer.push(next);
                    }
                    continue;
                }

                let stable_projected = self.project_to_inverse_dimensional_symmetry(current_avg_freq, i - 1);
                let next_projected = self.project_to_inverse_dimensional_symmetry(next.frequency, i);
                let projected_dev = (stable_projected - next_projected).abs();

                if projected_dev > self.threshold_limit {
                    if skipped_buffer.len() < self.buffer_capacity {
                        skipped_buffer.push(next);
                    }
                    continue;
                }
            }

            let scale_factor = 1.0 / (next.energy_scale.to_f64().unwrap_or(1.0) + 1.0);
            let combined_resonance = pure_dev * scale_factor;

            if combined_resonance <= self.threshold_limit {
                final_path.push(next.id);
                cumulative_frequency += next.frequency;
                active_count += 1.0;
            } else {
                if skipped_buffer.len() < self.buffer_capacity {
                    skipped_buffer.push(next);
                }
            }
        }

        let final_avg_freq = cumulative_frequency / active_count;

        for buffered_node in skipped_buffer {
            let pure_raw_dev = (final_avg_freq - buffered_node.frequency).abs();

            if pure_raw_dev > self.threshold_limit * 1.5 {
                continue;
            }

            let scale_factor = 1.0 / (buffered_node.energy_scale.to_f64().unwrap_or(1.0) + 1.0);
            if pure_raw_dev * scale_factor <= self.threshold_limit {
                final_path.push(buffered_node.id);
            }
        }

        let whitelist_pools = vec![
            address!("cf77A3bA9Aab7D3E44917635033322DF3f564171"),
            address!("2626664c2603336E57B271c5C0b26F421741e481"),
            address!("198FEe7650eAC16286848227e24eC0DFA5e51DA5"),
            address!("327Df1e6de05895D2Ab08513aADD931325260A99"),
            address!("089A8e0F6fCE8e00138F9b6E7Ff5B2FCC4Ac9D94"),
            address!("1b81D678ffb9C0263b24A97847620C99d213eB14"),
        ];

        let mut addresses = Vec::new();
        let mut payloads = Vec::new();

        for (idx, id) in final_path.iter().enumerate() {
            let pool = whitelist_pools[idx % whitelist_pools.len()];
            addresses.push(pool);
            payloads.push(vec![(*id as u8), 0x01, 0x02]);
        }

        (addresses, payloads)
    }
}

pub fn generate_astronomical_number(zeros: usize) -> BigUint {
    let mut num_str = "1".to_string();
    for _ in 0..zeros {
        num_str.push('0');
    }
    BigUint::parse_bytes(num_str.as_bytes(), 10).unwrap_or_else(BigUint::one)
}

sol! {
    #[sol(rpc)]
    contract BaseAtomicArbitrage {
        function triggerBalancerArbitrage(address tokenToBorrow, uint256 loanAmount, bytes calldata swapPathData) external;
        function triggerAaveArbitrage(address tokenToBorrow, uint256 loanAmount, bytes calldata swapPathData) external;
    }
}

async fn trigger_on_chain_arbitrage<P>(
    http_provider: P,
    contract_address: Address,
    target_path: (Vec<Address>, Vec<Vec<u8>>),
    signer_address: Address
) -> Result<(), Box<dyn std::error::Error>>
where
    P: Provider<Http<alloy::transports::http::Client>, Ethereum> + Clone,
{
    println!("🚀 [BOT -> CONTRACT] Executing Atomic Multi-Swap Command!");
    println!("🔗 Atomic Route Dispatched: Targets: {:?}, Payloads Count: {}", target_path.0, target_path.1.len());

    if target_path.0.is_empty() { return Ok(()); }

    let swap_path_data = alloy::dyn_abi::DynSolValue::Tuple(vec![
        alloy::dyn_abi::DynSolValue::Array(target_path.0.into_iter().map(alloy::dyn_abi::DynSolValue::Address).collect()),
        alloy::dyn_abi::DynSolValue::Array(target_path.1.into_iter().map(|p| alloy::dyn_abi::DynSolValue::Bytes(p.into())).collect()),
    ]).abi_encode();

    let contract = BaseAtomicArbitrage::new(contract_address, http_provider.clone());
    let token_to_borrow = address!("4200000000000000000000000000000000000006"); 
    let loan_amount = U256::from(1000000000000000000u64); 

    let tx_builder = contract.triggerBalancerArbitrage(token_to_borrow, loan_amount, swap_path_data.into())
        .from(signer_address);

    println!("🧪 Running Simulation Call via HTTP Provider for wallet: {:?}", signer_address);
    match tx_builder.call().await {
        Ok(_simulation_result) => {
            println!("✅ Simulation Passed Successfully! Sending Real Transaction...");
            let pending_tx = tx_builder.send().await?;
            println!("⏳ Transaction Sent! TX Hash: {:?}", pending_tx.tx_hash());
            let receipt = pending_tx.get_receipt().await?;
            println!("✅ Transaction Mined In Block: {:?}", receipt.block_number);
        }
        Err(e) => {
            println!("❌ Simulation Failed: {:?}. Aborting transaction to save gas.", e);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Initializing Predictive MEV Bot Core via Alloy...");

    let contract_addr_str = std::env::var("CONTRACT_ADDR")
        .unwrap_or_else(|_| "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string());
    let contract_address: Address = contract_addr_str.parse()?;

    let alchemy_http_url = std::env::var("ALCHEMY_HTTP_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());
    let alchemy_wss_url = std::env::var("ALCHEMY_WSS_URL")
        .unwrap_or_else(|_| "ws://127.0.0.1:8545".to_string());
    let private_key_str = std::env::var("PRIVATE_KEY")
        .unwrap_or_else(|_| "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string());

    let signer: PrivateKeySigner = private_key_str.parse()?;
    let signer_address = signer.address();
    let wallet = EthereumWallet::from(signer);

    println!("📡 Activating HTTP Connection to: {}", alchemy_http_url);
    let http_provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet.clone())
        .on_http(alchemy_http_url.parse()?);

    println!("📡 Activating WebSocket Connection to: {}", alchemy_wss_url);
    let ws = alloy::providers::WsConnect::new(alchemy_wss_url);
    let ws_provider = ProviderBuilder::new()
        .wallet(wallet)
        .on_ws(ws)
        .await?;

    let sub = ws_provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();

    let mut radar = MachineMetric::new();
    let mut block_counter = 0u64;

    while let Some(block) = stream.next().await {
        block_counter += 1;
        let block_num = block.header.number;
        println!("📦 Live WSS Block Synced: #{} (Internal counter: {})", block_num.unwrap_or(0), block_counter);

        let simulated_market_price = 1.005 - (block_counter % 3) as f64 * 0.01;
        let (direction, velocity) = radar.update_and_predict(simulated_market_price);

        if direction == Direction::Peak || direction == Direction::Bottom {
            println!("⚡ [RADAR ALERT] Velocity Pivot Discovered: {:.4}", velocity);
            let nodes = vec![
                QuantumNode { id: 1, energy_scale: generate_astronomical_number(1000usize), frequency: simulated_market_price },
                QuantumNode { id: 2, energy_scale: generate_astronomical_number(1000usize), frequency: 0.01 },
                QuantumNode { id: 3, energy_scale: generate_astronomical_number(1000usize), frequency: 0.015 },
            ];
            let system = CausalCollapseSystem::new(nodes);
            let optimized_path = system.execute_collapse();

            if let Err(e) = trigger_on_chain_arbitrage(http_provider.clone(), contract_address, optimized_path, signer_address).await {
                println!("❌ Error executing on-chain command: {:?}", e);
            }
        }
    }

    println!("🏁 Live stream simulation logs generated completely.");
    Ok(())
}
