use std::convert::TryFrom;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;

use coins_bip32::path::DerivationPath;

use anyhow::anyhow;
use anyhow::Context as _;
use ethers::abi::Detokenize;
use ethers::prelude::builders::ContractCall;
use ethers::prelude::*;
use ethers::types::transaction::eip712::Eip712;
use ethers::types::Chain;

use rad_terminal::args;
use rad_terminal::components as term;

pub const RADICLE_DOMAIN: &str = ".radicle.eth";

pub const SIGNER_OPTIONS: &str = r#"
    --ledger-hdpath <hdpath>     Account derivation path when using a Ledger hardware device
    --keystore <file>            Keystore file containing encrypted private key (default: none)
"#;

pub const PROVIDER_OPTIONS: &str = r#"
    --rpc-url <url>              JSON-RPC URL of Ethereum node (eg. http://localhost:8545)
"#;

pub const ENVIRONMENT_VARIABLES: &str = r#"
    ETH_RPC_URL  Ethereum JSON-RPC URL (overwrite with '--rpc-url')
    ETH_HDPATH   Hardware wallet derivation path (overwrite with '--ledger-hdpath')
"#;

#[derive(Default, Debug)]
pub struct SignerOptions {
    /// Account derivation path when using a Ledger hardware wallet.
    pub ledger_hdpath: Option<DerivationPath>,
    /// Keystore file containing encrypted private key (default: none).
    pub keystore: Option<PathBuf>,
}

impl SignerOptions {
    pub fn from(mut parser: lexopt::Parser) -> anyhow::Result<(Self, lexopt::Parser)> {
        use lexopt::prelude::*;

        let mut unparsed: Vec<OsString> = Vec::new();
        let mut options = Self {
            keystore: None,
            ledger_hdpath: env::var("ETH_HDPATH")
                .ok()
                .and_then(|v| DerivationPath::from_str(v.as_str()).ok()),
        };

        while let Some(arg) = parser.next()? {
            match arg {
                Long(flag @ "ledger-hdpath") => {
                    let flag = flag.to_owned();
                    let value = parser.value()?;

                    options.ledger_hdpath = Some(args::parse_value(&flag, value)?);
                }
                Long(flag @ "keystore") => {
                    let flag = flag.to_owned();
                    let value = parser.value()?;

                    options.keystore = Some(args::parse_value(&flag, value)?);
                }
                _ => unparsed.push(args::format(arg)),
            }
        }
        Ok((options, lexopt::Parser::from_args(unparsed)))
    }
}

#[derive(Default, Debug)]
pub struct ProviderOptions {
    pub rpc_url: Option<String>,
}

impl ProviderOptions {
    pub fn from(mut parser: lexopt::Parser) -> anyhow::Result<(Self, lexopt::Parser)> {
        use lexopt::prelude::*;

        let mut unparsed: Vec<OsString> = Vec::new();
        let mut options = Self::default();

        while let Some(arg) = parser.next()? {
            match arg {
                Long(flag @ "rpc-url") => {
                    let flag = flag.to_owned();
                    let value = parser.value()?;

                    options.rpc_url = Some(args::parse_value(&flag, value)?);
                }
                _ => unparsed.push(args::format(arg)),
            }
        }
        Ok((options, lexopt::Parser::from_args(unparsed)))
    }
}

pub fn provider(cfg: ProviderOptions) -> anyhow::Result<Provider<Http>> {
    let rpc_url = if let Some(url) = cfg.rpc_url {
        url
    } else {
        env::var("ETH_RPC_URL")
            .ok()
            .and_then(|url| if url.is_empty() { None } else { Some(url) })
            .ok_or_else(|| {
                anyhow::anyhow!("'ETH_RPC_URL' must be set to an Ethereum JSON-RPC URL")
            })?
    };

    let provider =
        Provider::<Http>::try_from(rpc_url.as_str()).context("JSON-RPC URL parsing failed")?;

    Ok(provider)
}

#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error(transparent)]
    Local(#[from] ethers::signers::WalletError),
    #[error("no wallet specified")]
    NoWallet,
}

#[derive(Debug)]
pub enum Wallet {
    Ledger(Ledger),
    Local(LocalWallet),
}

#[async_trait::async_trait]
impl Signer for Wallet {
    type Error = WalletError;

    fn chain_id(&self) -> u64 {
        match self {
            Self::Ledger(s) => s.chain_id(),
            Self::Local(s) => s.chain_id(),
        }
    }

    fn address(&self) -> Address {
        match self {
            Self::Ledger(s) => s.address(),
            Self::Local(s) => s.address(),
        }
    }

    fn with_chain_id<T: Into<u64>>(self, chain_id: T) -> Self {
        match self {
            Self::Ledger(s) => Self::Ledger(s.with_chain_id(chain_id)),
            Self::Local(s) => Self::Local(s.with_chain_id(chain_id)),
        }
    }

    async fn sign_typed_data<T: Eip712 + Send + Sync>(
        &self,
        payload: &T,
    ) -> Result<Signature, Self::Error> {
        match self {
            Self::Ledger(s) => s.sign_typed_data(payload).await.map_err(WalletError::from),
            Self::Local(s) => s.sign_typed_data(payload).await.map_err(WalletError::from),
        }
    }

    async fn sign_message<S: Send + Sync + AsRef<[u8]>>(
        &self,
        message: S,
    ) -> Result<Signature, Self::Error> {
        match self {
            Self::Ledger(s) => s.sign_message(message).await.map_err(WalletError::from),
            Self::Local(s) => s.sign_message(message).await.map_err(WalletError::from),
        }
    }

    async fn sign_transaction(
        &self,
        message: &ethers::types::transaction::eip2718::TypedTransaction,
    ) -> Result<Signature, Self::Error> {
        match self {
            Self::Ledger(s) => s.sign_transaction(message).await.map_err(WalletError::from),
            Self::Local(s) => s.sign_transaction(message).await.map_err(WalletError::from),
        }
    }
}

pub async fn signer<P>(options: SignerOptions, provider: Provider<P>) -> anyhow::Result<Wallet>
where
    P: JsonRpcClient + Clone + 'static,
{
    let chain_id = provider.get_chainid().await?.as_u64();

    if let Some(keypath) = &options.keystore {
        let password = term::secret_input_with_prompt("Keystore password");
        let spinner = term::spinner("Decrypting keystore...");
        let signer = LocalWallet::decrypt_keystore(keypath, password.unsecure())
            // Nb. Can fail if the file isn't found.
            .map_err(|e| anyhow!("keystore decryption failed: {}", e))?
            .with_chain_id(chain_id);

        spinner.finish();

        Ok(Wallet::Local(signer))
    } else if let Some(path) = &options.ledger_hdpath {
        let hdpath = path.derivation_string();
        let signer = Ledger::new(HDPath::Other(hdpath), chain_id)
            .await
            .context("Could not connect to Ledger device")?;

        Ok(Wallet::Ledger(signer))
    } else {
        Err(WalletError::NoWallet.into())
    }
}

pub async fn transaction<M, D>(call: ContractCall<M, D>) -> anyhow::Result<TransactionReceipt>
where
    D: Detokenize,
    M: Middleware + 'static,
{
    let receipt = loop {
        let spinner = term::spinner("Waiting for transaction to be signed...");
        let tx = match call.send().await {
            Ok(tx) => {
                spinner.finish();
                tx
            }
            Err(err) => {
                spinner.failed();
                return Err(err.into());
            }
        };
        term::success!(
            "Transaction {} submitted to the network.",
            term::format::highlight(self::hex(*tx))
        );

        let spinner = term::spinner("Waiting for transaction to be processed...");
        if let Some(receipt) = tx.await? {
            spinner.finish();
            break receipt;
        } else {
            spinner.failed();
        }
    };

    term::blank();
    term::info!(
        "Transaction included in block #{} ({}).",
        term::format::highlight(receipt.block_number.unwrap()),
        receipt.block_hash.unwrap(),
    );

    Ok(receipt)
}

pub fn chain_from_id(id: u64) -> Option<Chain> {
    match id {
        1 => Some(Chain::Mainnet),
        3 => Some(Chain::Ropsten),
        4 => Some(Chain::Rinkeby),
        5 => Some(Chain::Goerli),
        _ => None,
    }
}

pub fn hex(bytes: impl AsRef<[u8]>) -> String {
    format!("0x{}", hex::encode(bytes))
}
