use futures_util::StreamExt;
use tokio::time::{sleep, Duration};
use alloy::{
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    network::{EthereumWallet, Ethereum},
    primitives::{address, Address, U256, Bytes, Uint},
    sol, sol_types::SolCall,
};
use std::collections::HashMap;

type _U24 = Uint<24, 1>;
type _U160 = Uint<160, 3>;

// --- OFFICIAL BASE MAINNET CORE CONTRACTS ---
const USDC_BASE: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bda02913");
const WETH_BASE: Address = address!("4200000000000000000000000000000000000006");
const CBETH_BASE: Address = address!("0x2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc22");
const DAI_BASE: Address = address!("0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb");

const UNISWAP_V3_ROUTER: Address = address!("0xcf77A3bA9Aab7D3E44917635033322DF3f564171");
const AERODROME_ROUTER: Address = address!("0x2626664c2603336E57B271c5C0b26F421741e481");

// --- DEX POOL TYPE ENUM ---
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DexType {
    UniswapV3,
    Aerodrome,
    BaseSwap,
    PancakeSwapV3,
}

// --- POOL DEFINITION ---
#[derive(Debug, Clone)]
pub struct Pool {
    pub id: Address,
    pub token0: Address,
    pub token1: Address,
    pub dex_type: DexType,
    pub fee: u32,
}

// --- EDGE DEFINITION FOR GRAPH ---
#[derive(Debug, Clone)]
pub struct DirectedEdge {
    pub pool: Pool,
    pub from_token: Address,
    pub to_token: Address,
}

// --- GLOBAL ARBITRAGE GRAPH (Multi-hop Pathfinding) ---
#[derive(Default, Clone)]
pub struct ArbitrageGraph {
    pub adjacency_list: HashMap<Address, Vec<DirectedEdge>>,
}

impl ArbitrageGraph {
    pub fn new() -> Self {
        Self {
            adjacency_list: HashMap::new(),
        }
    }

    pub fn add_pool(&mut self, pool: Pool) {
        self.adjacency_list
            .entry(pool.token0)
            .or_default()
            .push(DirectedEdge {
                pool: pool.clone(),
                from_token: pool.token0,
                to_token: pool.token1,
            });
        self.adjacency_list
            .entry(pool.token1)
            .or_default()
            .push(DirectedEdge {
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
        if current_path.len() > max_hops {
            return;
        }

        if current_token == start_token && !current_path.is_empty() {
            if current_path.len() == 3 {
                let mut pool_ids = Vec::new();
                for edge in current_path.iter() {
                    pool_ids.push(edge.pool.id);
                }
                pool_ids.sort();
                pool_ids.dedup();

                if pool_ids.len() == current_path.len() {
                    cycles.push(current_path.clone());
                }
            }
            return;
        }

        if current_path.len() == max_hops {
            return;
        }

        if let Some(edges) = self.adjacency_list.get(&current_token) {
            for edge in edges {
                if current_path.iter().any(|e| e.pool.id == edge.pool.id) {
                    continue;
                }

                current_path.push(edge.clone());
                self.dfs_search(start_token, edge.to_token, max_hops, current_path, cycles);
                current_path.pop();
            }
        }
    }
}

// --- INITIALIZE GLOBAL LIQUIDITY POOLS REGISTRY ---
pub fn load_global_liquidity_graph() -> ArbitrageGraph {
    let mut graph = ArbitrageGraph::new();

    graph.add_pool(Pool {
        id: address!("0x6c561b446416e1a00e8e93e221854d6ea4171372"),
        token0: WETH_BASE,
        token1: USDC_BASE,
        dex_type: DexType::UniswapV3,
        fee: 100,
    });
    graph.add_pool(Pool {
        id: address!("cdac0d6c6c59727a65f871236188350531885c43"),
        token0: WETH_BASE,
        token1: USDC_BASE,
        dex_type: DexType::Aerodrome,
        fee: 0,
    });
    graph.add_pool(Pool {
        id: address!("0x21198fdfbc9c3b88b0a9dfbf65449cf77cecc0df"),
        token0: WETH_BASE,
        token1: CBETH_BASE,
        dex_type: DexType::Aerodrome,
        fee: 0,
    });
    graph.add_pool(Pool {
        id: address!("0x53009586111164998d36371dd0349f7e44a4f806"),
        token0: USDC_BASE,
        token1: DAI_BASE,
        dex_type: DexType::Aerodrome,
        fee: 1, 
    });
    graph.add_pool(Pool {
        id: address!("0x327cd1c2c31e42f9862b535d4ff3a6af6256f081"),
        token0: WETH_BASE,
        token1: DAI_BASE,
        dex_type: DexType::UniswapV3,
        fee: 500,
    });

    graph
}

// --- PAYLOAD ENCODER ---
pub struct PayloadEncoder;

impl PayloadEncoder {
    pub fn encode_v3_path(edges: &[DirectedEdge]) -> Vec<u8> {
        let mut path = Vec::new();
        if edges.is_empty() {
            return path;
        }
        path.extend_from_slice(edges[0].from_token.as_slice());
        for edge in edges {
            let fee_bytes = edge.pool.fee.to_be_bytes();
            path.extend_from_slice(&fee_bytes[1..4]);
            path.extend_from_slice(edge.to_token.as_slice());
        }
        path
    }

    pub fn build_execution_payload(
        cycle: &[DirectedEdge],
        recipient_contract: Address,
        amount_in: U256,
    ) -> (Vec<Address>, Vec<Bytes>) {
        let mut targets = Vec::new();
        let mut data = Vec::new();
        let mut current_amount = amount_in;

        for edge in cycle {
            targets.push(match edge.pool.dex_type {
                DexType::UniswapV3 | DexType::PancakeSwapV3 => UNISWAP_V3_ROUTER,
                DexType::Aerodrome | DexType::BaseSwap => AERODROME_ROUTER,
            });

            let min_out = current_amount * U256::from(9950) / U256::from(10000); 

            match edge.pool.dex_type {
                DexType::UniswapV3 | DexType::PancakeSwapV3 => {
                    let sub_path = Self::encode_v3_path(&[edge.clone()]);
                    let params = IUniswapV3Router::ExactInputParams {
                        path: Bytes::from(sub_path),
                        recipient: recipient_contract,
                        deadline: U256::from(u64::MAX),
                        amountIn: current_amount, 
                        amountOutMinimum: min_out,
                    };
                    let call_bytes = IUniswapV3Router::exactInputCall { params }.abi_encode();
                    data.push(Bytes::from(call_bytes));
                }
                DexType::Aerodrome | DexType::BaseSwap => {
                    let aero_route = IAerodromeRouter::Route {
                        from: edge.from_token,
                        to: edge.to_token,
                        stable: edge.pool.fee == 1, 
                        factory: address!("420DD381b31aEf6683db6B902084cB0FFECe40Da"),
                    };
                    let call_bytes = IAerodromeRouter::swapExactTokensForTokensCall {
                        amountIn: current_amount,
                        amountOutMin: min_out,
                        routes: vec![aero_route],
                        to: recipient_contract,
                        deadline: U256::from(u64::MAX),
                    }.abi_encode();
                    data.push(Bytes::from(call_bytes));
                }
            }
            current_amount = min_out;
        }

        (targets, data)
    }
}

sol! {
    #[sol(rpc)]
    contract IUniswapV2Pair {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }

    #[sol(rpc)]
    contract IUniswapV3Pool {
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
    }

    #[sol(rpc)]
    contract IUniswapV3Router {
        struct ExactInputParams {
            bytes path;
            address recipient;
            uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
        }
        function exactInput(ExactInputParams calldata params) external payable returns (uint256 amountOut);
    }

    #[sol(rpc)]
    contract IAerodromeRouter {
        struct Route {
            address from;
            address to;
            bool stable;
            address factory;
        }
        function swapExactTokensForTokens(
            uint256 amountIn,
            uint256 amountOutMin,
            Route[] calldata routes,
            address to,
            uint256 deadline
        ) external returns (uint256[] memory amounts);
    }

    #[sol(rpc)]
    contract BaseAtomicArbitrage {
        function triggerBalancerArbitrage(address tokenToBorrow, uint256 loanAmount, address[] calldata targets, bytes[] calldata data) external;
        function triggerAaveArbitrage(address tokenToBorrow, uint256 loanAmount, address[] calldata targets, bytes[] calldata data) external;
        function setTargetWhitelist(address target, bool status) external;
        
        error ArbitrageUnprofitable(uint256 expected, uint256 actual);
        error InsufficientLiquidity();
        error TargetUnauthorized();
        error UntrustedInitiator();
        error NotAuthorized();
        error ExternalCallFailed(bytes reason);
    }
}

// دالة لجلب وعرض أسعار المسبحات الحية في كل بلوك
async fn print_live_market_prices<P>(http_provider: P) 
where
    P: Provider<Ethereum> + Clone,
{
    // فحص مسبح Aerodrome WETH/USDC (العنوان: cdac0d6c6c59727a65f871236188350531885c43)
    let aero_pool = address!("cdac0d6c6c59727a65f871236188350531885c43");
    let pair_contract = IUniswapV2Pair::new(aero_pool, http_provider);
    if let Ok(res) = pair_contract.getReserves().call().await {
        let r0 = U256::from(res.reserve0); // WETH تقريباً
        let r1 = U256::from(res.reserve1); // USDC تقريباً
        if !r0.is_zero() && !r1.is_zero() {
            // حساب السعر التقريبي WETH/USDC
            let price_usdc = (r1 * U256::from(1_000_000u64)) / r0; 
            let price_float = price_usdc.to::<u128>() as f64 / 1_000_000.0;
            println!("   💹 [Live Market Price] Aerodrome WETH/USDC = ${:.2}", price_float);
        }
    }
}

async fn fetch_live_pool_liquidity_and_compute_loan<P>(
    http_provider: P,
    cycle: &[DirectedEdge],
) -> Result<U256, Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
{
    let mut min_reserve = U256::from(u128::MAX);

    for edge in cycle {
        match edge.pool.dex_type {
            DexType::Aerodrome | DexType::BaseSwap => {
                let pair_contract = IUniswapV2Pair::new(edge.pool.id, http_provider.clone());
                if let Ok(res) = pair_contract.getReserves().call().await {
                    let r0 = U256::from(res.reserve0);
                    let r1 = U256::from(res.reserve1);
                    if r0 < min_reserve { min_reserve = r0; }
                    if r1 < min_reserve { min_reserve = r1; }
                }
            }
            DexType::UniswapV3 | DexType::PancakeSwapV3 => {
                let default_v3_cap = U256::from(50_000_000_000u64);
                if default_v3_cap < min_reserve { min_reserve = default_v3_cap; }
            }
        }
    }

    if min_reserve == U256::from(u128::MAX) || min_reserve.is_zero() {
        return Ok(U256::from(1_000_000_000u64));
    }

    let dynamic_loan = (min_reserve * U256::from(5u64)) / U256::from(100u64);
    let final_loan = dynamic_loan.max(U256::from(1_000_000_000u64)).min(U256::from(50_000_000_000u64));

    Ok(final_loan)
}

async fn execute_triangular_arbitrage<P, PP>(
    http_provider: P,
    private_provider: PP,
    contract_address: Address,
    signer_address: Address,
    graph: &ArbitrageGraph,
) -> Result<(), Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
    PP: Provider<Ethereum> + Clone,
{
    // طباعة الأسعار الحية أولاً
    print_live_market_prices(http_provider.clone()).await;

    let cycles = graph.find_cycles(USDC_BASE, 3);
    if cycles.is_empty() {
        return Ok(());
    }

    for (i, target_cycle) in cycles.iter().enumerate() {
        let loan_amount = fetch_live_pool_liquidity_and_compute_loan(http_provider.clone(), target_cycle).await
            .unwrap_or(U256::from(2_000_000_000u64));

        let (targets, data) = PayloadEncoder::build_execution_payload(
            target_cycle,
            contract_address,
            loan_amount,
        );

        if targets.is_empty() || data.is_empty() {
            continue;
        }

        let loan_display = loan_amount.to::<u128>() as f64 / 1_000_000.0;
        println!("   🔍 [Simulating Cycle #{}] Loan: {:.2} USDC across {} hops...", i + 1, loan_display, target_cycle.len());

        let contract_sim = BaseAtomicArbitrage::new(contract_address, http_provider.clone());
        let simulation_builder = contract_sim
            .triggerBalancerArbitrage(USDC_BASE, loan_amount, targets.clone(), data.clone())
            .from(signer_address)
            .gas(1_800_000);

        match simulation_builder.call().await {
            Ok(_) => {
                println!("🎉 [SIMULATION PASSED!] Profitable Multi-hop Arbitrage Verified! Dispatching via PRIVATE RPC...");
                
                let contract_private = BaseAtomicArbitrage::new(contract_address, private_provider.clone());
                let private_builder = contract_private
                    .triggerBalancerArbitrage(USDC_BASE, loan_amount, targets, data)
                    .from(signer_address)
                    .gas(1_800_000);

                if let Ok(pending_tx) = private_builder.send().await {
                    println!("🚀 [PRIVATE MEV-PROTECTED TX DISPATCHED!] Real Hash: {:?}", pending_tx.tx_hash());
                }
                break; 
            }
            Err(_sim_err) => {
                println!("   💤 [Unprofitable / Reverted] Cycle #{}: صفقة غير مربحة حالياً في هذا البلوك", i + 1);
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Initializing Multi-Hop Bot with Live Price Ticker & Diagnostics...");

    let contract_addr_str = std::env::var("CONTRACT_ADDR")
        .unwrap_or_else(|_| "0x94409D722D4b21A496A3C073477F06Ca388B9C3B".to_string());
    let contract_address: Address = contract_addr_str.parse()?;

    let primary_http_url = std::env::var("ALCHEMY_HTTP_URL")
        .unwrap_or_else(|_| "https://mainnet.base.org".to_string());
    
    let private_http_url = std::env::var("PRIVATE_RPC_URL")
        .unwrap_or_else(|_| primary_http_url.clone());

    let alchemy_wss_url = std::env::var("ALCHEMY_WSS_URL")
        .unwrap_or_else(|_| "ws://127.0.0.1:8545".to_string());

    let private_key_str = std::env::var("PRIVATE_KEY")
        .unwrap_or_else(|_| "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string());
    let signer: PrivateKeySigner = private_key_str.parse()?;
    let signer_address = signer.address();
    let wallet = EthereumWallet::from(signer.clone());
    let wallet_private = EthereumWallet::from(signer);

    let http_provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_http(primary_http_url.parse()?);

    let private_provider = ProviderBuilder::new()
        .wallet(wallet_private)
        .connect_http(private_http_url.parse()?);

    let liquidity_graph = load_global_liquidity_graph();
    println!("🌐 Global Liquidity Graph Loaded Successfully!");

    let ws = alloy::providers::WsConnect::new(alchemy_wss_url);
    let ws_provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(private_key_str.parse::<PrivateKeySigner>()?))
        .connect_ws(ws)
        .await?;

    let sub = ws_provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();
    let mut block_counter = 0u64;

    while let Some(block) = stream.next().await {
        block_counter += 1;
        let block_num = block.inner.number;
        println!("\n📦 WSS Block Synced: #{} (Counter: {}) - Scanning Prices & Cycles...", block_num, block_counter);

        let _ = execute_triangular_arbitrage(
            http_provider.clone(),
            private_provider.clone(),
            contract_address,
            signer_address,
            &liquidity_graph,
        ).await;

        sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}
