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
const UNISWAP_V3_ROUTER: Address = address!("0xcf77A3bA9Aab7D3E44917635033322DF3f564171");

// ⚠️ تم توحيد عنوان Aerodrome Router ليطابق تماماً العقد المنشور على البلوكشين
const AERODROME_V2_ROUTER: Address = address!("0x2626664c2603336E57B271c5C0b26F421741e481");
const AERODROME_V2_FACTORY: Address = address!("0x420DD381b31aEf6683db6B902084cB0FFECe40Da");

#[derive(Debug, Clone)]
pub struct TokenPairConfig {
    pub symbol: &'static str,
    pub target_token: Address,
    pub aero_pool: Address,
    pub uni_v3_pool: Address,
    pub decimals: u32,
    pub fee_tier_v3: u32,
}

#[derive(Clone, Debug)]
pub struct QuantumNode {
    pub id: usize,
    pub energy_scale: BigUint,
    pub frequency: f64,
}

pub struct CausalCollapseSystem {
    pub loan_amount: U256,
    pub borrowed_token: Address,
    pub pair_config: TokenPairConfig,
    pub price_aero: f64,
    pub price_v3: f64,
}

impl CausalCollapseSystem {
    pub fn new(
        loan_amount: U256,
        borrowed_token: Address,
        pair_config: TokenPairConfig,
        price_aero: f64,
        price_v3: f64,
    ) -> Self {
        Self {
            loan_amount,
            borrowed_token,
            pair_config,
            price_aero,
            price_v3,
        }
    }

    pub fn execute_collapse(&self, contract_address: Address) -> (Vec<Address>, Vec<Vec<u8>>) {
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

        let target_repay_amount = self.loan_amount + flash_loan_fee + (self.loan_amount * U256::from(30) / U256::from(10000));

        // تقليل هامش الانزلاق إلى 9920 (0.8% فقط) للسماح للصفقات الحقيقية بالمرور والربح
        let min_amount_out_leg1 = expected_target_units * U256::from(9920) / U256::from(10000); 
        let min_amount_out_leg2 = target_repay_amount + U256::from(500_000); // هامش ربح إضافي صافي

        if buy_on_univ3 {
            addresses.push(UNISWAP_V3_ROUTER);
            let mut path_bytes = Vec::new();
            path_bytes.extend_from_slice(self.borrowed_token.as_slice());
            let fee_bytes = self.pair_config.fee_tier_v3.to_be_bytes();
            path_bytes.extend_from_slice(&fee_bytes[1..4]);
            path_bytes.extend_from_slice(self.pair_config.target_token.as_slice());

            let exact_input_params = IUniswapV3Router::ExactInputParams {
                path: Bytes::from(path_bytes),
                recipient: contract_address,
                deadline: U256::from(u64::MAX),
                amountIn: self.loan_amount,
                amountOutMinimum: min_amount_out_leg1,
            };
            payloads.push(IUniswapV3Router::exactInputCall { params: exact_input_params }.abi_encode());

            addresses.push(AERODROME_V2_ROUTER);
            let aero_route = IAerodromeRouter::Route {
                from: self.pair_config.target_token,
                to: self.borrowed_token,
                stable: self.pair_config.symbol == "cbETH",
                factory: AERODROME_V2_FACTORY,
            };
            payloads.push(IAerodromeRouter::swapExactTokensForTokensCall {
                amountIn: min_amount_out_leg1,
                amountOutMin: min_amount_out_leg2,
                routes: vec![aero_route],
                to: contract_address,
                deadline: U256::from(u64::MAX),
            }.abi_encode());
        } else {
            addresses.push(AERODROME_V2_ROUTER);
            let aero_route = IAerodromeRouter::Route {
                from: self.borrowed_token,
                to: self.pair_config.target_token,
                stable: self.pair_config.symbol == "cbETH",
                factory: AERODROME_V2_FACTORY,
            };
            payloads.push(IAerodromeRouter::swapExactTokensForTokensCall {
                amountIn: self.loan_amount,
                amountOutMin: min_amount_out_leg1,
                routes: vec![aero_route],
                to: contract_address,
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
                recipient: contract_address,
                deadline: U256::from(u64::MAX),
                amountIn: min_amount_out_leg1,
                amountOutMinimum: min_amount_out_leg2,
            };
            payloads.push(IUniswapV3Router::exactInputCall { params: exact_input_params }.abi_encode());
        }

        (addresses, payloads)
    }
}

sol! {
    #[sol(rpc)]
    contract IUniswapV3Factory {
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool);
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

        function getAmountsOut(uint256 amountIn, Route[] calldata routes) external view returns (uint256[] memory amounts);
    }

    #[sol(rpc)]
    contract BaseAtomicArbitrage {
        function triggerBalancerArbitrage(address tokenToBorrow, uint256 loanAmount, bytes calldata swapPathData) external;
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
            decimals: 18,
            fee_tier_v3: 500,
        },
        TokenPairConfig {
            symbol: "VIRTUAL",
            target_token: VIRTUAL_BASE,
            aero_pool: address!("3f0296bf652e19bca772ec3df08b32732f93014a"),
            uni_v3_pool: fetch_v3_pool(&uni_factory, VIRTUAL_BASE, WETH_BASE, fee_tier_10000).await,
            decimals: 18,
            fee_tier_v3: 10000,
        },
        TokenPairConfig {
            symbol: "cbETH",
            target_token: CBETH_BASE,
            aero_pool: address!("47cA96EA59C13f72745928887F84c9F52C3D7348"),
            uni_v3_pool: fetch_v3_pool(&uni_factory, WETH_BASE, CBETH_BASE, fee_tier_500).await,
            decimals: 18,
            fee_tier_v3: 500,
        },
    ])
}

fn calculate_v3_price_dynamic(sqrt_price_x96: _U160, pool_token0: Address, target_token: Address, weth_price_usd: f64) -> f64 {
    let sqrt_f64 = sqrt_price_x96.to_string().parse::<f64>().unwrap_or(0.0);
    if sqrt_f64 <= 0.0 { return 0.0; }

    let ratio = sqrt_f64 / 2.0f64.powi(96);
    let price_raw = ratio * ratio;
    if price_raw <= 0.0 { return 0.0; }

    if target_token == WETH_BASE {
        let p = price_raw * 10f64.powi(12);
        return if p > 100.0 && p < 20000.0 { p } else if p > 0.0 { 1.0 / p * 10f64.powi(12) } else { 0.0 };
    }

    let is_target_token0 = pool_token0 == target_token;
    let price_in_weth = if is_target_token0 { price_raw } else { 1.0 / price_raw };
    let calculated_usd = price_in_weth * weth_price_usd;

    if calculated_usd.is_finite() && calculated_usd > 0.0 { calculated_usd } else { 0.0 }
}

async fn fetch_live_market_data_for_pair<P>(http_provider: P, config: &TokenPairConfig, weth_price_usd: f64) -> Result<(f64, f64, f64), Box<dyn std::error::Error>>
where P: Provider<Ethereum> + Clone {
    let mut price_aero = 0.0;

    let aero_router = IAerodromeRouter::new(AERODROME_V2_ROUTER, http_provider.clone());
    let sample_amount = U256::from(10_u128.pow(config.decimals));
    let route = IAerodromeRouter::Route {
        from: config.target_token,
        to: if config.symbol == "WETH" { USDC_BASE } else { WETH_BASE },
        stable: config.symbol == "cbETH",
        factory: AERODROME_V2_FACTORY,
    };

    if let Ok(amounts_out) = aero_router.getAmountsOut(sample_amount, vec![route]).call().await {
        if !amounts_out.is_empty() {
            let out_f64 = amounts_out.last().unwrap().to_string().parse::<f64>().unwrap_or(0.0);
            if config.symbol == "WETH" {
                price_aero = out_f64 / 1e6;
            } else {
                price_aero = (out_f64 / 1e18) * weth_price_usd;
            }
        }
    }

    let mut price_v3 = 0.0;
    if config.uni_v3_pool != Address::ZERO {
        let v3_pool = IUniswapV3Pool::new(config.uni_v3_pool, http_provider.clone());
        if let (Ok(slot0), Ok(p_token0)) = (v3_pool.slot0().call().await, v3_pool.token0().call().await) {
            let token0_addr: Address = p_token0.into();
            price_v3 = calculate_v3_price_dynamic(slot0.sqrtPriceX96, token0_addr, config.target_token, weth_price_usd);
        }
    }

    let spread_gap = if price_aero > 0.0 && price_v3 > 0.0 {
        (price_aero - price_v3).abs()
    } else {
        0.0
    };

    Ok((price_aero, price_v3, spread_gap))
}

async fn trigger_cross_dex_arbitrage<P, PP>(
    http_provider: P, 
    private_provider: PP, 
    contract_address: Address, 
    signer_address: Address, 
    spread_gap: f64, 
    pair_cfg: &TokenPairConfig, 
    price_aero: f64, 
    price_v3: f64
) -> Result<(), Box<dyn std::error::Error>>
where P: Provider<Ethereum> + Clone, PP: Provider<Ethereum> + Clone {
    if spread_gap < 5.0 { return Ok(()); } // خفضنا العتبة إلى $5 لالتقاط المكاسب فوراً

    let raw_loan = (spread_gap * 200.0) as u128 * 1_000_000;
    let dynamic_loan = raw_loan.clamp(1_000_000_000, 2_400_000_000); 
    let loan_amount = U256::from(dynamic_loan);

    println!("🎯 [Cross-DEX {}] Real Gap: ${:.2} | Optimal Loan: {} USDC... Simulating...", pair_cfg.symbol, spread_gap, dynamic_loan as f64 / 1_000_000.0);

    let system = CausalCollapseSystem::new(loan_amount, USDC_BASE, pair_cfg.clone(), price_aero, price_v3);
    let target_path = system.execute_collapse(contract_address);
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
            println!("🎉 [SIMULATION PASSED! 🎉] Dispatching profitable trade via Private RPC...");
            let contract_private = BaseAtomicArbitrage::new(contract_address, private_provider);
            if let Ok(tx) = contract_private.triggerBalancerArbitrage(USDC_BASE, loan_amount, swap_path_data.into()).from(signer_address).gas(1_500_000).send().await {
                println!("🚀 [Dispatched Successfully] Hash: {:?}", tx.tx_hash());
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
                        BaseAtomicArbitrage::BaseAtomicArbitrageErrors::InsufficientLiquidity => {
                            eprintln!("   💧 [Revert]: InsufficientLiquidity!");
                        }
                    }
                }
            }
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

    for pair in &monitored_pairs {
        let _ = BaseAtomicArbitrage::new(contract_address, http_provider.clone()).setTargetWhitelist(pair.target_token, true).from(signer_address).send().await;
    }

    let ws = alloy::providers::WsConnect::new(alchemy_wss_url);
    let ws_provider = ProviderBuilder::new().wallet(EthereumWallet::from(signer.clone())).connect_ws(ws).await?;
    let sub = ws_provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();
    let mut block_counter = 0u64;

    while let Some(block) = stream.next().await {
        block_counter += 1;
        let block_num = block.inner.number;
        println!("\n📦 WSS Block Synced: #{} (Counter: {}) - Scanning Markets...", block_num, block_counter);

        let mut current_weth_price = 2416.28;
        if let Some(weth_pair) = monitored_pairs.iter().find(|p| p.symbol == "WETH") {
            if let Ok((_, p_v3, _)) = fetch_live_market_data_for_pair(http_provider.clone(), weth_pair, 1.0).await {
                if p_v3 > 0.0 { current_weth_price = p_v3; }
            }
        }

        for pair in &monitored_pairs {
            if let Ok((p_aero, p_v3, spread_gap)) = fetch_live_market_data_for_pair(http_provider.clone(), pair, current_weth_price).await {
                println!(
                    " 📈 [{}] Aero: ${:.4} | UniV3: ${:.4} | Max Gap: ${:.2}",
                    pair.symbol, p_aero, p_v3, spread_gap
                );

                if spread_gap >= 5.0 {
                    let _ = trigger_cross_dex_arbitrage(
                        http_provider.clone(),
                        private_provider.clone(),
                        contract_address,
                        signer_address,
                        spread_gap,
                        pair,
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
