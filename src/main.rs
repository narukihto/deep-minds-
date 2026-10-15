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

const UNISWAP_V3_ROUTER: Address = address!("0xcf77A3bA9Aab7D3E44917635033322DF3f564171");
const AERODROME_ROUTER: Address = address!("0x2626664c2603336E57B271c5C0b26F421741e481");
const AERODROME_FACTORY: Address = address!("0x420DD381b31aEf6683db6B902084cB0FFECe40Da");

const AERO_POOL_USDC_WETH: Address = address!("cdac0d6c6c59727a65f871236188350531885c43");
const UNISWAP_V3_POOL_USDC_WETH: Address = address!("0x6c561b446416e1a00e8e93e221854d6ea4171372");

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

        let primary_node = &active_nodes[0];
        
        let mut addresses = Vec::new();
        let mut payloads = Vec::new();

        // الساق الأولى: الشراء من Aerodrome (مثلاً اقتراض USDC لشراء WETH)
        addresses.push(AERODROME_ROUTER);
        
        // حساب الحد الأدنى بدقة لتقليل الانزلاق (0.3% فقط انزلاق مسموح لحماية الربح)
        let min_amount_out_leg1 = self.loan_amount * U256::from(997) / U256::from(1000);

        let aero_route_1 = IAerodromeRouter::Route {
            from: self.borrowed_token, // USDC
            to: WETH_BASE,            // WETH
            stable: false,
            factory: AERODROME_FACTORY,
        };

        let payload_leg1 = IAerodromeRouter::swapExactTokensForTokensCall {
            amountIn: self.loan_amount,
            amountOutMin: min_amount_out_leg1,
            routes: vec![aero_route_1],
            to: self.contract_address,
            deadline: U256::from(u64::MAX),
        }
        .abi_encode();

        payloads.push(payload_leg1);

        // الساق الثانية: البيع في Uniswap V3 لإغلاق الدائرة واسترجاع USDC بربح
        addresses.push(UNISWAP_V3_ROUTER);

        // حساب كمية WETH الناتجة تقريبياً لبيعها بالكامل
        let estimated_weth_received = min_amount_out_leg1; 
        let min_amount_out_leg2 = self.loan_amount * U256::from(1001) / U256::from(1000); // يجب أن تغطي أصل القرض + هامش ربح طفيف

        let mut path_bytes = Vec::new();
        path_bytes.extend_from_slice(WETH_BASE.as_slice());
        path_bytes.extend_from_slice(&[0x00, 0x01, 0xf4]); // 0.3% fee tier
        path_bytes.extend_from_slice(self.borrowed_token.as_slice());

        let exact_input_params = IUniswapV3Router::ExactInputParams {
            path: Bytes::from(path_bytes),
            recipient: self.contract_address,
            deadline: U256::from(u64::MAX),
            amountIn: estimated_weth_received,
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
        error NotAuthorized();
        error NotOwner();
        error UntrustedLender();
        error UntrustedInitiator();
        error UntrustedOriginalInitiator();
        error UntrustedAavePool();
        error LengthMismatch();
        error SelfCallBlocked();
        error TargetUnauthorized(address target);
        error ExternalCallFailed(bytes reason);
        error ArbitrageUnprofitable(uint256 required, uint256 actual);
        error BalancerRepaymentFailed();
        error AaveApproveFailed();
        error NoBalance();
        error NoEthBalance();
        error TransferFailed();
        error EthTransferFailed();

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

async fn fetch_live_market_data<P>(
    http_provider: P,
    cached_pools: &[Address],
) -> Result<(f64, Address, U256, Address, Address, Vec<Address>), Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
{
    let aero_pair = IUniswapV2Pair::new(AERO_POOL_USDC_WETH, http_provider.clone());
    let mut price_aero = 0.0;
    if let Ok(res) = aero_pair.getReserves().call().await {
        let r0 = res.reserve0.to::<u128>() as f64 / 10f64.powi(18);
        let r1 = res.reserve1.to::<u128>() as f64 / 10f64.powi(6);
        if r0 > 0.0 {
            price_aero = r1 / r0;
        }
    }

    let v3_pool = IUniswapV3Pool::new(UNISWAP_V3_POOL_USDC_WETH, http_provider.clone());
    let mut price_v3 = 0.0;
    if let Ok(slot0) = v3_pool.slot0().call().await {
        let sqrt_price_x96 = slot0.sqrtPriceX96.to_string().parse::<f64>().unwrap_or(0.0);
        if sqrt_price_x96 > 0.0 {
            let ratio = sqrt_price_x96 / (2.0f64.powi(96));
            let raw_price = ratio * ratio;
            price_v3 = raw_price * 10f64.powi(12);
            if price_v3 > 0.0 && price_v3 < 1000.0 {
                price_v3 = 1.0 / price_v3 * 10f64.powi(12);
            }
        }
    }

    if price_v3 <= 0.0 {
        price_v3 = price_aero * 0.998; 
    }

    let spread_gap = (price_aero - price_v3).abs();
    println!(" 📈 [REALTIME PRICES] Aerodrome ETH: ${:.2} | Uniswap V3 ETH: ${:.2} | Gap: ${:.4}", price_aero, price_v3, spread_gap);

    let discovered_pools = if cached_pools.is_empty() {
        vec![UNISWAP_V3_ROUTER, AERODROME_ROUTER]
    } else {
        cached_pools.to_vec()
    };

    Ok((
        spread_gap,
        USDC_BASE,
        U256::from(50_000_000_000u64), // 50,000 USDC Loan
        USDC_BASE,
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
    spread_gap: f64,
) -> Result<(), Box<dyn std::error::Error>>
where
    P: Provider<Ethereum> + Clone,
{
    if loan_amount == U256::ZERO || target_path.0.is_empty() {
        println!("🛡 [PRE-FLIGHT SHIELD] Invalid loan or empty path. Skipping execution.");
        return Ok(());
    }

    let eth_price = 2429.0;
    let profit_ratio = spread_gap / eth_price;
    let loan_f64 = loan_amount.to::<u128>() as f64;
    let gross_profit_units = loan_f64 * profit_ratio;
    
    let aave_fee = loan_amount * U256::from(5) / U256::from(10000); 
    let aave_fee_units = aave_fee.to::<u128>() as f64;

    if gross_profit_units <= aave_fee_units {
        println!("🛡 [PROFIT SHIELD] Estimated profit <= Flash Loan Fee. Skipping...");
        return Ok(());
    }

    if spread_gap < 0.50 {
        println!("🛡 [PROFIT SHIELD] Gap is below threshold. Skipping execution.");
        return Ok(());
    }

    println!("🚀 [BOT -> CONTRACT] Simulating Atomic Multi-Swap Command locally...");

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
            println!("✅ Local Simulation Passed Successfully! Dispatching Real Transaction to Blockchain...");
            match balancer_builder.send().await {
                Ok(pending_tx) => {
                    println!("✅ Transaction Dispatched! Tx Hash: {:?}", pending_tx.tx_hash());
                    match pending_tx.get_receipt().await {
                        Ok(receipt) => {
                            println!("📦 Transaction Mined In Block: {:?} | Status: {:?}", receipt.block_number, receipt.status());
                        }
                        Err(e) => {
                            println!("❌ [TRANSACTION RECEIPT FAILED]: {:#?}", e);
                        }
                    }
                }
                Err(e_send) => {
                    println!("❌ [SEND TRANSACTION FAILED]: {:#?}", e_send);
                }
            }
        }
        Err(_e_sim) => {
            println!("🛡 [SIMULATION REVERTED SAFELY]: الصفقة غير مربحة أو ستفشل محلياً، تم منع إرسالها لتوفير رسوم الغاز.");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Initializing Universal Dynamic Token Scanner Core via Alloy...");

    let contract_addr_str = std::env::var("CONTRACT_ADDR")
        .unwrap_or_else(|_| "0x94409D722D4b21A496A3C073477F06Ca388B9C3B".to_string());
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

    let cached_pools = match fetch_dynamic_pools(http_provider.clone()).await {
        Ok(pools) => pools,
        Err(_) => vec![],
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

        if live_market_price > 0.00001 {
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
                    live_market_price,
                ).await {
                    println!("❌ Error executing on-chain command: {:?}", e);
                }
            }
        }

        sleep(Duration::from_millis(150)).await;
    }

    println!("🏁 Live stream processing terminated.");
    Ok(())
}
