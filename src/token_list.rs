// token_list.rs

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
use ethers::types::Address;
use std::error::Error;
use std::str::FromStr;

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

pub fn create_tokens(chain_params_list: &Vec<&ChainParams>) -> Vec<Box<dyn Token>> {
    let mut tokens = Vec::new();
    for chain_params in chain_params_list.iter() {
        tokens.extend(chain_params.tokens.iter().map(|&(symbol, address)| {
            let token_address = Address::from_str(address).unwrap();
            let token = if chain_params.chain_id == 56 {
                Box::new(BscToken::new(
                    BlockChain::BscChain {
                        chain_id: chain_params.chain_id,
                    },
                    chain_params.rpc_node_url.to_owned(),
                    token_address,
                    symbol.to_owned(),
                    None,
                    chain_params.free_rate,
                )) as Box<dyn Token>
            } else if chain_params.chain_id == 137 {
                Box::new(PolygonToken::new(
                    BlockChain::PolygonChain {
                        chain_id: chain_params.chain_id,
                    },
                    chain_params.rpc_node_url.to_owned(),
                    token_address,
                    symbol.to_owned(),
                    None,
                    chain_params.free_rate,
                )) as Box<dyn Token>
            } else {
                unimplemented!("unsupported chain id: {}", chain_params.chain_id);
            };
            token
        }));
    }
    tokens
}

pub fn create_usdt_token(chain_params: &ChainParams) -> Result<Box<dyn Token>, Box<dyn Error>> {
    let usdt_symbol = "USDT";
    let usdt_address = Address::from_str(BSC_USDT_ADDRESS).unwrap();

    let usdt_token: Box<dyn Token> = if chain_params.chain_id == 56 {
        let token = BscToken::new(
            BlockChain::BscChain {
                chain_id: chain_params.chain_id,
            },
            chain_params.rpc_node_url.to_owned(),
            usdt_address,
            usdt_symbol.to_owned(),
            None,
            chain_params.free_rate,
        );
        Box::new(token)
    } else if chain_params.chain_id == 137 {
        let token = PolygonToken::new(
            BlockChain::PolygonChain {
                chain_id: chain_params.chain_id,
            },
            chain_params.rpc_node_url.to_owned(),
            usdt_address,
            usdt_symbol.to_owned(),
            None,
            chain_params.free_rate,
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
