// token_manager.rs

use crate::{
    addresses::{
        BSC_ADA_ADDRESS, BSC_BTCB_ADDRESS, BSC_BUSD_ADDRESS, BSC_CAKE_ADDRESS, BSC_DAI_ADDRESS,
        BSC_ETH_ADDRESS, BSC_LINK_ADDRESS, BSC_TUSD_ADDRESS, BSC_USDC_ADDRESS, BSC_USDT_ADDRESS,
        BSC_WBNB_ADDRESS, BSC_XRP_ADDRESS,
    },
    token::{
        token::{BlockChain, Token},
        BscToken, PolygonToken,
    },
};
use ethers::providers::{Http, Provider};
use ethers::signers::LocalWallet;
use ethers::types::Address;
use std::str::FromStr;
use std::{error::Error, sync::Arc};

#[derive(Clone, Debug)]
pub struct ChainParams {
    pub chain_id: u64,
    pub rpc_node_url: &'static str,
    pub tokens: &'static [(&'static str, &'static str)],
    pub free_rate: f64,
}

pub const BSC_CHAIN_PARAMS: ChainParams = ChainParams {
    chain_id: 56,
    // "https://bsc-dataseed.binance.org/"
    // "https://bsc-dataseed1.ninicoin.io/"
    // "https://bsc-dataseed2.ninicoin.io/"
    rpc_node_url: "https://bsc-dataseed1.defibit.io/",
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
};

pub const POLYGON_CHAIN_PARAMS: ChainParams = ChainParams {
    chain_id: 137,
    rpc_node_url: "https://rpc-mainnet.maticvigil.com/",
    tokens: &[],
    free_rate: 0.3,
};

pub fn create_provider(chain_params: &ChainParams) -> Result<Arc<Provider<Http>>, Box<dyn Error>> {
    let provider = Provider::<Http>::try_from(chain_params.rpc_node_url)?;

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
