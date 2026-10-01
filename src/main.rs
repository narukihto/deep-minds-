use num_bigint::BigUint;
use num_traits::{ToPrimitive, One};
use rayon::prelude::*;
use std::time::Instant;
use futures_util::StreamExt;
use tokio::time::{sleep, Duration};
use alloy::{
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    network::{EthereumWallet, Ethereum},
    primitives::{address, Address, U256, Bytes, Uint},
    sol,
    sol_types::SolCall,
};

type U24 = Uint<24, 1>;

const WETH_BASE: Address = address!("4200000000000000000000000000000000000006");
const USDC_BASE: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bda02913");
const CBBTC_BASE: Address = address!("cbB7C7A63551000b48A8503b87936a229a4b3FE3");
const USDBC_BASE: Address = address!("d9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA");
const AERO_BASE: Address = address!("940181a94A35A4569E4529A3CDfB74e38FD98631");

// Whitelisted Router Addresses on Base Network
const UNISWAP_V3_ROUTER: Address = address!("262664982A6941F909fCF2f8358D64E19349e25C");
const AERODROME_ROUTER: Address = address!("cF77a3Ba9A5CA399B7f97cbf339178ffc51eda8c");

// Factory references retained for backward compatibility
const UNISWAP_V3_FACTORY: Address = address!("33128a8fC17869843ccE68212c64714156f52d62");
const AERODROME_FACTORY: Address = address!("420DD381b31aEf6683db6B902084cB0FFECe40Da");

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
}

impl CausalCollapseSystem {
    pub fn new(nodes: Vec<QuantumNode>, dynamic_pools: Vec<Address>, contract_address: Address, loan_amount: U256) -> Self {
        Self {
            nodes,
            threshold_limit: 0.02,
            buffer_capacity: 16,
            dynamic_pools,
            contract_address,
            loan_amount,
        }
    }

    fn project_to_inverse_dimensional_symmetry(&self, raw_value: f64, index: usize) -> f64 {
        let dimension_factor = (index as f64 + 1.0).ln();
        let high_dimensional_shadow = (raw_value * dimension_factor).sin();
        high_dimensional_shadow.abs()
    }

    pub fn execute_collapse(&self) -> (Vec<Address>, Vec<Vec<u8>>) {
        if self.nodes.is_empty() { 
            return (vec![], vec![]); 
        }

        let effective_pools = if self.dynamic_pools.is_empty() {
            vec![AERODROME_ROUTER]
        } else {
            self.dynamic_pools.clone()
        };

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

        final_path.push(active_nodes[0].clone());

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
                final_path.push(next.clone());
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
                final_path.push(buffered_node.clone());
            }
        }

        let mut addresses = Vec::new();
        let mut payloads = Vec::new();

        for (idx, node) in final_path.iter().enumerate() {
            let pool = effective_pools[idx % effective_pools.len()];
            addresses.push(pool);

            let payload = if pool == UNISWAP_V3_ROUTER {
                let exact_input_params = IUniswapV3Router::ExactInputParams {
                    path: {
                        let mut packed = Vec::new();
                        packed.extend_from_slice(node.token0.as_slice());
                        packed.extend_from_slice(&[0x00, 0x0b, 0xb8]); 
                        packed.extend_from_slice(node.token1.as_slice());
                        Bytes::from(packed)
                    },
                    recipient: self.contract_address,
                    deadline: U256::from(u64::MAX),
                    amountIn: self.loan_amount,
                    amountOutMinimum: U256::ZERO,
                };
                IUniswapV3Router::exactInputCall { params: exact_input_params }.abi_encode()
            } else if pool == AERODROME_ROUTER {
                let aero_route = IAerodromeRouter::Route {
                    from: node.token0,
                    to: node.token1,
                    stable: false,
                    factory: AERODROME_FACTORY,
                };
                IAerodromeRouter::swapExactTokensForTokensCall {
                    amountIn: self.loan_amount,
                    amountOutMin: U256::ZERO,
                    routes: vec![aero_route],
                    to: self.contract_address,
                    deadline: U256::from(u64::MAX),
                }.abi_encode()
            } else {
                IUniswapV2Router02::swapExactTokensForTokensCall {
                    amountIn: self.loan_amount,
                    amountOutMin: U256::ZERO,
                    path: vec![node.token0, node.token1],
                    to: self.contract_address,
                    deadline: U256::from(u64::MAX),
                }.abi_encode()
            };

            payloads.push(payload);
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
    contract IAerodromeFactory {
        function getPool(address tokenA, address tokenB, bool stable) external view returns (address pool);
        function allPairs(uint256) external view returns (address pair);
        function allPairsLength() external view returns (uint256);
    }

    #[sol(rpc)]
    contract IUniswapV2Factory {
        function allPairs(uint256) external view returns (address pair);
        function allPairsLength() external view returns (uint256);
        function getPair(address tokenA, address tokenB) external view returns (address pair);
    }

    #[sol(rpc)]
    contract IUniswapV3Factory {
        function getPool(address tokenA, address token2, uint24 fee) external view returns (address pool);
    }

    #[sol(rpc)]
    contract IUniswapV2Pair {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
        function token0() external view returns (address);
        function token1() external view returns (address);
    }

    #[sol(rpc)]
    contract IUniswapV3Pool {
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint32 feeProtocol, bool unlocked);
        function token0() external view returns (address);
        function token1() external view returns (address);
    }

    #[sol(rpc)]
    contract IERC20 {
        function decimals() external view returns (uint8);
    }

    #[sol(rpc)]
    contract IUniswapV2Router02 {
        function swapExactTokensForTokens(
            uint256 amountIn,
            uint256 amountOutMin,
            address[] calldata path,
            address to,
            uint256 deadline
        ) external returns (uint256[] memory amounts);
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
        function triggerAaveArbitrage(address tokenToBorrow, uint256 loanAmount, bytes calldata swapPathData) external;
    }
}

async fn fetch_dynamic_pools<P>(
    _http_provider: P,
) -> Result<Vec<Address>, Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
{
    Ok(vec![
        address!("B4885Bc4757b22775b47c0b31a24d588865d64c1"),
        address!("d0b53D9277d7407987AF31b2A31BCCD68F5d7a75"),
        address!("0606B4916aFDb81249bCBF51A18671b26CCf7D6C"),
        address!("2A1c9966EEb6D649AA3b49F4472d24CE7F99FdAc"),
        address!("7f670f66e028D139613b482276563c65c69AD752"),
        address!("16629737119B61E47eDb8095AA508823f9E42Bdf"),
    ])
}

struct AssetArbitrageOpportunity {
    token: Address,
    _price_aero: f64,
    _price_v3: f64,
    spread_gap: f64,
    _aero_pool: Address,
    _v3_pool: Address,
    loan_amount: U256,
}

async fn fetch_live_market_data<P>(
    http_provider: P,
    _cached_pools: &[Address],
) -> Result<(f64, Address, U256, Address, Address, Vec<Address>), Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
{
    let mut opportunities = Vec::new();
    let mut discovered_pools = Vec::new();

    // 1. USDC / WETH Pair
    let aero_usdc_pool = address!("B4885Bc4757b22775b47c0b31a24d588865d64c1");
    let v3_usdc_pool = address!("d0b53D9277d7407987AF31b2A31BCCD68F5d7a75");
    discovered_pools.push(aero_usdc_pool);
    discovered_pools.push(v3_usdc_pool);

    let aero_pair_usdc = IUniswapV2Pair::new(aero_usdc_pool, http_provider.clone());
    let v3_pool_usdc = IUniswapV3Pool::new(v3_usdc_pool, http_provider.clone());

    let aero_builder_usdc = aero_pair_usdc.getReserves();
    let v3_builder_usdc = v3_pool_usdc.slot0();
    let aero_fut_usdc = aero_builder_usdc.call();
    let v3_fut_usdc = v3_builder_usdc.call();

    if let Ok((res_aero, slot0_v3)) = tokio::try_join!(
        aero_fut_usdc,
        v3_fut_usdc
    ) {
        let r0 = res_aero.reserve0.to::<u128>() as f64; // USDC
        let r1 = res_aero.reserve1.to::<u128>() as f64; // WETH
        let price_aero = if r0 > 0.0 { (r1 / r0) * 10f64.powi(6 - 18) } else { 0.0 };

        let sqrt_price_x96 = slot0_v3.sqrtPriceX96;
        if U256::from(sqrt_price_x96) > U256::ZERO {
            let raw_ratio = (sqrt_price_x96.to_string().parse::<f64>().unwrap_or(0.0) / 2f64.powi(96)).powi(2);
            let price_v3 = raw_ratio * 10f64.powi(6 - 18);

            if price_aero > 0.0 && price_v3 > 0.0 {
                let spread_gap = (price_aero - price_v3).abs();
                opportunities.push(AssetArbitrageOpportunity {
                    token: USDC_BASE,
                    _price_aero: price_aero,
                    _price_v3: price_v3,
                    spread_gap,
                    _aero_pool: aero_usdc_pool,
                    _v3_pool: v3_usdc_pool,
                    loan_amount: U256::from(10_000_000_000_000_000u64),
                });
            }
        }
    }

    // 2. cbBTC / WETH Pair
    let aero_cbbtc_pool = address!("0606B4916aFDb81249bCBF51A18671b26CCf7D6C");
    let v3_cbbtc_pool = address!("2A1c9966EEb6D649AA3b49F4472d24CE7F99FdAc");
    discovered_pools.push(aero_cbbtc_pool);
    discovered_pools.push(v3_cbbtc_pool);

    let aero_pair_cbbtc = IUniswapV2Pair::new(aero_cbbtc_pool, http_provider.clone());
    let v3_pool_cbbtc = IUniswapV3Pool::new(v3_cbbtc_pool, http_provider.clone());

    let aero_builder_cbbtc = aero_pair_cbbtc.getReserves();
    let v3_builder_cbbtc = v3_pool_cbbtc.slot0();
    let aero_fut_cbbtc = aero_builder_cbbtc.call();
    let v3_fut_cbbtc = v3_builder_cbbtc.call();

    if let Ok((res_aero, slot0_v3)) = tokio::try_join!(
        aero_fut_cbbtc,
        v3_fut_cbbtc
    ) {
        let r0 = res_aero.reserve0.to::<u128>() as f64; // cbBTC
        let r1 = res_aero.reserve1.to::<u128>() as f64; // WETH
        let price_aero = if r0 > 0.0 { (r1 / r0) * 10f64.powi(8 - 18) } else { 0.0 };

        let sqrt_price_x96 = slot0_v3.sqrtPriceX96;
        if U256::from(sqrt_price_x96) > U256::ZERO {
            let raw_ratio = (sqrt_price_x96.to_string().parse::<f64>().unwrap_or(0.0) / 2f64.powi(96)).powi(2);
            let price_v3 = raw_ratio * 10f64.powi(8 - 18);

            if price_aero > 0.0 && price_v3 > 0.0 {
                let spread_gap = (price_aero - price_v3).abs();
                opportunities.push(AssetArbitrageOpportunity {
                    token: CBBTC_BASE,
                    _price_aero: price_aero,
                    _price_v3: price_v3,
                    spread_gap,
                    _aero_pool: aero_cbbtc_pool,
                    _v3_pool: v3_cbbtc_pool,
                    loan_amount: U256::from(1_000_000_000_000_000u64),
                });
            }
        }
    }

    // 3. AERO / WETH Pair
    let aero_aero_pool = address!("7f670f66e028D139613b482276563c65c69AD752");
    let v3_aero_pool = address!("16629737119B61E47eDb8095AA508823f9E42Bdf");
    discovered_pools.push(aero_aero_pool);
    discovered_pools.push(v3_aero_pool);

    let aero_pair_aero = IUniswapV2Pair::new(aero_aero_pool, http_provider.clone());
    let v3_pool_aero = IUniswapV3Pool::new(v3_aero_pool, http_provider.clone());

    let aero_builder_aero = aero_pair_aero.getReserves();
    let v3_builder_aero = v3_pool_aero.slot0();
    let aero_fut_aero = aero_builder_aero.call();
    let v3_fut_aero = v3_builder_aero.call();

    if let Ok((res_aero, slot0_v3)) = tokio::try_join!(
        aero_fut_aero,
        v3_fut_aero
    ) {
        let r0 = res_aero.reserve0.to::<u128>() as f64;
        let r1 = res_aero.reserve1.to::<u128>() as f64;
        let price_aero = if r0 > 0.0 { r1 / r0 } else { 0.0 };

        let sqrt_price_x96 = slot0_v3.sqrtPriceX96;
        if U256::from(sqrt_price_x96) > U256::ZERO {
            let raw_ratio = (sqrt_price_x96.to_string().parse::<f64>().unwrap_or(0.0) / 2f64.powi(96)).powi(2);
            let price_v3 = raw_ratio;

            if price_aero > 0.0 && price_v3 > 0.0 {
                let spread_gap = (price_aero - price_v3).abs();
                opportunities.push(AssetArbitrageOpportunity {
                    token: AERO_BASE,
                    _price_aero: price_aero,
                    _price_v3: price_v3,
                    spread_gap,
                    _aero_pool: aero_aero_pool,
                    _v3_pool: v3_aero_pool,
                    loan_amount: U256::from(10_000_000_000_000_000u64),
                });
            }
        }
    }

    if opportunities.is_empty() {
        println!("⚠ [DEBUG] opportunities is empty. All hardcoded pair checks failed or returned zero reserves.");
        let fallback_pool = discovered_pools.first().copied().unwrap_or(Address::ZERO);
        return Ok((0.0, WETH_BASE, U256::ZERO, WETH_BASE, USDC_BASE, vec![fallback_pool]));
    }

    opportunities.sort_by(|a, b| b.spread_gap.partial_cmp(&a.spread_gap).unwrap_or(std::cmp::Ordering::Equal));
    let best = &opportunities[0];

    Ok((
        best.spread_gap,
        best.token,
        best.loan_amount,
        best.token,
        WETH_BASE,
        discovered_pools,
    ))
}

async fn trigger_on_chain_arbitrage<P>(
    http_provider: P,
    contract_address: Address,
    target_path: (Vec<Address>, Vec<Vec<u8>>),
    signer_address: Address,
    token_to_borrow: Address,
    loan_amount: U256,
) -> Result<(), Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
{
    if loan_amount == U256::ZERO || target_path.0.is_empty() {
        println!("🛡 [PRE-FLIGHT SHIELD] Invalid loan or empty path. Skipping execution.");
        return Ok(());
    }

    println!("🚀 [BOT -> CONTRACT] Executing Atomic Multi-Swap Command!");
    println!("🔗 Atomic Route Dispatched: Targets: {:?}, Payloads Count: {}", target_path.0, target_path.1.len());

    let inner_swap_path_tuple = alloy::dyn_abi::DynSolValue::Tuple(vec![
        alloy::dyn_abi::DynSolValue::Array(target_path.0.clone().into_iter().map(alloy::dyn_abi::DynSolValue::Address).collect()),
        alloy::dyn_abi::DynSolValue::Array(target_path.1.into_iter().map(|p| alloy::dyn_abi::DynSolValue::Bytes(p.into())).collect()),
    ]);

    let swap_path_data = alloy::dyn_abi::DynSolValue::Tuple(vec![
        alloy::dyn_abi::DynSolValue::Address(signer_address),
        alloy::dyn_abi::DynSolValue::Uint(loan_amount, 256),
        alloy::dyn_abi::DynSolValue::Address(token_to_borrow),
        alloy::dyn_abi::DynSolValue::Bytes(inner_swap_path_tuple.abi_encode()),
    ]).abi_encode();

    let contract = BaseAtomicArbitrage::new(contract_address, http_provider.clone());

    let balancer_builder = contract.triggerBalancerArbitrage(token_to_borrow, loan_amount, swap_path_data.clone().into())
        .from(signer_address);

    match balancer_builder.call().await {
        Ok(_) => {
            println!("✅ Balancer Simulation Passed Successfully! Dispatching Real Transaction...");
            let pending_tx = balancer_builder.send().await?;
            let receipt = pending_tx.get_receipt().await?;
            println!("✅ Transaction Mined In Block: {:?}", receipt.block_number);
        }
        Err(e_balancer) => {
            println!("⚠ Balancer Simulation Failed ({:?}). Activating Aave Fallback Route...", e_balancer);
            sleep(Duration::from_millis(150)).await;

            let aave_builder = contract.triggerAaveArbitrage(token_to_borrow, loan_amount, swap_path_data.into())
                .from(signer_address);

            match aave_builder.call().await {
                Ok(_) => {
                    println!("✅ Aave Simulation Passed Successfully! Dispatching Real Transaction...");
                    let pending_tx = aave_builder.send().await?;
                    let receipt = pending_tx.get_receipt().await?;
                    println!("✅ Transaction Mined In Block: {:?}", receipt.block_number);
                }
                Err(e_aave) => {
                    println!("❌ Both Simulations Failed. Balancer: {:?}, Aave: {:?}", e_balancer, e_aave);
                }
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Initializing Universal Dynamic Token Scanner Core via Alloy...");

    let contract_addr_str = std::env::var("CONTRACT_ADDR")
        .unwrap_or_else(|_| "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string());
    let contract_address: Address = contract_addr_str.parse()?;

    let alchemy_http_urls = vec![
        std::env::var("ALCHEMY_HTTP_URL").unwrap_or_else(|_| "http://127.0.0.1:8545".to_string()),
        std::env::var("BACKUP_HTTP_URL").unwrap_or_else(|_| "https://mainnet.base.org".to_string()),
    ];
    let primary_http_url = alchemy_http_urls[0].clone();

    let alchemy_wss_url = std::env::var("ALCHEMY_WSS_URL")
        .unwrap_or_else(|_| "ws://127.0.0.1:8545".to_string());
    let private_key_str = std::env::var("PRIVATE_KEY")
        .unwrap_or_else(|_| "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string());

    let signer: PrivateKeySigner = private_key_str.parse()?;
    let signer_address = signer.address();
    let wallet = EthereumWallet::from(signer);

    println!("📡 Activating HTTP Connection to: {}", primary_http_url);
    let http_provider = ProviderBuilder::new()
        .wallet(wallet.clone())
        .connect_http(primary_http_url.parse()?);

    println!("📡 Activating WebSocket Connection to: {}", alchemy_wss_url);
    let ws = alloy::providers::WsConnect::new(alchemy_wss_url);
    let ws_provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_ws(ws)
        .await?;

    println!("🔍 [INIT CACHE] Initializing direct high-performance pool cache...");
    let cached_pools = match fetch_dynamic_pools(http_provider.clone()).await {
        Ok(pools) => {
            println!("✅ Successfully initialized {} direct pool addresses in memory.", pools.len());
            pools
        }
        Err(_) => {
            vec![]
        }
    };

    let sub = ws_provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();

    let mut radar = MachineMetric::new();
    let mut block_counter = 0u64;

    while let Some(block) = stream.next().await {
        block_counter += 1;
        let block_num = block.inner.number;
        println!("📦 Live WSS Block Synced: #{} (Internal counter: {})", block_num, block_counter);

        let (live_market_price, dynamic_token, dynamic_loan, token0, token1, scanned_addresses) = 
            fetch_live_market_data(http_provider.clone(), &cached_pools).await?;

        println!("   📊 [METRIC FEED] Cross-DEX Spread Gap: {:.6}, Checking Velocity Pivots...", live_market_price);

        if live_market_price > 0.0 {
            let (direction, velocity) = radar.update_and_predict(live_market_price);

            if direction == Direction::Peak || direction == Direction::Bottom {
                println!("⚡ [RADAR ALERT] Velocity Pivot Discovered: {:.4}", velocity);
                let nodes = vec![
                    QuantumNode { id: 1, energy_scale: generate_astronomical_number(1000usize), frequency: live_market_price, token0, token1 },
                    QuantumNode { id: 2, energy_scale: generate_astronomical_number(1000usize), frequency: 0.01, token0, token1 },
                    QuantumNode { id: 3, energy_scale: generate_astronomical_number(1000usize), frequency: 0.015, token0, token1 },
                ];

                let system = CausalCollapseSystem::new(nodes, scanned_addresses, contract_address, dynamic_loan);
                let optimized_path = system.execute_collapse();

                if let Err(e) = trigger_on_chain_arbitrage(http_provider.clone(), contract_address, optimized_path, signer_address, dynamic_token, dynamic_loan).await {
                    println!("❌ Error executing on-chain command: {:?}", e);
                }
            }
        }

        sleep(Duration::from_millis(100)).await;
    }

    println!("🏁 Live stream processing terminated.");
    Ok(())
}
