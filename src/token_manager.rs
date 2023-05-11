use crate::{
    addresses::{
        BSC_ADA_ADDRESS, BSC_BTCB_ADDRESS, BSC_BUSD_ADDRESS, BSC_CAKE_ADDRESS, BSC_DAI_ADDRESS,
        BSC_ETH_ADDRESS, BSC_LINK_ADDRESS, BSC_TUSD_ADDRESS, BSC_USDC_ADDRESS, BSC_USDT_ADDRESS,
        BSC_WBNB_ADDRESS, BSC_XRP_ADDRESS, TESTNET_BSC_BTCB_ADDRESS, TESTNET_BSC_WBNB_ADDRESS,
        TESTNET_POLYGON_MATIC_ADDRESS,
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
    pub free_rate: f64,
    pub current_rpc_url: Arc<Mutex<usize>>,
}

lazy_static! {
    pub static ref DEX_LIST: Vec<&'static str> = vec!["PancakeSwap", "BiSwap" /*"BakerySwap", "ApeSwap" */];

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
        free_rate: 0.3,
        current_rpc_url: Arc::new(Mutex::new(0)),
    };

    pub static ref BSC_TESTNET_CHAIN_PARAMS: ChainParams = ChainParams {
        chain_id: 97, // This is the chain ID for Binance Smart Chain Testnet
        rpc_node_urls: &["https://data-seed-prebsc-1-s1.binance.org:8545/"],
        tokens: &[
            // Update these with the correct testnet token addresses
            ("WBNB", TESTNET_BSC_WBNB_ADDRESS),
            ("BTCB", TESTNET_BSC_BTCB_ADDRESS),
            // add other token addresses here...
        ],
        free_rate: 0.3,
        current_rpc_url: Arc::new(Mutex::new(0)),
    };

    pub static ref POLYGON_CHAIN_PARAMS: ChainParams = ChainParams {
        chain_id: 137,
        rpc_node_urls: &["https://rpc-mainnet.maticvigil.com/"],
        tokens: &[],
        free_rate: 0.3,
        current_rpc_url: Arc::new(Mutex::new(0)),
    };

    pub static ref POLYGON_TESTNET_CHAIN_PARAMS: ChainParams = ChainParams {
        chain_id: 80001, // This is the chain ID for Mumbai Testnet
        rpc_node_urls: &["https://rpc-mumbai.maticvigil.com"],
        tokens: &[
            // Update these with the correct testnet token addresses
            ("MATIC", TESTNET_POLYGON_MATIC_ADDRESS),
            // add other token addresses here...
        ],
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
) -> Result<Vec<Box<dyn Token>>, Box<dyn Error>> {
    let mut tokens = Vec::new();
    let provider = create_provider(chain_params)?;

    for &(symbol, address) in chain_params.tokens.iter() {
        let token_address = Address::from_str(address).unwrap();
        let token = if chain_params.chain_id == 56 {
            Box::new(BscToken::new(
                BlockChain::BscChain {
                    chain_id: chain_params.chain_id,
                },
                provider.clone(),
                token_address,
                symbol.to_owned(),
                None,
                chain_params.free_rate,
                wallet.clone(),
            )) as Box<dyn Token>
        } else if chain_params.chain_id == 137 {
            Box::new(PolygonToken::new(
                BlockChain::PolygonChain {
                    chain_id: chain_params.chain_id,
                },
                provider.clone(),
                token_address,
                symbol.to_owned(),
                None,
                chain_params.free_rate,
                wallet.clone(),
            )) as Box<dyn Token>
        } else {
            unimplemented!("unsupported chain id: {}", chain_params.chain_id);
        };
        tokens.push(token);
    }

    Ok(tokens)
}

pub fn create_usdt_token(
    chain_params: &ChainParams,
    wallet: Arc<LocalWallet>,
) -> Result<Box<dyn Token>, Box<dyn Error>> {
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
            wallet,
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
            wallet,
        );
        Box::new(token)
    } else {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unsupported chain id: {}", chain_params.chain_id),
        )));
    };

    Ok(usdt_token)
}

pub fn create_dexes(
    chain_params: &ChainParams,
) -> Result<Arc<Vec<(String, Box<dyn Dex>)>>, Box<dyn std::error::Error>> {
    let provider = create_provider(chain_params)?;

    // Initialize DEX instances
    let dexes: Vec<(String, Box<dyn Dex>)> = DEX_LIST
        .iter()
        .map(|&dex_name| {
            let dex: Box<dyn Dex> = match dex_name {
                "PancakeSwap" => Box::new(PancakeSwap::new(provider.clone())),
                "BiSwap" => Box::new(BiSwap::new(provider.clone())),
                "BakerySwap" => Box::new(BakerySwap::new(provider.clone())),
                "ApeSwap" => Box::new(ApeSwap::new(provider.clone())),
                _ => panic!("Unknown DEX: {}", dex_name),
            };
            (dex_name.to_string(), dex)
        })
        .collect();

    Ok(Arc::new(dexes))
}
