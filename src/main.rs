use alloy::{
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    network::{EthereumWallet, Ethereum},
    primitives::{address, Address, U256, Bytes},
    sol,
};
use futures_util::StreamExt;
use tokio::time::{sleep, Duration};
use std::str::FromStr;
use std::collections::HashMap;

pub const WETH_BASE: Address = address!("4200000000000000000000000000000000000006");
pub const USDC_BASE: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bda02913");

pub const BALANCER_V3_BATCH_ROUTER: Address = address!("0x136f1EFcC3f8f88516B9E94110D56FDBfB1778d1");

sol! {
    #[sol(rpc)]
    contract BaseAtomicArbitrage {
        struct SwapPathStep {
            address pool;
            address tokenOut;
            bytes userData;
        }
        struct SwapPathExactAmountIn {
            address tokenIn;
            uint256 exactAmountIn;
            SwapPathStep[] steps;
        }

        function executeBatchRouterArbitrage(
            address baseToken,
            uint256 initialAmountIn,
            uint256 minProfit,
            SwapPathExactAmountIn[] calldata paths
        ) external;

        error ArbitrageUnprofitable(uint256 required, uint256 actual);
    }

    #[sol(rpc)]
    contract IBalancerV3BatchRouter {
        struct SwapPathStep {
            address pool;
            address tokenOut;
            bytes userData;
        }
        struct SwapPathExactAmountIn {
            address tokenIn;
            uint256 exactAmountIn;
            SwapPathStep[] steps;
        }
        function querySwapExactIn(SwapPathExactAmountIn[] calldata paths) external view returns (uint256 totalAmountOut);
    }
}

#[derive(Debug, Clone)]
pub struct Pool {
    pub id: Address,
    pub token0: Address,
    pub token1: Address,
    pub fee: u32,
}

#[derive(Debug, Clone)]
pub struct DirectedEdge {
    pub pool: Pool,
    pub from_token: Address,
    pub to_token: Address,
}

#[derive(Default, Clone)]
pub struct ArbitrageGraph {
    pub adjacency_list: HashMap<Address, Vec<DirectedEdge>>,
}

impl ArbitrageGraph {
    pub fn new() -> Self {
        Self { adjacency_list: HashMap::new() }
    }

    pub fn add_pool(&mut self, pool: Pool) {
        self.adjacency_list.entry(pool.token0).or_default().push(DirectedEdge {
            pool: pool.clone(),
            from_token: pool.token0,
            to_token: pool.token1,
        });
        self.adjacency_list.entry(pool.token1).or_default().push(DirectedEdge {
            pool: pool.clone(),
            from_token: pool.token1,
            to_token: pool.token0,
        });
    }

    pub fn find_cycles(&self, start_token: Address, max_hops: usize) -> Vec<Vec<DirectedEdge>> {
        let mut cycles = Vec::new();
        let mut current_path = Vec::new();
        self.dfs_search(start_token, start_token, max_hops, &mut current_path, &mut cycles);
        cycles
    }

    fn dfs_search(
        &self,
        start_token: Address,
        current_token: Address,
        max_hops: usize,
        current_path: &mut Vec<DirectedEdge>,
        cycles: &mut Vec<Vec<DirectedEdge>>,
    ) {
        if current_path.len() > max_hops { return; }
        if current_token == start_token && !current_path.is_empty() {
            if current_path.len() >= 2 && current_path.len() <= 3 {
                let mut pool_ids = Vec::new();
                for edge in current_path.iter() { pool_ids.push(edge.pool.id); }
                pool_ids.sort();
                pool_ids.dedup();
                if pool_ids.len() == current_path.len() {
                    cycles.push(current_path.clone());
                }
            }
            return;
        }
        if current_path.len() == max_hops { return; }
        if let Some(edges) = self.adjacency_list.get(&current_token) {
            for edge in edges {
                if current_path.iter().any(|e| e.pool.id == edge.pool.id) { continue; }
                current_path.push(edge.clone());
                self.dfs_search(start_token, edge.to_token, max_hops, current_path, cycles);
                current_path.pop();
            }
        }
    }
}

pub async fn load_global_liquidity_graph() -> ArbitrageGraph {
    let mut graph = ArbitrageGraph::new();
    let client = reqwest::Client::new();
    
    let query = serde_json::json!({
        "query": "{ poolGetPools(where: { chainIn: [BASE], minTvl: 1000 }) { pools { address poolTokens { address decimals } } } }"
    });

    if let Ok(res) = client.post("https://balancer.fi").json(&query).send().await {
        if let Ok(json) = res.json::<serde_json::Value>().await {
            if let Some(pools) = json["data"]["poolGetPools"]["pools"].as_array() {
                for p in pools {
                    if let (Some(addr_str), Some(tokens)) = (p["address"].as_str(), p["poolTokens"].as_array()) {
                        if let Ok(pool_addr) = addr_str.trim().parse::<Address>() {
                            if tokens.len() >= 2 {
                                if let (Some(t0_str), Some(t1_str)) = (tokens[0]["address"].as_str(), tokens[1]["address"].as_str()) {
                                    if let (Ok(t0), Ok(t1)) = (t0_str.trim().parse::<Address>(), t1_str.trim().parse::<Address>()) {
                                        graph.add_pool(Pool {
                                            id: pool_addr,
                                            token0: t0,
                                            token1: t1,
                                            fee: 300,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if graph.adjacency_list.is_empty() {
        graph.add_pool(Pool { id: address!("cdac0d6c6c59727a65f871236188350531885c43"), token0: WETH_BASE, token1: USDC_BASE, fee: 500 });
    }

    graph
}

async fn simulate_and_diagnose_arbitrage<P: Provider<Ethereum> + Clone + 'static>(
    provider: P,
    contract_address: Address,
    signer_address: Address,
    base_token: Address,
    initial_amount_in: U256,
    min_profit: U256,
    paths: Vec<BaseAtomicArbitrage::SwapPathExactAmountIn>,
) -> eyre::Result<bool> {
    let contract_instance = BaseAtomicArbitrage::new(contract_address, provider);
    let call_builder = contract_instance.executeBatchRouterArbitrage(base_token, initial_amount_in, min_profit, paths).from(signer_address);

    match call_builder.call().await {
        Ok(_) => {
            println!("✅ [Local Simulation] PASSED successfully!");
            Ok(true)
        }
        Err(e) => {
            println!("❌ [Simulation Raw Error Details]: {:?}", e);
            Ok(false)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Initializing Balancer V3 BatchRouter MEV Bot...");

    let contract_addr_str = std::env::var("CONTRACT_ADDR").unwrap_or_else(|_| "0x23f6045a9171a18179784974c3e87c649a02c633".to_string());
    let contract_address: Address = contract_addr_str.trim().parse()?;

    let primary_http_url = std::env::var("ALCHEMY_HTTP_URL").unwrap_or_else(|_| "https://base-mainnet.g.alchemy.com/v2/alch_c3Z7QmOs3ZZFLUr0zngV6".to_string());
    let private_http_url = std::env::var("PRIVATE_RPC_URL").unwrap_or_else(|_| primary_http_url.clone());
    let alchemy_wss_url = std::env::var("ALCHEMY_WSS_URL").unwrap_or_else(|_| "ws://127.0.0.1:8545".to_string());
    
    let private_key_str = std::env::var("PRIVATE_KEY").unwrap_or_else(|_| "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string());
    let signer = PrivateKeySigner::from_str(private_key_str.trim())?;
    let signer_address = signer.address();

    let http_provider = ProviderBuilder::new().wallet(EthereumWallet::from(signer.clone())).connect_http(primary_http_url.trim().parse()?);
    let private_provider = ProviderBuilder::new().wallet(EthereumWallet::from(signer.clone())).connect_http(private_http_url.trim().parse()?);

    let liquidity_graph = load_global_liquidity_graph().await;
    let router_contract = IBalancerV3BatchRouter::new(BALANCER_V3_BATCH_ROUTER, http_provider.clone());

    let ws = alloy::providers::WsConnect::new(alchemy_wss_url.trim().to_string());
    let ws_provider = ProviderBuilder::new().wallet(EthereumWallet::from(signer.clone())).connect_ws(ws).await?;
    let sub = ws_provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();
    let mut block_counter = 0u64;

    let test_base_tokens = vec![USDC_BASE, WETH_BASE];

    while let Some(block) = stream.next().await {
        block_counter += 1;
        let block_num = block.inner.number;
        println!("\n📦 WSS Block Synced: #{} (Counter: {}) - Scanning Balancer V3 Markets...", block_num, block_counter);

        for &base_token in &test_base_tokens {
            let cycles = liquidity_graph.find_cycles(base_token, 3);
            if cycles.is_empty() { continue; }

            let initial_amount_in = if base_token == USDC_BASE { 500_000_000u64 } else { 200_000_000_000_000_000u64 };
            let min_profit = initial_amount_in / 1000;

            for cycle in &cycles {
                let mut router_steps = Vec::new();

                for edge in cycle {
                    router_steps.push(IBalancerV3BatchRouter::SwapPathStep {
                        pool: edge.pool.id,
                        tokenOut: edge.to_token,
                        userData: Bytes::default(),
                    });
                }

                let batch_paths = vec![IBalancerV3BatchRouter::SwapPathExactAmountIn {
                    tokenIn: base_token,
                    exactAmountIn: U256::from(initial_amount_in),
                    steps: router_steps,
                }];

                let contract_paths = batch_paths.iter().map(|p| {
                    let steps = p.steps.iter().map(|s| {
                        BaseAtomicArbitrage::SwapPathStep {
                            pool: s.pool,
                            tokenOut: s.tokenOut,
                            userData: s.userData.clone(),
                        }
                    }).collect();
                    BaseAtomicArbitrage::SwapPathExactAmountIn {
                        tokenIn: p.tokenIn,
                        exactAmountIn: p.exactAmountIn,
                        steps,
                    }
                }).collect();

                match router_contract.querySwapExactIn(batch_paths).call().await {
                    Ok(total_out) => {
                        let amount_out = total_out;
                        let target_required = U256::from(initial_amount_in) + U256::from(min_profit);

                        println!(
                            "📊 [Price Check] Input: {} | Expected Output: {} | Target Required (Min Profit): {}",
                            initial_amount_in, amount_out, target_required
                        );

                        if amount_out > target_required {
                            println!("🎯 [Profitable V3 Cycle Found!] Expected Out: {} | Required: {}", amount_out, target_required);

                            let is_profitable = simulate_and_diagnose_arbitrage(
                                private_provider.clone(),
                                contract_address,
                                signer_address,
                                base_token,
                                U256::from(initial_amount_in),
                                U256::from(min_profit),
                                contract_paths,
                            ).await.unwrap_or(false);

                            if is_profitable {
                                let dispatch_paths = liquidity_graph.find_cycles(base_token, 3);
                                if let Some(target_cycle) = dispatch_paths.first() {
                                    let mut d_steps = Vec::new();
                                    for edge in target_cycle {
                                        d_steps.push(BaseAtomicArbitrage::SwapPathStep {
                                            pool: edge.pool.id,
                                            tokenOut: edge.to_token,
                                            userData: Bytes::default(),
                                        });
                                    }
                                    let final_paths = vec![BaseAtomicArbitrage::SwapPathExactAmountIn {
                                        tokenIn: base_token,
                                        exactAmountIn: U256::from(initial_amount_in),
                                        steps: d_steps,
                                    }];

                                    let contract_private = BaseAtomicArbitrage::new(contract_address, private_provider.clone());
                                    match contract_private
                                        .executeBatchRouterArbitrage(base_token, U256::from(initial_amount_in), U256::from(min_profit), final_paths)
                                        .from(signer_address)
                                        .gas(3_500_000)
                                        .send()
                                        .await
                                    {
                                        Ok(tx) => println!("🚀 [Dispatched Successfully] Hash: {:?}", tx.tx_hash()),
                                        Err(e) => eprintln!("❌ [Dispatch Failed]: {:?}", e),
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }

        sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}
