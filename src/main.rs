use num_bigint::BigUint;
use num_traits::One;
use futures_util::StreamExt;
use tokio::time::{sleep, Duration};
use std::str::FromStr;
use alloy::{
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    network::{EthereumWallet, Ethereum},
    primitives::{address, Address, U256, Bytes, Uint},
    sol,
    sol_types::{SolCall, SolInterface},
};
use std::collections::HashMap;

type _U24 = Uint<24, 1>;
type _U160 = Uint<160, 3>;

pub const WETH_BASE: Address = address!("4200000000000000000000000000000000000006");
pub const USDC_BASE: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913");
pub const CBETH_BASE: Address = address!("2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc22");
pub const VIRTUAL_BASE: Address = address!("0b3e328455c4059EEb9e3f84b5543F74E24e7E1b");

const UNISWAP_V3_FACTORY: Address = address!("33128a8fC17869897dcE68Ed026d694621f6FDfD");

// العناوين المطابقة 100% للعقد المنشور
const UNISWAP_V3_ROUTER: Address = address!("0xcf77A3bA9Aab7D3E44917635033322DF3f564171");
const AERODROME_ROUTER: Address = address!("0x2626664c2603336E57B271c5C0b26F421741e481");
const AERODROME_V2_FACTORY: Address = address!("0x420DD381b31aEf6683db6B902084cB0FFECe40Da");
const PANCAKESWAP_V3_ROUTER: Address = address!("0x1b81D678ffb9C0263b24A97847620C99d213eB14"); 

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

    pub fn execute_collapse(&self) -> (Vec<Address>, Vec<Vec<u8>>) {
        let mut addresses = Vec::new();
        let mut payloads = Vec::new();

        if self.price_aero <= 0.0 || self.price_v3 <= 0.0 {
            return (vec![], vec![]);
        }

        let flash_loan_fee = self.loan_amount / U256::from(10000); 
        let loan_usdc_f64 = self.loan_amount.to_string().parse::<f64>().unwrap_or(0.0) / 1_000_000.0;
        let buy_on_univ3 = self.price_v3 < self.price_aero;
        let buy_price = if buy_on_univ3 { self.price_v3 } else { self.price_aero };

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
    graph.add_pool(Pool { id: address!("cdac0d6c6c59727a65f871236188350531885c43"), token0: WETH_BASE, token1: USDC_BASE, dex_type: DexType::Aerodrome, fee: 500 });
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
                DexType::UniswapV3 | DexType::PancakeSwapV3 => UNISWAP_V3_ROUTER,
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
        function token0() external view returns (address);
        function token1() external view returns (address);
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

async fn fetch_v3_pool<P: Provider<Ethereum> + Clone>(
    uni_factory: &IUniswapV3Factory::IUniswapV3FactoryInstance<P>,
    token_a: Address,
    token_b: Address,
    fee: _U24,
) -> Address {
    match uni_factory.getPool(token_a, token_b, fee).call().await {
        Ok(res) => { let addr: Address = res.0.into(); addr },
        Err(_) => Address::ZERO,
    }
}

async fn get_monitored_pairs<P>(http_provider: P) -> Result<Vec<TokenPairConfig>, Box<dyn std::error::Error>>
where P: Provider<Ethereum> + Clone {
    let uni_factory = IUniswapV3Factory::new(UNISWAP_V3_FACTORY, http_provider.clone());
    let fee_tier_500 = _U24::from(500);
    let fee_tier_10000 = _U24::from(10000);

    Ok(vec![
        TokenPairConfig {
            symbol: "WETH",
            target_token: WETH_BASE,
            aero_pool: address!("cdac0d6c6c59727a65f871236188350531885c43"),
            uni_v3_pool: fetch_v3_pool(&uni_factory, WETH_BASE, USDC_BASE, fee_tier_500).await,
            baseswap_pool: Address::ZERO,
            pancakeswap_pool: Address::ZERO,
            decimals: 18,
            fee_tier_v3: 500,
            token0: WETH_BASE,
            token1: USDC_BASE,
            is_v3_aero: false,
            is_v3_baseswap: false,
        },
        TokenPairConfig {
            symbol: "VIRTUAL",
            target_token: VIRTUAL_BASE,
            aero_pool: address!("3f0296bf652e19bca772ec3df08b32732f93014a"),
            uni_v3_pool: fetch_v3_pool(&uni_factory, VIRTUAL_BASE, WETH_BASE, fee_tier_10000).await,
            baseswap_pool: Address::ZERO,
            pancakeswap_pool: Address::ZERO,
            decimals: 18,
            fee_tier_v3: 10000,
            token0: VIRTUAL_BASE,
            token1: WETH_BASE,
            is_v3_aero: true,
            is_v3_baseswap: false,
        },
        TokenPairConfig {
            symbol: "cbETH",
            target_token: CBETH_BASE,
            aero_pool: address!("47cA96EA59C13f72745928887F84c9F52C3D7348"),
            uni_v3_pool: fetch_v3_pool(&uni_factory, WETH_BASE, CBETH_BASE, fee_tier_500).await,
            baseswap_pool: Address::ZERO,
            pancakeswap_pool: Address::ZERO,
            decimals: 18,
            fee_tier_v3: 500,
            token0: CBETH_BASE,
            token1: WETH_BASE,
            is_v3_aero: true,
            is_v3_baseswap: false,
        },
    ])
}

fn calculate_v2_price(reserve0: u128, reserve1: u128, token0: Address, token1: Address, target_decimals: u32, weth_price_usd: f64) -> f64 {
    if reserve0 == 0 || reserve1 == 0 { return 0.0; }
    let usdc_decimals = 6;
    let d0 = if token0 == USDC_BASE { usdc_decimals } else { target_decimals };
    let d1 = if token1 == USDC_BASE { usdc_decimals } else { target_decimals };
    let r0 = reserve0 as f64 / 10f64.powi(d0 as i32);
    let r1 = reserve1 as f64 / 10f64.powi(d1 as i32);
    let price = if r0 > 0.0 { r1 / r0 } else { 0.0 };

    if token0 == USDC_BASE || token1 == USDC_BASE {
        return if token0 == USDC_BASE { if price > 0.0 { 1.0 / price } else { 0.0 } } else { price };
    }
    if token0 == WETH_BASE || token1 == WETH_BASE {
        let weth_ratio = if token0 == WETH_BASE { if r1 > 0.0 { r0 / r1 } else { 0.0 } } else { price };
        return weth_ratio * weth_price_usd;
    }
    price
}

fn calculate_v3_price_dynamic(sqrt_price_x96: _U160, pool_token0: Address, target_token: Address, weth_price_usd: f64, symbol: &str) -> f64 {
    let sqrt_f64 = sqrt_price_x96.to_string().parse::<f64>().unwrap_or(0.0);
    if sqrt_f64 <= 0.0 { return 0.0; }

    let ratio = sqrt_f64 / 2.0f64.powi(96);
    let price_raw = ratio * ratio;
    if price_raw <= 0.0 { return 0.0; }

    if symbol == "WETH" {
        let price = price_raw * 10f64.powi(12);
        return if price > 100.0 && price < 20000.0 { price } else { if price > 0.0 { 1.0 / price * 10f64.powi(12) } else { 0.0 } };
    }

    let is_target_token0 = pool_token0 == target_token;

    let price_in_weth = if is_target_token0 {
        price_raw 
    } else {
        if price_raw > 0.0 { 1.0 / price_raw } else { 0.0 } 
    };

    let calculated_usd = price_in_weth * weth_price_usd;

    let final_price = match symbol {
        "cbETH" => {
            if calculated_usd > 15_000.0 || calculated_usd < 300.0 {
                if price_in_weth > 0.0 { (1.0 / price_in_weth) * weth_price_usd } else { 2699.0 }
            } else { calculated_usd }
        }
        "VIRTUAL" => {
            if calculated_usd > 50.0 || calculated_usd < 0.01 {
                0.6404
            } else { calculated_usd }
        }
        _ => calculated_usd,
    };

    if final_price.is_finite() && final_price > 0.0 { final_price } else { 0.0 }
}

async fn fetch_live_market_data_for_pair<P>(http_provider: P, config: &TokenPairConfig, weth_price_usd: f64) -> Result<(f64, f64, f64, f64, f64), Box<dyn std::error::Error>>
where P: Provider<Ethereum> + Clone {
    let mut price_aero = 0.0;
    if config.aero_pool != Address::ZERO {
        if config.is_v3_aero {
            let v3_pool = IUniswapV3Pool::new(config.aero_pool, http_provider.clone());
            if let (Ok(slot0), Ok(p_token0)) = (v3_pool.slot0().call().await, v3_pool.token0().call().await) {
                let token0_addr: Address = p_token0.0.into();
                price_aero = calculate_v3_price_dynamic(slot0.sqrtPriceX96, token0_addr, config.target_token, weth_price_usd, config.symbol);
            }
        } else {
            let aero_pair = IUniswapV2Pair::new(config.aero_pool, http_provider.clone());
            if let Ok(res) = aero_pair.getReserves().call().await {
                price_aero = calculate_v2_price(res.reserve0.to::<u128>(), res.reserve1.to::<u128>(), config.token0, config.token1, config.decimals, weth_price_usd);
            }
        }
    }

    if price_aero <= 0.0 {
        if config.symbol == "VIRTUAL" { price_aero = 0.6404; }
        else if config.symbol == "cbETH" { price_aero = 2699.0; }
        else if config.symbol == "WETH" { price_aero = weth_price_usd; }
    }

    let mut price_v3 = 0.0;
    if config.uni_v3_pool != Address::ZERO {
        let v3_pool = IUniswapV3Pool::new(config.uni_v3_pool, http_provider.clone());
        if let (Ok(slot0), Ok(p_token0)) = (v3_pool.slot0().call().await, v3_pool.token0().call().await) {
            let token0_addr: Address = p_token0.0.into();
            price_v3 = calculate_v3_price_dynamic(slot0.sqrtPriceX96, token0_addr, config.target_token, weth_price_usd, config.symbol);
        }
    }

    let price_baseswap = 0.0;
    let price_pancake = 0.0;

    let mut prices = vec![price_aero, price_v3];
    prices.retain(|&p| p > 0.000001 && p <= 100000.0);

    let max_price = prices.iter().cloned().fold(0.0, f64::max);
    let min_price = prices.iter().cloned().fold(f64::INFINITY, f64::min);
    let spread_gap = if !prices.is_empty() && min_price != f64::INFINITY { max_price - min_price } else { 0.0 };

    Ok((price_aero, price_v3, price_baseswap, price_pancake, spread_gap))
}

async fn trigger_cross_dex_arbitrage<P, PP>(http_provider: P, private_provider: PP, contract_address: Address, signer_address: Address, spread_gap: f64, pair_cfg: &TokenPairConfig, price_aero: f64, price_v3: f64, target_price_usd: f64) -> Result<(), Box<dyn std::error::Error>>
where P: Provider<Ethereum> + Clone, PP: Provider<Ethereum> + Clone {
    if spread_gap < 8.0 { return Ok(()); }

    let raw_loan = (spread_gap * 150.0) as u128 * 1_000_000;
    let dynamic_loan = raw_loan.clamp(1_000_000_000, 2_400_000_000); 
    let loan_amount = U256::from(dynamic_loan);

    println!("🎯 [Cross-DEX {}] Gap: ${:.2} | Optimal Loan: {} USDC... Simulating...", pair_cfg.symbol, spread_gap, dynamic_loan as f64 / 1_000_000.0);

    let nodes = vec![QuantumNode { id: 1, energy_scale: generate_astronomical_number(1000), frequency: spread_gap, token0: USDC_BASE, token1: pair_cfg.target_token }];
    let system = CausalCollapseSystem::new(nodes, vec![UNISWAP_V3_ROUTER, PANCAKESWAP_V3_ROUTER, AERODROME_ROUTER], contract_address, loan_amount, USDC_BASE, pair_cfg.clone(), price_aero, price_v3, target_price_usd);
    let target_path = system.execute_collapse();
    if target_path.0.is_empty() { return Ok(()); }

    let swap_path_data = alloy::dyn_abi::DynSolValue::Tuple(vec![
        alloy::dyn_abi::DynSolValue::Array(target_path.0.into_iter().map(alloy::dyn_abi::DynSolValue::Address).collect()),
        alloy::dyn_abi::DynSolValue::Array(target_path.1.into_iter().map(alloy::dyn_abi::DynSolValue::Bytes).collect()),
    ]).abi_encode();

    let contract_sim = BaseAtomicArbitrage::new(contract_address, http_provider);
    let simulation_call = contract_sim
        .triggerBalancerArbitrage(USDC_BASE, loan_amount, swap_path_data.clone().into())
        .from(signer_address)
        .gas(1_500_000);

    match simulation_call.call().await {
        Ok(_) => {
            println!("🎉 [SIMULATION PASSED!] Dispatching via Private RPC...");
            let contract_private = BaseAtomicArbitrage::new(contract_address, private_provider);
            if let Ok(tx) = contract_private.triggerBalancerArbitrage(USDC_BASE, loan_amount, swap_path_data.into()).from(signer_address).gas(1_500_000).send().await {
                println!("🚀 [Dispatched] Hash: {:?}", tx.tx_hash());
            }
        }
        Err(contract_error) => {
            eprintln!("❌ [SIMULATION FAILED for {}]", pair_cfg.symbol);
            if let Some(revert_bytes) = contract_error.as_revert_data() {
                if let Ok(err_decoded) = BaseAtomicArbitrage::BaseAtomicArbitrageErrors::abi_decode(revert_bytes.as_ref()) {
                    match err_decoded {
                        BaseAtomicArbitrage::BaseAtomicArbitrageErrors::ArbitrageUnprofitable(err_data) => {
                            eprintln!("   💰 [Revert]: ArbitrageUnprofitable! Required: {}, Actual: {}", err_data.required, err_data.actual);
                        }
                        BaseAtomicArbitrage::BaseAtomicArbitrageErrors::InsufficientLiquidity(_) => {
                            eprintln!("   💧 [Revert]: InsufficientLiquidity!");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

async fn trigger_triangular_arbitrage<P, PP>(http_provider: P, private_provider: PP, contract_address: Address, signer_address: Address, graph: &ArbitrageGraph) -> Result<(), Box<dyn std::error::Error>>
where P: Provider<Ethereum> + Clone, PP: Provider<Ethereum> + Clone {
    let cycles = graph.find_cycles(USDC_BASE, 3);
    if cycles.is_empty() { return Ok(()); }

    for (i, target_cycle) in cycles.iter().enumerate() {
        let loan_amount = U256::from(1_500_000_000u64);
        let (targets, data) = TriangularPayloadEncoder::build_execution_payload(target_cycle, contract_address, loan_amount);
        if targets.is_empty() || data.is_empty() { continue; }

        let contract_sim = BaseAtomicArbitrage::new(contract_address, http_provider.clone());
        let sim_call = contract_sim.triggerBalancerArbitrageMulti(USDC_BASE, loan_amount, targets.clone(), data.clone()).from(signer_address).gas(1_800_000);

        match sim_call.call().await {
            Ok(_) => {
                println!("🎉 [Triangular SIMULATION PASSED!] Cycle #{}. Dispatching...", i + 1);
                let contract_private = BaseAtomicArbitrage::new(contract_address, private_provider.clone());
                if let Ok(tx) = contract_private.triggerBalancerArbitrageMulti(USDC_BASE, loan_amount, targets, data).from(signer_address).gas(1_800_000).send().await {
                    println!("🚀 [Triangular Dispatched] Hash: {:?}", tx.tx_hash());
                }
                break;
            }
            Err(_e) => {}
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Initializing Ultimate Multi-DEX MEV Bot...");

    let contract_addr_str = std::env::var("CONTRACT_ADDR").unwrap_or_else(|_| "0x457820b3D7d2d607AEEF8248B46Ffa6ccD60121A".to_string());
    let contract_address: Address = contract_addr_str.parse()?;

    let primary_http_url = std::env::var("ALCHEMY_HTTP_URL").unwrap_or_else(|_| "https://base-mainnet.g.alchemy.com/v2/alch_c3Z7QmOs3ZZFLUr0zngV6".to_string());
    let private_http_url = std::env::var("PRIVATE_RPC_URL").unwrap_or_else(|_| primary_http_url.clone());
    let alchemy_wss_url = std::env::var("ALCHEMY_WSS_URL").unwrap_or_else(|_| "ws://127.0.0.1:8545".to_string());
    let private_key_str = std::env::var("PRIVATE_KEY").unwrap_or_else(|_| "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string());

    let signer = PrivateKeySigner::from_str(&private_key_str)?;
    let signer_address = signer.address();
    
    let http_provider = ProviderBuilder::new().wallet(EthereumWallet::from(signer.clone())).connect_http(primary_http_url.parse()?);
    let private_provider = ProviderBuilder::new().wallet(EthereumWallet::from(signer.clone())).connect_http(private_http_url.parse()?);

    let monitored_pairs = get_monitored_pairs(http_provider.clone()).await?;
    let liquidity_graph = load_global_liquidity_graph();

    for pair in &monitored_pairs {
        let _ = BaseAtomicArbitrage::new(contract_address, http_provider.clone()).setTargetWhitelist(pair.target_token, true).from(signer_address).send().await;
    }
    let _ = BaseAtomicArbitrage::new(contract_address, http_provider.clone()).setTargetWhitelist(CBETH_BASE, true).from(signer_address).send().await;

    let ws = alloy::providers::WsConnect::new(alchemy_wss_url);
    let ws_provider = ProviderBuilder::new().wallet(EthereumWallet::from(signer.clone())).connect_ws(ws).await?;
    let sub = ws_provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();
    let mut block_counter = 0u64;

    while let Some(block) = stream.next().await {
        block_counter += 1;
        let block_num = block.inner.number;
        println!("\n📦 WSS Block Synced: #{} (Counter: {}) - Scanning Ultimate Markets...", block_num, block_counter);

        let mut current_weth_price = 2416.28;
        // تم تصحيح علامة الخطأ هنا من `>` إلى `=`
        if let Some(weth_pair) = monitored_pairs.iter().find(|p| p.symbol == "WETH") {
            if let Ok((_, p_v3, _, _, _)) = fetch_live_market_data_for_pair(http_provider.clone(), weth_pair, 1.0).await {
                if p_v3 > 0.0 { current_weth_price = p_v3; }
            }
        }

        for pair in &monitored_pairs {
            if let Ok((p_aero, p_v3, _p_baseswap, _p_pancake, spread_gap)) = fetch_live_market_data_for_pair(http_provider.clone(), pair, current_weth_price).await {
                println!(
                    " 📈 [{}] Aero: ${:.4} | UniV3: ${:.4} \vert{} Max Gap:${:.2}",
                    pair.symbol, p_aero, p_v3, spread_gap
                );

                if spread_gap >= 8.0 {
                    let target_price = if p_v3 > 0.0 { p_v3 } else { current_weth_price };
                    let _ = trigger_cross_dex_arbitrage(
                        http_provider.clone(),
                        private_provider.clone(),
                        contract_address,
                        signer_address,
                        spread_gap,
                        pair,
                        p_aero,
                        p_v3,
                        target_price,
                    ).await;
                }
            }
        }

        let _ = trigger_triangular_arbitrage(
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
