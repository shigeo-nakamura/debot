// blockchain_factory.rs

use crate::{
    addresses::{
        BSC_BISWAP_ROUTER, BSC_ETH_ADDRESS, BSC_MAINNET_CHAIN_ID, BSC_PANCAKESWAP_ROUTER,
        BSC_TESTNET_CHAIN_ID, BSC_USDT_ADDRESS, BSC_WBNB_ADDRESS, POLYGON_MAINNET_CHAIN_ID,
        POLYGON_QUICKSWAP_ROUTER, POLYGON_SUSHISWAP_ROUTER, POLYGON_TESTNET_CHAIN_ID,
        POLYGON_USDC_ADDRESS, POLYGON_WMATIC_ADDRESS, TESTNET_BSC_APESWAP_ROUTER,
        TESTNET_BSC_BUSD_ADDRESS, TESTNET_BSC_PANCAKESWAP_ROUTER, TESTNET_BSC_WBNB_ADDRESS,
        TESTNET_POLYGON_DAI_ADDRESS, TESTNET_POLYGON_QUICKSWAP_ROUTER,
        TESTNET_POLYGON_SUSHISWAP_ROUTER, TESTNET_POLYGON_USDC_ADDRESS,
    },
    dex::{
        ApeSwap, ApeSwapPolygon, BabyDoge, BakerySwap, BiSwap, Dex, Dyfn, MeshSwap, PancakeSwap,
        QuickSwap, SushiSwap,
    },
    token::{
        token::{BlockChain, Token},
        BscToken, PolygonToken,
    },
};
use ethers::providers::{Http, Provider};
use ethers::signers::LocalWallet;
use ethers::types::Address;
use ethers_middleware::{NonceManagerMiddleware, SignerMiddleware};
use lazy_static::lazy_static;
use std::{error::Error, sync::Arc};
use std::{str::FromStr, sync::Mutex};

#[derive(Clone, Debug)]
pub struct ChainParams {
    pub chain_id: u64,
    pub chain_name: &'static str,
    pub rpc_node_urls: &'static [&'static str],
    pub tokens: &'static [(&'static str, &'static str)],
    pub dex_list: &'static [(&'static str, &'static str)],
    pub gas: f64,
    pub current_rpc_url: Arc<Mutex<usize>>,
    pub base_token: &'static str,
    pub min_gas_token_amount: f64,
}

lazy_static! {
    pub static ref BSC_CHAIN_PARAMS: ChainParams = ChainParams {
        chain_id: 56,
        chain_name: "BSC",
        rpc_node_urls: &[
            "https://bsc-dataseed.binance.org/",
            "https://bsc-dataseed1.ninicoin.io/",
            "https://bsc-dataseed2.ninicoin.io/",
            "https://bsc-dataseed1.defibit.io/",
        ],
        tokens: &[
            ("WBNB", BSC_WBNB_ADDRESS),
            // ("BTCB", BSC_BTCB_ADDRESS),
            ("ETH", BSC_ETH_ADDRESS),
            // ("BUSD", BSC_BUSD_ADDRESS),
            // ("USDC", BSC_USDC_ADDRESS),
            ("USDT", BSC_USDT_ADDRESS),
            // ("CAKE", BSC_CAKE_ADDRESS),
            // ("TUSD", BSC_TUSD_ADDRESS),
            // ("DAI", BSC_DAI_ADDRESS),
            // ("XRP", BSC_XRP_ADDRESS),
            // ("ADA", BSC_ADA_ADDRESS),
            // ("LINK", BSC_LINK_ADDRESS),
            // ("LTC", BSC_LTC_ADDRESS),
            // ("DOT", BSC_DOT_ADDRESS),
            // ("DOGE", BSC_DOGE_ADDRESS),
            // ("XVS", BSC_XVS_ADDRESS),
            // ("PEPE", BSC_PEPE_ADDRESS),
            // ("STK_BNB", BSC_STK_BNB_ADDRESS),
            // ("BNBX", BSC_BNBX_ADDRESS),
            // ("ANKR_BNB", BSC_ANKR_BNB_ADDRESS),
        ],
        dex_list: &[
            ("PancakeSwap", BSC_PANCAKESWAP_ROUTER),
            ("BiSwap", BSC_BISWAP_ROUTER),
            // ("ApeSwap", BSC_APESWAP_ROUTER),
            // ("BakerySwap", BSC_BAKERYSWAP_ROUTER),
            // ("BabyDoge", BSC_BABYDOGE_ROUTER),
        ],
        gas: 0.3,
        current_rpc_url: Arc::new(Mutex::new(0)),
        base_token: "USDT",
        min_gas_token_amount: 10.0,
    };

    pub static ref TESTNET_BSC_CHAIN_PARAMS: ChainParams = ChainParams {
        chain_id: 97, // This is the chain ID for Binance Smart Chain Testnet
        chain_name: "BSC_TESTNET",
        rpc_node_urls: &["https://data-seed-prebsc-1-s1.binance.org:8545/"],
        tokens: &[
            ("WBNB", TESTNET_BSC_WBNB_ADDRESS),
            ("BUSD", TESTNET_BSC_BUSD_ADDRESS),
            //("CAKE", TESTNET_BSC_CAKE_ADDRESS),
        ],
        dex_list: &[
            ("PancakeSwap", TESTNET_BSC_PANCAKESWAP_ROUTER),
            ("ApeSwap", TESTNET_BSC_APESWAP_ROUTER)
        ],
        gas: 0.3,
        current_rpc_url: Arc::new(Mutex::new(0)),
        base_token: "BUSD",
        min_gas_token_amount: 10.0,
    };

    pub static ref POLYGON_CHAIN_PARAMS: ChainParams = ChainParams {
        chain_id: 137,
        chain_name: "POLYGON",
        rpc_node_urls: &[
            "https://rpc-mainnet.maticvigil.com/",
            "https://polygon-rpc.com",
            ],
        tokens: &[
            ("USDC", POLYGON_USDC_ADDRESS),
            // ("USDT", POLYGON_USDT_ADDRESS),
            // ("WBTC", POLYGON_WBTC_ADDRESS),
            // ("WETH", POLYGON_WETH_ADDRESS),
            // ("DAI", POLYGON_DAI_ADDRESS),
            // ("MAI", POLYGON_MAI_ADDRESS),
            ("WMATIC", POLYGON_WMATIC_ADDRESS),
            // ("AAVE", POLYGON_AAVE_ADDRESS),
            // ("CRV", POLYGON_CRV_ADDRESS),
            // ("WSTETH", POLYGON_WSTETH_ADDRESS),
            // ("STMATIC", POLYGON_STMATIC_ADDRESS),
            // ("MATICX", POLYGON_MATICX_ADDRESS),
            // ("BAL", POLYGON_BAL_ADDRESS),
            // ("LINK", POLYGON_LINK_ADDRESS),
            // ("LDO", POLYGON_LDO_ADDRESS),
        ],
        dex_list: &[
            ("QuickSwap", POLYGON_QUICKSWAP_ROUTER),
            ("SushiSwap", POLYGON_SUSHISWAP_ROUTER),
            // ("MeshSwap", POLYGON_MESHSWAP_ROUTER),
            // ("Dyfn", POLYGON_DYFN_ROUTER),
            // ("ApeSwapPolygon", POLYGON_APESWAP_ROUTER),
        ],
        gas: 0.03,
        current_rpc_url: Arc::new(Mutex::new(0)),
        base_token: "USDC",
        min_gas_token_amount: 1.0,
    };

    pub static ref TESTNET_POLYGON_CHAIN_PARAMS: ChainParams = ChainParams {
        chain_id: 80001, // This is the chain ID for Mumbai Testnet
        chain_name: "POLYGON_TESTNET",
        rpc_node_urls: &["https://rpc-mumbai.maticvigil.com"],
        tokens: &[
            ("USDC", TESTNET_POLYGON_USDC_ADDRESS),
            ("DAI", TESTNET_POLYGON_DAI_ADDRESS),
        ],
        dex_list: &[
            ("SushiSwap", TESTNET_POLYGON_SUSHISWAP_ROUTER),
            ("QuickSwap", TESTNET_POLYGON_QUICKSWAP_ROUTER)
        ],
        gas: 0.03,
        current_rpc_url: Arc::new(Mutex::new(0)),
        base_token: "USDC",
        min_gas_token_amount: 1.0,
    };
}

fn create_token(
    chain_id: u64,
    provider: Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
    token_address: Address,
    symbol: String,
) -> Result<Box<dyn Token>, Box<dyn Error>> {
    match chain_id {
        BSC_MAINNET_CHAIN_ID | BSC_TESTNET_CHAIN_ID => Ok(Box::new(BscToken::new(
            BlockChain::BscChain { chain_id },
            provider.clone(),
            token_address,
            symbol,
            None,
        ))),
        POLYGON_MAINNET_CHAIN_ID | POLYGON_TESTNET_CHAIN_ID => Ok(Box::new(PolygonToken::new(
            BlockChain::PolygonChain { chain_id },
            provider.clone(),
            token_address,
            symbol,
            None,
        ))),
        _ => Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unsupported chain id: {}", chain_id),
        ))),
    }
}

pub async fn create_tokens(
    provider: Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
    chain_params: &ChainParams,
) -> Result<Arc<Vec<Box<dyn Token>>>, Box<dyn Error + Send + Sync + 'static>> {
    let tokens: Result<Vec<Box<dyn Token>>, Box<dyn Error>> = chain_params
        .tokens
        .iter()
        .map(|&(symbol, address)| {
            let token_address = Address::from_str(address)?;
            create_token(
                chain_params.chain_id,
                provider.clone(),
                token_address,
                symbol.to_owned(),
            )
        })
        .collect();

    let mut initialized_tokens = Vec::new();
    for mut token in tokens.unwrap() {
        token.initialize().await?;
        initialized_tokens.push(token);
    }

    Ok(Arc::new(initialized_tokens))
}

pub async fn create_base_token(
    provider: Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
    chain_params: &ChainParams,
) -> Result<Arc<Box<dyn Token>>, Box<dyn Error + Send + Sync + 'static>> {
    let base_token_symbol = chain_params.base_token;
    let base_token_address = chain_params
        .tokens
        .iter()
        .find(|(symbol, _)| *symbol == base_token_symbol)
        .ok_or_else(|| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Base token {} not found", base_token_symbol),
            )) as Box<dyn Error>
        })
        .unwrap()
        .1;

    let base_token_address = Address::from_str(base_token_address)?;
    let mut token = create_token(
        chain_params.chain_id,
        provider.clone(),
        base_token_address,
        base_token_symbol.to_owned(),
    )
    .unwrap();
    token.initialize().await?;
    Ok(Arc::new(token))
}

pub async fn create_dexes(
    provider: Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
    chain_params: &ChainParams,
) -> Result<Arc<Vec<Box<dyn Dex>>>, Box<dyn Error + Send + Sync>> {
    // Initialize DEX instances
    let dexes: Vec<Box<dyn Dex>> = chain_params
        .dex_list
        .iter()
        .map(|&(dex_name, router_address)| {
            let dex_router_address = Address::from_str(router_address).unwrap();
            let dex: Box<dyn Dex> = match dex_name {
                "PancakeSwap" => Box::new(PancakeSwap::new(provider.clone(), dex_router_address)),
                "BiSwap" => Box::new(BiSwap::new(provider.clone(), dex_router_address)),
                "BakerySwap" => Box::new(BakerySwap::new(provider.clone(), dex_router_address)),
                "ApeSwap" => Box::new(ApeSwap::new(provider.clone(), dex_router_address)),
                "BabyDoge" => Box::new(BabyDoge::new(provider.clone(), dex_router_address)),
                "SushiSwap" => Box::new(SushiSwap::new(provider.clone(), dex_router_address)),
                "Dyfn" => Box::new(Dyfn::new(provider.clone(), dex_router_address)),
                "QuickSwap" => Box::new(QuickSwap::new(provider.clone(), dex_router_address)),
                "MeshSwap" => Box::new(MeshSwap::new(provider.clone(), dex_router_address)),
                "ApeSwapPolygon" => {
                    Box::new(ApeSwapPolygon::new(provider.clone(), dex_router_address))
                }
                _ => panic!("Unknown DEX: {}", dex_name),
            };
            dex
        })
        .collect();

    let mut initialized_dexes = Vec::new();
    for mut dex in dexes {
        dex.initialize().await?;
        initialized_dexes.push(dex);
    }

    Ok(Arc::new(initialized_dexes))
}
