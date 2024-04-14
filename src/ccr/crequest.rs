use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use serde::{Deserialize, Serialize};
use eyre::{Result, eyre};
use alloy::{
    primitives::{self, Address, Bytes, FixedBytes, U256, ChainId, Signature, TxKind}, 
    consensus::{SignableTransaction, Signed, Transaction}, 
    eips::eip2718::{Decodable2718, Encodable2718}
};
use super::crecord::{ConfidentialComputeRecord, CRecordRLP};


const CONFIDENTIAL_COMPUTE_RECORD_TYPE: u8 = 0x42;
const CONFIDENTIAL_COMPUTE_REQUEST_TYPE: u8 = 0x43;


#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfidentialComputeRequest {
    pub confidential_compute_record: ConfidentialComputeRecord,
    pub confidential_inputs: Bytes,
}

impl ConfidentialComputeRequest {

    pub fn new(
        mut confidential_compute_record: ConfidentialComputeRecord, 
        confidential_inputs: Bytes,
    ) -> Self {
        let ci_hash = primitives::keccak256(&confidential_inputs);
        confidential_compute_record.set_confidential_inputs_hash(ci_hash);

        Self {
            confidential_compute_record,
            confidential_inputs,
        }
    }

    pub fn rlp_encode(&self) -> Result<Bytes> {
        let cc_record = &self.confidential_compute_record;
        if cc_record.has_missing_field() {
            return Err(eyre!("Missing fields"));
        }
        let rlp_encoded = encode_with_prefix(
            CONFIDENTIAL_COMPUTE_REQUEST_TYPE, 
            CRequestRLP::from(self)
        );
        
        Ok(rlp_encoded)
    }

    fn hash(&self) -> FixedBytes<32> {
        let rlp_encoded = encode_with_prefix(
            CONFIDENTIAL_COMPUTE_RECORD_TYPE, 
            CRequestHashParams::from(self)
        );
        let hash = primitives::keccak256(&rlp_encoded);
        hash
    }

}

impl Transaction for ConfidentialComputeRequest {

    fn input(&self) -> &[u8] {
        self.confidential_compute_record.input.as_ref()
    }

    fn to(&self) -> TxKind {
        TxKind::Call(self.confidential_compute_record.to)
    }

    fn value(&self) -> U256 {
        self.confidential_compute_record.value
    }

    fn chain_id(&self) -> Option<ChainId> {
        Some(self.confidential_compute_record.chain_id)
    }

    fn nonce(&self) -> u64 {
        self.confidential_compute_record.nonce
    }

    fn gas_limit(&self) -> u64 {
        self.confidential_compute_record.gas
    }

    fn gas_price(&self) -> Option<U256> {
        Some(self.confidential_compute_record.gas_price)
    }

}

impl SignableTransaction<Signature> for ConfidentialComputeRequest {

    fn set_chain_id(&mut self, chain_id: ChainId) {
        self.confidential_compute_record.chain_id = chain_id;
    }

    fn encode_for_signing(&self, out: &mut dyn alloy_rlp::BufMut) {
        out.put_u8(CONFIDENTIAL_COMPUTE_RECORD_TYPE);
        CRequestHashParams::from(self).encode(out);
    }

    fn payload_len_for_signature(&self) -> usize {
        let chain_id = self.confidential_compute_record.chain_id as usize;
        CRequestHashParams::from(self).fields_len() + chain_id + 2
    }

    fn into_signed(self, signature: Signature) -> Signed<Self, Signature> where Self: Sized {
        let hash = self.hash().into();
        Signed::new_unchecked(self, signature.with_parity_bool(), hash)
    }

}

impl Decodable2718 for ConfidentialComputeRequest {
    fn typed_decode(ty: u8, buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        match ty {
            CONFIDENTIAL_COMPUTE_REQUEST_TYPE => {
                let crequest_prerlp = CRequestRLP::decode(buf)?;
                Ok(crequest_prerlp.into())
            }
            _ => Err(alloy_rlp::Error::Custom("Only ConfidentialComputeRequest"))
        }
    }

    fn fallback_decode(_buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        Err(alloy_rlp::Error::Custom("Only ConfidentialComputeRequest"))
    }
}

impl Encodable2718 for ConfidentialComputeRequest {
    fn type_flag(&self) -> Option<u8> {
        Some(CONFIDENTIAL_COMPUTE_REQUEST_TYPE)
    }

    fn encode_2718_len(&self) -> usize {
        CRequestRLP::from(self).fields_len()
    }

    fn encode_2718(&self, out: &mut dyn alloy_rlp::BufMut) {
        out.put_u8(CONFIDENTIAL_COMPUTE_REQUEST_TYPE);
        CRequestRLP::from(self).encode(out);
    }
}

#[derive(Debug, RlpEncodable, RlpDecodable, PartialEq)]
struct CRequestRLP {
    request: CRecordRLP,
    confidential_inputs: Bytes,
}

impl CRequestRLP {
    fn fields_len(&self) -> usize {
        let mut len = 0;
        len += self.request.fields_len();
        len += self.confidential_inputs.length();
        len
    }
}

impl From<&ConfidentialComputeRequest> for CRequestRLP {
    fn from(ccr: &ConfidentialComputeRequest) -> Self {
        Self {
            request: (&ccr.confidential_compute_record).into(),
            confidential_inputs: ccr.confidential_inputs.clone(),
        }
    }
}

impl Into<ConfidentialComputeRequest> for CRequestRLP {
    fn into(self) -> ConfidentialComputeRequest {
        let cc_record: ConfidentialComputeRecord = self.request.into();
        ConfidentialComputeRequest {
            confidential_compute_record: cc_record,
            confidential_inputs: self.confidential_inputs,
        }
    }
}

#[derive(Debug, RlpEncodable, PartialEq)]
struct CRequestHashParams {
    execution_node: Address,
    confidential_inputs_hash: FixedBytes<32>,
    nonce: u64,
    gas_price: U256,
    gas: u64,
    to: Address,
    value: U256,
    data: Bytes,
}

impl CRequestHashParams {
    fn fields_len(&self) -> usize {
        let mut len = 0;
        len += self.execution_node.length();
        len += self.confidential_inputs_hash.length();
        len += self.nonce.length();
        len += self.gas_price.length();
        len += self.to.length();
        len += self.value.length();
        len += self.data.0.length();
        len
    }
}

impl From<&ConfidentialComputeRequest> for CRequestHashParams {
    fn from(ccr: &ConfidentialComputeRequest) -> Self {
        let cinputs_hash = ccr.confidential_compute_record.confidential_inputs_hash
            .expect("Missing confidential_inputs_hash");
        Self {
            execution_node: ccr.confidential_compute_record.kettle_address,
            confidential_inputs_hash: cinputs_hash,
            nonce: ccr.confidential_compute_record.nonce,
            gas_price: ccr.confidential_compute_record.gas_price,
            gas: ccr.confidential_compute_record.gas,
            to: ccr.confidential_compute_record.to,
            value: ccr.confidential_compute_record.value,
            data: ccr.confidential_compute_record.input.clone(),
        }
    }
}

fn encode_with_prefix<T: Encodable>(prefix: u8, item: T) -> Bytes {
    let mut buffer = vec![prefix];
    item.encode(&mut buffer);
    Bytes::from(buffer)
}


#[cfg(test)]
mod tests {    
    use super::*;
    use std::str::FromStr;
    use alloy::{
        rpc::types::eth::TransactionRequest, 
        signers::wallet::LocalWallet,
        network::{
            TransactionBuilder, 
            TxSigner
        }, 
    };
    use eyre::Ok;

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
            .with_input(input);
        
        let mut cc_record = ConfidentialComputeRecord::from_tx_request(tx, execution_node)?;

        let v = 0;
        let r = U256::from_str("0x1567c31c4bebcd1061edbaf22dd73fd40ff30f9a3ba4525037f23b2dc61e3473").unwrap();
        let s = U256::from_str("0x2dce69262794a499d525c5d58edde33e06a5847b4d321d396b743700a2fd71a8").unwrap();
        let sig = Signature::from_rs_and_parity(r, s, v)?;
        cc_record.set_sig(sig);

        let confidential_inputs = Bytes::from_str("0x000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000001ea7b22747873223a5b7b2274797065223a22307830222c226e6f6e6365223a22307830222c22746f223a22307863613135656439393030366236623130363038653236313631373361313561343766383933613661222c22676173223a22307835323038222c226761735072696365223a22307864222c226d61785072696f72697479466565506572476173223a6e756c6c2c226d6178466565506572476173223a6e756c6c2c2276616c7565223a223078336538222c22696e707574223a223078222c2276223a2230786366323838222c2272223a22307863313764616536383866396262393632376563636439626636393133626661346539643232383139353134626539323066343435653263666165343366323965222c2273223a22307835633337646235386263376161336465306535656638613432353261366632653464313462613639666338323631636333623630633962643236613634626265222c2268617368223a22307862643263653662653964333461366132393934373239346662656137643461343834646663363565643963383931396533626539366131353634363630656265227d5d2c2270657263656e74223a31302c224d617463684964223a5b302c302c302c302c302c302c302c302c302c302c302c302c302c302c302c305d7d00000000000000000000000000000000000000000000").unwrap();
        let cc_request = ConfidentialComputeRequest::new(cc_record, confidential_inputs);
        let rlp_encoded = cc_request.rlp_encode().unwrap();

        let expected_rlp_encoded = Bytes::from_str("0x43f903a9f9016322843b9aca00830f424094780675d71ebe3d3ef05fae379063071147dd3aee80b8c4236eb5a70000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000780675d71ebe3d3ef05fae379063071147dd3aee0000000000000000000000000000000000000000000000000000000000000000947d83e42b214b75bf1f3e57adc3415da573d97bffa089ee438ca379ac86b0478517d43a6a9e078cf51543acac0facd68aff313e2ff18306793280a01567c31c4bebcd1061edbaf22dd73fd40ff30f9a3ba4525037f23b2dc61e3473a02dce69262794a499d525c5d58edde33e06a5847b4d321d396b743700a2fd71a8b90240000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000001ea7b22747873223a5b7b2274797065223a22307830222c226e6f6e6365223a22307830222c22746f223a22307863613135656439393030366236623130363038653236313631373361313561343766383933613661222c22676173223a22307835323038222c226761735072696365223a22307864222c226d61785072696f72697479466565506572476173223a6e756c6c2c226d6178466565506572476173223a6e756c6c2c2276616c7565223a223078336538222c22696e707574223a223078222c2276223a2230786366323838222c2272223a22307863313764616536383866396262393632376563636439626636393133626661346539643232383139353134626539323066343435653263666165343366323965222c2273223a22307835633337646235386263376161336465306535656638613432353261366632653464313462613639666338323631636333623630633962643236613634626265222c2268617368223a22307862643263653662653964333461366132393934373239346662656137643461343834646663363565643963383931396533626539366131353634363630656265227d5d2c2270657263656e74223a31302c224d617463684964223a5b302c302c302c302c302c302c302c302c302c302c302c302c302c302c302c305d7d00000000000000000000000000000000000000000000").unwrap();
    
        assert_eq!(rlp_encoded, expected_rlp_encoded);

        Ok(())
    }

    #[test]
    fn test_crequesthashparams_hash() {
        let execution_node = Address::from_str("0x7d83e42b214b75bf1f3e57adc3415da573d97bff").unwrap();
        let to_add = Address::from_str("0x772092ff73c43883a547bea1e1e007ec0d33478e").unwrap();
        let input = Bytes::from_str("0x236eb5a70000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000772092ff73c43883a547bea1e1e007ec0d33478e0000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let cinputs = Bytes::from_str("0x000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000001ea7b22747873223a5b7b2274797065223a22307830222c226e6f6e6365223a22307830222c22746f223a22307838626265386333346637396433353534666631626236643932313733613237666661356237313233222c22676173223a22307835323038222c226761735072696365223a22307864222c226d61785072696f72697479466565506572476173223a6e756c6c2c226d6178466565506572476173223a6e756c6c2c2276616c7565223a223078336538222c22696e707574223a223078222c2276223a2230786366323837222c2272223a22307862396433643236643135633630376237653537353235333761336163326432363330643161653036386163353138616539393862613439313236323134383135222c2273223a22307835636534666439613565376533373138656630613731386533633462333135306538373036376533373361333439323538643962333330353930396332303565222c2268617368223a22307863633934626637386463366631373963663331376638643839353438393364393730303366333266353332623530623865333861626631333939353364643664227d5d2c2270657263656e74223a31302c224d617463684964223a5b302c302c302c302c302c302c302c302c302c302c302c302c302c302c302c305d7d00000000000000000000000000000000000000000000").unwrap();
        let cinputs_hash = primitives::keccak256(&cinputs);

        let hash_params = CRequestHashParams {
            execution_node,
            confidential_inputs_hash: cinputs_hash,
            nonce: 0x18,
            gas_price: U256::from_str("0x3b9aca00").unwrap(),
            gas: 0x0f4240,
            to: to_add,
            value: U256::ZERO,
            data: input,
        };
        let encoded = encode_with_prefix(CONFIDENTIAL_COMPUTE_RECORD_TYPE, hash_params);
        let hash = primitives::keccak256(&encoded);

        let expected_hash = FixedBytes::from_str("0x72ffab40c5116931200ca87052360787559871297b3615a8c2ff28be738ac59f").unwrap();
        assert_eq!(hash, expected_hash);
    }

    #[test]
    fn test_crequest_hash() {
        let execution_node = Address::from_str("0x7d83e42b214b75bf1f3e57adc3415da573d97bff").unwrap();
        let to_add = Address::from_str("0x772092ff73c43883a547bea1e1e007ec0d33478e").unwrap();
        let input = Bytes::from_str("0x236eb5a70000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000772092ff73c43883a547bea1e1e007ec0d33478e0000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let cinputs = Bytes::from_str("0x000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000001ea7b22747873223a5b7b2274797065223a22307830222c226e6f6e6365223a22307830222c22746f223a22307838626265386333346637396433353534666631626236643932313733613237666661356237313233222c22676173223a22307835323038222c226761735072696365223a22307864222c226d61785072696f72697479466565506572476173223a6e756c6c2c226d6178466565506572476173223a6e756c6c2c2276616c7565223a223078336538222c22696e707574223a223078222c2276223a2230786366323837222c2272223a22307862396433643236643135633630376237653537353235333761336163326432363330643161653036386163353138616539393862613439313236323134383135222c2273223a22307835636534666439613565376533373138656630613731386533633462333135306538373036376533373361333439323538643962333330353930396332303565222c2268617368223a22307863633934626637386463366631373963663331376638643839353438393364393730303366333266353332623530623865333861626631333939353364643664227d5d2c2270657263656e74223a31302c224d617463684964223a5b302c302c302c302c302c302c302c302c302c302c302c302c302c302c302c305d7d00000000000000000000000000000000000000000000").unwrap();
        let cinputs_hash = primitives::keccak256(&cinputs);

        let crecord = ConfidentialComputeRecord {
            kettle_address: execution_node,
            confidential_inputs_hash: Some(cinputs_hash),
            nonce: 0x18,
            gas_price: U256::from_str("0x3b9aca00").unwrap(),
            gas: 0x0f4240,
            to: to_add,
            value: U256::ZERO,
            input,
            signature: None,
            chain_id: 1
        };
        let crequest = ConfidentialComputeRequest {
            confidential_compute_record: crecord,
            confidential_inputs: cinputs
        };
        let hash = crequest.hash();

        let expected_hash = FixedBytes::from_str("0x72ffab40c5116931200ca87052360787559871297b3615a8c2ff28be738ac59f").unwrap();
        assert_eq!(hash, expected_hash);
    }

    #[tokio::test]
    async fn test_ccr_sign() -> Result<()> {
        // Create a cc request 
        let cinputs = Bytes::from_str("0x000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000001ea7b22747873223a5b7b2274797065223a22307830222c226e6f6e6365223a22307830222c22746f223a22307863613135656439393030366236623130363038653236313631373361313561343766383933613661222c22676173223a22307835323038222c226761735072696365223a22307864222c226d61785072696f72697479466565506572476173223a6e756c6c2c226d6178466565506572476173223a6e756c6c2c2276616c7565223a223078336538222c22696e707574223a223078222c2276223a2230786366323838222c2272223a22307863313764616536383866396262393632376563636439626636393133626661346539643232383139353134626539323066343435653263666165343366323965222c2273223a22307835633337646235386263376161336465306535656638613432353261366632653464313462613639666338323631636333623630633962643236613634626265222c2268617368223a22307862643263653662653964333461366132393934373239346662656137643461343834646663363565643963383931396533626539366131353634363630656265227d5d2c2270657263656e74223a31302c224d617463684964223a5b302c302c302c302c302c302c302c302c302c302c302c302c302c302c302c305d7d00000000000000000000000000000000000000000000").unwrap();
        let execution_node = Address::from_str("0x7d83e42b214b75bf1f3e57adc3415da573d97bff").unwrap();
        let nonce = 0x22;
        let to_add = Address::from_str("0x780675d71ebe3d3ef05fae379063071147dd3aee").unwrap();
        let gas = 0x0f4240;
        let gas_price = U256::from_str("0x3b9aca00").unwrap();
        let input = Bytes::from_str("0x236eb5a70000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000780675d71ebe3d3ef05fae379063071147dd3aee0000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let chain_id = 0x067932;
        let tx = TransactionRequest::default()
            .to(Some(to_add))
            .gas_limit(U256::from(gas))
            .with_gas_price(gas_price)
            .with_chain_id(chain_id)
            .with_nonce(nonce)
            .with_input(input);
        let cc_record = ConfidentialComputeRecord::from_tx_request(tx, execution_node)?;
        let mut cc_request = ConfidentialComputeRequest::new(cc_record, cinputs); 

        // Sign
        let pk = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let wallet: LocalWallet = pk.parse().unwrap();    
        let sig = wallet.sign_transaction(&mut cc_request).await.unwrap();

        // Check signature
        assert_eq!(sig.v().recid().to_byte(), 0_u8);
        assert_eq!(sig.r(), U256::from_str("0x1567c31c4bebcd1061edbaf22dd73fd40ff30f9a3ba4525037f23b2dc61e3473").unwrap());
        assert_eq!(sig.s(), U256::from_str("0x2dce69262794a499d525c5d58edde33e06a5847b4d321d396b743700a2fd71a8").unwrap());

        Ok(())
    }

    #[test]
    fn test_encode_decode() -> Result<()> {
        let cinputs = Bytes::from_str("0x000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000001ea7b22747873223a5b7b2274797065223a22307830222c226e6f6e6365223a22307830222c22746f223a22307863613135656439393030366236623130363038653236313631373361313561343766383933613661222c22676173223a22307835323038222c226761735072696365223a22307864222c226d61785072696f72697479466565506572476173223a6e756c6c2c226d6178466565506572476173223a6e756c6c2c2276616c7565223a223078336538222c22696e707574223a223078222c2276223a2230786366323838222c2272223a22307863313764616536383866396262393632376563636439626636393133626661346539643232383139353134626539323066343435653263666165343366323965222c2273223a22307835633337646235386263376161336465306535656638613432353261366632653464313462613639666338323631636333623630633962643236613634626265222c2268617368223a22307862643263653662653964333461366132393934373239346662656137643461343834646663363565643963383931396533626539366131353634363630656265227d5d2c2270657263656e74223a31302c224d617463684964223a5b302c302c302c302c302c302c302c302c302c302c302c302c302c302c302c305d7d00000000000000000000000000000000000000000000").unwrap();
        let execution_node = Address::from_str("0x7d83e42b214b75bf1f3e57adc3415da573d97bff").unwrap();
        let nonce = 0x22;
        let to_add = Address::from_str("0x780675d71ebe3d3ef05fae379063071147dd3aee").unwrap();
        let gas = 0x0f4240;
        let gas_price = U256::from_str("0x3b9aca00").unwrap();
        let input = Bytes::from_str("0x236eb5a70000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000780675d71ebe3d3ef05fae379063071147dd3aee0000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let chain_id = 0x067932;
        let tx = TransactionRequest::default()
            .to(Some(to_add))
            .gas_limit(U256::from(gas))
            .with_gas_price(gas_price)
            .with_chain_id(chain_id)
            .with_nonce(nonce)
            .with_input(input);
        let mut cc_record = ConfidentialComputeRecord::from_tx_request(tx, execution_node)?;

        let v = 0_u64;
        let r = U256::from_str("0x1567c31c4bebcd1061edbaf22dd73fd40ff30f9a3ba4525037f23b2dc61e3473").unwrap();
        let s = U256::from_str("0x2dce69262794a499d525c5d58edde33e06a5847b4d321d396b743700a2fd71a8").unwrap();
        cc_record.signature = Some(Signature::from_rs_and_parity(r, s, v).unwrap());
        let cc_request = ConfidentialComputeRequest::new(cc_record, cinputs);

        let mut encoded = Vec::new();
        cc_request.encode_2718(&mut encoded);
        let decoded = ConfidentialComputeRequest::decode_2718(&mut encoded.as_slice())?;

        assert_eq!(cc_request, decoded);

        Ok(())
    }

}