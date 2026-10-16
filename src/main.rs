use num_bigint::BigUint;
use num_traits::One;
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

type _U24 = Uint<24, 1>;
type _U160 = Uint<160, 3>;

// الرموز الأساسية والعناوين الدقيقة على شبكة Base
const USDC_BASE: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bda02913");
const WETH_BASE: Address = address!("4200000000000000000000000000000000000006");
const AERO_BASE: Address = address!("940181a94A35A4569E4529A3CDfB74e38FD98631");
const DEGEN_BASE: Address = address!("4ed4e862860bed51a9570b96d89af5e1b0efefed");
const CBBTC_BASE: Address = address!("cbB7C0000aB88B473b1f5aFd9ef808440eed33Bf");

// موجهات المنصات المعتمدة بالكامل في القائمة البيضاء لعقدك الذكي (Whitelisted Routers)
const UNISWAP_V3_ROUTER: Address = address!("0xcf77A3bA9Aab7D3E44917635033322DF3f564171");
const AERODROME_ROUTER: Address = address!("0x2626664c2603336E57B271c5C0b26F421741e481");
const BASESWAP_ROUTER: Address = address!("0x327Df1e6de05895D2Ab08513aADD931325260A99"); 
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
    pub pair_config: TokenPairConfig,
}

impl CausalCollapseSystem {
    pub fn new(
        nodes: Vec<QuantumNode>,
        dynamic_pools: Vec<Address>,
        contract_address: Address,
        loan_amount: U256,
        borrowed_token: Address,
        pair_config: TokenPairConfig,
    ) -> Self {
        Self {
            nodes,
            threshold_limit: 0.02,
            buffer_capacity: 16,
            dynamic_pools,
            contract_address,
            loan_amount,
            borrowed_token,
            pair_config,
        }
    }

    pub fn execute_collapse(&self) -> (Vec<Address>, Vec<Vec<u8>>) {
        if self.nodes.is_empty() {
            return (vec![], vec![]);
        }

        let mut addresses = Vec::new();
        let mut payloads = Vec::new();

        // الساق الأولى: الشراء عبر Aerodrome Router
        addresses.push(AERODROME_ROUTER);
        
        let min_amount_out_leg1 = self.loan_amount * U256::from(997) / U256::from(1000);

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

        // الساق الثانية: البيع عبر PancakeSwap V3 Router أو Uniswap V3
        addresses.push(PANCAKESWAP_V3_ROUTER);

        let estimated_token_received = min_amount_out_leg1; 
        let min_amount_out_leg2 = self.loan_amount * U256::from(10005) / U256::from(10000);

        let mut path_bytes = Vec::new();
        path_bytes.extend_from_slice(self.pair_config.target_token.as_slice());
        
        let fee_bytes = self.pair_config.fee_tier_v3.to_be_bytes();
        path_bytes.extend_from_slice(&fee_bytes[1..4]);
        
        path_bytes.extend_from_slice(self.borrowed_token.as_slice());

        let exact_input_params = IUniswapV3Router::ExactInputParams {
            path: Bytes::from(path_bytes),
            recipient: self.contract_address,
            deadline: U256::from(u64::MAX),
            amountIn: estimated_token_received,
            amountOutMinimum: min_amount_out_leg2,
        };

        let payload_leg2 = IUniswapV3Router::exactInputCall {
            params: exact_input_params,
        }
        .abi_encode();

        payloads.push(payload_leg2);

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
    }
}

pub fn get_monitored_pairs() -> Vec<TokenPairConfig> {
    vec![
        TokenPairConfig {
            symbol: "WETH",
            target_token: WETH_BASE,
            aero_pool: address!("cdac0d6c6c59727a65f871236188350531885c43"),
            uni_v3_pool: address!("0x6c561b446416e1a00e8e93e221854d6ea4171372"),
            baseswap_pool: address!("0x88AAlc56B7A16892fa56F420d418DE926De7c04e"), // BaseSwap V2/V3 Pool
            pancakeswap_pool: address!("0x1a877526715f3e4e93fb25a176840fc0f8d9b139"), // PancakeSwap V3 Pool
            decimals: 18,
            fee_tier_v3: 500,
            token0: USDC_BASE,
            token1: WETH_BASE,
        },
        TokenPairConfig {
            symbol: "AERO",
            target_token: AERO_BASE,
            aero_pool: address!("6cdcb1c4a4d1c3c6d054b27ac5b77e89eafb971d"),
            uni_v3_pool: address!("0xe5b5f522e98b5a2baae212d4da66b865b781db90"),
            baseswap_pool: address!("0x6cdcb1c4a4d1c3c6d054b27ac5b77e89eafb971d"),
            pancakeswap_pool: address!("0xe5b5f522e98b5a2baae212d4da66b865b781db90"),
            decimals: 18,
            fee_tier_v3: 500,
            token0: USDC_BASE,
            token1: AERO_BASE,
        },
        TokenPairConfig {
            symbol: "DEGEN",
            target_token: DEGEN_BASE,
            aero_pool: address!("a19acc3b4f11c46c2b1fc36b5f592af422ee338c"),
            uni_v3_pool: address!("29715d8d279cab143a12ff515b40a2b35d7bad37"),
            baseswap_pool: address!("a19acc3b4f11c46c2b1fc36b5f592af422ee338c"),
            pancakeswap_pool: address!("29715d8d279cab143a12ff515b40a2b35d7bad37"),
            decimals: 18,
            fee_tier_v3: 3000,
            token0: DEGEN_BASE,
            token1: USDC_BASE,
        },
        TokenPairConfig {
            symbol: "cbBTC",
            target_token: CBBTC_BASE,
            aero_pool: address!("9c38b55f9a9aba91bbcedeb12bf4428f47a6a0b8"),
            uni_v3_pool: address!("4e962bb3889bf030368f56810a9c96b83cb3e778"),
            baseswap_pool: address!("9c38b55f9a9aba91bbcedeb12bf4428f47a6a0b8"),
            pancakeswap_pool: address!("4e962bb3889bf030368f56810a9c96b83cb3e778"),
            decimals: 8,
            fee_tier_v3: 500,
            token0: USDC_BASE,
            token1: CBBTC_BASE,
        },
    ]
}

async fn fetch_live_market_data_for_pair<P>(
    http_provider: P,
    config: &TokenPairConfig,
) -> Result<(f64, f64, f64, f64, f64), Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
{
    // 1. Aerodrome Price
    let mut price_aero = 0.0;
    if config.symbol == "DEGEN" {
        let aero_v3_pool = IUniswapV3Pool::new(config.aero_pool, http_provider.clone());
        if let Ok(slot0) = aero_v3_pool.slot0().call().await {
            let sqrt_price_x96 = slot0.sqrtPriceX96.to_string().parse::<f64>().unwrap_or(0.0);
            if sqrt_price_x96 > 0.0 {
                let ratio = sqrt_price_x96 / (2.0f64.powi(96));
                price_aero = ratio * ratio * 10f64.powi(12);
            }
        }
    } else {
        let aero_pair = IUniswapV2Pair::new(config.aero_pool, http_provider.clone());
        if let Ok(res) = aero_pair.getReserves().call().await {
            let r0 = res.reserve0.to::<u128>() as f64;
            let r1 = res.reserve1.to::<u128>() as f64;
            if r0 > 0.0 && r1 > 0.0 {
                match config.symbol {
                    "WETH" => { price_aero = (r1 / 10f64.powi(6)) / (r0 / 10f64.powi(18)); }
                    "AERO" => { price_aero = (r1 / 10f64.powi(18)) / (r0 / 10f64.powi(6)); }
                    "cbBTC" => { price_aero = (r0 / 10f64.powi(6)) / (r1 / 10f64.powi(8)); }
                    _ => {}
                }
            }
        }
    }

    // 2. Uniswap V3 Price
    let mut price_v3 = 0.0;
    let v3_pool = IUniswapV3Pool::new(config.uni_v3_pool, http_provider.clone());
    if let Ok(slot0) = v3_pool.slot0().call().await {
        let sqrt_price_x96 = slot0.sqrtPriceX96.to_string().parse::<f64>().unwrap_or(0.0);
        if sqrt_price_x96 > 0.0 {
            let ratio = sqrt_price_x96 / (2.0f64.powi(96));
            let raw_price = ratio * ratio;
            price_v3 = if config.symbol == "cbBTC" { raw_price * 10f64.powi(4) } else { raw_price * 10f64.powi(12) };
        }
    }

    // 3. BaseSwap Price
    let mut price_baseswap = 0.0;
    let baseswap_pool = IUniswapV3Pool::new(config.baseswap_pool, http_provider.clone());
    if let Ok(slot0) = baseswap_pool.slot0().call().await {
        let sqrt_price_x96 = slot0.sqrtPriceX96.to_string().parse::<f64>().unwrap_or(0.0);
        if sqrt_price_x96 > 0.0 {
            let ratio = sqrt_price_x96 / (2.0f64.powi(96));
            let raw_price = ratio * ratio;
            price_baseswap = if config.symbol == "cbBTC" { raw_price * 10f64.powi(4) } else { raw_price * 10f64.powi(12) };
        }
    }

    // 4. PancakeSwap V3 Price
    let mut price_pancake = 0.0;
    let pancake_pool = IUniswapV3Pool::new(config.pancakeswap_pool, http_provider.clone());
    if let Ok(slot0) = pancake_pool.slot0().call().await {
        let sqrt_price_x96 = slot0.sqrtPriceX96.to_string().parse::<f64>().unwrap_or(0.0);
        if sqrt_price_x96 > 0.0 {
            let ratio = sqrt_price_x96 / (2.0f64.powi(96));
            let raw_price = ratio * ratio;
            price_pancake = if config.symbol == "cbBTC" { raw_price * 10f64.powi(4) } else { raw_price * 10f64.powi(12) };
        }
    }

    // حساب أقصى الفجوات السعرية بين جميع المنصات المتاحة
    let mut prices = vec![price_aero, price_v3, price_baseswap, price_pancake];
    prices.retain(|&p| p > 0.0);

    let max_price = prices.iter().cloned().fold(0.0, f64::max);
    let min_price = prices.iter().cloned().fold(f64::INFINITY, f64::min);

    let spread_gap = if !prices.is_empty() && min_price != f64::INFINITY {
        max_price - min_price
    } else {
        0.0
    };

    Ok((price_aero, price_v3, price_baseswap, price_pancake, spread_gap))
}

async fn trigger_on_chain_arbitrage<P>(
    http_provider: P,
    contract_address: Address,
    target_path: (Vec<Address>, Vec<Vec<u8>>),
    signer_address: Address,
    token_to_borrow: Address,
    loan_amount: U256,
    spread_gap: f64,
    pair_symbol: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
{
    if loan_amount == U256::ZERO || target_path.0.is_empty() {
        return Ok(());
    }

    let min_gap_threshold = match pair_symbol {
        "WETH" => 0.25,
        "cbBTC" => 50.00,
        "AERO" => 0.005,
        "DEGEN" => 0.0001,
        _ => 0.25,
    };

    if spread_gap < min_gap_threshold {
        println!("🛡 [PROFIT SHIELD - {}] Gap (${:.4}) below minimum threshold. Skipping...", pair_symbol, spread_gap);
        return Ok(());
    }

    println!("🚀 [BOT -> CONTRACT | {}] Simulating Multi-DEX Arbitrage Command locally...", pair_symbol);

    let swap_path_data = alloy::dyn_abi::DynSolValue::Tuple(vec![
        alloy::dyn_abi::DynSolValue::Array(
            target_path.0.clone().into_iter().map(alloy::dyn_abi::DynSolValue::Address).collect()
        ),
        alloy::dyn_abi::DynSolValue::Array(
            target_path.1.into_iter().map(alloy::dyn_abi::DynSolValue::Bytes).collect()
        ),
    ]).abi_encode();

    let contract = BaseAtomicArbitrage::new(contract_address, http_provider.clone());

    let balancer_builder = contract
        .triggerBalancerArbitrage(token_to_borrow, loan_amount, swap_path_data.clone().into())
        .from(signer_address)
        .gas(1_500_000);

    match balancer_builder.call().await {
        Ok(_) => {
            println!("✅ Local Simulation Passed for {}! Dispatching Real Transaction to Blockchain...", pair_symbol);
            match balancer_builder.send().await {
                Ok(pending_tx) => {
                    println!("✅ Transaction Dispatched! Tx Hash: {:?}", pending_tx.tx_hash());
                    match pending_tx.get_receipt().await {
                        Ok(receipt) => {
                            println!("📦 Transaction Mined In Block: {:?} | Status: {:?}", receipt.block_number, receipt.status());
                        }
                        Err(e) => println!("❌ [TRANSACTION RECEIPT FAILED]: {:#?}", e),
                    }
                }
                Err(e_send) => println!("❌ [SEND TRANSACTION FAILED]: {:#?}", e_send),
            }
        }
        Err(_e_sim) => {
            println!("🛡 [SIMULATION REVERTED SAFELY - {}]: الصفقة غير مربحة أو ستفشل محلياً، تم منع إرسالها لتوفير رسوم الغاز.", pair_symbol);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Initializing Multi-DEX Cross-Matrix Scanner Engine via Alloy...");

    let contract_addr_str = std::env::var("CONTRACT_ADDR")
        .unwrap_or_else(|_| "0x94409D722D4b21A496A3C073477F06Ca388B9C3B".to_string());
    let contract_address: Address = contract_addr_str.parse()?;

    let primary_http_url = std::env::var("ALCHEMY_HTTP_URL")
        .unwrap_or_else(|_| "https://mainnet.base.org".to_string());
    let alchemy_wss_url = std::env::var("ALCHEMY_WSS_URL")
        .unwrap_or_else(|_| "ws://127.0.0.1:8545".to_string());

    let private_key_str = std::env::var("PRIVATE_KEY")
        .unwrap_or_else(|_| "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string());
    let signer: PrivateKeySigner = private_key_str.parse()?;
    let signer_address = signer.address();
    let wallet = EthereumWallet::from(signer);

    println!("📡 Activating HTTP Connection to Base...");
    let http_provider = ProviderBuilder::new()
        .wallet(wallet.clone())
        .connect_http(primary_http_url.parse()?);

    println!("📡 Activating WebSocket Connection to: {}", alchemy_wss_url);
    let ws = alloy::providers::WsConnect::new(alchemy_wss_url);
    let ws_provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_ws(ws)
        .await?;

    let monitored_pairs = get_monitored_pairs();
    println!("🎯 Active Monitored Tokens: {:?}", monitored_pairs.iter().map(|p| p.symbol).collect::<Vec<&str>>());
    println!("⚡ Loaded DEXs: Uniswap V3, Aerodrome, BaseSwap, PancakeSwap V3");

    let sub = ws_provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();
    let mut block_counter = 0u64;

    let loan_amount = U256::from(10_000_000_000u64); // 10,000 USDC

    while let Some(block) = stream.next().await {
        block_counter += 1;
        let block_num = block.inner.number;
        println!("\n📦 Live WSS Block Synced: #{} (Internal counter: {})", block_num, block_counter);

        for pair in &monitored_pairs {
            let provider = http_provider.clone();
            let pair_cfg = pair.clone();

            match fetch_live_market_data_for_pair(provider.clone(), &pair_cfg).await {
                Ok((price_aero, price_v3, price_baseswap, price_pancake, spread_gap)) => {
                    println!(
                        " 📈 [{}] Aero: ${:.4} | UniV3: ${:.4} | BaseSwap: ${:.4} | Pancake: ${:.4} | Max Gap: ${:.4}",
                        pair_cfg.symbol, price_aero, price_v3, price_baseswap, price_pancake, spread_gap
                    );

                    if spread_gap > 0.00001 {
                        let nodes = vec![
                            QuantumNode {
                                id: 1,
                                energy_scale: generate_astronomical_number(1000usize),
                                frequency: spread_gap,
                                token0: USDC_BASE,
                                token1: pair_cfg.target_token,
                            },
                        ];

                        let system = CausalCollapseSystem::new(
                            nodes,
                            vec![UNISWAP_V3_ROUTER, PANCAKESWAP_V3_ROUTER, BASESWAP_ROUTER, AERODROME_ROUTER],
                            contract_address,
                            loan_amount,
                            USDC_BASE,
                            pair_cfg.clone(),
                        );

                        let optimized_path = system.execute_collapse();

                        if let Err(e) = trigger_on_chain_arbitrage(
                            provider,
                            contract_address,
                            optimized_path,
                            signer_address,
                            USDC_BASE,
                            loan_amount,
                            spread_gap,
                            pair_cfg.symbol,
                        ).await {
                            println!("❌ Error executing on-chain command for {}: {:?}", pair_cfg.symbol, e);
                        }
                    }
                }
                Err(e) => {
                    println!("⚠️ Could not fetch market data for {}: {:?}", pair_cfg.symbol, e);
                }
            }
        }

        sleep(Duration::from_millis(100)).await;
    }

    println!("🏁 Live stream processing terminated.");
    Ok(())
}
