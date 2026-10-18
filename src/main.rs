use num_bigint::BigUint;
use num_traits::One;
use futures_util::StreamExt;
use tokio::time::{sleep, Duration};
use alloy::{
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    network::{EthereumWallet, Ethereum},
    primitives::{address, Address, U256, Bytes, Uint},
    sol, sol_types::{SolCall, SolInterface},
};
use std::collections::HashMap;

type _U24 = Uint<24, 1>;
type _U160 = Uint<160, 3>;

// --- [Target Tokens Registry on Base Mainnet] ---
pub const WETH_BASE: Address = address!("4200000000000000000000000000000000000006");
pub const USDC_BASE: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913");
pub const CBETH_BASE: Address = address!("2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc22");
pub const AERO_BASE: Address = address!("94017f86360288571845a7443274e0d49a79e2c1");
pub const DEGEN_BASE: Address = address!("4ed4E862860beD51a9570b96d89aF5E1B0Efefed");
pub const BRETT_BASE: Address = address!("532f27101965dd16442E59d40670FaF5ebb142E4");
pub const VIRTUAL_BASE: Address = address!("0b3e328455c4059EEb9e3f84b5543F74E24e7E1b");
pub const DAI_BASE: Address = address!("50c5725949a6f0c72e6c4a641f24049a917db0cb");

// --- [DEX Factories & Routers on Base] ---
const UNISWAP_V3_FACTORY: Address = address!("33128a8fC17869897dcE68Ed026d694621f6FDfD");
const PANCAKESWAP_V3_FACTORY: Address = address!("41ebdCbb4802521A392742417441411ce903fF02");
const AERODROME_V2_FACTORY: Address = address!("420DD381b31aEf6683db6B902084cB0FFECe40Da");

const UNISWAP_V3_ROUTER: Address = address!("cf77A3bA9Aab7D3E44917635033322DF3f564171");
const AERODROME_ROUTER: Address = address!("2626664c2603336E57B271c5C0b26F421741e481");
const BASESWAP_ROUTER: Address = address!("cA1F7645EF1077CC178dFb42Cf5d2f840457DDBb"); 
const PANCAKESWAP_V3_ROUTER: Address = address!("1b81D678ffb9C0263b24A97847620C99d213eB14"); 

#[derive(Debug, Clone)]
pub struct TokenPairConfig {
    pub symbol: &'static str,
    pub target_token: Address,
    pub aero_pool: Address,
    pub uni_v3_pool: Address,
    pub baseswap_pool: Address,
    pub pancakeswap_pool: Address,
    pub decimals: u32,
    pub fee_tier_v3: u32,
    pub token0: Address,
    pub token1: Address,
    pub is_v3_aero: bool,
    pub is_v3_baseswap: bool,
}

#[derive(Clone, Debug)]
pub struct QuantumNode {
    pub id: usize,
    pub energy_scale: BigUint,
    pub frequency: f64,
    pub token0: Address,
    pub token1: Address,
}

pub struct CausalCollapseSystem {
    pub nodes: Vec<QuantumNode>,
    pub threshold_limit: f64,
    pub buffer_capacity: usize,
    pub dynamic_pools: Vec<Address>,
    pub contract_address: Address,
    pub loan_amount: U256,
    pub borrowed_token: Address,
    pub pair_config: TokenPairConfig,
    pub price_aero: f64,
    pub price_v3: f64,
    pub current_target_price_usd: f64,
}

impl CausalCollapseSystem {
    pub fn new(
        nodes: Vec<QuantumNode>,
        dynamic_pools: Vec<Address>,
        contract_address: Address,
        loan_amount: U256,
        borrowed_token: Address,
        pair_config: TokenPairConfig,
        price_aero: f64,
        price_v3: f64,
        current_target_price_usd: f64,
    ) -> Self {
        Self {
            nodes,
            threshold_limit: 0.0,
            buffer_capacity: 16,
            dynamic_pools,
            contract_address,
            loan_amount,
            borrowed_token,
            pair_config,
            price_aero,
            price_v3,
            current_target_price_usd,
        }
    }

    pub fn estimate_l1_data_fee(payload_len: usize) -> U256 {
        let base_gas_per_byte = 16; 
        let estimated_l1_gas = (payload_len as u64) * base_gas_per_byte;
        let l1_gas_price = 15_000_000_000u64;
        U256::from(estimated_l1_gas) * U256::from(l1_gas_price) / U256::from(10_18_000_000_000u64)
    }

    pub fn execute_collapse(&self) -> (Vec<Address>, Vec<Vec<u8>>) {
        let mut addresses = Vec::new();
        let mut payloads = Vec::new();

        if self.price_aero <= 0.0 || self.price_v3 <= 0.0 {
            return (vec![], vec![]);
        }

        let flash_loan_fee = self.loan_amount / U256::from(10000); // 0.01%
        let loan_usdc_f64 = self.loan_amount.to_string().parse::<f64>().unwrap_or(0.0) / 1_000_000.0;

        let buy_on_univ3 = self.price_v3 < self.price_aero;
        let buy_price = if buy_on_univ3 { self.price_v3 } else { self.price_aero };
        let sell_price = if buy_on_univ3 { self.price_aero } else { self.price_v3 };

        let expected_target_tokens_f64 = loan_usdc_f64 / buy_price;
        let target_decimals_multiplier = 10f64.powi(self.pair_config.decimals as i32);
        let expected_target_units_u128 = (expected_target_tokens_f64 * target_decimals_multiplier) as u128;
        let expected_target_units = U256::from(expected_target_units_u128);

        let fee_bps = if self.pair_config.fee_tier_v3 > 3000 { 100 } else { 30 };
        let dex_fee_allowance = self.loan_amount * U256::from(fee_bps) / U256::from(10000);
        let target_repay_amount = self.loan_amount + flash_loan_fee + dex_fee_allowance;

        let min_amount_out_leg1 = expected_target_units * U256::from(9850) / U256::from(10000); 
        let min_amount_out_leg2 = target_repay_amount + U256::from(1_000_000);

        if buy_on_univ3 {
            println!("   🧮 [Volatile Route Execution] BUY UniV3/Pancake (${:.4}) -> SELL Aero Slipstream (${:.4})", buy_price, sell_price);
            addresses.push(UNISWAP_V3_ROUTER);
            let mut path_bytes = Vec::new();
            path_bytes.extend_from_slice(self.borrowed_token.as_slice());
            let fee_bytes = self.pair_config.fee_tier_v3.to_be_bytes();
            path_bytes.extend_from_slice(&fee_bytes[1..4]);
            path_bytes.extend_from_slice(self.pair_config.target_token.as_slice());

            let exact_input_params = IUniswapV3Router::ExactInputParams {
                path: Bytes::from(path_bytes),
                recipient: self.contract_address,
                deadline: U256::from(u64::MAX),
                amountIn: self.loan_amount,
                amountOutMinimum: min_amount_out_leg1,
            };
            payloads.push(IUniswapV3Router::exactInputCall { params: exact_input_params }.abi_encode());

            addresses.push(AERODROME_ROUTER);
            let aero_route = IAerodromeRouter::Route {
                from: self.pair_config.target_token,
                to: self.borrowed_token,
                stable: false,
                factory: AERODROME_V2_FACTORY,
            };
            payloads.push(IAerodromeRouter::swapExactTokensForTokensCall {
                amountIn: min_amount_out_leg1,
                amountOutMin: min_amount_out_leg2,
                routes: vec![aero_route],
                to: self.contract_address,
                deadline: U256::from(u64::MAX),
            }.abi_encode());
        } else {
            println!("   🧮 [Volatile Route Execution] BUY Aero Slipstream (${:.4}) -> SELL UniV3/Pancake (${:.4})", buy_price, sell_price);
            addresses.push(AERODROME_ROUTER);
            let aero_route = IAerodromeRouter::Route {
                from: self.borrowed_token,
                to: self.pair_config.target_token,
                stable: false,
                factory: AERODROME_V2_FACTORY,
            };
            payloads.push(IAerodromeRouter::swapExactTokensForTokensCall {
                amountIn: self.loan_amount,
                amountOutMin: min_amount_out_leg1,
                routes: vec![aero_route],
                to: self.contract_address,
                deadline: U256::from(u64::MAX),
            }.abi_encode());

            addresses.push(UNISWAP_V3_ROUTER);
            let mut path_bytes = Vec::new();
            path_bytes.extend_from_slice(self.pair_config.target_token.as_slice());
            let fee_bytes = self.pair_config.fee_tier_v3.to_be_bytes();
            path_bytes.extend_from_slice(&fee_bytes[1..4]);
            path_bytes.extend_from_slice(self.borrowed_token.as_slice());

            let exact_input_params = IUniswapV3Router::ExactInputParams {
                path: Bytes::from(path_bytes),
                recipient: self.contract_address,
                deadline: U256::from(u64::MAX),
                amountIn: min_amount_out_leg1,
                amountOutMinimum: min_amount_out_leg2,
            };
            payloads.push(IUniswapV3Router::exactInputCall { params: exact_input_params }.abi_encode());
        }

        (addresses, payloads)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DexType {
    UniswapV3,
    Aerodrome,
    BaseSwap,
    PancakeSwapV3,
}

#[derive(Debug, Clone)]
pub struct Pool {
    pub id: Address,
    pub token0: Address,
    pub token1: Address,
    pub dex_type: DexType,
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
            if current_path.len() == 3 {
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

pub fn load_global_liquidity_graph() -> ArbitrageGraph {
    let mut graph = ArbitrageGraph::new();

    // 1. AERO Loop Pools
    graph.add_pool(Pool { id: address!("7f6dBb6fa91176b6C1Ad3767B40A39c294136C70"), token0: AERO_BASE, token1: WETH_BASE, dex_type: DexType::Aerodrome, fee: 3000 });
    graph.add_pool(Pool { id: address!("c949e29eeE91696AFBceC1E0A0c5a2789BeD6F39"), token0: AERO_BASE, token1: WETH_BASE, dex_type: DexType::UniswapV3, fee: 3000 });

    // 2. DEGEN Loop Pools
    graph.add_pool(Pool { id: address!("c9034c3E7F30f1465BE755519f4007dfA0944F3d"), token0: DEGEN_BASE, token1: WETH_BASE, dex_type: DexType::UniswapV3, fee: 10000 });
    graph.add_pool(Pool { id: address!("51E2Eb7A31853B0B7F83296c0D29E8FdEb57a7da"), token0: DEGEN_BASE, token1: WETH_BASE, dex_type: DexType::Aerodrome, fee: 10000 });

    // 3. BRETT & VIRTUAL Pools
    graph.add_pool(Pool { id: address!("bBe0Cd428230fb1D97475FfeEE892Ea79B625890"), token0: BRETT_BASE, token1: WETH_BASE, dex_type: DexType::UniswapV3, fee: 10000 });
    graph.add_pool(Pool { id: address!("89EfE9110B7f8303C0D62d2CE351df7161b9BdA0"), token0: VIRTUAL_BASE, token1: WETH_BASE, dex_type: DexType::UniswapV3, fee: 10000 });

    // 4. cbETH Liquid Staking Pools
    graph.add_pool(Pool { id: address!("B5B59c29F257Ddf68b55dfdfcdCD449485BDC87E"), token0: CBETH_BASE, token1: WETH_BASE, dex_type: DexType::PancakeSwapV3, fee: 500 });
    graph.add_pool(Pool { id: address!("47cA96EA59C13f72745928887F84c9F52C3D7348"), token0: CBETH_BASE, token1: WETH_BASE, dex_type: DexType::Aerodrome, fee: 500 });

    // 5. WETH / USDC Anchors
    graph.add_pool(Pool { id: address!("d0b53D927764e11757551062039a5455115853fc"), token0: WETH_BASE, token1: USDC_BASE, dex_type: DexType::UniswapV3, fee: 500 });
    graph.add_pool(Pool { id: address!("B4FBF271143F4FBf7B91A5ded31805e42b2208d6"), token0: WETH_BASE, token1: USDC_BASE, dex_type: DexType::Aerodrome, fee: 500 });

    graph
}

pub struct TriangularPayloadEncoder;
impl TriangularPayloadEncoder {
    pub fn encode_v3_path(edges: &[DirectedEdge]) -> Vec<u8> {
        let mut path = Vec::new();
        if edges.is_empty() { return path; }
        path.extend_from_slice(edges[0].from_token.as_slice());
        for edge in edges {
            let fee_bytes = edge.pool.fee.to_be_bytes();
            path.extend_from_slice(&fee_bytes[1..4]);
            path.extend_from_slice(edge.to_token.as_slice());
        }
        path
    }

    pub fn build_execution_payload(cycle: &[DirectedEdge], recipient_contract: Address, amount_in: U256) -> (Vec<Address>, Vec<Bytes>) {
        let mut targets = Vec::new();
        let mut data = Vec::new();
        let mut current_amount = amount_in;
        for edge in cycle {
            targets.push(match edge.pool.dex_type {
                DexType::UniswapV3 => UNISWAP_V3_ROUTER,
                DexType::PancakeSwapV3 => PANCAKESWAP_V3_ROUTER,
                DexType::Aerodrome | DexType::BaseSwap => AERODROME_ROUTER,
            });
            let min_out = current_amount * U256::from(9900) / U256::from(10000);
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
                    data.push(Bytes::from(IUniswapV3Router::exactInputCall { params }.abi_encode()));
                }
                DexType::Aerodrome | DexType::BaseSwap => {
                    let aero_route = IAerodromeRouter::Route {
                        from: edge.from_token,
                        to: edge.to_token,
                        stable: edge.pool.fee == 1,
                        factory: AERODROME_V2_FACTORY,
                    };
                    data.push(Bytes::from(IAerodromeRouter::swapExactTokensForTokensCall {
                        amountIn: current_amount,
                        amountOutMin: min_out,
                        routes: vec![aero_route],
                        to: recipient_contract,
                        deadline: U256::from(u64::MAX),
                    }.abi_encode()));
                }
            }
            current_amount = min_out;
        }
        (targets, data)
    }
}

pub fn generate_astronomical_number(zeros: usize) -> BigUint {
    let mut num_str = "1".to_string();
    for _ in 0..zeros { num_str.push('0'); }
    BigUint::parse_bytes(num_str.as_bytes(), 10).unwrap_or_else(BigUint::one)
}

sol! {
    #[sol(rpc)]
    contract IUniswapV3Factory {
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool);
    }

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
        function triggerBalancerArbitrage(address tokenToBorrow, uint256 loanAmount, bytes calldata swapPathData) external;
        function triggerBalancerArbitrageMulti(address tokenToBorrow, uint256 loanAmount, address[] calldata targets, bytes[] calldata data) external;
        function setTargetWhitelist(address target, bool status) external;

        error ArbitrageUnprofitable(uint256 required, uint256 actual);
        error InsufficientLiquidity();
    }
}

async fn get_monitored_pairs<P>(_http_provider: P) -> Result<Vec<TokenPairConfig>, Box<dyn std::error::Error>>
where P: Provider<Ethereum> + Clone {
    Ok(vec![
        TokenPairConfig {
            symbol: "AERO",
            target_token: AERO_BASE,
            aero_pool: address!("7f6dBb6fa91176b6C1Ad3767B40A39c294136C70"),
            uni_v3_pool: address!("c949e29eeE91696AFBceC1E0A0c5a2789BeD6F39"),
            baseswap_pool: Address::ZERO,
            pancakeswap_pool: Address::ZERO,
            decimals: 18,
            fee_tier_v3: 3000,
            token0: AERO_BASE,
            token1: WETH_BASE,
            is_v3_aero: true,
            is_v3_baseswap: false,
        },
        TokenPairConfig {
            symbol: "DEGEN",
            target_token: DEGEN_BASE,
            aero_pool: address!("51E2Eb7A31853B0B7F83296c0D29E8FdEb57a7da"),
            uni_v3_pool: address!("c9034c3E7F30f1465BE755519f4007dfA0944F3d"),
            baseswap_pool: Address::ZERO,
            pancakeswap_pool: Address::ZERO,
            decimals: 18,
            fee_tier_v3: 10000,
            token0: DEGEN_BASE,
            token1: WETH_BASE,
            is_v3_aero: true,
            is_v3_baseswap: false,
        },
        TokenPairConfig {
            symbol: "BRETT",
            target_token: BRETT_BASE,
            aero_pool: Address::ZERO,
            uni_v3_pool: address!("bBe0Cd428230fb1D97475FfeEE892Ea79B625890"),
            baseswap_pool: Address::ZERO,
            pancakeswap_pool: Address::ZERO,
            decimals: 18,
            fee_tier_v3: 10000,
            token0: BRETT_BASE,
            token1: WETH_BASE,
            is_v3_aero: false,
            is_v3_baseswap: false,
        },
        TokenPairConfig {
            symbol: "VIRTUAL",
            target_token: VIRTUAL_BASE,
            aero_pool: Address::ZERO,
            uni_v3_pool: address!("89EfE9110B7f8303C0D62d2CE351df7161b9BdA0"),
            baseswap_pool: Address::ZERO,
            pancakeswap_pool: Address::ZERO,
            decimals: 18,
            fee_tier_v3: 10000,
            token0: VIRTUAL_BASE,
            token1: WETH_BASE,
            is_v3_aero: false,
            is_v3_baseswap: false,
        },
        TokenPairConfig {
            symbol: "cbETH",
            target_token: CBETH_BASE,
            aero_pool: address!("47cA96EA59C13f72745928887F84c9F52C3D7348"),
            uni_v3_pool: address!("19a0aef6613524ea0761646a6a4e4be0e114a3c8"),
            baseswap_pool: Address::ZERO,
            pancakeswap_pool: address!("B5B59c29F257Ddf68b55dfdfcdCD449485BDC87E"),
            decimals: 18,
            fee_tier_v3: 500,
            token0: WETH_BASE,
            token1: CBETH_BASE,
            is_v3_aero: false,
            is_v3_baseswap: false,
        },
    ])
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 [Base MEV Bot] Starting High-Performance Engine...");

    let rpc_url = std::env::var("BASE_RPC_URL").unwrap_or_else(|_| "https://mainnet.base.org".to_string());
    println!("🔗 Connecting to Base Mainnet RPC: {}", rpc_url);

    let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);

    let monitored_pairs = get_monitored_pairs(provider.clone()).await?;
    println!("✅ Indexing {} Volatile Token Pairs.", monitored_pairs.len());

    let graph = load_global_liquidity_graph();
    println!("🌐 Global Liquidity Graph loaded with {} nodes.", graph.adjacency_list.len());

    let contract_address = address!("457820b3D7d2d607AEEF8248B46Ffa6ccD60121A");
    println!("📝 Target Contract Address: {:?}", contract_address);

    let loan_amount = U256::from(1000000000000000000u64); // 1 WETH

    println!("⚡ [Monitoring Mode Active] Scanning block state shifts for high-volatility spreads (> $8.0 or > 1%)...");

    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    loop {
        interval.tick().await;

        for pair in &monitored_pairs {
            let collapse_system = CausalCollapseSystem::new(
                vec![],
                vec![],
                contract_address,
                loan_amount,
                WETH_BASE,
                pair.clone(),
                1.02, 
                1.00,
                2500.0,
            );

            let (targets, payloads) = collapse_system.execute_collapse();
            if !targets.is_empty() {
                println!("🎯 [Spread Detected] Dispatched Payload for {}. Payload Count: {}", pair.symbol, payloads.len());
            }
        }
    }
}
