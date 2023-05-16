use crate::{
    addresses::{
        BSC_ADA_ADDRESS, BSC_BI_SWAP_ROUTER, BSC_BTCB_ADDRESS, BSC_BUSD_ADDRESS, BSC_CAKE_ADDRESS,
        BSC_DAI_ADDRESS, BSC_ETH_ADDRESS, BSC_LINK_ADDRESS, BSC_PANCAKE_SWAP_ROUTER,
        BSC_TUSD_ADDRESS, BSC_USDC_ADDRESS, BSC_USDT_ADDRESS, BSC_WBNB_ADDRESS, BSC_XRP_ADDRESS,
        TESTNET_BSC_APE_SWAP_ROUTER, TESTNET_BSC_BUSD_ADDRESS, TESTNET_BSC_PANCAKE_SWAP_ROUTER,
        TESTNET_BSC_WBNB_ADDRESS, TESTNET_POLYGON_MATIC_ADDRESS,
    },
    dex::{ApeSwap, BakerySwap, BiSwap, Dex, PancakeSwap},
    token::{
        token::{BlockChain, Token},
        BscToken, PolygonToken,
    },
};
use ethers::signers::LocalWallet;
use ethers::types::Address;
use ethers::{
    providers::{Http, Provider},
    utils::hex,
};
use ethers_middleware::core::k256::elliptic_curve::SecretKey;
use lazy_static::lazy_static;
use std::{error::Error, sync::Arc};
use std::{str::FromStr, sync::Mutex};

#[derive(Clone, Debug)]
pub struct ChainParams {
    pub chain_id: u64,
    pub rpc_node_urls: &'static [&'static str],
    pub tokens: &'static [(&'static str, &'static str)],
    pub dex_list: &'static [(&'static str, &'static str)],
    pub free_rate: f64,
    pub current_rpc_url: Arc<Mutex<usize>>,
}

lazy_static! {
    pub static ref BSC_CHAIN_PARAMS: ChainParams = ChainParams {
        chain_id: 56,
        rpc_node_urls: &[
            "https://bsc-dataseed.binance.org/",
            "https://bsc-dataseed1.ninicoin.io/",
            "https://bsc-dataseed2.ninicoin.io/",
            "https://bsc-dataseed1.defibit.io/",
        ],
        tokens: &[
            ("WBNB", BSC_WBNB_ADDRESS),
            ("BTCB", BSC_BTCB_ADDRESS),
            ("ETH ", BSC_ETH_ADDRESS),
            ("BUSD", BSC_BUSD_ADDRESS),
            ("USDC", BSC_USDC_ADDRESS),
            ("USDT", BSC_USDT_ADDRESS),
            // ("DAI ", BSC_DAI_ADDRESS),
            // ("XRP ", BSC_XRP_ADDRESS),
            // ("ADA ", BSC_ADA_ADDRESS),
            // ("LINK", BSC_LINK_ADDRESS),
            // ("CAKE", BSC_CAKE_ADDRESS),
            // ("TUSD", BSC_TUSD_ADDRESS),
        ],
        dex_list: &[
            ("PancakeSwap", BSC_PANCAKE_SWAP_ROUTER),
            ("BiSwap", BSC_BI_SWAP_ROUTER)
        ],
        free_rate: 0.3,
        current_rpc_url: Arc::new(Mutex::new(0)),
    };

    pub static ref TESTNET_BSC_CHAIN_PARAMS: ChainParams = ChainParams {
        chain_id: 97, // This is the chain ID for Binance Smart Chain Testnet
        rpc_node_urls: &["https://data-seed-prebsc-1-s1.binance.org:8545/"],
        tokens: &[
            // Update these with the correct testnet token addresses
            ("WBNB", TESTNET_BSC_WBNB_ADDRESS),
            ("USD", TESTNET_BSC_BUSD_ADDRESS),
            // add other token addresses here...
        ],
        dex_list: &[
            ("PancakeSwap", TESTNET_BSC_PANCAKE_SWAP_ROUTER),
            ("ApeSwap", TESTNET_BSC_APE_SWAP_ROUTER)
        ],
        free_rate: 0.3,
        current_rpc_url: Arc::new(Mutex::new(0)),
    };

    pub static ref POLYGON_CHAIN_PARAMS: ChainParams = ChainParams {
        chain_id: 137,
        rpc_node_urls: &["https://rpc-mainnet.maticvigil.com/"],
        tokens: &[],
        dex_list: &[],
        free_rate: 0.3,
        current_rpc_url: Arc::new(Mutex::new(0)),
    };

    pub static ref TESTNET_POLYGON_CHAIN_PARAMS: ChainParams = ChainParams {
        chain_id: 80001, // This is the chain ID for Mumbai Testnet
        rpc_node_urls: &["https://rpc-mumbai.maticvigil.com"],
        tokens: &[
            // Update these with the correct testnet token addresses
            ("MATIC", TESTNET_POLYGON_MATIC_ADDRESS),
            // add other token addresses here...
        ],
        dex_list: &[],
        free_rate: 0.3,
        current_rpc_url: Arc::new(Mutex::new(0)),
    };
}

pub fn create_wallet() -> Result<Arc<LocalWallet>, Box<dyn std::error::Error>> {
    let private_key_bytes =
        hex::decode("dd84b3084618a0ff534b482c5e3665b53805ce97c7ed1a46e39b671b3b897047")?;
    let secret_key = SecretKey::from_slice(&private_key_bytes)?;

    let wallet = LocalWallet::from(secret_key);
    Ok(Arc::new(wallet))
}

pub fn create_provider(chain_params: &ChainParams) -> Result<Arc<Provider<Http>>, Box<dyn Error>> {
    let mut current_rpc_url = chain_params.current_rpc_url.lock().unwrap();
    let provider = Provider::<Http>::try_from(chain_params.rpc_node_urls[*current_rpc_url])?;
    *current_rpc_url = (*current_rpc_url + 1) % chain_params.rpc_node_urls.len();

    Ok(Arc::new(provider))
}

pub fn create_tokens(
    chain_params: &ChainParams,
    wallet: Arc<LocalWallet>,
) -> Result<Arc<Vec<Box<dyn Token>>>, Box<dyn Error>> {
    let mut tokens = Vec::new();
    let provider = create_provider(chain_params)?;

    for &(symbol, address) in chain_params.tokens.iter() {
        let token_address = Address::from_str(address).unwrap();
        let token: Box<dyn Token> = if chain_params.chain_id == 56 {
            let bsc_token = BscToken::new(
                BlockChain::BscChain {
                    chain_id: chain_params.chain_id,
                },
                provider.clone(),
                token_address,
                symbol.to_owned(),
                None,
                chain_params.free_rate,
                wallet.clone(),
            );
            Box::new(bsc_token)
        } else if chain_params.chain_id == 137 {
            let polygon_token = PolygonToken::new(
                BlockChain::PolygonChain {
                    chain_id: chain_params.chain_id,
                },
                provider.clone(),
                token_address,
                symbol.to_owned(),
                None,
                chain_params.free_rate,
                wallet.clone(),
            );
            Box::new(polygon_token)
        } else {
            unimplemented!("unsupported chain id: {}", chain_params.chain_id);
        };
        tokens.push(token);
    }

    Ok(Arc::new(tokens))
}

pub fn create_usdt_token(
    chain_params: &ChainParams,
    wallet: Arc<LocalWallet>,
) -> Result<Arc<Box<dyn Token>>, Box<dyn Error>> {
    let usdt_symbol = "USDT";
    let usdt_address = Address::from_str(BSC_USDT_ADDRESS).unwrap();
    let provider = create_provider(chain_params)?;

    let usdt_token: Box<dyn Token> = if chain_params.chain_id == 56 {
        let token = BscToken::new(
            BlockChain::BscChain {
                chain_id: chain_params.chain_id,
            },
            provider.clone(),
            usdt_address,
            usdt_symbol.to_owned(),
            None,
            chain_params.free_rate,
            wallet.clone(),
        );
        Box::new(token)
    } else if chain_params.chain_id == 137 {
        let token = PolygonToken::new(
            BlockChain::PolygonChain {
                chain_id: chain_params.chain_id,
            },
            provider.clone(),
            usdt_address,
            usdt_symbol.to_owned(),
            None,
            chain_params.free_rate,
            wallet.clone(),
        );
        Box::new(token)
    } else {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unsupported chain id: {}", chain_params.chain_id),
        )));
    };

    Ok(Arc::new(usdt_token))
}

pub fn create_dexes(
    chain_params: &ChainParams,
) -> Result<Arc<Vec<Box<dyn Dex>>>, Box<dyn std::error::Error>> {
    let provider = create_provider(chain_params)?;

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
                _ => panic!("Unknown DEX: {}", dex_name),
            };
            dex
        })
        .collect();

    Ok(Arc::new(dexes))
}
