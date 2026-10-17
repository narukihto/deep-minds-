use num_bigint::BigUint;
use num_traits::One;
use futures_util::StreamExt;
use tokio::time::{sleep, Duration};
use alloy::{
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    network::{EthereumWallet, Ethereum},
    primitives::{address, Address, U256, Bytes, Uint},
    sol, sol_types::SolCall,
};

type _U24 = Uint<24, 1>;
type _U160 = Uint<160, 3>;

const USDC_BASE: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bda02913");
const WETH_BASE: Address = address!("4200000000000000000000000000000000000006");

const UNISWAP_V3_ROUTER: Address = address!("0xcf77A3bA9Aab7D3E44917635033322DF3f564171");
const AERODROME_ROUTER: Address = address!("0x2626664c2603336E57B271c5C0b26F421741e481");
const BASESWAP_ROUTER: Address = address!("0xcA1F7645EF1077CC178dFb42Cf5d2f840457DDBb"); 
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
        }
    }

    pub fn execute_collapse(&self) -> (Vec<Address>, Vec<Vec<u8>>) {
        if self.nodes.is_empty() {
            return (vec![], vec![]);
        }

        let mut addresses = Vec::new();
        let mut payloads = Vec::new();

        let min_amount_out_leg1 = self.loan_amount * U256::from(9980) / U256::from(10000);
        let min_amount_out_leg2 = self.loan_amount * U256::from(10002) / U256::from(10000);

        if self.price_aero <= self.price_v3 {
            addresses.push(AERODROME_ROUTER);

            let aero_route = IAerodromeRouter::Route {
                from: self.borrowed_token,
                to: self.pair_config.target_token,
                stable: false,
                factory: address!("420DD381b31aEf6683db6B902084cB0FFECe40Da"),
            };

            let payload_leg1 = IAerodromeRouter::swapExactTokensForTokensCall {
                amountIn: self.loan_amount,
                amountOutMin: min_amount_out_leg1,
                routes: vec![aero_route],
                to: self.contract_address,
                deadline: U256::from(u64::MAX),
            }
            .abi_encode();
            payloads.push(payload_leg1);

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

            let payload_leg2 = IUniswapV3Router::exactInputCall {
                params: exact_input_params,
            }
            .abi_encode();
            payloads.push(payload_leg2);

        } else {
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
                amountOutMinimum: U256::ZERO,
            };

            let payload_leg1 = IUniswapV3Router::exactInputCall {
                params: exact_input_params,
            }
            .abi_encode();
            payloads.push(payload_leg1);

            addresses.push(AERODROME_ROUTER);
            let aero_route = IAerodromeRouter::Route {
                from: self.pair_config.target_token,
                to: self.borrowed_token,
                stable: false,
                factory: address!("420DD381b31aEf6683db6B902084cB0FFECe40Da"),
            };

            let payload_leg2 = IAerodromeRouter::swapExactTokensForTokensCall {
                amountIn: min_amount_out_leg1,
                amountOutMin: min_amount_out_leg2,
                routes: vec![aero_route],
                to: self.contract_address,
                deadline: U256::from(u64::MAX),
            }
            .abi_encode();
            payloads.push(payload_leg2);
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
        function triggerAaveArbitrage(address tokenToBorrow, uint256 loanAmount, bytes calldata swapPathData) external;
        function setTargetWhitelist(address target, bool status) external;
        
        error ArbitrageUnprofitable(uint256 expected, uint256 actual);
        error InsufficientLiquidity();
        error TargetUnauthorized();
        error UntrustedInitiator();
        error NotAuthorized();
        error ExternalCallFailed(bytes reason);
    }
}

pub fn get_monitored_pairs() -> Vec<TokenPairConfig> {
    vec![
        TokenPairConfig {
            symbol: "WETH",
            target_token: WETH_BASE,
            aero_pool: address!("cdac0d6c6c59727a65f871236188350531885c43"),
            uni_v3_pool: address!("0x6c561b446416e1a00e8e93e221854d6ea4171372"),
            baseswap_pool: address!("ab067c01c7f5734da168c699ae9d23a4512c9fdb"),
            pancakeswap_pool: address!("72ab388e2e2f6facef59e3c3fa2c4e29011c2d38"),
            decimals: 18,
            fee_tier_v3: 100,
            token0: WETH_BASE,
            token1: USDC_BASE,
        },
    ]
}

fn calculate_v2_price(
    reserve0: u128,
    reserve1: u128,
    token0: Address,
    token1: Address,
    target_decimals: u32,
) -> f64 {
    if reserve0 == 0 || reserve1 == 0 {
        return 0.0;
    }
    let usdc_decimals = 6;
    let r0 = reserve0 as f64 / 10f64.powi(if token0 == USDC_BASE { usdc_decimals } else { target_decimals } as i32);
    let r1 = reserve1 as f64 / 10f64.powi(if token1 == USDC_BASE { usdc_decimals } else { target_decimals } as i32);

    if token0 == USDC_BASE {
        if r1 > 0.0 { r0 / r1 } else { 0.0 }
    } else {
        if r0 > 0.0 { r1 / r0 } else { 0.0 }
    }
}

fn calculate_v3_price(
    sqrt_price_x96: _U160,
    token0: Address,
    token1: Address,
    target_decimals: u32,
) -> f64 {
    let sqrt_f64 = sqrt_price_x96.to_string().parse::<f64>().unwrap_or(0.0);
    if sqrt_f64 <= 0.0 {
        return 0.0;
    }
    let ratio = sqrt_f64 / 2.0f64.powi(96);
    let price_raw = ratio * ratio;

    let usdc_decimals = 6;
    let d0 = if token0 == USDC_BASE { usdc_decimals } else { target_decimals };
    let d1 = if token1 == USDC_BASE { usdc_decimals } else { target_decimals };

    let adjusted_price = price_raw * 10f64.powi((d0 as i32) - (d1 as i32));

    if token0 == USDC_BASE {
        if adjusted_price > 0.0 { 1.0 / adjusted_price } else { 0.0 }
    } else {
        adjusted_price
    }
}

async fn fetch_live_market_data_for_pair<P>(
    http_provider: P,
    config: &TokenPairConfig,
) -> Result<(f64, f64, f64, f64, f64), Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
{
    let mut price_aero = 0.0;
    let aero_pair = IUniswapV2Pair::new(config.aero_pool, http_provider.clone());
    if let Ok(res) = aero_pair.getReserves().call().await {
        price_aero = calculate_v2_price(
            res.reserve0.to::<u128>(),
            res.reserve1.to::<u128>(),
            config.token0,
            config.token1,
            config.decimals,
        );
    }

    let mut price_v3 = 0.0;
    let v3_pool = IUniswapV3Pool::new(config.uni_v3_pool, http_provider.clone());
    if let Ok(slot0) = v3_pool.slot0().call().await {
        price_v3 = calculate_v3_price(
            slot0.sqrtPriceX96,
            config.token0,
            config.token1,
            config.decimals,
        );
    }

    let mut price_baseswap = 0.0;
    let baseswap_pair = IUniswapV2Pair::new(config.baseswap_pool, http_provider.clone());
    if let Ok(res) = baseswap_pair.getReserves().call().await {
        price_baseswap = calculate_v2_price(
            res.reserve0.to::<u128>(),
            res.reserve1.to::<u128>(),
            config.token0,
            config.token1,
            config.decimals,
        );
    }

    let mut price_pancake = 0.0;
    let pancake_pool = IUniswapV3Pool::new(config.pancakeswap_pool, http_provider.clone());
    if let Ok(slot0) = pancake_pool.slot0().call().await {
        price_pancake = calculate_v3_price(
            slot0.sqrtPriceX96,
            config.token0,
            config.token1,
            config.decimals,
        );
    }

    let mut prices = vec![price_aero, price_v3, price_baseswap, price_pancake];
    prices.retain(|&p| p >= 100.0 && p <= 100000.0);

    let max_price = prices.iter().cloned().fold(0.0, f64::max);
    let min_price = prices.iter().cloned().fold(f64::INFINITY, f64::min);

    let spread_gap = if !prices.is_empty() && min_price != f64::INFINITY {
        max_price - min_price
    } else {
        0.0
    };

    Ok((price_aero, price_v3, price_baseswap, price_pancake, spread_gap))
}

async fn trigger_on_chain_arbitrage<P, PP>(
    http_provider: P,
    private_provider: PP,
    contract_address: Address,
    signer_address: Address,
    token_to_borrow: Address,
    spread_gap: f64,
    pair_cfg: &TokenPairConfig,
    price_aero: f64,
    price_v3: f64,
) -> Result<(), Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
    PP: Provider<Ethereum> + Clone,
{
    if spread_gap < 15.0 {
        println!("💤 [WETH] Gap (${:.2}) below threshold ($15.0). Skipping.", spread_gap);
        return Ok(());
    }

    let raw_loan = (spread_gap * 3000.0) as u64 * 1_000_000;
    let dynamic_loan = raw_loan.clamp(5_000_000_000, 100_000_000_000);
    let loan_amount = U256::from(dynamic_loan);

    println!("🎯 [WETH PRIVATE SNIPER] High Value Gap: ${:.2} | Dynamic Loan: {} USDC... Simulating locally!", 
             spread_gap, dynamic_loan / 1_000_000);

    let nodes = vec![QuantumNode {
        id: 1,
        energy_scale: generate_astronomical_number(1000usize),
        frequency: spread_gap,
        token0: USDC_BASE,
        token1: pair_cfg.target_token,
    }];

    let system = CausalCollapseSystem::new(
        nodes,
        vec![UNISWAP_V3_ROUTER, PANCAKESWAP_V3_ROUTER, BASESWAP_ROUTER, AERODROME_ROUTER],
        contract_address,
        loan_amount,
        USDC_BASE,
        pair_cfg.clone(),
        price_aero,
        price_v3,
    );

    let target_path = system.execute_collapse();

    if target_path.0.is_empty() {
        return Ok(());
    }

    let swap_path_data = alloy::dyn_abi::DynSolValue::Tuple(vec![
        alloy::dyn_abi::DynSolValue::Array(
            target_path.0.clone().into_iter().map(alloy::dyn_abi::DynSolValue::Address).collect()
        ),
        alloy::dyn_abi::DynSolValue::Array(
            target_path.1.into_iter().map(alloy::dyn_abi::DynSolValue::Bytes).collect()
        ),
    ]).abi_encode();

    // 1️⃣ المحاكاة المحلية باستخدام مزود القراءة العام لتفادي أي استهلاك رسوم
    let contract_sim = BaseAtomicArbitrage::new(contract_address, http_provider);
    let simulation_builder = contract_sim
        .triggerBalancerArbitrage(token_to_borrow, loan_amount, swap_path_data.clone().into())
        .from(signer_address)
        .gas(1_500_000);

    match simulation_builder.call().await {
        Ok(_) => {
            println!("🎉 [SIMULATION PASSED!] Profitable WETH Trade Verified. Dispatching via PRIVATE RPC (MEV Protected)...");
            
            // 2️⃣ الإرسال الفعلي عبر الـ Private Provider الحصري لمنع الـ Front-running
            let contract_private = BaseAtomicArbitrage::new(contract_address, private_provider);
            let private_builder = contract_private
                .triggerBalancerArbitrage(token_to_borrow, loan_amount, swap_path_data.into())
                .from(signer_address)
                .gas(1_500_000);

            if let Ok(pending_tx) = private_builder.send().await {
                println!("🚀 [PRIVATE MEV-PROTECTED TX DISPATCHED!] Real Hash: {:?}", pending_tx.tx_hash());
            }
        }
        Err(sim_err) => {
            println!("🛡️ [GAS SAVED] Simulation reverted locally. No gas wasted. Reason: {:?}", sim_err);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Initializing WETH Sniper Engine with Private MEV Protection...");

    let contract_addr_str = std::env::var("CONTRACT_ADDR")
        .unwrap_or_else(|_| "0x94409D722D4b21A496A3C073477F06Ca388B9C3B".to_string());
    let contract_address: Address = contract_addr_str.parse()?;

    let primary_http_url = std::env::var("ALCHEMY_HTTP_URL")
        .unwrap_or_else(|_| "https://mainnet.base.org".to_string());
    
    // رابط القناة الخاصة (إن لم يوجد يتم افتراضه مطادقاً للرابط الأساسي)
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

    // المزود العام لجلب البيانات والمحاكاة
    let http_provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_http(primary_http_url.parse()?);

    // المزود الخاص (Private RPC Provider) للإرسال المعزول
    let private_provider = ProviderBuilder::new()
        .wallet(wallet_private)
        .connect_http(private_http_url.parse()?);

    println!("🛡️ Ensuring WETH ({:?}) is whitelisted on contract...", WETH_BASE);
    let contract_init = BaseAtomicArbitrage::new(contract_address, http_provider.clone());
    let whitelist_call = contract_init.setTargetWhitelist(WETH_BASE, true).from(signer_address);
    let _ = whitelist_call.send().await;

    let ws = alloy::providers::WsConnect::new(alchemy_wss_url);
    let ws_provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(private_key_str.parse::<PrivateKeySigner>()?))
        .connect_ws(ws)
        .await?;

    let monitored_pairs = get_monitored_pairs();
    let sub = ws_provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();
    let mut block_counter = 0u64;

    while let Some(block) = stream.next().await {
        block_counter += 1;
        let block_num = block.inner.number;
        println!("\n📦 WSS Block Synced: #{} (Counter: {})", block_num, block_counter);

        for pair in &monitored_pairs {
            let provider = http_provider.clone();
            let priv_provider = private_provider.clone();
            let pair_cfg = pair.clone();

            if let Ok((p_aero, p_v3, p_baseswap, p_pancake, spread_gap)) = fetch_live_market_data_for_pair(provider.clone(), &pair_cfg).await {
                println!(
                    " 📈 [{}] Aero: ${:.2} | UniV3: ${:.2} | BaseSwap: ${:.2} | Pancake: ${:.2} | Max Gap: ${:.2}",
                    pair_cfg.symbol, p_aero, p_v3, p_baseswap, p_pancake, spread_gap
                );

                if spread_gap >= 15.0 { 
                    let _ = trigger_on_chain_arbitrage(
                        provider,
                        priv_provider,
                        contract_address,
                        signer_address,
                        USDC_BASE,
                        spread_gap,
                        &pair_cfg,
                        p_aero,
                        p_v3,
                    ).await;
                }
            }
        }

        sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}
