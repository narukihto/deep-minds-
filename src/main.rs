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

// الرموز الأساسية والعناوين الدقيقة على شبكة Base
const USDC_BASE: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bda02913");
const WETH_BASE: Address = address!("4200000000000000000000000000000000000006");
const AERO_BASE: Address = address!("940181a94A35A4569E4529A3CDfB74e38FD98631");
const DEGEN_BASE: Address = address!("4ed4e862860bed51a9570b96d89af5e1b0efefed");
const CBBTC_BASE: Address = address!("cbB7C0000aB88B473b1f5aFd9ef808440eed33Bf");

// موجهات المنصات الرئيسية (Routers) في القائمة البيضاء
const UNISWAP_V3_ROUTER: Address = address!("0xcf77A3bA9Aab7D3E44917635033322DF3f564171");
const AERODROME_ROUTER: Address = address!("0x2626664c2603336E57B271c5C0b26F421741e481");
const BASESWAP_ROUTER: Address = address!("0x327Df1e6de05895D2Ab08513aADD931325260A99"); 
const PANCAKESWAP_V3_ROUTER: Address = address!("0x1b81D678ffb9C0263b24A97847620C99d213eB14"); 

// عناوين مصانع المجمعات (Factory Contracts) الرسمية والدقيقة لشبكة Base Mainnet
const AERO_FACTORY: Address = address!("420DD381b31aEf6683db6B902084cB0FFECe40Da");
const UNI_V3_FACTORY: Address = address!("0x1F98431c8aD98523631AE4a59f267346ea31F984");
const BASESWAP_FACTORY: Address = address!("0xFda619b6d20975be80A10332cd39b9a4b0FAa8BB");
const PANCAKE_V3_FACTORY: Address = address!("0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865");

#[derive(Debug, Clone)]
pub struct PoolInfo {
    pub pool_address: Address,
    pub token0: Address,
    pub token1: Address,
}

#[derive(Debug, Clone)]
pub struct TokenPairConfig {
    pub symbol: &'static str,
    pub target_token: Address,
    pub decimals: u32,
    pub fee_tier_v3: u32,
    pub aero: Option<PoolInfo>,
    pub uni_v3: Option<PoolInfo>,
    pub baseswap: Option<PoolInfo>,
    pub pancakeswap: Option<PoolInfo>,
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
    pub prices: (f64, f64, f64, f64),
}

impl CausalCollapseSystem {
    pub fn new(
        nodes: Vec<QuantumNode>,
        dynamic_pools: Vec<Address>,
        contract_address: Address,
        loan_amount: U256,
        borrowed_token: Address,
        pair_config: TokenPairConfig,
        prices: (f64, f64, f64, f64),
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
            prices,
        }
    }

    pub fn execute_collapse(&self) -> (Vec<Address>, Vec<Vec<u8>>) {
        if self.nodes.is_empty() {
            return (vec![], vec![]);
        }

        let (p_aero, p_v3, p_baseswap, p_pancake) = self.prices;

        let mut dex_offers = Vec::new();
        if p_aero > 1.0 { dex_offers.push(("Aero", p_aero, AERODROME_ROUTER)); }
        if p_v3 > 1.0 { dex_offers.push(("UniV3", p_v3, UNISWAP_V3_ROUTER)); }
        if p_baseswap > 1.0 { dex_offers.push(("BaseSwap", p_baseswap, BASESWAP_ROUTER)); }
        if p_pancake > 1.0 { dex_offers.push(("PancakeV3", p_pancake, PANCAKESWAP_V3_ROUTER)); }

        if dex_offers.len() < 2 {
            return (vec![], vec![]);
        }

        dex_offers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let buy_dex = dex_offers.first().unwrap(); 
        let sell_dex = dex_offers.last().unwrap(); 

        let mut addresses = Vec::new();
        let mut payloads = Vec::new();

        // الخطوة الأولى: الشراء (السماح بانزلاق سعر بسيط 0.5% لضمان نجاح الصفقة محلياً)
        addresses.push(buy_dex.2);
        let min_amount_out_leg1 = self.loan_amount * U256::from(995) / U256::from(1000);

        let payload_leg1 = if buy_dex.0 == "Aero" || buy_dex.0 == "BaseSwap" {
            let aero_route = IAerodromeRouter::Route {
                from: self.borrowed_token,
                to: self.pair_config.target_token,
                stable: false,
                factory: AERO_FACTORY,
            };
            IAerodromeRouter::swapExactTokensForTokensCall {
                amountIn: self.loan_amount,
                amountOutMin: min_amount_out_leg1,
                routes: vec![aero_route],
                to: self.contract_address,
                deadline: U256::from(u64::MAX),
            }
            .abi_encode()
        } else {
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
            IUniswapV3Router::exactInputCall { params: exact_input_params }.abi_encode()
        };
        payloads.push(payload_leg1);

        // الخطوة الثانية: البيع (تغطية أصل القرض والرسوم بدقة مع هامش ربح آمن)
        addresses.push(sell_dex.2);
        let min_amount_out_leg2 = self.loan_amount * U256::from(10002) / U256::from(10000);

        let payload_leg2 = if sell_dex.0 == "Aero" || sell_dex.0 == "BaseSwap" {
            let aero_route_sell = IAerodromeRouter::Route {
                from: self.pair_config.target_token,
                to: self.borrowed_token,
                stable: false,
                factory: AERO_FACTORY,
            };
            IAerodromeRouter::swapExactTokensForTokensCall {
                amountIn: min_amount_out_leg1,
                amountOutMin: min_amount_out_leg2,
                routes: vec![aero_route_sell],
                to: self.contract_address,
                deadline: U256::from(u64::MAX),
            }
            .abi_encode()
        } else {
            let mut path_bytes_sell = Vec::new();
            path_bytes_sell.extend_from_slice(self.pair_config.target_token.as_slice());
            let fee_bytes = self.pair_config.fee_tier_v3.to_be_bytes();
            path_bytes_sell.extend_from_slice(&fee_bytes[1..4]);
            path_bytes_sell.extend_from_slice(self.borrowed_token.as_slice());

            let exact_input_params_sell = IUniswapV3Router::ExactInputParams {
                path: Bytes::from(path_bytes_sell),
                recipient: self.contract_address,
                deadline: U256::from(u64::MAX),
                amountIn: min_amount_out_leg1,
                amountOutMinimum: min_amount_out_leg2,
            };
            IUniswapV3Router::exactInputCall { params: exact_input_params_sell }.abi_encode()
        };
        payloads.push(payload_leg2);

        println!("⚡ [STRICT DYNAMIC ROUTER] Validated Buy on {} (${:.2}) -> Sell on {} (${:.2})", 
            buy_dex.0, buy_dex.1, sell_dex.0, sell_dex.1);

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
    contract IUniswapV2Factory {
        function getPair(address tokenA, address tokenB) external view returns (address pair);
    }

    #[sol(rpc)]
    contract IAerodromeFactory {
        function getPool(address tokenA, address tokenB, bool stable) external view returns (address pool);
    }

    #[sol(rpc)]
    contract IUniswapV3Factory {
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool);
    }

    #[sol(rpc)]
    contract IUniswapV2Pair {
        function token0() external view returns (address);
        function token1() external view returns (address);
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }

    #[sol(rpc)]
    contract IUniswapV3Pool {
        function token0() external view returns (address);
        function token1() external view returns (address);
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
    }
}

async fn resolve_pool_info<P>(
    provider: P,
    pool_address: Address,
) -> Option<PoolInfo>
where
    P: Provider<Ethereum> + Clone,
{
    if pool_address.is_zero() {
        return None;
    }
    let pair_contract = IUniswapV2Pair::new(pool_address, provider);
    let t0 = pair_contract.token0().call().await.ok().map(|r| Address::from(r.0))?;
    let t1 = pair_contract.token1().call().await.ok().map(|r| Address::from(r.0))?;

    Some(PoolInfo {
        pool_address,
        token0: t0,
        token1: t1,
    })
}

async fn find_v3_pool_across_fees<P>(
    provider: P,
    factory_address: Address,
    token_a: Address,
    token_b: Address,
) -> Option<(Address, u32)>
where
    P: Provider<Ethereum> + Clone,
{
    let factory = IUniswapV3Factory::new(factory_address, provider.clone());
    let fee_tiers = [100u32, 500u32, 3000u32, 10000u32];

    for &fee in &fee_tiers {
        if let Ok(res) = factory.getPool(token_a, token_b, _U24::from(fee)).call().await {
            let addr = Address::from(res.0);
            if !addr.is_zero() {
                let pool = IUniswapV3Pool::new(addr, provider.clone());
                if let Ok(slot0) = pool.slot0().call().await {
                    if slot0.sqrtPriceX96 > _U160::ZERO {
                        return Some((addr, fee));
                    }
                }
            }
        }
    }
    None
}

async fn resolve_token_pair<P>(
    provider: P,
    symbol: &'static str,
    target_token: Address,
    decimals: u32,
) -> TokenPairConfig
where
    P: Provider<Ethereum> + Clone,
{
    let aero_factory = IAerodromeFactory::new(AERO_FACTORY, provider.clone());
    let aero_addr = aero_factory.getPool(USDC_BASE, target_token, false).call().await
        .map(|r| Address::from(r.0)).unwrap_or_else(|_| Address::ZERO);
    let aero = resolve_pool_info(provider.clone(), aero_addr).await;

    let mut selected_fee = 500u32;
    let uni_v3 = if let Some((uni_addr, fee)) = find_v3_pool_across_fees(provider.clone(), UNI_V3_FACTORY, USDC_BASE, target_token).await {
        selected_fee = fee;
        resolve_pool_info(provider.clone(), uni_addr).await
    } else {
        None
    };

    let baseswap_factory = IUniswapV2Factory::new(BASESWAP_FACTORY, provider.clone());
    let baseswap_addr = baseswap_factory.getPair(USDC_BASE, target_token).call().await
        .map(|r| Address::from(r.0)).unwrap_or_else(|_| Address::ZERO);
    let baseswap = resolve_pool_info(provider.clone(), baseswap_addr).await;

    let pancakeswap = if let Some((pancake_addr, _)) = find_v3_pool_across_fees(provider.clone(), PANCAKE_V3_FACTORY, USDC_BASE, target_token).await {
        resolve_pool_info(provider.clone(), pancake_addr).await
    } else {
        None
    };

    TokenPairConfig {
        symbol,
        target_token,
        decimals,
        fee_tier_v3: selected_fee,
        aero,
        uni_v3,
        baseswap,
        pancakeswap,
    }
}

fn calculate_v2_price(
    reserve0: u128,
    reserve1: u128,
    token0: Address,
    token1: Address,
    target_token: Address,
    target_decimals: u32,
) -> f64 {
    if reserve0 == 0 || reserve1 == 0 {
        return 0.0;
    }
    let usdc_decimals = 6;
    let d0 = if token0 == USDC_BASE { usdc_decimals } else { target_decimals };
    let d1 = if token1 == USDC_BASE { usdc_decimals } else { target_decimals };

    let r0 = reserve0 as f64 / 10f64.powi(d0 as i32);
    let r1 = reserve1 as f64 / 10f64.powi(d1 as i32);

    if target_token == token1 {
        r0 / r1
    } else {
        r1 / r0
    }
}

fn calculate_v3_price(
    sqrt_price_x96: _U160,
    token0: Address,
    token1: Address,
    target_token: Address,
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

    let price_t1_in_t0 = price_raw * 10f64.powi((d0 as i32) - (d1 as i32));

    if target_token == token1 {
        price_t1_in_t0
    } else {
        if price_t1_in_t0 > 0.0 { 1.0 / price_t1_in_t0 } else { 0.0 }
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
    if let Some(pool) = &config.aero {
        let pair = IUniswapV2Pair::new(pool.pool_address, http_provider.clone());
        if let Ok(res) = pair.getReserves().call().await {
            price_aero = calculate_v2_price(
                res.reserve0.to::<u128>(),
                res.reserve1.to::<u128>(),
                pool.token0,
                pool.token1,
                config.target_token,
                config.decimals,
            );
        }
    }

    let mut price_v3 = 0.0;
    if let Some(pool) = &config.uni_v3 {
        let v3_pool = IUniswapV3Pool::new(pool.pool_address, http_provider.clone());
        if let Ok(slot0) = v3_pool.slot0().call().await {
            price_v3 = calculate_v3_price(
                slot0.sqrtPriceX96,
                pool.token0,
                pool.token1,
                config.target_token,
                config.decimals,
            );
        }
    }

    let mut price_baseswap = 0.0;
    if let Some(pool) = &config.baseswap {
        let pair = IUniswapV2Pair::new(pool.pool_address, http_provider.clone());
        if let Ok(res) = pair.getReserves().call().await {
            price_baseswap = calculate_v2_price(
                res.reserve0.to::<u128>(),
                res.reserve1.to::<u128>(),
                pool.token0,
                pool.token1,
                config.target_token,
                config.decimals,
            );
        }
    }

    let mut price_pancake = 0.0;
    if let Some(pool) = &config.pancakeswap {
        let v3_pool = IUniswapV3Pool::new(pool.pool_address, http_provider.clone());
        if let Ok(slot0) = v3_pool.slot0().call().await {
            price_pancake = calculate_v3_price(
                slot0.sqrtPriceX96,
                pool.token0,
                pool.token1,
                config.target_token,
                config.decimals,
            );
        }
    }

    let mut prices = vec![price_aero, price_v3, price_baseswap, price_pancake];
    
    let (min_valid, max_valid) = match config.symbol {
        "WETH" => (100.0, 100000.0),
        "AERO" => (0.1, 10.0),
        "DEGEN" => (0.0001, 10.0),
        "cbBTC" => (10000.0, 200000.0),
        _ => (0.01, 1000000.0),
    };

    prices.retain(|&p| p >= min_valid && p <= max_valid);

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

    if spread_gap < 0.25 {
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
            println!("✅ Local Simulation Passed for {}! Dispatching Real Transaction...", pair_symbol);
            if let Ok(pending_tx) = balancer_builder.send().await {
                println!("✅ Tx Dispatched! Hash: {:?}", pending_tx.tx_hash());
            }
        }
        Err(e) => {
            // طباعة رسالة الخطأ النصية الفعلية القادمة من العقد الذكي إن وجدت
            println!("🛡 [SIMULATION REVERTED - {}]: السبب الفعلي للفشل: {:?}", pair_symbol, e);
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

    let http_provider = ProviderBuilder::new()
        .wallet(wallet.clone())
        .connect_http(primary_http_url.parse()?);

    let ws = alloy::providers::WsConnect::new(alchemy_wss_url);
    let ws_provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_ws(ws)
        .await?;

    println!("🔄 Resolving Pool Addresses Dynamically across official Base Factory Contracts...");
    
    let mut monitored_pairs = Vec::new();
    monitored_pairs.push(resolve_token_pair(http_provider.clone(), "WETH", WETH_BASE, 18).await);
    monitored_pairs.push(resolve_token_pair(http_provider.clone(), "AERO", AERO_BASE, 18).await);
    monitored_pairs.push(resolve_token_pair(http_provider.clone(), "DEGEN", DEGEN_BASE, 18).await);
    monitored_pairs.push(resolve_token_pair(http_provider.clone(), "cbBTC", CBBTC_BASE, 8).await);

    let sub = ws_provider.subscribe_blocks().await?;
    let mut stream = sub.into_stream();
    let mut block_counter = 0u64;

    let loan_amount = U256::from(10_000_000_000u64); // 10,000 USDC

    while let Some(block) = stream.next().await {
        block_counter += 1;
        let block_num = block.inner.number;
        println!("\n📦 WSS Block Synced: #{} (Counter: {})", block_num, block_counter);

        for pair in &monitored_pairs {
            let provider = http_provider.clone();
            let pair_cfg = pair.clone();

            if let Ok((p_aero, p_v3, p_baseswap, p_pancake, spread_gap)) = fetch_live_market_data_for_pair(provider.clone(), &pair_cfg).await {
                println!(
                    " 📈 [{}] Aero: ${:.2} | UniV3: ${:.2} | BaseSwap: ${:.2} | Pancake: ${:.2} | Max Gap: ${:.2}",
                    pair_cfg.symbol, p_aero, p_v3, p_baseswap, p_pancake, spread_gap
                );

                if spread_gap > 0.25 {
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
                        (p_aero, p_v3, p_baseswap, p_pancake),
                    );

                    let optimized_path = system.execute_collapse();

                    let _ = trigger_on_chain_arbitrage(
                        provider,
                        contract_address,
                        optimized_path,
                        signer_address,
                        USDC_BASE,
                        loan_amount,
                        spread_gap,
                        pair_cfg.symbol,
                    ).await;
                }
            }
        }

        sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}
