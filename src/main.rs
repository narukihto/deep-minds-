use num_bigint::BigUint;
use num_traits::{One, ToPrimitive};
use rayon::prelude::*;
use std::time::Instant;
use futures_util::StreamExt;
use tokio::time::{sleep, Duration};
use alloy::{
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    network::{EthereumWallet, Ethereum},
    primitives::{address, Address, U256, Bytes, Uint},
    sol, sol_types::SolCall,
};

type U24 = Uint<24, 1>;
type U160 = Uint<160, 3>;

const WETH_BASE: Address = address!("4200000000000000000000000000000000000006");
const USDC_BASE: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bda02913");
const CBBTC_BASE: Address = address!("cbB7C7A63551000b48A8503b87936a229a4b3FE3");
const AERO_BASE: Address = address!("940181a94A35A4569E4529A3CDfB74e38FD98631");

// --- تم توحيد العناوين 100% لتطابق القائمة البيضاء (Whitelisted Targets) في العقد الذكي ---
const UNISWAP_V3_ROUTER: Address = address!("0xcf77A3bA9Aab7D3E44917635033322DF3f564171");
const AERODROME_ROUTER: Address = address!("0x2626664c2603336E57B271c5C0b26F421741e481");

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
                self.last_state = Some(SystemState {
                    value: current_value,
                    timestamp: now,
                });
                return (direction, v_inst);
            }
        }
        self.last_state = Some(SystemState {
            value: current_value,
            timestamp: now,
        });
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
    pub borrowed_token: Address,
}

impl CausalCollapseSystem {
    pub fn new(
        nodes: Vec<QuantumNode>,
        dynamic_pools: Vec<Address>,
        contract_address: Address,
        loan_amount: U256,
        borrowed_token: Address,
    ) -> Self {
        Self {
            nodes,
            threshold_limit: 0.02,
            buffer_capacity: 16,
            dynamic_pools,
            contract_address,
            loan_amount,
            borrowed_token,
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

        let active_nodes: Vec<QuantumNode> = ordered_nodes
            .par_iter()
            .map(|node| {
                let mut triggered = node.clone();
                if triggered.frequency == 0.0 {
                    triggered.frequency = 0.01;
                }
                triggered
            })
            .collect();

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
                let stable_projected =
                    self.project_to_inverse_dimensional_symmetry(current_avg_freq, i - 1);
                let next_projected =
                    self.project_to_inverse_dimensional_symmetry(next.frequency, i);
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
            let scale_factor =
                1.0 / (buffered_node.energy_scale.to_f64().unwrap_or(1.0) + 1.0);
            if pure_raw_dev * scale_factor <= self.threshold_limit {
                final_path.push(buffered_node.clone());
            }
        }

        let mut addresses = Vec::new();
        let mut payloads = Vec::new();

        // الخطوة الأولى: مسار الشراء الأساسي (مثلاً من التوكن المقترض إلى الـ WETH)
        let primary_node = &final_path[0];
        let pool = effective_pools[0];
        addresses.push(pool);

        let payload = if pool == UNISWAP_V3_ROUTER {
            let exact_input_params = IUniswapV3Router::ExactInputParams {
                path: {
                    let mut packed = Vec::new();
                    packed.extend_from_slice(primary_node.token0.as_slice());
                    packed.extend_from_slice(&[0x00, 0x0b, 0xb8]);
                    packed.extend_from_slice(primary_node.token1.as_slice());
                    Bytes::from(packed)
                },
                recipient: self.contract_address,
                deadline: U256::from(u64::MAX),
                amountIn: self.loan_amount,
                amountOutMinimum: U256::ZERO,
            };
            IUniswapV3Router::exactInputCall {
                params: exact_input_params,
            }
            .abi_encode()
        } else if pool == AERODROME_ROUTER {
            let aero_route = IAerodromeRouter::Route {
                from: primary_node.token0,
                to: primary_node.token1,
                stable: false,
                factory: AERODROME_FACTORY,
            };
            IAerodromeRouter::swapExactTokensForTokensCall {
                amountIn: self.loan_amount,
                amountOutMin: U256::ZERO,
                routes: vec![aero_route],
                to: self.contract_address,
                deadline: U256::from(u64::MAX),
            }
            .abi_encode()
        } else {
            IUniswapV2Router02::swapExactTokensForTokensCall {
                amountIn: self.loan_amount,
                amountOutMin: U256::ZERO,
                path: vec![primary_node.token0, primary_node.token1],
                to: self.contract_address,
                deadline: U256::from(u64::MAX),
            }
            .abi_encode()
        };
        payloads.push(payload);

        // الخطوة الثانية: مسار العودة الإجباري (Return Path) لإغلاق الدورة وإعادة العملة المقترضة للعقد
        if primary_node.token1 != self.borrowed_token {
            let pool_return = effective_pools[1 % effective_pools.len()];
            addresses.push(pool_return);

            let return_payload = if pool_return == UNISWAP_V3_ROUTER {
                let return_params = IUniswapV3Router::ExactInputParams {
                    path: {
                        let mut packed = Vec::new();
                        packed.extend_from_slice(primary_node.token1.as_slice());
                        packed.extend_from_slice(&[0x00, 0x0b, 0xb8]);
                        packed.extend_from_slice(self.borrowed_token.as_slice());
                        Bytes::from(packed)
                    },
                    recipient: self.contract_address,
                    deadline: U256::from(u64::MAX),
                    amountIn: self.loan_amount,
                    amountOutMinimum: U256::ZERO,
                };
                IUniswapV3Router::exactInputCall { params: return_params }.abi_encode()
            } else {
                let return_route = IAerodromeRouter::Route {
                    from: primary_node.token1,
                    to: self.borrowed_token,
                    stable: false,
                    factory: AERODROME_FACTORY,
                };
                IAerodromeRouter::swapExactTokensForTokensCall {
                    amountIn: self.loan_amount,
                    amountOutMin: U256::ZERO,
                    routes: vec![return_route],
                    to: self.contract_address,
                    deadline: U256::from(u64::MAX),
                }
                .abi_encode()
            };
            payloads.push(return_payload);
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
    contract IUniswapV3Quoter {
        function quoteExactInputSingle(
            address tokenIn,
            address tokenOut,
            uint24 fee,
            uint256 amountIn,
            uint160 sqrtPriceLimitX96
        ) external returns (uint256 amountOut);
    }

    #[sol(rpc)]
    contract IAerodromeQuoter {
        function quoteExactInputSingle(
            address tokenIn,
            address tokenOut,
            bool stable,
            uint256 amountIn
        ) external view returns (uint256 amountOut);
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
        UNISWAP_V3_ROUTER,
        AERODROME_ROUTER,
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

// --- حساب رسوم الغاز الديناميكي اللحظي على شبكة Base لضمان ربحية صافية حقيقية ---
async fn calculate_dynamic_net_profit_threshold<P>(
    provider: &P,
    loan_amount: U256,
) -> Result<f64, Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
{
    let gas_price = provider.get_gas_price().await.unwrap_or(100_000_000);
    let estimated_gas_limit: u128 = 350_000;
    let gas_cost_wei = gas_price * estimated_gas_limit;

    let loan_f64 = loan_amount.to_string().parse::<f64>().unwrap_or(1.0);
    let flash_fee_estimated = loan_f64 * 0.0009; // 0.09% Aave max fee

    let total_cost_threshold = (gas_cost_wei as f64 + flash_fee_estimated) / loan_f64;
    Ok(total_cost_threshold + 0.0005)
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

    discovered_pools.push(UNISWAP_V3_ROUTER);
    discovered_pools.push(AERODROME_ROUTER);

    let v3_quoter = IUniswapV3Quoter::new(UNISWAP_V3_ROUTER, http_provider.clone());
    let aero_quoter = IAerodromeQuoter::new(AERODROME_ROUTER, http_provider.clone());

    // 1. USDC / WETH Pair
    let amount_in_usdc = U256::from(1_000_000u64);
    let loan_amount_usdc = U256::from(10_000_000_000_000_000u64);
    let dynamic_threshold = calculate_dynamic_net_profit_threshold(&http_provider, loan_amount_usdc).await.unwrap_or(0.001);

    let aero_quote_usdc = aero_quoter.quoteExactInputSingle(USDC_BASE, WETH_BASE, false, amount_in_usdc).call().await;
    let v3_quote_usdc = v3_quoter.quoteExactInputSingle(USDC_BASE, WETH_BASE, U24::from(500u32), amount_in_usdc, U160::ZERO).call().await;

    if let (Ok(out_aero), Ok(out_v3)) = (aero_quote_usdc, v3_quote_usdc) {
        let price_aero = out_aero.to_string().parse::<f64>().unwrap_or(0.0) / 10f64.powi(18);
        let price_v3 = out_v3.to_string().parse::<f64>().unwrap_or(0.0) / 10f64.powi(18);
        let raw_spread = (price_aero - price_v3).abs();

        if raw_spread > dynamic_threshold && price_aero > 0.0 && price_v3 > 0.0 {
            opportunities.push(AssetArbitrageOpportunity {
                token: USDC_BASE,
                _price_aero: price_aero,
                _price_v3: price_v3,
                spread_gap: raw_spread,
                _aero_pool: AERODROME_ROUTER,
                _v3_pool: UNISWAP_V3_ROUTER,
                loan_amount: loan_amount_usdc,
            });
        }
    }

    // 2. cbBTC / WETH Pair
    let amount_in_cbbtc = U256::from(100_000_000u64);
    let loan_amount_cbbtc = U256::from(1_000_000_000_000_000u64);

    let aero_quote_cbbtc = aero_quoter.quoteExactInputSingle(CBBTC_BASE, WETH_BASE, false, amount_in_cbbtc).call().await;
    let v3_quote_cbbtc = v3_quoter.quoteExactInputSingle(CBBTC_BASE, WETH_BASE, U24::from(500u32), amount_in_cbbtc, U160::ZERO).call().await;

    if let (Ok(out_aero), Ok(out_v3)) = (aero_quote_cbbtc, v3_quote_cbbtc) {
        let price_aero = out_aero.to_string().parse::<f64>().unwrap_or(0.0) / 10f64.powi(18);
        let price_v3 = out_v3.to_string().parse::<f64>().unwrap_or(0.0) / 10f64.powi(18);
        let raw_spread = (price_aero - price_v3).abs();

        if raw_spread > dynamic_threshold && price_aero > 0.0 && price_v3 > 0.0 {
            opportunities.push(AssetArbitrageOpportunity {
                token: CBBTC_BASE,
                _price_aero: price_aero,
                _price_v3: price_v3,
                spread_gap: raw_spread,
                _aero_pool: AERODROME_ROUTER,
                _v3_pool: UNISWAP_V3_ROUTER,
                loan_amount: loan_amount_cbbtc,
            });
        }
    }

    // 3. AERO / WETH Pair
    let amount_in_aero = U256::from(1_000_000_000_000_000_000u64);
    let loan_amount_aero = U256::from(10_000_000_000_000_000u64);

    let aero_quote_aero = aero_quoter.quoteExactInputSingle(AERO_BASE, WETH_BASE, false, amount_in_aero).call().await;
    let v3_quote_aero = v3_quoter.quoteExactInputSingle(AERO_BASE, WETH_BASE, U24::from(3000u32), amount_in_aero, U160::ZERO).call().await;

    if let (Ok(out_aero), Ok(out_v3)) = (aero_quote_aero, v3_quote_aero) {
        let price_aero = out_aero.to_string().parse::<f64>().unwrap_or(0.0) / 10f64.powi(18);
        let price_v3 = out_v3.to_string().parse::<f64>().unwrap_or(0.0) / 10f64.powi(18);
        let raw_spread = (price_aero - price_v3).abs();

        if raw_spread > dynamic_threshold && price_aero > 0.0 && price_v3 > 0.0 {
            opportunities.push(AssetArbitrageOpportunity {
                token: AERO_BASE,
                _price_aero: price_aero,
                _price_v3: price_v3,
                spread_gap: raw_spread,
                _aero_pool: AERODROME_ROUTER,
                _v3_pool: UNISWAP_V3_ROUTER,
                loan_amount: loan_amount_aero,
            });
        }
    }

    if opportunities.is_empty() {
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
        alloy::dyn_abi::DynSolValue::Array(
            target_path.0.clone().into_iter().map(alloy::dyn_abi::DynSolValue::Address).collect()
        ),
        alloy::dyn_abi::DynSolValue::Array(
            target_path.1.into_iter().map(|p| alloy::dyn_abi::DynSolValue::Bytes(p.into())).collect()
        ),
    ]);

    let swap_path_data = alloy::dyn_abi::DynSolValue::Tuple(vec![
        alloy::dyn_abi::DynSolValue::Address(signer_address),
        alloy::dyn_abi::DynSolValue::Uint(loan_amount, 256),
        alloy::dyn_abi::DynSolValue::Address(token_to_borrow),
        alloy::dyn_abi::DynSolValue::Bytes(inner_swap_path_tuple.abi_encode()),
    ]).abi_encode();

    let contract = BaseAtomicArbitrage::new(contract_address, http_provider.clone());

    let balancer_builder = contract
        .triggerBalancerArbitrage(token_to_borrow, loan_amount, swap_path_data.clone().into())
        .from(signer_address);

    match balancer_builder.call().await {
        Ok(_) => {
            println!("✅ Balancer Simulation Passed Successfully! Dispatching Real Transaction...");
            let pending_tx = balancer_builder.send().await?;
            let receipt = pending_tx.get_receipt().await?;
            println!("✅ Transaction Mined In Block: {:?}", receipt.block_number);
        }
        Err(e_balancer) => {
            println!("⚠ Balancer Simulation Failed. Reason details: {:?}. Activating Aave Fallback Route...", e_balancer);
            sleep(Duration::from_millis(150)).await;

            let aave_builder = contract
                .triggerAaveArbitrage(token_to_borrow, loan_amount, swap_path_data.into())
                .from(signer_address);

            match aave_builder.call().await {
                Ok(_) => {
                    println!("✅ Aave Simulation Passed Successfully! Dispatching Real Transaction...");
                    let pending_tx = aave_builder.send().await?;
                    let receipt = pending_tx.get_receipt().await?;
                    println!("✅ Transaction Mined In Block: {:?}", receipt.block_number);
                }
                Err(e_aave) => {
                    println!("❌ Both Simulations Failed. Balancer Revert: {:?}, Aave Revert: {:?}", e_balancer, e_aave);
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
        std::env::var("ALCHEMY_HTTP_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8545".to_string()),
        std::env::var("BACKUP_HTTP_URL")
            .unwrap_or_else(|_| "https://mainnet.base.org".to_string()),
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

        println!(" 📊 [METRIC FEED] Cross-DEX Spread Gap: {:.6}, Checking Velocity Pivots...", live_market_price);

        if live_market_price > 0.0 {
            let (direction, velocity) = radar.update_and_predict(live_market_price);
            if direction == Direction::Peak || direction == Direction::Bottom {
                println!("⚡ [RADAR ALERT] Velocity Pivot Discovered: {:.4}", velocity);

                let nodes = vec![
                    QuantumNode {
                        id: 1,
                        energy_scale: generate_astronomical_number(1000usize),
                        frequency: live_market_price,
                        token0,
                        token1,
                    },
                    QuantumNode {
                        id: 2,
                        energy_scale: generate_astronomical_number(1000usize),
                        frequency: 0.01,
                        token0,
                        token1,
                    },
                    QuantumNode {
                        id: 3,
                        energy_scale: generate_astronomical_number(1000usize),
                        frequency: 0.015,
                        token0,
                        token1,
                    },
                ];

                let system = CausalCollapseSystem::new(
                    nodes,
                    scanned_addresses,
                    contract_address,
                    dynamic_loan,
                    dynamic_token,
                );

                let optimized_path = system.execute_collapse();

                if let Err(e) = trigger_on_chain_arbitrage(
                    http_provider.clone(),
                    contract_address,
                    optimized_path,
                    signer_address,
                    dynamic_token,
                    dynamic_loan,
                ).await {
                    println!("❌ Error executing on-chain command: {:?}", e);
                }
            }
        }

        sleep(Duration::from_millis(100)).await;
    }

    println!("🏁 Live stream processing terminated.");
    Ok(())
}
