use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        Self { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self.value {
            0x00..=0xFC => vec![self.value as u8],
            0xFD..=0xFFFF => {
                let mut bytes = Vec::with_capacity(3);
                bytes.push(0xFD);
                bytes.extend_from_slice(&(self.value as u16).to_le_bytes());
                bytes
            }
            0x1_0000..=0xFFFF_FFFF => {
                let mut bytes = Vec::with_capacity(5);
                bytes.push(0xFE);
                bytes.extend_from_slice(&(self.value as u32).to_le_bytes());
                bytes
            }
            _ => {
                let mut bytes = Vec::with_capacity(9);
                bytes.push(0xFF);
                bytes.extend_from_slice(&self.value.to_le_bytes());
                bytes
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let first = *bytes.first().ok_or(BitcoinError::InsufficientBytes)?;

        match first {
            0x00..=0xFC => Ok((Self::new(first as u64), 1)),
            0xFD => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                Ok((
                    Self::new(u16::from_le_bytes([bytes[1], bytes[2]]) as u64),
                    3,
                ))
            }
            0xFE => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                Ok((
                    Self::new(u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as u64),
                    5,
                ))
            }
            0xFF => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                Ok((
                    Self::new(u64::from_le_bytes([
                        bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                        bytes[8],
                    ])),
                    9,
                ))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut hex = [0u8; 64];
        hex::encode_to_slice(self.0, &mut hex).map_err(serde::ser::Error::custom)?;
        let hex = std::str::from_utf8(&hex).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(hex)
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        let bytes = hex::decode(hex).map_err(serde::de::Error::custom)?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|bytes: Vec<u8>| serde::de::Error::invalid_length(bytes.len(), &"32 bytes"))?;
        Ok(Self(bytes))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        Self {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(36);
        bytes.extend_from_slice(&self.txid.0);
        bytes.extend_from_slice(&self.vout.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes[..32]);
        let vout = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);

        Ok((Self::new(txid, vout), 36))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = CompactSize::new(self.bytes.len() as u64).to_bytes();
        bytes.reserve(self.bytes.len());
        bytes.extend_from_slice(&self.bytes);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (length, length_size) = CompactSize::from_bytes(bytes)?;
        let script_len = length.value as usize;
        let end = length_size + script_len;

        if bytes.len() < end {
            return Err(BitcoinError::InsufficientBytes);
        }

        Ok((Self::new(bytes[length_size..end].to_vec()), end))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        Self {
            previous_output,
            script_sig,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let script_sig = self.script_sig.to_bytes();
        let mut bytes = Vec::with_capacity(36 + script_sig.len() + 4);
        bytes.extend_from_slice(&self.previous_output.to_bytes());
        bytes.extend_from_slice(&script_sig);
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (previous_output, outpoint_size) = OutPoint::from_bytes(bytes)?;
        let (script_sig, script_size) = Script::from_bytes(&bytes[outpoint_size..])?;
        let sequence_offset = outpoint_size + script_size;

        if bytes.len() < sequence_offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let sequence = u32::from_le_bytes([
            bytes[sequence_offset],
            bytes[sequence_offset + 1],
            bytes[sequence_offset + 2],
            bytes[sequence_offset + 3],
        ]);

        Ok((
            Self::new(previous_output, script_sig, sequence),
            sequence_offset + 4,
        ))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        Self {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let input_bytes: Vec<Vec<u8>> =
            self.inputs.iter().map(TransactionInput::to_bytes).collect();
        let input_len: usize = input_bytes.iter().map(Vec::len).sum();
        let input_count = CompactSize::new(self.inputs.len() as u64).to_bytes();

        let mut bytes = Vec::with_capacity(4 + input_count.len() + input_len + 4);
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.extend_from_slice(&input_count);
        for input in input_bytes {
            bytes.extend_from_slice(&input);
        }
        bytes.extend_from_slice(&self.lock_time.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let (input_count, input_count_size) = CompactSize::from_bytes(&bytes[4..])?;
        let mut offset = 4 + input_count_size;
        let mut inputs = Vec::with_capacity(input_count.value as usize);

        for _ in 0..input_count.value {
            let (input, input_size) = TransactionInput::from_bytes(&bytes[offset..])?;
            inputs.push(input);
            offset += input_size;
        }

        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let lock_time = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);

        Ok((Self::new(version, inputs, lock_time), offset + 4))
    }
}


impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f, "Inputs: {}", self.inputs.len())?;

        for (index, input) in self.inputs.iter().enumerate() {
            writeln!(f, "Input {}:", index)?;
            writeln!(
                f,
                "  Previous Output Txid: {}",
                hex::encode(input.previous_output.txid.0)
            )?;
            writeln!(f, "  Previous Output Vout: {}", input.previous_output.vout)?;
            writeln!(f, "  ScriptSig Length: {}", input.script_sig.bytes.len())?;
            writeln!(
                f,
                "  ScriptSig Bytes: {}",
                hex::encode(&input.script_sig.bytes)
            )?;
            writeln!(f, "  Sequence: {}", input.sequence)?;
        }

        write!(f, "Lock Time: {}", self.lock_time)
    }
}
