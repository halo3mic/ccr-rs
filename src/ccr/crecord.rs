use serde::{Deserialize, Serialize};
use alloy_rlp::{Encodable, RlpDecodable, RlpEncodable};
use eyre::{Result, eyre};
use alloy::{
    primitives::{Address, Bytes, FixedBytes, U256, Signature}, 
    rpc::types::eth::TransactionRequest,
    serde as alloy_serde,
};


#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfidentialComputeRecord {
    #[serde(with = "alloy_serde::num::u64_hex")]
    pub nonce: u64,
    pub to: Address,
    #[serde(with = "alloy_serde::num::u64_hex")]
    pub gas: u64,
    pub gas_price: U256,
    pub value: U256,
    pub input: Bytes,
    pub kettle_address: Address,
    #[serde(with = "alloy_serde::num::u64_hex")]
    pub chain_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidential_inputs_hash: Option<FixedBytes<32>>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Signature>,
}

impl ConfidentialComputeRecord {

    pub fn from_tx_request(
        tx_req: TransactionRequest, 
        execution_node: Address, 
    ) -> Result<Self> {
        let gas: u64 = tx_req.gas
            .ok_or_else(|| eyre!("Missing gas field"))
            .and_then(|g| g.try_into().map_err(|_| eyre!("Gas overflow")))?;
        let chain_id = tx_req.chain_id.ok_or(eyre!("Missing chain_id field"))?;
        Ok(Self {
            input: tx_req.input.input.unwrap_or(Bytes::new()),
            gas_price: tx_req.gas_price.unwrap_or(U256::ZERO),
            value: tx_req.value.unwrap_or(U256::ZERO),
            to: tx_req.to.unwrap_or(Address::ZERO),
            nonce: tx_req.nonce.unwrap_or(0),
            kettle_address: execution_node,
            chain_id,
            gas,
            confidential_inputs_hash: None,
            signature: None,
        })
    }

    pub fn set_confidential_inputs_hash(&mut self, confidential_inputs_hash: FixedBytes<32>) {
        self.confidential_inputs_hash = Some(confidential_inputs_hash);
    }

    pub fn set_sig(&mut self, signature: Signature) {
        self.signature = Some(signature);
    }

    pub fn has_missing_field(&self) -> bool {
        self.confidential_inputs_hash.is_none() || self.signature.is_none()
    }

}


#[derive(Debug, RlpEncodable, RlpDecodable, PartialEq)]
pub struct CRecordRLP {
    nonce: u64,
    gas_price: U256,
    gas: u64,
    to: Address,
    value: U256,
    data: Bytes,
    execution_node: Address,
    confidential_inputs_hash: FixedBytes<32>,
    chain_id: u64,
    v: u8,
    r: U256,
    s: U256,
}

impl CRecordRLP {
    pub fn fields_len(&self) -> usize {
        let mut len = 0;
        len += self.nonce.length();
        len += self.gas_price.length();
        len += self.gas.length();
        len += self.to.length();
        len += self.value.length();
        len += self.data.0.length();
        len += self.execution_node.length();
        len += self.confidential_inputs_hash.length();
        len += self.chain_id.length();
        len += self.v.length();
        len += self.r.length();
        len += self.s.length();
        len
    }
}

impl From<&ConfidentialComputeRecord> for CRecordRLP {
    fn from(ccr: &ConfidentialComputeRecord) -> Self {
        let sig = ccr.signature
            .expect("Missing signature field");
        let cinputs_hash = ccr.confidential_inputs_hash
            .expect("Missing confidential_inputs_hash");
        let (v, r, s) = signature_to_vrs(sig);

        Self {
            nonce: ccr.nonce,
            gas_price: ccr.gas_price,
            gas: ccr.gas,
            to: ccr.to,
            value: ccr.value,
            data: ccr.input.clone(),
            execution_node: ccr.kettle_address,
            confidential_inputs_hash: cinputs_hash,
            chain_id: ccr.chain_id,
            v, r, s
        }
    }
}

impl Into<ConfidentialComputeRecord> for CRecordRLP {
    fn into(self) -> ConfidentialComputeRecord {
        let sig = Signature::from_rs_and_parity(self.r, self.s, self.v as u64)
            .expect("Invalid signature");
        ConfidentialComputeRecord {
            nonce: self.nonce,
            gas_price: self.gas_price,
            gas: self.gas,
            to: self.to,
            value: self.value,
            input: self.data,
            kettle_address: self.execution_node,
            chain_id: self.chain_id,
            confidential_inputs_hash: Some(self.confidential_inputs_hash),
            signature: Some(sig),
        }
    }

}

pub(crate) fn signature_to_vrs(sig: Signature) -> (u8, U256, U256) {
    let v = sig.v().recid().to_byte();
    let r = sig.r();
    let s = sig.s();
    (v, r, s)
}


#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use alloy::{
        network::TransactionBuilder, 
        rpc::types::eth::TransactionRequest, 
    };


    #[test]
    fn test_ccr_rlp_encode() -> Result<()> {
        let chain_id = 0x067932;
        let execution_node = Address::from_str("0x7d83e42b214b75bf1f3e57adc3415da573d97bff").unwrap();
        let to_add = Address::from_str("0x780675d71ebe3d3ef05fae379063071147dd3aee").unwrap();
        let input = Bytes::from_str("0x236eb5a70000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000780675d71ebe3d3ef05fae379063071147dd3aee0000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let tx = TransactionRequest::default()
            .to(Some(to_add))
            .gas_limit(U256::from(0x0f4240))
            .with_gas_price(U256::from(0x3b9aca00))
            .with_chain_id(chain_id)
            .with_nonce(0x22)
            .with_input(input)
            .with_value(U256::from(0x2233));
        
        let cc_record = ConfidentialComputeRecord::from_tx_request(tx.clone(), execution_node)?;
        assert_eq!(cc_record.kettle_address, execution_node);
        assert_eq!(cc_record.to, to_add);
        assert_eq!(U256::from(cc_record.gas), tx.gas.unwrap());
        assert_eq!(cc_record.gas_price, tx.gas_price.unwrap());
        assert_eq!(cc_record.chain_id, chain_id);
        assert_eq!(cc_record.nonce, tx.nonce.unwrap());
        assert_eq!(cc_record.input, tx.input.input.unwrap());
        assert_eq!(cc_record.value, tx.value.unwrap());
        assert!(cc_record.confidential_inputs_hash.is_none());
        assert!(cc_record.signature.is_none());

        Ok(())
    }

    #[test]
    fn test_ccr_rlp_encode_missing_fields() -> Result<()> {
        let chain_id = 0x067932;
        let execution_node = Address::from_str("0x7d83e42b214b75bf1f3e57adc3415da573d97bff").unwrap();
        let tx = TransactionRequest::default()
            .gas_limit(U256::from(0x0f4240))
            .with_chain_id(chain_id);
        
        let cc_record = ConfidentialComputeRecord::from_tx_request(tx.clone(), execution_node)?;
        assert_eq!(cc_record.kettle_address, execution_node);
        assert_eq!(cc_record.to, Address::ZERO);
        assert_eq!(U256::from(cc_record.gas), tx.gas.unwrap());
        assert_eq!(cc_record.gas_price, U256::ZERO);
        assert_eq!(cc_record.chain_id, chain_id);
        assert_eq!(cc_record.nonce, 0);
        assert_eq!(cc_record.input, Bytes::new());
        assert_eq!(cc_record.value, U256::ZERO);
        assert!(cc_record.confidential_inputs_hash.is_none());
        assert!(cc_record.signature.is_none());

        Ok(())
    }

    #[test]
    fn test_missing_vals() {
        let chain_id = 0x067932;
        let execution_node = Address::from_str("0x7d83e42b214b75bf1f3e57adc3415da573d97bff").unwrap();

        let tx = TransactionRequest::default()
            .gas_limit(U256::from(0x0f4240));
        let cc_record_res = ConfidentialComputeRecord::from_tx_request(tx, execution_node);
        assert!(cc_record_res.is_err());

        let tx = TransactionRequest::default()
            .with_chain_id(chain_id);
        let cc_record_res = ConfidentialComputeRecord::from_tx_request(tx, execution_node);
        assert!(cc_record_res.is_err());
    }

}
