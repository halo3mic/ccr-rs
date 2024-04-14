use alloy::{
    network::{TxSigner, NetworkSigner, Network},
    signers::Result as SignerResult,
    primitives::Signature,
};
use async_trait::async_trait;
use std::sync::Arc;

use crate::ccr::ConfidentialComputeRequest;


#[derive(Clone)]
pub struct SuaveSigner(Arc<dyn TxSigner<Signature> + Send + Sync>);

impl std::fmt::Debug for SuaveSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SuaveSigner").finish()
    }
}

impl<S> From<S> for SuaveSigner
where
    S: TxSigner<Signature> + Send + Sync + 'static,
{
    fn from(signer: S) -> Self {
        Self::new(signer)
    }
}

impl SuaveSigner {
    pub fn new<S>(signer: S) -> Self
    where
        S: TxSigner<Signature> + Send + Sync + 'static,
    {
        Self(Arc::new(signer))
    }

    async fn sign_transaction(
        &self,
        tx: &mut ConfidentialComputeRequest,
    ) -> SignerResult<ConfidentialComputeRequest> {
        self.0.sign_transaction(tx).await.map(|sig| {
            tx.confidential_compute_record.set_sig(sig);
            tx.clone()
        })
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<N> NetworkSigner<N> for SuaveSigner
where
    N: Network<UnsignedTx = ConfidentialComputeRequest, TxEnvelope = ConfidentialComputeRequest>,
{
    async fn sign_transaction(
        &self,
        tx: ConfidentialComputeRequest,
    ) -> SignerResult<ConfidentialComputeRequest> {
        let mut tx = tx;
        self.sign_transaction(&mut tx).await
    }
}


#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use eyre::Result;
    use alloy::{
        primitives::{B256, Address, Bytes, U256}, 
        providers::{Provider, ProviderBuilder}, 
        rpc::types::eth::TransactionRequest, 
        signers::wallet::LocalWallet,
        network::TransactionBuilder,
    };
    use crate::ccr::ConfidentialComputeRecord;
    use super::super::network::SuaveNetwork;
    use super::*;


    #[tokio::test]
    async fn test_send_tx_rigil() -> Result<()> {
        let rpc_url = url::Url::parse("https://rpc.rigil.suave.flashbots.net").unwrap();
        let provider = ProviderBuilder::new().on_reqwest_http(rpc_url.clone()).unwrap();
        let wallet_address = Address::from_str("0x19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap();
        let tx_count: u64 = provider.get_transaction_count(wallet_address, None).await.unwrap().to();

        // Create a cc request 
        let cinputs = Bytes::new();
        let execution_node = Address::from_str("0x03493869959c866713c33669ca118e774a30a0e5").unwrap();
        let nonce = tx_count;
        let to_add = Address::from_str("0xc803334c79650708Daf3a3462AC4B48296b1352a").unwrap();
        let gas = 0x0f4240;
        let gas_price = U256::from_str("0x1c9aca00").unwrap();
        let input = Bytes::from_str("0x50723553000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000074554485553445400").unwrap();
        let chain_id = 0x1008c45;
        let tx = TransactionRequest::default()
            .to(Some(to_add))
            .gas_limit(U256::from(gas))
            .with_gas_price(gas_price)
            .with_chain_id(chain_id)
            .with_nonce(nonce)
            .with_input(input);
        let cc_record = ConfidentialComputeRecord::from_tx_request(tx, execution_node)?;
        let cc_request = ConfidentialComputeRequest::new(cc_record, cinputs);
        
        let pk = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let wallet: LocalWallet = pk.parse().unwrap();    
        let signer = SuaveSigner::from(wallet.clone());
        let provider = ProviderBuilder::<_, SuaveNetwork>::default().signer(signer).on_reqwest_http(rpc_url).unwrap();
        
        let result = provider.send_transaction(cc_request).await.unwrap();
        let tx_hash = B256::from_slice(&result.tx_hash().to_vec());
        let tx_response = provider.get_transaction_by_hash(tx_hash).await.unwrap();
        
        let cc_result = tx_response.other.get("confidentialComputeResult").unwrap().as_str().unwrap();
        let price = u128::from_str_radix(cc_result.trim_start_matches("0x"), 16).unwrap();
        assert!(price > 0);

        Ok(())
    }

}